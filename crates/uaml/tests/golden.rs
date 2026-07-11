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
    assert_eq!(m.diagrams[0].groups.iter().map(|g| g.members.len()).sum::<usize>(), 5);

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

    // The associates edge (declared on order.md as "1 order to 1 customer")
    // resolves order -> customer, near role "order" and far role "customer".
    let assoc = m.edges.iter().find(|e| e.kind.as_str() == "associates").unwrap();
    assert_eq!(assoc.source, "order");
    assert_eq!(assoc.target, "customer");
    assert_eq!(assoc.from_end.role.as_deref(), Some("order"));
    assert_eq!(assoc.to_end.role.as_deref(), Some("customer"));

    // The Money value-object's own attribute types are bare tokens (no matching docs).
    let money = m.node("money").unwrap();
    assert_eq!(money.attributes[0].ty.name, "Decimal");
    assert_eq!(money.attributes[0].ty.ref_, None);

    // Order has 3 attributes (id, status, total); total resolves to Money.
    let order = m.node("order").unwrap();
    assert_eq!(order.attributes.len(), 3);
    let total = order.attributes.iter().find(|a| a.name == "total").unwrap();
    assert_eq!(total.ty.name, "Money");
    assert_eq!(total.ty.ref_.as_deref(), Some("money"));
}

#[test]
fn every_doc_is_a_serialize_fixpoint() {
    for (_path, text) in split_bundle(FIXTURE) {
        let once = serialize_document(&parse_document(&text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice, "serialize must be idempotent per document");
    }
}

#[test]
fn orders_domain_has_no_diagnostics() {
    let bundle = uaml::parse::split_bundle(FIXTURE);
    let diags = uaml::validate::validate(&bundle);
    assert!(diags.is_empty(), "expected clean fixture, got: {diags:?}");
}
