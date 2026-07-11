#![cfg(feature = "serde")]
//! Pins the JSON shape of `Model` to the TS field names in
//! `packages/okf/src/types.ts`. If a rename drifts, this fails.
use uaml::model::{AssocName, Visibility};
use uaml::multiplicity::Multiplicity;
use uaml::parse::build_model;

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
    assert_eq!(node["key"], "order");
    // Attribute.type is a TypeRef ({ name, ref? }); multiplicity is canonical string.
    assert_eq!(node["attributes"][0]["name"], "id");
    assert_eq!(node["attributes"][0]["type"]["name"], "OrderId");
    assert_eq!(node["attributes"][0]["multiplicity"], "1");

    let edge = &v["edges"][0];
    // TS ModelEdge uses `from`/`to`, kind lowercase string.
    assert_eq!(edge["kind"], "composes");
    assert_eq!(edge["from"], "order");
    assert_eq!(edge["to"], "line");
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
