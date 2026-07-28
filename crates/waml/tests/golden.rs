use waml::parse::{build_model, parse_document, split_bundle};
use waml::serialize::serialize_document;

const FIXTURE: &str = include_str!("fixtures/orders-domain.md");

const PARSER_PLATFORM_FIXTURES: &[(&str, &str)] = &[
    ("generic.md", include_str!("fixtures/parser-platform/generic.md")),
    ("unknown-uml.md", include_str!("fixtures/parser-platform/unknown-uml.md")),
    ("index.md", include_str!("fixtures/parser-platform/index.md")),
    ("log.md", include_str!("fixtures/parser-platform/log.md")),
    ("class.md", include_str!("fixtures/parser-platform/class.md")),
    ("enum.md", include_str!("fixtures/parser-platform/enum.md")),
    ("object.md", include_str!("fixtures/parser-platform/object.md")),
    ("diagram.md", include_str!("fixtures/parser-platform/diagram.md")),
    ("activity.md", include_str!("fixtures/parser-platform/activity.md")),
    ("state-machine.md", include_str!("fixtures/parser-platform/state-machine.md")),
    ("sequence.md", include_str!("fixtures/parser-platform/sequence.md")),
    ("malformed-crlf-unicode.md", include_str!("fixtures/parser-platform/malformed-crlf-unicode.md")),
];

#[test]
fn parser_platform_baseline_keeps_every_fixture_serializable() {
    for (path, source) in PARSER_PLATFORM_FIXTURES {
        let once = serialize_document(&parse_document(source));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice, "{path}: serialize fixpoint");
    }
}

#[test]
fn parser_platform_baseline_keeps_okf_membership_and_selective_uml_claims() {
    use waml::source::SourceBundle;

    let source = SourceBundle::try_from_pairs(
        PARSER_PLATFORM_FIXTURES
            .iter()
            .map(|(path, text)| (*path, *text)),
    )
    .unwrap();
    let okf = waml::okf::Bundle::parse(&source).unwrap();
    let projection = waml::uml::project(&okf);

    assert_eq!(okf.concepts().len(), 10, "index and log are reserved");
    assert!(okf.index("/").unwrap().authored);
    assert!(okf.log("/").is_some());
    assert!(okf.concept("generic").is_some());
    assert!(okf.concept("unknown-uml").is_some());
    assert!(projection.contains_concept("class"));
    assert!(projection.contains_concept("enum"));
    assert!(!projection.contains_concept("object"));
    assert!(!projection.contains_concept("generic"));
    assert!(!projection.contains_concept("unknown-uml"));
    assert_eq!(projection.diagrams.len(), 1);

    let diagnostics = waml::validate::validate(&source.to_pairs());
    assert!(diagnostics.iter().any(|diagnostic| {
        (diagnostic.code.as_str(), diagnostic.severity, diagnostic.file.as_str(), diagnostic.line)
            == ("unknown-type", waml::diagnostic::Severity::Warning, "unknown-uml.md", 2)
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "frontmatter-not-clean"
            && diagnostic.file == "malformed-crlf-unicode.md"
            && diagnostic.line == 1
    }));
}

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
    assert_eq!(money.attributes[0].ty.name, "Decimal");
    assert_eq!(money.attributes[0].ty.ref_, None);

    // Order has 3 attributes (id, status, total); total resolves to Money.
    let order = m.node("shop/order").unwrap();
    // Title now lives ONLY on the concept (single authoritative source).
    assert_eq!(order.concept.title.as_deref(), Some("Order"));
    assert_eq!(order.attributes.len(), 3);
    let total = order.attributes.iter().find(|a| a.name == "total").unwrap();
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
    use waml::index_md::reindex_source;
    use waml::source::SourceBundle;
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
    let bundle2 = reindex_source(&SourceBundle::try_from_pairs(b.clone()).unwrap()).to_pairs();
    let m2 = build_model(&bundle2);
    // packages + members stable across the round-trip
    let names = |m: &waml::model::Model| {
        let mut v: Vec<_> = m
            .packages
            .iter()
            .map(|p| (p.key.clone(), p.members.clone()))
            .collect();
        v.sort();
        v
    };
    assert_eq!(names(&m1), names(&m2));
    // blurb from description survived into sales/index.md
    let idx = bundle2.iter().find(|(p, _)| p == "sales/index.md").unwrap();
    assert!(idx.1.contains("[Customer](./customer.md) - A buyer."));
    // second reindex is a fixpoint
    let bundle3 =
        reindex_source(&SourceBundle::try_from_pairs(bundle2.clone()).unwrap()).to_pairs();
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
