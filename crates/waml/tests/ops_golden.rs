use waml::ops::{apply, Op};
use waml::bundle_envelope::split_bundle;

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

    let out = apply(
        &bundle,
        &[Op::NodeRename {
            from: "order-line".into(),
            to: "line-item".into(),
        }],
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

    let out = apply(
        &bundle,
        &[Op::PkgRetitle {
            path: "sales".into(),
            title: "Sales Domain".into(),
        }],
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

    let out = apply(
        &bundle,
        &[Op::NodeRename {
            from: "order-line".into(),
            to: "line-item".into(),
        }],
    )
    .unwrap();
    let order = &out.iter().find(|(path, _)| path == "order.md").unwrap().1;
    assert!(order.contains("- depends [OrderLine](./line-item.md)\n"));
    assert!(order.contains("- malformed [OrderLine](./order-line.md\n"));
    assert!(order.contains("Example only: [OrderLine](./order-line.md)\n"));
}
