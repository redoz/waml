use std::sync::Arc;

use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, MarkdownLinkKind,
    MarkdownReparseOutcome, SourceText, TextChange, TextRange, TextSize,
};

fn text(value: &str) -> SourceText {
    SourceText::new(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn reference_destinations(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    snapshot
        .queries()
        .links()
        .filter(|link| link.kind == MarkdownLinkKind::Reference)
        .map(|link| link.destination.to_string())
        .collect()
}

#[test]
fn definition_change_updates_non_contiguous_reference_dependents() {
    // This fails if definition edits only reparse their local shell window and
    // leave reference annotations in non-contiguous paragraphs unchanged.
    let old = "[id]: /one\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let new = "[id]: /two\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();

    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(6, 10),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();
    let oracle = parse_markdown(
        DocumentRevision::new(1),
        text(new),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();

    assert_eq!(
        reference_destinations(&update.snapshot),
        vec!["/two", "/two"]
    );
    assert_eq!(
        reference_destinations(&update.snapshot),
        reference_destinations(&oracle)
    );
    assert!(update.affected_ranges.len() >= 2);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental {
            reparsed_range: None,
            ..
        }
    ));
}

#[test]
fn local_edit_publishes_the_caller_source_and_a_single_normalized_range() {
    // This fails if direct incremental reparse allocates another source or if
    // normalization reports an unrelated shell window.
    let old = "# one\n";
    let new = "# two\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let next_source = text(new);
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        next_source.clone(),
        &[TextChange {
            old_range: range(2, 5),
            replacement: Arc::from("two"),
        }],
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        update.snapshot.text().shared(),
        next_source.shared()
    ));
    assert_eq!(update.snapshot.tree().write_to_string(), new);
    assert_eq!(update.affected_ranges.len(), 1);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental {
            shared_source_independent_green,
            reparsed_range: Some(_),
        } if shared_source_independent_green > 0
    ));
}
