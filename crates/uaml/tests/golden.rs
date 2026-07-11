use uaml::parse::{build_model, parse_document, split_bundle};
use uaml::serialize::serialize_document;

const FIXTURE: &str = include_str!("fixtures/orders-domain.md");

#[test]
fn orders_domain_builds_the_expected_model() {
    let bundle = split_bundle(FIXTURE);
    let m = build_model(&bundle);

    // Five classifiers, one diagram.
    assert_eq!(m.nodes.len(), 5);
    assert_eq!(m.diagrams.len(), 1);
    assert_eq!(m.diagrams[0].members.len(), 5);

    // Two edges: composes + associates.
    assert_eq!(m.edges.len(), 2);
    let kinds: Vec<_> = m.edges.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"composes"));
    assert!(kinds.contains(&"associates"));

    // The composition target resolves and carries the far role.
    let comp = m.edges.iter().find(|e| e.kind.as_str() == "composes").unwrap();
    assert_eq!(comp.source, "order");
    assert_eq!(comp.target, "order-line");
    assert_eq!(comp.to_end.role.as_deref(), Some("lines"));

    // The Money value-object's attribute types are bare tokens (no matching docs).
    let money = m.node("money").unwrap();
    assert_eq!(money.attributes[0].ty.name, "Decimal");
    assert_eq!(money.attributes[0].ty.ref_, None);
}

#[test]
fn every_doc_is_a_serialize_fixpoint() {
    for (_path, text) in split_bundle(FIXTURE) {
        let once = serialize_document(&parse_document(&text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice, "serialize must be idempotent per document");
    }
}
