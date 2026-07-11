//! Native (non-wasm) tests over the pure `*_json` cores. The `#[wasm_bindgen]`
//! surface is a thin serde-wasm-bindgen shell around these, exercised in JS.
use uaml_wasm::{apply_ops_bundle, build_model_json, fmt_bundle, validate_json};

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
    assert_eq!(v["nodes"][0]["key"], "order");
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
