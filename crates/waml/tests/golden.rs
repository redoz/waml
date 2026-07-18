use waml::parse::{build_model, parse_document, split_bundle};
use waml::serialize::serialize_document;

const FIXTURE: &str = include_str!("fixtures/orders-domain.md");

#[test]
fn orders_domain_builds_the_expected_model() {
    let bundle = split_bundle(FIXTURE);
    let m = build_model(&bundle);

    // Five classifiers, one diagram.
    assert_eq!(m.nodes.len(), 5);
    assert_eq!(m.diagrams.len(), 1);
    assert_eq!(
        m.diagrams[0]
            .groups
            .iter()
            .map(|g| g.members.len())
            .sum::<usize>(),
        5
    );

    // Two edges: composes + associates.
    assert_eq!(m.edges.len(), 2);
    let kinds: Vec<_> = m.edges.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"composes"));
    assert!(kinds.contains(&"associates"));

    // The composition target resolves and carries the far role.
    let comp = m
        .edges
        .iter()
        .find(|e| e.kind.as_str() == "composes")
        .unwrap();
    assert_eq!(comp.source, "shop/order");
    assert_eq!(comp.target, "shop/order-line");
    assert_eq!(comp.to_end.role.as_deref(), Some("lines"));

    // The associates edge (declared on order.md as "1 order to 1 customer")
    // resolves order -> customer, near role "order" and far role "customer".
    let assoc = m
        .edges
        .iter()
        .find(|e| e.kind.as_str() == "associates")
        .unwrap();
    assert_eq!(assoc.source, "shop/order");
    assert_eq!(assoc.target, "shop/customer");
    assert_eq!(assoc.from_end.role.as_deref(), Some("order"));
    assert_eq!(assoc.to_end.role.as_deref(), Some("customer"));

    // The Money value-object's own attribute types are bare tokens (no matching docs).
    let money = m.node("shop/money").unwrap();
    assert_eq!(money.attributes()[0].ty.name, "Decimal");
    assert_eq!(money.attributes()[0].ty.ref_, None);

    // Order has 3 attributes (id, status, total); total resolves to Money.
    let order = m.node("shop/order").unwrap();
    assert_eq!(order.label, "Order");
    assert_eq!(order.attributes().len(), 3);
    let total = order
        .attributes()
        .iter()
        .find(|a| a.name == "total")
        .unwrap();
    assert_eq!(total.ty.name, "Money");
    assert_eq!(total.ty.ref_.as_deref(), Some("shop/money"));
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
fn nested_packages_round_trip_through_reindex() {
    use waml::index_md::reindex_bundle;
    let b = vec![
        (
            "sales/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "sales/customer.md".to_string(),
            "---\ntype: uml.Class\ntitle: Customer\ndescription: A buyer.\n---\n# Customer\n"
                .to_string(),
        ),
        (
            "sales/orders/line.md".to_string(),
            "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".to_string(),
        ),
    ];
    let m1 = build_model(&b);
    let bundle2 = reindex_bundle(&b);
    let m2 = build_model(&bundle2);
    // packages + members stable across the round-trip
    let names = |m: &waml::model::Model| {
        let mut v: Vec<_> = m
            .packages
            .iter()
            .map(|p| (p.key.clone(), p.members().to_vec()))
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&m1), names(&m2));
    // blurb from description survived into sales/index.md
    let idx = bundle2.iter().find(|(p, _)| p == "sales/index.md").unwrap();
    assert!(idx.1.contains("[Customer](./customer.md) - A buyer."));
    // second reindex is a fixpoint
    let bundle3 = reindex_bundle(&bundle2);
    assert_eq!(
        bundle2
            .iter()
            .find(|(p, _)| p == "sales/index.md")
            .unwrap()
            .1,
        bundle3
            .iter()
            .find(|(p, _)| p == "sales/index.md")
            .unwrap()
            .1
    );
}

#[test]
fn orders_domain_has_no_diagnostics() {
    let bundle = waml::parse::split_bundle(FIXTURE);
    let diags = waml::validate::validate(&bundle);
    assert!(diags.is_empty(), "expected clean fixture, got: {diags:?}");
}
