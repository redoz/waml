use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    edit::{EditBatch, EditContext},
    okf::{self, DirectoryAddress},
    source::SourceBundle,
};

fn lower(source: &SourceBundle, ops: Vec<okf::Op>) -> Result<SourceBundle, waml::edit::EditError> {
    let okf = analyze_okf(source, None, 7)?;
    let uml = waml::uml::analyze(
        DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 7,
        },
        None,
    )?;
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
    assert!(index.starts_with("# Commerce\r\n\r\nIntro.\r\n\r\n"));
    assert!(index.contains("* [Order](./order.md)\r\n\r\n## Notes\r\n"));
    assert!(index.ends_with("## Notes\r\nKeep me.\r\n"));
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

#[test]
fn cumulative_index_edits_rewrite_only_the_confirmed_member_block() {
    let unknown_section = "## Notes 😀\r\n\r\n\
* [Keep café reference](./zebra.md)\r\n\
* [External α](https://example.com/α)\r\n";
    let source = SourceBundle::try_from_pairs([
        (
            "sales/index.md",
            format!(
                "# Sales\r\n\r\nIntro Ω.\r\n\r\n\
* [Zebra](./zebra.md)\r\n\
* [Alpha](./alpha.md)\r\n\r\n\
{unknown_section}"
            ),
        ),
        ("sales/zebra.md", "# Zebra\r\n".to_owned()),
        ("sales/alpha.md", "# Alpha\r\n".to_owned()),
    ])
    .unwrap();
    let operations = vec![
        okf::Op::IndexReorder {
            directory: directory("/sales"),
            order: vec!["sales/zebra".into(), "sales/alpha".into()],
        },
        okf::Op::IndexRetitle {
            directory: directory("/sales"),
            title: "Café Sales".into(),
        },
        okf::Op::IndexSort {
            directory: directory("/sales"),
        },
        okf::Op::IndexSort {
            directory: directory("/sales"),
        },
    ];

    let candidate = lower(&source, operations.clone()).unwrap();
    let index_path = waml::source::BundlePath::parse("sales/index.md").unwrap();
    let index = candidate.document(&index_path).unwrap().text();
    assert!(index.starts_with("# Café Sales\r\n\r\nIntro Ω.\r\n\r\n"));
    let generated_end = index.find("## Notes 😀").unwrap();
    let generated = &index[..generated_end];
    assert!(generated.find("./alpha.md").unwrap() < generated.find("./zebra.md").unwrap());
    assert_eq!(&index[generated_end..], unknown_section);

    let repeated = lower(&candidate, operations).unwrap();
    assert_eq!(repeated.document(&index_path).unwrap().text(), index);
}

#[test]
fn nested_unknown_headings_bound_the_confirmed_member_preamble() {
    let unknown_sections = "### Références 😀\r\n\r\n\
* [Alpha mention](./alpha.md)\r\n\r\n\
#### Über details\r\n\r\n\
* [Zebra mention](./zebra.md)\r\n";
    let source = SourceBundle::try_from_pairs([
        (
            "sales/index.md",
            format!(
                "# Sales\r\n\r\nIntro Ω.\r\n\r\n\
* [Zebra](./zebra.md)\r\n\
* [Alpha](./alpha.md)\r\n\r\n\
{unknown_sections}"
            ),
        ),
        ("sales/zebra.md", "# Zebra\r\n".to_owned()),
        ("sales/alpha.md", "# Alpha\r\n".to_owned()),
    ])
    .unwrap();
    let operations = vec![
        okf::Op::IndexReorder {
            directory: directory("/sales"),
            order: vec!["sales/zebra".into(), "sales/alpha".into()],
        },
        okf::Op::IndexRetitle {
            directory: directory("/sales"),
            title: "Café Sales".into(),
        },
        okf::Op::IndexSort {
            directory: directory("/sales"),
        },
    ];

    let candidate = lower(&source, operations.clone()).unwrap();
    let index_path = waml::source::BundlePath::parse("sales/index.md").unwrap();
    let index = candidate.document(&index_path).unwrap().text();
    let nested_start = index.find("### Références 😀").unwrap();
    let generated = &index[..nested_start];
    assert!(generated.find("./alpha.md").unwrap() < generated.find("./zebra.md").unwrap());
    assert_eq!(&index[nested_start..], unknown_sections);

    let repeated = lower(&candidate, operations).unwrap();
    assert_eq!(repeated.document(&index_path).unwrap().text(), index);
}
