use waml::source::SourceBundle;

/// `rel.add` writes the target's title into the relationship line, and reads
/// that title out of frontmatter.
///
/// The UML side used to read that value by splitting the entry's line on `:`
/// and unquoting the remainder itself — a second, hand-rolled reading of a
/// grammar the parser owns, and one that knew only double quotes. A
/// single-quoted title reached the relationship still wearing its quotes,
/// while the `okf` side of the same crate read it correctly through the tree.
#[test]
fn a_single_quoted_title_reaches_a_relationship_decoded() {
    let bundle = SourceBundle::try_from_pairs([
        (
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
        (
            "line.md",
            "---\ntype: uml.Class\ntitle: 'Order Line'\n---\n# Order Line\n",
        ),
    ])
    .unwrap();
    let out = waml::edit::apply(
        &bundle,
        &waml::edit::Batch::new(vec![waml::edit::Step::Uml(
            waml::uml::Op::RelationshipAdd {
                source: "order".into(),
                kind: waml::model::RelationshipKind::Associates,
                target: "line".into(),
                name: None,
                ends: Some((
                    waml::model::RelEnd::default(),
                    waml::model::RelEnd::default(),
                )),
            },
        )]),
    )
    .expect("rel.add applies");
    let order = out
        .to_pairs()
        .into_iter()
        .find(|(p, _)| p == "order.md")
        .unwrap()
        .1;
    assert!(
        !order.contains("'Order Line'"),
        "the title must reach the relationship decoded:\n{order}"
    );
}
