use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    edit::{EditBatch, EditContext},
    okf::{self, DirectoryAddress},
    source::SourceBundle,
};

fn lower(source: &SourceBundle, ops: Vec<okf::Op>) -> Result<SourceBundle, waml::edit::EditError> {
    let okf = analyze_okf(source, None, 7).unwrap();
    let uml = waml::uml::analyze(
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
    okf::Batch(ops).lower(EditContext {
        source,
        okf_analysis: &okf,
        session_revision: 7,
        uml: &uml,
    })
}

fn directory(path: &str) -> DirectoryAddress {
    DirectoryAddress::parse(path).unwrap()
}

#[test]
fn import_then_retitle_observes_the_imported_directory_and_preserves_unknown_markdown() {
    let source = SourceBundle::try_from_pairs([("untouched.md", "# Untouched\r\n")]).unwrap();
    let imported = SourceBundle::try_from_pairs([
        (
            "template/index.md",
            "# Template\r\n\r\nIntro.\r\n\r\n<!-- keep -->\r\n",
        ),
        (
            "template/order.md",
            "---\r\ntype: Note\r\n---\r\n# Order\r\n",
        ),
    ])
    .unwrap();

    let candidate = lower(
        &source,
        vec![
            okf::Op::BundleImport {
                parent: directory("/"),
                name: "sales".into(),
                bundle: imported,
            },
            okf::Op::IndexRetitle {
                directory: directory("/sales"),
                title: "Sales Domain".into(),
            },
        ],
    )
    .unwrap();

    let index = candidate
        .document(&waml::source::BundlePath::parse("sales/index.md").unwrap())
        .unwrap()
        .text();
    assert!(index.starts_with("# Sales Domain\r\n"));
    assert!(index.contains("Intro.\r\n"));
    assert!(index.contains("<!-- keep -->\r\n"));
    assert!(source.shares_text_with(&candidate, "untouched.md"));
}

#[test]
fn rename_then_retitle_uses_the_new_path_and_preserves_crlf_body() {
    let source = SourceBundle::try_from_pairs([
        (
            "sales/index.md",
            "# Sales\r\n\r\nIntro.\r\n\r\n## Notes\r\nKeep me.\r\n",
        ),
        ("sales/order.md", "# Order\r\n"),
        ("untouched.md", "# Untouched\r\n"),
    ])
    .unwrap();

    let candidate = lower(
        &source,
        vec![
            okf::Op::DirectoryRename {
                directory: directory("/sales"),
                name: "commerce".into(),
            },
            okf::Op::IndexRetitle {
                directory: directory("/commerce"),
                title: "Commerce".into(),
            },
        ],
    )
    .unwrap();

    let index = candidate
        .document(&waml::source::BundlePath::parse("commerce/index.md").unwrap())
        .unwrap()
        .text();
    assert!(index.starts_with("# Commerce\r\n\r\nIntro.\r\n\r\n## Notes\r\nKeep me.\r\n"));
    assert!(index.ends_with("* [Order](./order.md)\r\n"));
    assert!(source.shares_text_with(&candidate, "untouched.md"));
}

#[test]
fn two_edits_to_one_synthesized_index_observe_the_first_edit() {
    let source = SourceBundle::try_from_pairs([
        ("sales/zebra.md", "# Zebra\n"),
        ("sales/alpha.md", "# Alpha\n"),
    ])
    .unwrap();

    let candidate = lower(
        &source,
        vec![
            okf::Op::IndexRetitle {
                directory: directory("/sales"),
                title: "Sales Domain".into(),
            },
            okf::Op::IndexSort {
                directory: directory("/sales"),
            },
        ],
    )
    .unwrap();

    let index = candidate
        .document(&waml::source::BundlePath::parse("sales/index.md").unwrap())
        .unwrap()
        .text();
    assert!(index.starts_with("# Sales Domain\n"));
    assert!(index.find("alpha.md").unwrap() < index.find("zebra.md").unwrap());
}

#[test]
fn late_collision_reports_the_stable_step_and_leaves_input_unchanged() {
    let source = SourceBundle::try_from_pairs([
        ("sales/order.md", "# Order\n"),
        ("archive/existing.md", "# Existing\n"),
        ("untouched.md", "# Untouched\n"),
    ])
    .unwrap();
    let original = source.clone();

    let error = lower(
        &source,
        vec![
            okf::Op::DirectoryRename {
                directory: directory("/sales"),
                name: "commerce".into(),
            },
            okf::Op::DirectoryRename {
                directory: directory("/commerce"),
                name: "archive".into(),
            },
        ],
    )
    .unwrap_err();

    assert_eq!(error.index, 1);
    assert_eq!(error.op, "pkg.rename");
    assert_eq!(source, original);
    assert!(source.shares_text_with(&original, "untouched.md"));
}
