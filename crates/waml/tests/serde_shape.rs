#![cfg(feature = "serde")]
//! Pins the JSON shape of `Model` to the TS field names in
//! `packages/okf/src/types.ts`. If a rename drifts, this fails.
use waml::diagnostic::{DiagCode, Diagnostic, Severity};
use waml::model::{AssocName, ClassifierType, Model, Node, UmlMetaclass, Visibility};
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
fn model_json_matches_ts_field_names() {
    let model = build_model(&bundle());
    let v = serde_json::to_value(&model).unwrap();

    let node = &v["nodes"][0];
    // TS ModelNode uses `type` and `key`, not `ty`.
    assert_eq!(node["type"], "uml.Class");
    assert_eq!(node["key"], "m/order");
    // Flat title/description/body are DELETED — the concept is the single source.
    assert!(node.get("title").is_none(), "flat title deleted: {node}");
    assert!(node.get("description").is_none(), "flat description deleted: {node}");
    assert!(node.get("body").is_none(), "flat body deleted: {node}");
    assert_eq!(node["concept"]["id"], "m/order");
    assert_eq!(node["concept"]["title"], "Order");
    // Attribute.type is a TypeRef ({ name, ref? }); multiplicity is canonical string.
    assert_eq!(node["attributes"][0]["name"], "id");
    assert_eq!(node["attributes"][0]["type"]["name"], "OrderId");
    assert_eq!(node["attributes"][0]["multiplicity"], "1");

    let edge = &v["edges"][0];
    // TS ModelEdge uses `from`/`to`, kind lowercase string.
    assert_eq!(edge["kind"], "composes");
    assert_eq!(edge["from"], "m/order");
    assert_eq!(edge["to"], "m/line");
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
    let pkg = Node {
        concept: waml::okf::project("sales/index.md", "# sales\n\nSales bounded context.\n"),
        key: "sales".into(),
        ty: ClassifierType::Uml(UmlMetaclass::Package),
        stereotypes: vec![],
        abstract_: false,
        attributes: vec![],
        values: vec![],
        note_body: None,
        annotates: vec![],
        members: vec!["order".into(), "customer".into()],
    };
    let model = Model {
        nodes: vec![],
        edges: vec![],
        diagrams: vec![],
        path: "acme-model".into(),
        packages: vec![pkg],
    };
    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("\"path\":\"acme-model\""));
    assert!(json.contains("\"members\":[\"order\",\"customer\"]"));
    // classifier with no members must omit field entirely.
    let bare = Node {
        concept: waml::okf::project("order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
        key: "order".into(),
        ty: ClassifierType::Uml(UmlMetaclass::Class),
        stereotypes: vec![],
        abstract_: false,
        attributes: vec![],
        values: vec![],
        note_body: None,
        annotates: vec![],
        members: vec![],
    };
    let bj = serde_json::to_string(&bare).unwrap();
    assert!(!bj.contains("members"), "empty members must be omitted: {bj}");
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
