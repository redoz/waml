#![cfg(feature = "serde")]
//! Pins the JSON shape of `Model` to the TS field names in
//! `packages/okf/src/types.ts`. If a rename drifts, this fails.
use waml::diagnostic::{DiagCode, Diagnostic, Severity};
use waml::model::{AssocName, BehaviorKind, ElementType, Model, Node, UmlMetaclass, Visibility};
use waml::multiplicity::Multiplicity;
use waml::parse::build_model;

fn bundle() -> Vec<(String, String)> {
    vec![
        (
            "m/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n\n## Relationships\n- composes [Line](./line.md): 1 to 1..*\n".into(),
        ),
        (
            "m/line.md".into(),
            "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".into(),
        ),
    ]
}

#[test]
fn node_reshape_keeps_wire_flat_and_accessors_work() {
    let m = build_model(&[(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\nstereotype: [entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string(),
    )]);
    // Object model: Concept is OFF the node; it lives in Model.concepts.
    let n = m.nodes.iter().find(|n| n.key == "shop/order").unwrap();
    assert_eq!(n.label, "Order");
    assert!(n.kind.is_classifier());
    assert_eq!(n.attributes().len(), 1);
    assert_eq!(n.stereotypes(), ["entity".to_string()]);
    assert_eq!(
        m.concept("shop/order").and_then(|c| c.title.as_deref()),
        Some("Order")
    );
    // Wire is still flat + carries the re-joined concept.
    let v = serde_json::to_value(waml::wire::build_wire(&m)).unwrap();
    let wn = &v["nodes"][0];
    assert_eq!(wn["type"], "uml.Class");
    assert_eq!(wn["key"], "shop/order");
    assert_eq!(wn["concept"]["title"], "Order");
    assert_eq!(wn["stereotypes"][0], "entity");
    assert_eq!(wn["attributes"][0]["name"], "id");
}

#[test]
fn wire_json_matches_ts_field_names() {
    let model = build_model(&bundle());
    let wire = waml::wire::build_wire(&model);
    let v = serde_json::to_value(&wire).unwrap();

    let node = &v["nodes"][0];
    assert_eq!(node["type"], "uml.Class");
    assert_eq!(node["key"], "m/order");
    assert_eq!(node["concept"]["id"], "m/order");
    assert_eq!(node["concept"]["title"], "Order");
    assert_eq!(node["attributes"][0]["name"], "id");
    assert_eq!(node["attributes"][0]["type"]["name"], "OrderId");

    let edge = &v["edges"][0];
    assert_eq!(edge["kind"], "composes");
    assert_eq!(edge["from"], "m/order");
    assert_eq!(edge["to"], "m/line");
}

#[test]
fn wire_diagram_members_are_flattened_in_rust() {
    // A diagram with a nested group forest must surface a FLAT `members` list.
    let b = vec![(
        "d.md".to_string(),
        "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Group A\n- [X](./x.md)\n\n#### Sub\n- [Y](./y.md)\n".to_string(),
    ),
        ("x.md".to_string(), "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string()),
        ("y.md".to_string(), "---\ntype: uml.Class\ntitle: Y\n---\n# Y\n".to_string()),
    ];
    let wire = waml::wire::build_wire(&build_model(&b));
    let v = serde_json::to_value(&wire).unwrap();
    let members = v["diagrams"][0]["members"].as_array().unwrap();
    assert!(
        members.iter().any(|m| m == "x"),
        "flat members must include x: {members:?}"
    );
    assert!(
        members.iter().any(|m| m == "y"),
        "flat nested members must include y: {members:?}"
    );
    assert!(
        v["diagrams"][0].get("groups").is_none(),
        "wire diagram has no groups: {}",
        v["diagrams"][0]
    );
}

#[test]
fn stringy_newtypes_serialize_as_their_canonical_string() {
    // Multiplicity ⇒ bare string.
    assert_eq!(
        serde_json::to_value(Multiplicity::parse("1..*").unwrap()).unwrap(),
        serde_json::json!("1..*")
    );
    // Visibility ⇒ single-char marker string.
    assert_eq!(
        serde_json::to_value(Visibility::Private).unwrap(),
        serde_json::json!("-")
    );
}

#[test]
fn assoc_name_matches_ts_union_shape() {
    // TS: name?: string | { ref: string }
    assert_eq!(
        serde_json::to_value(AssocName::Label("has".into())).unwrap(),
        serde_json::json!("has")
    );
    assert_eq!(
        serde_json::to_value(AssocName::Assoc("employment".into())).unwrap(),
        serde_json::json!({ "ref": "employment" })
    );
}

#[test]
fn package_node_and_model_path() {
    use waml::model::NodeKind;
    use waml::uml::{Classifier, ClassifierKind, Structural, UmlNode};
    let pkg = Node {
        key: "sales".into(),
        label: "sales".into(),
        kind: NodeKind::Uml(UmlNode::Structural(Structural::Package {
            members: vec!["order".into(), "customer".into()],
        })),
    };
    let model = Model {
        nodes: vec![],
        edges: vec![],
        diagrams: vec![],
        path: "acme-model".into(),
        packages: vec![pkg],
        ..Default::default()
    };
    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("\"path\":\"acme-model\""));
    assert!(json.contains("\"members\":[\"order\",\"customer\"]"));
    let bare = Node {
        key: "order".into(),
        label: "Order".into(),
        kind: NodeKind::Uml(UmlNode::Classifier(Classifier {
            kind: ClassifierKind::Class,
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
        })),
    };
    let bj = serde_json::to_string(&bare).unwrap();
    assert!(
        !bj.contains("members"),
        "a classifier has no members field: {bj}"
    );
}

#[test]
fn flow_doc_json_matches_ts_field_names() {
    let b = vec![
        ("m/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("m/lifecycle.md".to_string(),
         "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- else transitions to Cancelled\n\n### Placed\n- entry: `reserveStock`\n\n### Cancelled\n\n### final\n".to_string()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let f = &v["flows"][0];
    assert_eq!(f["key"], "m/lifecycle");
    assert_eq!(f["flavor"], "stateMachine");
    assert_eq!(f["describes"], "m/order");
    assert_eq!(f["nodes"][0]["kind"], "initial");
    assert_eq!(f["nodes"][2]["entry"], "reserveStock");
    let e = &f["edges"][1];
    assert_eq!(e["from"], "Draft");
    assert_eq!(e["trigger"], "place");
    assert_eq!(e["guard"], "items > 0");
    assert_eq!(e["effect"], "reserve");
    assert_eq!(f["edges"][2]["else"], true);
    // classifier-only models omit the field entirely
    let m2 = build_model(&vec![(
        "a.md".to_string(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string(),
    )]);
    let v2 = serde_json::to_value(&m2).unwrap();
    assert!(v2.get("flows").is_none());
}

#[test]
fn sequence_doc_json_matches_ts_field_names() {
    let b = vec![
        ("s/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("s/seq.md".to_string(),
         "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls order: `tick()`\n- opt\n  - when `ready`\n    - order sends order: `go()`\n".to_string()),
    ];
    let m = build_model(&b);
    let v = serde_json::to_value(&m).unwrap();
    let s = &v["interactions"][0];
    assert_eq!(s["lifelines"][0]["ref"], "s/order");
    assert_eq!(s["lifelines"][0]["alias"], "order");
    assert_eq!(s["messages"][0]["item"], "message");
    assert_eq!(s["messages"][0]["verb"], "calls");
    assert_eq!(s["messages"][0]["signature"], "tick()");
    assert_eq!(s["messages"][1]["item"], "fragment");
    assert_eq!(s["messages"][1]["kind"], "opt");
    assert_eq!(s["messages"][1]["operands"][0]["guard"], "ready");
}

#[test]
fn diagnostic_serializes_with_kebab_code_and_lowercase_severity() {
    let d = Diagnostic::new(DiagCode::UnresolvedTarget, "gone", "a.md", 3);
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["severity"], "error");
    assert_eq!(v["code"], "unresolved-target");
    assert_eq!(v["message"], "gone");
    assert_eq!(v["file"], "a.md");
    assert_eq!(v["line"], 3);
    // Severity round-trips as its lowercase string.
    assert_eq!(
        serde_json::to_value(Severity::Warning).unwrap(),
        serde_json::json!("warning")
    );
}

#[test]
fn classifier_type_wire_strings_are_stable() {
    assert_eq!(
        serde_json::to_string(&ElementType::Uml(UmlMetaclass::Class)).unwrap(),
        "\"uml.Class\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Behavior(BehaviorKind::Activity)).unwrap(),
        "\"uml.Activity\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Diagram).unwrap(),
        "\"Diagram\""
    );
    assert_eq!(
        serde_json::to_string(&ElementType::Unknown("bpmn.Task".to_string())).unwrap(),
        "\"bpmn.Task\""
    );
    // Deserialize round-trips through `From<String>`.
    let ct: ElementType = serde_json::from_str("\"uml.Class\"").unwrap();
    assert_eq!(ct, ElementType::Uml(UmlMetaclass::Class));
}
