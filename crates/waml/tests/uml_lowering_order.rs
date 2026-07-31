use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    edit::{EditBatch, EditContext},
    layout::Direction,
    model::ElementType,
    okf::DirectoryAddress,
    source::{BundlePath, SourceBundle},
    uml::{self, selector::RelBy, FieldEdit, NameSpec, RelationshipSelector},
};

fn lower(source: &SourceBundle, ops: Vec<uml::Op>) -> Result<SourceBundle, waml::edit::EditError> {
    let okf = analyze_okf(source, None, 7)?;
    let uml = uml::analyze(
        DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 7,
        },
        None,
    )?;
    uml::Batch(ops).lower(EditContext {
        source,
        okf_analysis: &okf,
        session_revision: 7,
        uml: &uml,
    })
}

fn new_class(slug: &str, title: &str) -> uml::Op {
    uml::Op::ClassifierNew {
        slug: slug.into(),
        directory: DirectoryAddress::parse("/").unwrap(),
        ty: ElementType::parse("uml.Class"),
        title: title.into(),
        stereotype: vec![],
        description: None,
        abstract_: false,
    }
}

#[test]
fn classifier_new_then_set_reads_the_inserted_candidate() {
    let source = SourceBundle::default();
    let changed = lower(
        &source,
        vec![
            new_class("invoice", "Invoice"),
            uml::Op::ClassifierSet {
                id: "invoice".into(),
                title: Some("Issued Invoice".into()),
                description: Some("Handed to the customer.".into()),
                stereotype: None,
                abstract_: None,
                ty: None,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed.document_by_concept_id("invoice").unwrap().text(),
        "---\ntype: uml.Class\ntitle: Issued Invoice\ndescription: Handed to the customer.\n---\n\n# Issued Invoice\n"
    );
}

#[test]
fn classifier_new_then_attribute_add_then_set_is_cumulative() {
    let source = SourceBundle::default();
    let changed = lower(
        &source,
        vec![
            new_class("invoice", "Invoice"),
            uml::Op::AttributeAdd {
                node: "invoice".into(),
                name: "number".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
            uml::Op::AttributeSet {
                node: "invoice".into(),
                name: "number".into(),
                ty_token: Some("InvoiceNumber".into()),
                multiplicity: FieldEdit::Unchanged,
                visibility: None,
                rename: Some("id".into()),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed.document_by_concept_id("invoice").unwrap().text(),
        "---\ntype: uml.Class\ntitle: Invoice\n---\n\n# Invoice\n\n## Attributes\n- id: InvoiceNumber\n"
    );
}

#[test]
fn classifier_rename_then_attribute_add_uses_the_new_path() {
    let source = SourceBundle::try_from_pairs([(
        "sales/order.md",
        "---\r\ntype: uml.Class\r\ntitle: Order\r\n---\r\n\r\n# Order\r\n\r\n## Operations\r\nkeep **exactly**\r\n",
    )])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::ClassifierRename {
                from: "order".into(),
                to: "invoice".into(),
            },
            uml::Op::AttributeAdd {
                node: "invoice".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed
            .documents()
            .iter()
            .find(|document| document.path().as_str() == "sales/invoice.md")
            .unwrap()
            .text(),
        "---\r\ntype: uml.Class\r\ntitle: Order\r\n---\r\n\r\n# Order\r\n\r\n## Operations\r\nkeep **exactly**\r\n\r\n## Attributes\r\n- id: String\r\n"
    );
}

#[test]
fn placement_set_then_remove_resolves_the_reparsed_layout() {
    let source = SourceBundle::try_from_pairs([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "diagram.md",
            "---\ntype: Diagram\nprofile: uml-domain\n---\n# Diagram\n\n## Notes\nProtected prose.\n",
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::PlacementSet {
                diagram: "diagram".into(),
                subject_title: "A".into(),
                subject_slug: "a".into(),
                reference_title: "B".into(),
                reference_slug: "b".into(),
                directions: vec![Direction::LeftOf],
            },
            uml::Op::PlacementRemove {
                diagram: "diagram".into(),
                subject_slug: "a".into(),
                reference_slug: "b".into(),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed.document_by_concept_id("diagram").unwrap().text(),
        "---\ntype: Diagram\nprofile: uml-domain\n---\n# Diagram\n\n## Notes\nProtected prose.\n"
    );
}

#[test]
fn late_invalid_selector_reports_stable_index_and_rolls_back() {
    let source = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )])
    .unwrap();
    let baseline = source.clone();
    let error = lower(
        &source,
        vec![
            uml::Op::AttributeAdd {
                node: "order".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
            uml::Op::AttributeRemove {
                node: "order".into(),
                name: "missing".into(),
            },
        ],
    )
    .unwrap_err();
    assert_eq!(error.index, 1);
    assert!(source.shares_text_with(&baseline, "order.md"));
    assert_eq!(
        source.documents()[0].text(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"
    );
}

#[test]
fn typed_field_edits_preserve_recovery_and_raw_islands() {
    let source = SourceBundle::try_from_pairs([
        (
            "order.md",
            concat!(
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n",
                "## Attributes\n- id: String\n- broken attribute\n\n",
                "## Values\n- OPEN\nnot a value bullet\n\n",
                "## Relationships\n- depends [Customer](./customer.md)\n",
                "- depends [Broken](./broken.md\n\n",
                "## Operations\nraw [Customer](./customer.md)   \n",
            ),
        ),
        (
            "customer.md",
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::AttributeSet {
                node: "order".into(),
                name: "id".into(),
                ty_token: Some("Uuid".into()),
                multiplicity: FieldEdit::Unchanged,
                visibility: None,
                rename: None,
            },
            uml::Op::ValueRemove {
                node: "order".into(),
                literal: "OPEN".into(),
            },
            uml::Op::RelationshipSet {
                selector: RelationshipSelector {
                    source: "order".into(),
                    by: RelBy::Endpoint {
                        kind: waml::model::RelationshipKind::Depends,
                        target: "customer".into(),
                    },
                },
                ends: None,
                name: Some(NameSpec::Label("customer".into())),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed.document_by_concept_id("order").unwrap().text(),
        concat!(
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n",
            "## Attributes\n- id: Uuid\n- broken attribute\n\n",
            "## Values\nnot a value bullet\n\n",
            "## Relationships\n- depends [Customer](./customer.md) as \"customer\"\n",
            "- depends [Broken](./broken.md\n\n",
            "## Operations\nraw [Customer](./customer.md)   \n",
        )
    );
}

#[test]
fn diagram_and_layout_edits_preserve_unowned_bytes() {
    let source = SourceBundle::try_from_pairs([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "diagram.md",
            concat!(
                "---\ntype: Diagram\nprofile: uml-domain\ntitle: Old\n---\n# Old\n\n",
                "## Layout\n- malformed layout ???\n\n",
                "## Notes\nkeep  \n",
            ),
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::DiagramSet {
                key: "diagram".into(),
                title: Some("New".into()),
                description: None,
                clear_description: false,
                display: None,
            },
            uml::Op::PlacementSet {
                diagram: "diagram".into(),
                subject_title: "A".into(),
                subject_slug: "a".into(),
                reference_title: "B".into(),
                reference_slug: "b".into(),
                directions: vec![Direction::Above],
            },
            uml::Op::PlacementRemove {
                diagram: "diagram".into(),
                subject_slug: "a".into(),
                reference_slug: "b".into(),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        changed.document_by_concept_id("diagram").unwrap().text(),
        concat!(
            "---\ntype: Diagram\nprofile: uml-domain\ntitle: New\n---\n# New\n\n",
            "## Layout\n- malformed layout ???\n\n",
            "## Notes\nkeep  \n",
        )
    );
}

#[test]
fn rename_rebinds_the_exact_path_when_basenames_are_duplicated() {
    let source = SourceBundle::try_from_pairs([
        (
            "left/order.md",
            "---\ntype: uml.Class\ntitle: Left Order\n---\n# Left Order\n",
        ),
        (
            "right/order.md",
            "---\ntype: uml.Class\ntitle: Right Order\n---\n# Right Order\n\n## Attributes\n- parent: [Right Order](./order.md)\n",
        ),
        (
            "right/invoice.md",
            "---\ntype: uml.Class\ntitle: Existing Invoice\n---\n# Existing Invoice\n",
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::ClassifierRename {
                from: "left/order".into(),
                to: "invoice".into(),
            },
            uml::Op::AttributeAdd {
                node: "left/invoice".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
        ],
    )
    .unwrap();
    assert!(changed
        .documents()
        .iter()
        .any(|document| document.path().as_str() == "left/invoice.md"
            && document.text().contains("- id: String")));
    assert_eq!(
        changed
            .documents()
            .iter()
            .find(|document| document.path().as_str() == "right/invoice.md")
            .unwrap()
            .text(),
        source
            .documents()
            .iter()
            .find(|document| document.path().as_str() == "right/invoice.md")
            .unwrap()
            .text()
    );
    assert!(changed
        .document(&BundlePath::parse("right/order.md").unwrap())
        .unwrap()
        .text()
        .contains("[Right Order](./order.md)"));
}

#[test]
fn rename_accepts_an_explicit_full_destination_id() {
    let source = SourceBundle::try_from_pairs([
        (
            "left/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
        (
            "left/customer.md",
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Attributes\n- order: [Order](./order.md)\n",
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            uml::Op::ClassifierRename {
                from: "left/order".into(),
                to: "archive/invoice".into(),
            },
            uml::Op::AttributeAdd {
                node: "archive/invoice".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
        ],
    )
    .unwrap();
    let renamed = changed
        .documents()
        .iter()
        .find(|document| document.path().as_str() == "archive/invoice.md")
        .unwrap();
    assert!(renamed.text().contains("- id: String"));
    assert!(changed
        .documents()
        .iter()
        .all(|document| document.path().as_str() != "left/archive/invoice.md"));
    assert!(changed
        .document(&BundlePath::parse("left/customer.md").unwrap())
        .unwrap()
        .text()
        .contains("[Order](../archive/invoice.md)"));
}

#[test]
fn rename_rewrites_exact_typed_hrefs_from_each_referrers_path() {
    let source = SourceBundle::try_from_pairs([
        (
            "domain/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- self: [Order](./order.md?scope=self#identity)\n",
        ),
        (
            "domain/customer.md",
            "---\r\ntype: uml.Class\r\ntitle: Café\r\n---\r\n# Café\r\n\r\n## Attributes\r\n- order: [Order](./order.md#summary)\r\n",
        ),
        (
            "views/dashboard.md",
            "---\ntype: uml.Class\ntitle: Dashboard\n---\n# Dashboard\n\n## Relationships\n- associates [Order](../domain/order.md?mode=compact#card): 1 to 1 order\n\n## Notes\n[protected](../domain/order.md?keep=yes#raw)\n",
        ),
        (
            "views/deep/report.md",
            "---\ntype: Diagram\ntitle: Report\nprofile: uml-domain\n---\n# Report\n\n## Members\n- [Order](..\\..\\domain\\order.md)\n",
        ),
        (
            "other/order.md",
            "---\ntype: uml.Class\ntitle: Other Order\n---\n# Other Order\n",
        ),
        (
            "other/referrer.md",
            "---\ntype: uml.Class\ntitle: Other Referrer\n---\n# Other Referrer\n\n## Attributes\n- order: [Other Order](./order.md?keep=1#same)\n",
        ),
    ])
    .unwrap();

    let changed = lower(
        &source,
        vec![
            uml::Op::ClassifierRename {
                from: "domain/order".into(),
                to: "archive/models/invoice".into(),
            },
            uml::Op::AttributeAdd {
                node: "archive/models/invoice".into(),
                name: "number".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            },
        ],
    )
    .unwrap();

    let text = |path| {
        changed
            .document(&BundlePath::parse(path).unwrap())
            .unwrap()
            .text()
    };
    assert!(
        text("archive/models/invoice.md")
            .contains("- self: [Order](./invoice.md?scope=self#identity)"),
        "{}",
        text("archive/models/invoice.md")
    );
    assert!(text("archive/models/invoice.md").contains("- number: String"));
    assert!(text("domain/customer.md")
        .contains("- order: [Order](../archive/models/invoice.md#summary)\r\n"));
    assert!(text("domain/customer.md").contains("title: Café\r\n"));
    assert!(text("views/dashboard.md").contains(
        "- associates [Order](../archive/models/invoice.md?mode=compact#card): 1 to 1 order"
    ));
    assert!(text("views/deep/report.md").contains("- [Order](..\\..\\archive\\models\\invoice.md)"));
    assert!(text("views/dashboard.md").contains("[protected](../domain/order.md?keep=yes#raw)"));
    assert!(text("other/referrer.md").contains("- order: [Other Order](./order.md?keep=1#same)"));
}

#[test]
fn rename_collision_and_invalid_destination_keep_stable_index_and_rollback() {
    let source = SourceBundle::try_from_pairs([
        (
            "left/order.md",
            "---\ntype: uml.Class\ntitle: Left\n---\n# Left\n",
        ),
        (
            "right/order.md",
            "---\ntype: uml.Class\ntitle: Right\n---\n# Right\n",
        ),
    ])
    .unwrap();
    for destination in ["right/order", "../escape"] {
        let error = lower(
            &source,
            vec![
                uml::Op::AttributeAdd {
                    node: "left/order".into(),
                    name: "id".into(),
                    ty_token: "String".into(),
                    multiplicity: None,
                    visibility: None,
                },
                uml::Op::ClassifierRename {
                    from: "left/order".into(),
                    to: destination.into(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(error.index, 1, "{destination}");
        assert_eq!(
            source
                .documents()
                .iter()
                .find(|document| document.path().as_str() == "left/order.md")
                .unwrap()
                .text(),
            "---\ntype: uml.Class\ntitle: Left\n---\n# Left\n"
        );
    }
}
