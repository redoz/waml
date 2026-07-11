//! Native (non-wasm) tests over the pure `*_json` cores. The `#[wasm_bindgen]`
//! surface is a thin serde-wasm-bindgen shell around these, exercised in JS.
use uaml_wasm::{build_model_json, validate_json};

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
