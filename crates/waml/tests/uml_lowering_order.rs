use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    edit::{EditBatch, EditContext},
    model::ElementType,
    okf::DirectoryAddress,
    source::SourceBundle,
    syntax::Direction,
    uml::{self, FieldEdit},
};

fn lower(source: &SourceBundle, ops: Vec<uml::Op>) -> Result<SourceBundle, waml::edit::EditError> {
    let okf = analyze_okf(source, None, 7).unwrap();
    let uml = uml::analyze(
        DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
            okf: &okf.bundle,
            session_revision: 7,
        },
        None,
    )
    .unwrap();
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
