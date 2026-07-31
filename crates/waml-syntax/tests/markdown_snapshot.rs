use waml_syntax::{
    parse_markdown, reparse_markdown, write_green_to, DocumentRevision, MarkdownDialect,
    MarkdownReparseOutcome, SourceText, TextChange, TextRange, TextSize,
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
    let expected = TextRange::new(TextSize::new(0), TextSize::new(6)).unwrap();
    assert_eq!(update.affected_ranges.as_ref(), &[expected]);
    match update.outcome {
        MarkdownReparseOutcome::Incremental {
            shared_source_independent_green,
            reparsed_range,
        } => {
            assert!(shared_source_independent_green > 0);
            assert_eq!(reparsed_range, Some(expected));
        }
        MarkdownReparseOutcome::Full { reason } => {
            panic!("same-width heading edit unexpectedly required full reparse: {reason:?}")
        }
    }
}

#[test]
fn reparse_rejects_same_and_older_revisions() {
    let revision = DocumentRevision::new(4);
    let first = parse_markdown(
        revision,
        SourceText::new("# one\n").unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    for requested in [revision, DocumentRevision::new(3)] {
        let error = reparse_markdown(&first, requested, SourceText::new("# one\n").unwrap(), &[])
            .err()
            .expect("non-monotonic revision must fail");
        assert!(matches!(
            error,
            waml_syntax::ParseError::NonMonotonicRevision {
                previous,
                requested: actual,
            } if previous == revision && actual == requested
        ));
    }
}

#[test]
fn width_changing_reparse_reports_new_snapshot_coordinates() {
    let revision = DocumentRevision::INITIAL;
    let first = parse_markdown(
        revision,
        SourceText::new("# one\n").unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &first,
        revision.checked_next().unwrap(),
        SourceText::new("# substantially longer\n").unwrap(),
        &[TextChange {
            old_range: TextRange::new(TextSize::new(2), TextSize::new(5)).unwrap(),
            replacement: "substantially longer".into(),
        }],
    )
    .unwrap();
    let expected = TextRange::new(TextSize::new(0), TextSize::new(23)).unwrap();

    assert_eq!(update.affected_ranges.as_ref(), &[expected]);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental {
            reparsed_range: Some(range),
            ..
        } if range == expected
    ));
}

#[test]
fn dialect_profiles_control_existing_markdown_structure_options() {
    let source = "| left | right |\n| --- | --- |\n| one | two |\n";
    let commonmark = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::COMMONMARK_0_31_2,
    )
    .unwrap();
    let waml = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    assert!(commonmark.structure().protected_ranges.is_empty());
    assert!(!waml.structure().protected_ranges.is_empty());
}
