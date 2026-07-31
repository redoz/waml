use std::sync::Arc;

use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    edit::{EditCommand, HistoryGroup, MarkdownEdit, MarkdownEditError},
    selection::{Affinity, Selection, SelectionError, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
};

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

#[test]
fn primary_tracks_the_requested_adjacent_selection() {
    let snapshot = snapshot("ab", 5);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let set = SelectionSet::from_selections(
        &snapshot,
        vec![Selection::new(p(0), p(1)), Selection::new(p(2), p(1))],
        1,
    )
    .unwrap();

    assert_eq!(set.primary_index(), 1);
}

fn replace(start: usize, end: usize, replacement: &str) -> TextChange {
    TextChange {
        old_range: TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
        replacement: Arc::from(replacement),
    }
}

#[test]
fn exact_edit_advances_once_and_reuses_the_returned_syntax_update() {
    let before = snapshot("# A\n", 10);
    let expected_text = SourceText::from_shared(Arc::new("# Bee\n".to_owned())).unwrap();
    let after_selection = SelectionSet::caret_in_text(
        DocumentRevision::new(11),
        &expected_text,
        TextSize::try_from_usize(5).unwrap(),
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::new(before);
    let proposal = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(10),
            changes: vec![replace(2, 3, "Bee")],
            selection_after: after_selection,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap();
    assert_eq!(session.local_revision().get(), 11);
    assert_eq!(session.snapshot().text().shared().as_str(), "# Bee\n");
    assert!(Arc::ptr_eq(
        session.snapshot().syntax(),
        &proposal.syntax_update.snapshot
    ));
    assert_eq!(proposal.edit.changes.len(), 1);
}

#[test]
fn stale_edit_reports_current_revision_without_mutation() {
    let before = snapshot("abc", 5);
    let selections = SelectionSet::caret(&before, TextSize::try_from_usize(0).unwrap()).unwrap();
    let mut session = MarkdownDocumentSession::new(before.clone());
    let error = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(4),
            changes: vec![replace(0, 0, "x")],
            selection_after: selections,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::StaleRevision {
            base,
            current,
        } if base == DocumentRevision::new(4)
            && current == DocumentRevision::new(5)
    ));
    assert_eq!(session.snapshot().text().shared().as_str(), "abc");
}

#[test]
fn invalid_utf8_change_is_typed_and_does_not_advance() {
    let before = snapshot("a😀b", 8);
    let selections = SelectionSet::caret_in_text(
        DocumentRevision::new(9),
        before.text(),
        TextSize::try_from_usize(0).unwrap(),
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::new(before);
    let error = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(8),
            changes: vec![replace(2, 2, "x")],
            selection_after: selections,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(error, MarkdownEditError::InvalidBoundary { .. }));
    assert_eq!(session.local_revision().get(), 8);
}

#[test]
fn multi_selection_insert_is_lowered_from_end_to_start() {
    let before = snapshot("ab cd", 20);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::caret(p(1)), Selection::new(p(3), p(5))],
        1,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    let outcome = session
        .execute(EditCommand::Insert(Arc::from("X")), HistoryGroup::named(9))
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aXb X");
    assert_eq!(outcome.proposal.unwrap().edit.changes.len(), 2);
}

#[test]
fn overlapping_selections_are_normalized_before_one_delete() {
    let before = snapshot("abcdef", 30);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::new(p(1), p(4)), Selection::new(p(3), p(5))],
        0,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    let outcome = session
        .execute(EditCommand::DeleteBackward, HistoryGroup::named(2))
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "af");
    assert_eq!(outcome.proposal.unwrap().edit.changes.len(), 1);
}

#[test]
fn grouped_undo_and_redo_restore_source_and_selection_together() {
    let before = snapshot("", 40);
    let mut session = MarkdownDocumentSession::new(before);
    for ch in ["a", "b", "c"] {
        session
            .execute(EditCommand::Insert(Arc::from(ch)), HistoryGroup::named(1))
            .unwrap();
    }
    assert!(session.can_undo());
    let undo = session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "");
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 0);
    assert_eq!(undo.edit.changes.len(), 1);
    session.redo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "abc");
}

#[test]
fn paste_cut_and_indent_keep_raw_markdown_in_transactions() {
    let before = snapshot("- a\n- b\n", 50);
    let mut session = MarkdownDocumentSession::new(before);
    session.select_all().unwrap();
    let cut = session
        .execute(EditCommand::Cut, HistoryGroup::isolated())
        .unwrap();
    assert_eq!(cut.clipboard.as_deref(), Some("- a\n- b\n"));
    assert_eq!(session.snapshot().text().shared().as_str(), "");
    session
        .execute(
            EditCommand::Paste(Arc::from("- a\n- b\n")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    session.select_all().unwrap();
    session
        .execute(EditCommand::Indent { spaces: 2 }, HistoryGroup::isolated())
        .unwrap();
    assert_eq!(
        session.snapshot().text().shared().as_str(),
        "  - a\n  - b\n"
    );
}

#[test]
fn history_break_does_not_create_an_empty_undo_step() {
    let before = snapshot("", 60);
    let mut session = MarkdownDocumentSession::new(before);
    session.break_history_group();
    assert!(!session.can_undo());
    assert!(session.undo().unwrap().is_none());
}

#[test]
fn indent_uses_crlf_logical_lines_and_translates_selection() {
    let before = snapshot("a\r\nb\r\n", 61);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections =
        SelectionSet::from_selections(&before, vec![Selection::new(p(0), p(4))], 0).unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    session
        .execute(EditCommand::Indent { spaces: 2 }, HistoryGroup::isolated())
        .unwrap();
    assert_eq!(
        session.snapshot().text().shared().as_str(),
        "  a\r\n  b\r\n"
    );
    assert_eq!(session.selections().primary().range().start().to_usize(), 2);
}

#[test]
fn closing_delimiter_skips_only_its_matching_caret() {
    let before = snapshot("() x", 62);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::caret(p(1)), Selection::caret(p(4))],
        0,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    session
        .execute(
            EditCommand::Insert(Arc::from(")")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "() x)");
    assert_eq!(session.selections().as_slice().len(), 2);
    assert_eq!(
        session.selections().as_slice()[0].cursor.offset.to_usize(),
        2
    );
}

#[test]
fn mixed_closer_skip_keeps_the_skipped_primary_selection_primary() {
    let before = snapshot("() x", 63);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::caret(p(1)), Selection::caret(p(4))],
        0,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    session
        .execute(
            EditCommand::Insert(Arc::from(")")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 2);
}
