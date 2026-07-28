use waml::{
    compat::{Batch, Step},
    edit::{EditBatch, EditContext},
    model::ElementType,
    okf::{self, DirectoryAddress},
    source::{BundlePath, SourceBundle},
    syntax::Direction,
    uml,
};

fn directory(value: &str) -> DirectoryAddress {
    DirectoryAddress::parse(value).unwrap()
}

fn lower(source: &SourceBundle, steps: Vec<Step>) -> Result<SourceBundle, waml::edit::EditError> {
    let okf = waml::analysis::analyze_okf(source, None, 19).unwrap();
    let uml = uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
            okf: &okf.bundle,
            session_revision: 19,
        },
        None,
    )
    .unwrap();
    Batch::new(steps).lower(EditContext {
        source,
        okf_analysis: &okf,
        session_revision: 19,
        uml: &uml,
    })
}

fn new_class(slug: &str, directory: &str, title: &str) -> uml::Op {
    uml::Op::ClassifierNew {
        slug: slug.into(),
        directory: self::directory(directory),
        ty: ElementType::parse("uml.Class"),
        title: title.into(),
        stereotype: vec![],
        description: None,
        abstract_: false,
    }
}

#[test]
fn okf_import_then_uml_set_reads_the_inserted_candidate() {
    let imported = SourceBundle::try_from_pairs([(
        "order.md",
        "---\r\ntype: uml.Class\r\ntitle: Order\r\n---\r\n\r\n# Order\r\n\r\n## Notes\r\nkeep 😀  \r\n",
    )])
    .unwrap();
    let changed = lower(
        &SourceBundle::default(),
        vec![
            Step::Okf(okf::Op::BundleImport {
                parent: directory("/"),
                name: "sales".into(),
                bundle: imported,
            }),
            Step::Uml(uml::Op::ClassifierSet {
                id: "sales/order".into(),
                title: Some("Issued Order".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }),
        ],
    )
    .unwrap();
    let text = changed
        .document(&BundlePath::parse("sales/order.md").unwrap())
        .unwrap()
        .text();
    assert!(text.contains("title: Issued Order\r\n"));
    assert!(text.contains("## Notes\r\nkeep 😀  \r\n"));
}

#[test]
fn uml_new_then_okf_directory_retitle_observes_the_new_directory() {
    let changed = lower(
        &SourceBundle::default(),
        vec![
            Step::Uml(new_class("invoice", "/sales", "Invoice")),
            Step::Okf(okf::Op::IndexRetitle {
                directory: directory("/sales"),
                title: "Sales Domain".into(),
            }),
        ],
    )
    .unwrap();
    assert_eq!(
        changed
            .document(&BundlePath::parse("sales/index.md").unwrap())
            .unwrap()
            .text(),
        "# Sales Domain\n\n* [Invoice](./invoice.md)\n"
    );
}

#[test]
fn okf_move_then_uml_attribute_add_uses_the_rebound_path() {
    let source = SourceBundle::try_from_pairs([(
        "sales/order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            Step::Okf(okf::Op::ConceptMove {
                id: "sales/order".into(),
                to_directory: directory("/archive"),
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "archive/order".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            }),
        ],
    )
    .unwrap();
    assert!(changed
        .document(&BundlePath::parse("archive/order.md").unwrap())
        .unwrap()
        .text()
        .contains("- id: String"));
}

#[test]
fn uml_rename_then_placement_set_rewrites_and_resolves_current_ids() {
    let source = SourceBundle::try_from_pairs([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "diagram.md",
            "---\ntype: Diagram\nprofile: uml-domain\n---\n# Diagram\n\n## Members\n- [A](./a.md)\n- [B](./b.md)\n",
        ),
    ])
    .unwrap();
    let changed = lower(
        &source,
        vec![
            Step::Uml(uml::Op::ClassifierRename {
                from: "a".into(),
                to: "renamed".into(),
            }),
            Step::Uml(uml::Op::PlacementSet {
                diagram: "diagram".into(),
                subject_title: "A".into(),
                subject_slug: "renamed".into(),
                reference_title: "B".into(),
                reference_slug: "b".into(),
                directions: vec![Direction::LeftOf],
            }),
        ],
    )
    .unwrap();
    let diagram = changed
        .document(&BundlePath::parse("diagram.md").unwrap())
        .unwrap()
        .text();
    assert!(diagram.contains("[A](./renamed.md)"));
    assert!(
        diagram.contains("- [A](./renamed.md) left of [B](./b.md)"),
        "{diagram}"
    );
}

#[test]
fn final_collision_reports_original_index_and_discards_the_candidate() {
    let source = SourceBundle::try_from_pairs([
        (
            "left/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
        (
            "right/order.md",
            "---\ntype: uml.Class\ntitle: Existing\n---\n# Existing\n",
        ),
    ])
    .unwrap();
    let baseline = source.clone();
    let error = lower(
        &source,
        vec![
            Step::Uml(uml::Op::AttributeAdd {
                node: "left/order".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            }),
            Step::Okf(okf::Op::ConceptMove {
                id: "left/order".into(),
                to_directory: directory("/right"),
            }),
        ],
    )
    .unwrap_err();
    assert_eq!(error.index, 1);
    assert_eq!(source, baseline);
    assert!(source.shares_text_with(&baseline, "left/order.md"));
    assert!(source.shares_text_with(&baseline, "right/order.md"));
}

#[test]
fn imported_generic_okf_concept_does_not_become_a_uml_claim() {
    let imported =
        SourceBundle::try_from_pairs([("note.md", "---\ntype: acme.Note\n---\n# Note\n")]).unwrap();
    let error = lower(
        &SourceBundle::default(),
        vec![
            Step::Okf(okf::Op::BundleImport {
                parent: directory("/"),
                name: "notes".into(),
                bundle: imported,
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "notes/note".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            }),
        ],
    )
    .unwrap_err();
    assert_eq!(error.index, 1);
}

#[test]
fn imported_quoted_uml_type_is_immediately_available_to_uml_lowering() {
    let imported = SourceBundle::try_from_pairs([(
        "order.md",
        "---\r\ntype: \"uml.Class\"\r\ntitle: Order\r\n---\r\n\r\n# Order\r\n",
    )])
    .unwrap();
    let changed = lower(
        &SourceBundle::default(),
        vec![
            Step::Okf(okf::Op::BundleImport {
                parent: directory("/"),
                name: "sales".into(),
                bundle: imported,
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "sales/order".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            }),
        ],
    )
    .unwrap();
    assert!(changed
        .document(&BundlePath::parse("sales/order.md").unwrap())
        .unwrap()
        .text()
        .contains("- id: String\r\n"));
}

#[test]
fn imported_unclosed_frontmatter_stays_unclaimed_and_rolls_back() {
    let imported = SourceBundle::try_from_pairs([(
        "order.md",
        "---\ntype: uml.Class\ntitle: Order\n# missing close fence\n",
    )])
    .unwrap();
    let source = SourceBundle::default();
    let baseline = source.clone();
    let error = lower(
        &source,
        vec![
            Step::Okf(okf::Op::BundleImport {
                parent: directory("/"),
                name: "sales".into(),
                bundle: imported,
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "sales/order".into(),
                name: "id".into(),
                ty_token: "String".into(),
                multiplicity: None,
                visibility: None,
            }),
        ],
    )
    .unwrap_err();
    assert_eq!(error.index, 1);
    assert_eq!(source, baseline);
    assert_eq!(source.len(), 0);
}
