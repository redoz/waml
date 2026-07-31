use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect,
    write_green_to, MarkdownReparseOutcome, SourceText, TextChange, TextRange, TextSize,
};

#[test]
fn snapshot_is_revisioned_immutable_and_query_ready() {
    let revision = DocumentRevision::INITIAL.checked_next().unwrap();
    let first = parse_markdown(
        revision,
        SourceText::new("# one\n").unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(first.revision(), revision);
    assert_eq!(first.text().shared().as_str(), "# one\n");
    let mut recovered = String::new();
    write_green_to(first.tree().root_green(), &mut recovered).unwrap();
    assert_eq!(recovered, "# one\n");
    assert_eq!(first.diagnostics().len(), first.tree().diagnostics().len());
    for (snapshot, tree) in first.diagnostics().iter().zip(first.tree().diagnostics()) {
        assert_eq!(snapshot.code, tree.code);
        assert_eq!(snapshot.severity, tree.severity);
        assert_eq!(snapshot.range, tree.range);
    }

    let update = reparse_markdown(
        &first,
        revision.checked_next().unwrap(),
        SourceText::new("# two\n").unwrap(),
        &[TextChange {
            old_range: TextRange::new(TextSize::new(2), TextSize::new(5)).unwrap(),
            replacement: "two".into(),
        }],
    )
    .unwrap();
    assert_eq!(update.snapshot.text().shared().as_str(), "# two\n");
    assert!(!update.affected_ranges.is_empty());
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental { .. }
            | MarkdownReparseOutcome::Full { .. }
    ));
}
