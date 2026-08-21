use waml::bundle_envelope::split_bundle;
use waml::edit::{apply, Batch, EditError, Step};
use waml::okf::{self, DirectoryAddress};
use waml::source::SourceBundle;
use waml::uml;

type Pairs = Vec<(String, String)>;

fn apply_pairs(bundle: &[(String, String)], steps: Vec<Step>) -> Result<Pairs, EditError> {
    let source =
        SourceBundle::try_from_pairs(bundle.iter().cloned()).expect("golden fixture is valid");
    apply(&source, &Batch::new(steps)).map(|bundle| bundle.to_pairs())
}

fn base(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .strip_suffix(".md")
        .unwrap_or(path)
        .to_string()
}

#[test]
fn rename_on_orders_domain_fixture_rewrites_all_referrers() {
    let blob = include_str!("fixtures/orders-domain.md");
    let bundle = split_bundle(blob)
        .expect("orders-domain envelope is valid")
        .expect("orders-domain fixture is an envelope");
    // Pick a slug the fixture actually defines and references. `order-line` is
    // composed by `order` and appears in the diagram's members. If the fixture
    // used a different slug, this would need to be retargeted.
    assert!(
        bundle.iter().any(|(p, _)| base(p) == "order-line"),
        "fixture defines order-line"
    );

    let out = apply_pairs(
        &bundle,
        vec![Step::Uml(uml::Op::ClassifierRename {
            from: "order-line".into(),
            to: "line-item".into(),
        })],
    )
    .unwrap();

    // The renamed doc is re-keyed to its new basename (directory preserved).
    assert!(
        out.iter().any(|(p, _)| base(p) == "line-item"),
        "renamed doc re-keyed to new basename"
    );
    assert!(
        !out.iter().any(|(p, _)| base(p) == "order-line"),
        "old slug no longer present"
    );

    // order.md's `composes` relationship target is rewritten.
    let order = &out.iter().find(|(p, _)| base(p) == "order").unwrap().1;
    assert!(
        order.contains("composes [OrderLine](./line-item.md)"),
        "order.md composes-target rewritten:\n{order}"
    );
    assert!(
        !order.contains("order-line.md"),
        "no stale link left in order.md"
    );

    // orders-domain.md's diagram member link is rewritten too.
    let diagram = &out
        .iter()
        .find(|(p, _)| base(p) == "orders-domain")
        .unwrap()
        .1;
    assert!(
        diagram.contains("[OrderLine](./line-item.md)"),
        "diagram member link rewritten:\n{diagram}"
    );
    assert!(
        !diagram.contains("order-line.md"),
        "no stale link left in orders-domain.md"
    );
}

#[test]
fn legacy_retitle_preserves_unknown_index_markdown_and_crlf() {
    let bundle = vec![
        (
            "sales/index.md".to_owned(),
            "# Sales\r\n\r\nIntro.\r\n\r\n## Notes\r\nKeep me.\r\n".to_owned(),
        ),
        ("sales/order.md".to_owned(), "# Order\r\n".to_owned()),
    ];

    let out = apply_pairs(
        &bundle,
        vec![Step::Okf(okf::Op::IndexRetitle {
            directory: DirectoryAddress::parse("/sales").unwrap(),
            title: "Sales Domain".into(),
        })],
    )
    .unwrap();

    let index = &out
        .iter()
        .find(|(path, _)| path == "sales/index.md")
        .unwrap()
        .1;
    assert!(index.starts_with("# Sales Domain\r\n\r\nIntro.\r\n"));
    assert!(index.contains("## Notes\r\nKeep me.\r\n"));
    assert!(index.contains("* [Order](./order.md)\r\n\r\n## Notes\r\n"));
    assert!(index.ends_with("## Notes\r\nKeep me.\r\n"));
}

#[test]
fn uml_rename_preserves_unknown_and_malformed_text_byte_for_byte() {
    let bundle = vec![
        (
            "order-line.md".to_owned(),
            "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_owned(),
        ),
        (
            "order.md".to_owned(),
            concat!(
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n",
                "## Relationships\n",
                "- depends [OrderLine](./order-line.md)\n",
                "- malformed [OrderLine](./order-line.md\n\n",
                "## Operations\n",
                "Example only: [OrderLine](./order-line.md)\n",
            )
            .to_owned(),
        ),
    ];

    let out = apply_pairs(
        &bundle,
        vec![Step::Uml(uml::Op::ClassifierRename {
            from: "order-line".into(),
            to: "line-item".into(),
        })],
    )
    .unwrap();
    let order = &out.iter().find(|(path, _)| path == "order.md").unwrap().1;
    assert!(order.contains("- depends [OrderLine](./line-item.md)\n"));
    assert!(order.contains("- malformed [OrderLine](./order-line.md\n"));
    assert!(order.contains("Example only: [OrderLine](./order-line.md)\n"));
}

/// `place.set` renders each endpoint as a href resolved relative to the diagram,
/// but the clear-then-append pass used to look for a needle it built itself as
/// `./{slug}.md`. For a bare, same-directory slug those agree by accident; for a
/// qualified slug they never do, so nothing was ever cleared and every call
/// appended another copy of the same placement. `place.rm` shared the needle and
/// so removed nothing at all.
///
/// Found by the `apply -> write -> reparse` property in
/// `waml-ops-dto/tests/edit_roundtrip_properties.rs`.
#[test]
fn placement_set_and_rm_address_a_qualified_slug() {
    let bundle: Pairs = vec![
        (
            "shop/order.md".to_owned(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_owned(),
        ),
        (
            "shop/customer.md".to_owned(),
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_owned(),
        ),
        (
            "shop/dia.md".to_owned(),
            concat!(
                "---\ntype: uml.ClassDiagram\ntitle: Dia\n---\n# Dia\n\n",
                "## Members\n- [Order](./order.md)\n- [Customer](./customer.md)\n",
            )
            .to_owned(),
        ),
    ];

    let set = || {
        Step::Uml(uml::Op::PlacementSet {
            diagram: "shop/dia".into(),
            subject_title: "Order".into(),
            subject_slug: "shop/order".into(),
            reference_title: "Customer".into(),
            reference_slug: "shop/customer".into(),
            directions: vec![waml::layout::Direction::LeftOf],
        })
    };
    let placements = |pairs: &Pairs| -> usize {
        pairs
            .iter()
            .find(|(path, _)| path == "shop/dia.md")
            .unwrap()
            .1
            .matches("left of")
            .count()
    };

    let once = apply_pairs(&bundle, vec![set()]).unwrap();
    assert_eq!(placements(&once), 1, "one placement after the first set");

    let twice = apply_pairs(&once, vec![set()]).unwrap();
    assert_eq!(twice, once, "place.set must replace, not append");

    let removed = apply_pairs(
        &twice,
        vec![Step::Uml(uml::Op::PlacementRemove {
            diagram: "shop/dia".into(),
            subject_slug: "shop/order".into(),
            reference_slug: "shop/customer".into(),
        })],
    )
    .unwrap();
    assert_eq!(
        placements(&removed),
        0,
        "place.rm must remove a placement it can address"
    );
}

/// `place.set` writes a `## Layout` section, which only a diagram has any use
/// for. Aiming one at a classifier used to succeed silently: the result parses
/// and round-trips, so nothing downstream ever objected.
#[test]
fn placement_ops_refuse_a_target_that_is_not_a_diagram() {
    let bundle = vec![
        (
            "order.md".to_string(),
            "---
type: uml.Class
title: Order
---
# Order
"
            .to_string(),
        ),
        (
            "customer.md".to_string(),
            "---
type: uml.Class
title: Customer
---
# Customer
"
            .to_string(),
        ),
    ];

    let error = apply_pairs(
        &bundle,
        vec![Step::Uml(uml::Op::PlacementSet {
            diagram: "order".into(),
            subject_title: "Customer".into(),
            subject_slug: "customer".into(),
            reference_title: "Order".into(),
            reference_slug: "order".into(),
            directions: vec![waml::layout::Direction::LeftOf],
        })],
    )
    .expect_err("a classifier is not a placement target");

    assert!(
        format!("{error:?}").contains("not a diagram"),
        "place.set at a classifier must say so, got: {error:?}"
    );
}
