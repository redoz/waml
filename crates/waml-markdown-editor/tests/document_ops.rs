use std::sync::Arc;

use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    selection::{Affinity, Selection, SelectionError, SelectionSet, TextPosition},
};
use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextSize};

fn snapshot(text: &str, revision: u64) -> Arc<MarkdownDocumentSnapshot> {
    let text = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::new(revision),
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    Arc::new(MarkdownDocumentSnapshot::new(syntax))
}

#[test]
fn document_snapshot_shares_the_syntax_text_and_builds_one_line_index() {
    let snapshot = snapshot("a\r\nβ\n", 7);
    assert_eq!(snapshot.revision().get(), 7);
    assert_eq!(snapshot.text().shared().as_str(), "a\r\nβ\n");
    assert_eq!(
        snapshot
            .line_index()
            .line_col(snapshot.text(), TextSize::try_from_usize(5).unwrap())
            .unwrap()
            .line,
        1
    );
}

#[test]
fn selection_set_rejects_wrong_revision_and_non_boundaries() {
    let snapshot = snapshot("a😀b", 3);
    let inside_emoji = TextSize::try_from_usize(2).unwrap();
    assert!(matches!(
        SelectionSet::single(
            &snapshot,
            Selection::caret(TextPosition::new(inside_emoji, Affinity::Before))
        ),
        Err(SelectionError::InvalidBoundary { offset }) if offset == inside_emoji
    ));
    let set = SelectionSet::caret(&snapshot, TextSize::try_from_usize(1).unwrap()).unwrap();
    assert_eq!(set.revision(), snapshot.revision());
    assert_eq!(set.primary_index(), 0);
}

#[test]
fn overlapping_selections_are_sorted_and_normalized() {
    let snapshot = snapshot("abcdef", 4);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let set = SelectionSet::from_selections(
        &snapshot,
        vec![
            Selection::new(p(4), p(1)),
            Selection::new(p(3), p(5)),
            Selection::caret(p(0)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(set.as_slice().len(), 2);
    assert_eq!(set.as_slice()[1].range().start().to_usize(), 1);
    assert_eq!(set.as_slice()[1].range().end().to_usize(), 5);
    assert_eq!(set.primary_index(), 1);
}
