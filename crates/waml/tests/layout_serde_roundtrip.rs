//! Guards that Diagram.layout survives a serde round-trip now that it is no
//! longer `#[serde(skip)]`. Requires the `serde` feature (dev-deps enable it via
//! serde_json; the crate's own `serde` feature must be on for the derives).
#![cfg(feature = "serde")]

use waml::parse::build_model;

fn bundle() -> Vec<(String, String)> {
    vec![
        ("shop/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ("shop/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        (
            "shop/orders-domain.md".into(),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n".into(),
        ),
    ]
}

#[test]
fn diagram_layout_survives_serde_roundtrip() {
    let model = build_model(&bundle());
    let diagram = &model.diagrams[0];
    assert!(!diagram.layout.is_empty(), "fixture must have layout statements");

    let json = serde_json::to_string(diagram).unwrap();
    let back: waml::model::Diagram = serde_json::from_str(&json).unwrap();

    assert_eq!(back.layout, diagram.layout, "layout must round-trip byte-equal");
}
