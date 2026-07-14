//! Native (non-wasm) tests over the pure `*_json` cores. The `#[wasm_bindgen]`
//! surface is a thin serde-wasm-bindgen shell around these, exercised in JS.
use std::collections::BTreeMap;
use waml::solve::{Rect, Size, SizeMap, SolveConfig};
use waml_wasm::{
    apply_ops_bundle, build_bundle_json, build_model_json, fmt_bundle, solve_bundle,
    validate_json,
};

fn bundle() -> Vec<(String, String)> {
    vec![(
        "m/order.md".into(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n".into(),
    )]
}

#[test]
fn build_model_json_emits_ts_shaped_nodes() {
    let json = build_model_json(&bundle());
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["nodes"][0]["type"], "uml.Class");
    assert_eq!(v["nodes"][0]["key"], "m/order");
    assert_eq!(v["nodes"][0]["attributes"][0]["name"], "id");
}

#[test]
fn validate_json_flags_unresolved_relationship_target() {
    let bad = vec![(
        "m/order.md".into(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [Ghost](./ghost.md): 1 to 1\n".to_string(),
    )];
    let json = validate_json(&bad);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v.as_array().unwrap();
    assert!(
        arr.iter().any(|d| d["code"] == "unresolved-target"),
        "expected an unresolved-target diagnostic, got: {json}"
    );
}

#[test]
fn apply_ops_adds_attribute() {
    let src = vec![(
        "m/a.md".to_string(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string(),
    )];
    let ops = r#"[{"op":"attr.add","node":"a","name":"id","ty":"AId"}]"#;
    let out = apply_ops_bundle(&src, ops).unwrap();
    let a = &out.iter().find(|(p, _)| p == "m/a.md").unwrap().1;
    assert!(a.contains("## Attributes"), "got:\n{a}");
    assert!(a.contains("- id: AId"), "got:\n{a}");
}

#[test]
fn apply_ops_surfaces_op_errors() {
    let src = vec![(
        "m/a.md".to_string(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string(),
    )];
    // attr.add on a non-existent node ⇒ Err, message carries the op index.
    let ops = r#"[{"op":"attr.add","node":"ghost","name":"id","ty":"AId"}]"#;
    let err = apply_ops_bundle(&src, ops).unwrap_err();
    assert!(err.starts_with("op 0:"), "got: {err}");
}

#[test]
fn build_bundle_json_round_trips_every_okf_field_and_leaves_uml_intact() {
    // A mixed bundle: a `uaml.Class` doc plus a non-UML `Playbook` carrying the
    // full OKF field set — tags, resource, timestamp, a body link, and a
    // citation. `build_bundle` must project every doc to a Concept losslessly.
    let bundle = vec![
        (
            "shop/order.md".to_string(),
            "---\ntype: uaml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n".to_string(),
        ),
        (
            "playbooks/dataplex.md".to_string(),
            "---\n\
                type: Playbook\n\
                title: Dataplex Playbook\n\
                description: How to onboard Dataplex.\n\
                resource: /playbooks/dataplex\n\
                tags: [data, governance]\n\
                timestamp: 2026-05-22\n\
                owner: data-team\n\
                ---\n\
                # Dataplex Playbook\n\n\
                See the [customers table](/tables/customers.md) for the join key.\n\n\
                # Citations\n\n\
                [1] [BigQuery announcement](https://cloud.google.com/blog/x)\n"
                .to_string(),
        ),
    ];

    let json = build_bundle_json(&bundle);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let concepts = v["concepts"].as_array().expect("concepts array");
    assert_eq!(concepts.len(), 2, "every doc projects to a concept: {json}");

    // The UML doc still projects, keyed by full path minus `.md`.
    let order = concepts
        .iter()
        .find(|c| c["id"] == "shop/order")
        .expect("order concept");
    assert_eq!(order["type"], "uaml.Class");
    assert_eq!(order["title"], "Order");

    // The non-UML Playbook round-trips every OKF field.
    let pb = concepts
        .iter()
        .find(|c| c["id"] == "playbooks/dataplex")
        .expect("playbook concept");
    assert_eq!(pb["type"], "Playbook");
    assert_eq!(pb["title"], "Dataplex Playbook");
    assert_eq!(pb["description"], "How to onboard Dataplex.");
    assert_eq!(pb["resource"], "/playbooks/dataplex");
    assert_eq!(pb["tags"][0], "data");
    assert_eq!(pb["tags"][1], "governance");
    assert_eq!(pb["timestamp"], "2026-05-22");
    assert!(pb["body"].as_str().unwrap().contains("# Dataplex Playbook"));
    assert_eq!(pb["links"][0]["href"], "/tables/customers.md");
    assert_eq!(pb["citations"][0]["href"], "https://cloud.google.com/blog/x");
    // Unknown frontmatter survives in `extra`; known keys do not leak in.
    assert_eq!(pb["extra"]["owner"], "data-team");
    assert!(pb["extra"].get("type").is_none());

    // The pre-existing `build_model_json` output is unchanged by this addition:
    // the same UML doc still yields the legacy node shape.
    let model_json = build_model_json(&bundle);
    let m: serde_json::Value = serde_json::from_str(&model_json).unwrap();
    let order_node = m["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["key"] == "shop/order")
        .expect("order node still present");
    assert_eq!(order_node["type"], "uaml.Class");
}

#[test]
fn build_model_json_nests_okf_concept_on_each_node() {
    // Additive OKF expand: every `Node` now carries a nested `concept` mirroring
    // the doc's lossless `okf::project`. A non-UML `Playbook` (which still becomes
    // a classifier node) carries every OKF field — tags, resource, timestamp, a
    // body link, a citation — on `concept`, none of which the flat Node exposes.
    let bundle = vec![
        (
            "shop/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n".to_string(),
        ),
        (
            "playbooks/dataplex.md".to_string(),
            "---\n\
                type: Playbook\n\
                title: Dataplex Playbook\n\
                description: How to onboard Dataplex.\n\
                resource: /playbooks/dataplex\n\
                tags: [data, governance]\n\
                timestamp: 2026-05-22\n\
                owner: data-team\n\
                ---\n\
                # Dataplex Playbook\n\n\
                See the [customers table](/tables/customers.md) for the join key.\n\n\
                # Citations\n\n\
                [1] [BigQuery announcement](https://cloud.google.com/blog/x)\n"
                .to_string(),
        ),
    ];

    let json = build_model_json(&bundle);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let nodes = v["nodes"].as_array().expect("nodes array");

    // The UML node no longer carries flat title/description/body — those live
    // only on the nested `concept` (single authoritative source), which the node
    // still gains alongside its UML-tier fields.
    let order = nodes.iter().find(|n| n["key"] == "shop/order").expect("order node");
    assert_eq!(order["type"], "uml.Class");
    assert!(order.get("title").is_none(), "flat title deleted: {order}");
    assert!(order.get("description").is_none(), "flat description deleted: {order}");
    assert!(order.get("body").is_none(), "flat body deleted: {order}");
    assert_eq!(order["attributes"][0]["name"], "id");
    // The nested concept mirrors okf::project: id = full path minus `.md`, and is
    // the single source of the resolved title.
    assert_eq!(order["concept"]["id"], "shop/order");
    assert_eq!(order["concept"]["type"], "uml.Class");
    assert_eq!(order["concept"]["title"], "Order");

    // The non-UML Playbook is still a classifier node; its `concept` carries
    // every OKF field the flat Node drops.
    let pb = nodes.iter().find(|n| n["key"] == "playbooks/dataplex").expect("playbook node");
    let c = &pb["concept"];
    assert_eq!(c["id"], "playbooks/dataplex");
    assert_eq!(c["type"], "Playbook");
    assert_eq!(c["title"], "Dataplex Playbook");
    assert_eq!(c["description"], "How to onboard Dataplex.");
    assert_eq!(c["resource"], "/playbooks/dataplex");
    assert_eq!(c["tags"][0], "data");
    assert_eq!(c["tags"][1], "governance");
    assert_eq!(c["timestamp"], "2026-05-22");
    assert!(c["body"].as_str().unwrap().contains("# Dataplex Playbook"));
    assert_eq!(c["links"][0]["href"], "/tables/customers.md");
    assert_eq!(c["citations"][0]["href"], "https://cloud.google.com/blog/x");
    assert_eq!(c["extra"]["owner"], "data-team");
}

#[test]
fn fmt_is_idempotent() {
    // A document with loose spacing; fmt canonicalizes, and re-fmt is a no-op.
    let src = vec![(
        "m/a.md".to_string(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId {1}\n".to_string(),
    )];
    let once = fmt_bundle(&src);
    let twice = fmt_bundle(&once);
    assert_eq!(once, twice, "fmt is not idempotent");
}

fn layout_bundle() -> Vec<(String, String)> {
    vec![
        ("shop/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ("shop/account.md".into(), "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n".into()),
        ("shop/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        (
            // `Diagram.key` is the full bundle-relative id (`okf::id_of`), so
            // this resolves to key "shop/orders" — matching the golden
            // fixture in `crates/waml/tests/solver_golden.rs`.
            "shop/orders.md".into(),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n".into(),
        ),
    ]
}

fn sizes_200x90() -> SizeMap {
    let mut s: SizeMap = BTreeMap::new();
    for k in ["shop/customer", "shop/account", "shop/order"] {
        s.insert(k.into(), Size { w: 200.0, h: 90.0 });
    }
    s
}

#[test]
fn solve_bundle_matches_golden_rects() {
    let r = solve_bundle(&layout_bundle(), "shop/orders", sizes_200x90(), SolveConfig::default()).unwrap();
    assert!(r.diagnostics.is_empty(), "expected no diagnostics, got: {:?}", r.diagnostics);
    assert_eq!(r.solved.nodes["shop/customer"], Rect { x: 16.0, y: 16.0, w: 200.0, h: 90.0 });
    assert_eq!(r.solved.nodes["shop/account"], Rect { x: 16.0, y: 122.0, w: 200.0, h: 90.0 });
    assert_eq!(r.solved.nodes["shop/order"], Rect { x: 264.0, y: 69.0, w: 200.0, h: 90.0 });
    // Two groups: framed "Users" shrink "Orders".
    assert_eq!(r.solved.groups.len(), 2);
}

#[test]
fn solve_bundle_unknown_key_errs() {
    let err = solve_bundle(&layout_bundle(), "nope", sizes_200x90(), SolveConfig::default()).unwrap_err();
    assert!(err.contains("nope"), "error should name missing key, got: {err}");
}

#[test]
fn solve_bundle_surfaces_unresolved_operand_diagnostic() {
    let mut b = layout_bundle();
    // Append layout line referencing non-existent operand. `left of` (not
    // `left`) so it parses as a valid Placement statement — an unresolved
    // operand is a resolve-time diagnostic, not a parse error, and a
    // malformed layout line (e.g. missing "of") is silently dropped by the
    // parser before it ever reaches resolve.
    let diagram = b.last_mut().unwrap();
    diagram.1.push_str("- Ghosts left of Orders\n");
    let r = solve_bundle(&b, "shop/orders", sizes_200x90(), SolveConfig::default()).unwrap();
    assert!(
        r.diagnostics.iter().any(|d| d.code == waml::diagnostic::DiagCode::UnresolvedLayoutRef),
        "expected unresolved-layout-ref diagnostic, got: {:?}", r.diagnostics
    );
}
