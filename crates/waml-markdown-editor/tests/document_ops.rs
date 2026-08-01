use std::sync::Arc;

use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    edit::{EditCommand, HistoryGroup, HostSnapshotMismatch, MarkdownEdit, MarkdownEditError},
    input::ScrollState,
    selection::{Affinity, Selection, SelectionError, SelectionSet, TextPosition},
    session::{HostSnapshotCause, HostSyncOutcome, MarkdownDocumentSession},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
};

fn snapshot(text: &str, revision: u64) -> Arc<MarkdownDocumentSnapshot> {
    let text = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    snapshot_from_source(text, revision)
}

fn snapshot_from_source(text: SourceText, revision: u64) -> Arc<MarkdownDocumentSnapshot> {
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
    assert_eq!(session.selections().primary().range().start().to_usize(), 0);
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

#[test]
fn grouped_history_replays_multi_change_entries_in_one_coordinate_space() {
    let before = snapshot("ab cd ef", 64);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![
            Selection::caret(p(1)),
            Selection::new(p(3), p(5)),
            Selection::caret(p(8)),
        ],
        0,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    session
        .execute(EditCommand::Insert(Arc::from("X")), HistoryGroup::named(7))
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aXb X efX");
    session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "ab cd ef");
    session.redo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aXb X efX");
}

#[test]
fn insert_delete_replace_matrix_has_exact_changes_and_one_revision_step() {
    struct Case {
        source: &'static str,
        selection: (usize, usize),
        command: EditCommand,
        old_range: (usize, usize),
        replacement: &'static str,
        result: &'static str,
    }
    let cases = [
        Case {
            source: "",
            selection: (0, 0),
            command: EditCommand::Insert(Arc::from("x")),
            old_range: (0, 0),
            replacement: "x",
            result: "x",
        },
        Case {
            source: "ab",
            selection: (2, 2),
            command: EditCommand::DeleteBackward,
            old_range: (1, 2),
            replacement: "",
            result: "a",
        },
        Case {
            source: "abc",
            selection: (1, 2),
            command: EditCommand::Insert(Arc::from("XY")),
            old_range: (1, 2),
            replacement: "XY",
            result: "aXYc",
        },
    ];
    for case in cases {
        let before = snapshot(case.source, 70);
        let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
        let selections = SelectionSet::single(
            &before,
            Selection::new(p(case.selection.0), p(case.selection.1)),
        )
        .unwrap();
        let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
        let proposal = session
            .execute(case.command, HistoryGroup::isolated())
            .unwrap()
            .proposal
            .unwrap();
        assert_eq!(proposal.edit.base_revision, DocumentRevision::new(70));
        assert_eq!(proposal.edit.changes.len(), 1);
        assert_eq!(
            proposal.edit.changes[0].old_range,
            TextRange::new(
                TextSize::try_from_usize(case.old_range.0).unwrap(),
                TextSize::try_from_usize(case.old_range.1).unwrap(),
            )
            .unwrap()
        );
        assert_eq!(
            proposal.edit.changes[0].replacement.as_ref(),
            case.replacement
        );
        assert_eq!(session.snapshot().text().shared().as_str(), case.result);
        assert_eq!(session.local_revision(), DocumentRevision::new(71));
        assert_eq!(session.selections().revision(), DocumentRevision::new(71));
    }
}

#[test]
fn stale_proposal_after_one_accepted_local_edit_is_rejected_exactly() {
    let before = snapshot("ab", 80);
    let stale_selection = SelectionSet::caret_in_text(
        DocumentRevision::new(81),
        before.text(),
        TextSize::try_from_usize(0).unwrap(),
    )
    .unwrap();
    let stale = MarkdownEdit {
        base_revision: DocumentRevision::new(80),
        changes: vec![replace(0, 0, "stale")],
        selection_after: stale_selection,
        history_group: HistoryGroup::isolated(),
    };
    let mut session = MarkdownDocumentSession::new(before);
    session
        .execute(
            EditCommand::Insert(Arc::from("x")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    let error = session.apply_edit(stale).unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::StaleRevision { base, current }
            if base == DocumentRevision::new(80) && current == DocumentRevision::new(81)
    ));
    assert_eq!(session.snapshot().text().shared().as_str(), "xab");
}

#[test]
fn explicit_history_break_splits_named_insert_groups() {
    let mut session = MarkdownDocumentSession::new(snapshot("", 90));
    session
        .execute(EditCommand::Insert(Arc::from("a")), HistoryGroup::named(1))
        .unwrap();
    session.break_history_group();
    session
        .execute(EditCommand::Insert(Arc::from("b")), HistoryGroup::named(1))
        .unwrap();
    session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "a");
    session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "");
}

#[test]
fn failed_undo_keeps_history_group_available_and_ordered() {
    let mut session = MarkdownDocumentSession::new(snapshot("", u64::MAX - 1));
    session
        .execute(
            EditCommand::Insert(Arc::from("a")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    for _ in 0..2 {
        let error = session.undo().unwrap_err();
        assert!(matches!(
            error,
            MarkdownEditError::RevisionOverflow { current }
                if current == DocumentRevision::new(u64::MAX)
        ));
        assert!(session.can_undo());
        assert!(!session.can_redo());
        assert_eq!(session.snapshot().text().shared().as_str(), "a");
    }
}

#[test]
fn failed_redo_keeps_history_group_available_and_ordered() {
    let mut session = MarkdownDocumentSession::new(snapshot("", u64::MAX - 2));
    session
        .execute(
            EditCommand::Insert(Arc::from("a")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    session.undo().unwrap().unwrap();
    assert_eq!(session.local_revision(), DocumentRevision::new(u64::MAX));
    for _ in 0..2 {
        let error = session.redo().unwrap_err();
        assert!(matches!(
            error,
            MarkdownEditError::RevisionOverflow { current }
                if current == DocumentRevision::new(u64::MAX)
        ));
        assert!(session.can_redo());
        assert!(!session.can_undo());
        assert_eq!(session.snapshot().text().shared().as_str(), "");
    }
}

#[test]
fn read_only_rejects_all_direct_mutation_apis_without_state_change() {
    let mut session = MarkdownDocumentSession::new(snapshot("ab", 100));
    session
        .execute(
            EditCommand::Insert(Arc::from("x")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    session.set_read_only(true);
    let before = session.snapshot().clone();
    let selections = session.selections().clone();
    for command in [
        EditCommand::Insert(Arc::from("x")),
        EditCommand::ReplaceSelections(Arc::from("x")),
        EditCommand::Paste(Arc::from("x")),
        EditCommand::Cut,
        EditCommand::DeleteBackward,
        EditCommand::DeleteForward,
        EditCommand::Indent { spaces: 2 },
        EditCommand::Outdent { spaces: 2 },
    ] {
        assert!(matches!(
            session.execute(command, HistoryGroup::isolated()),
            Err(MarkdownEditError::ReadOnly)
        ));
    }
    assert!(matches!(session.undo(), Err(MarkdownEditError::ReadOnly)));
    assert!(matches!(session.redo(), Err(MarkdownEditError::ReadOnly)));
    assert!(Arc::ptr_eq(session.snapshot(), &before));
    assert_eq!(session.selections(), &selections);
    assert!(session.can_undo());
}

#[test]
fn host_acknowledgement_keeps_selection_scroll_history_and_ime() {
    let mut session = MarkdownDocumentSession::new(snapshot("# A\n", 10));
    let proposal = session
        .execute(EditCommand::Insert(Arc::from("x")), HistoryGroup::named(1))
        .unwrap()
        .proposal
        .unwrap();
    session.set_scroll_state(ScrollState { x: 3.0, y: 48.0 });
    session.begin_ime().unwrap();
    let selection = session.selections().clone();

    assert_eq!(
        session
            .synchronize_from_host(
                proposal.snapshot.clone(),
                Some(&proposal.edit.changes),
                HostSnapshotCause::AcknowledgedLocalEdit,
            )
            .unwrap(),
        HostSyncOutcome::Acknowledged
    );
    assert!(Arc::ptr_eq(session.snapshot(), &proposal.snapshot));
    assert_eq!(session.selections(), &selection);
    assert_eq!(session.scroll_state().x, 3.0);
    assert_eq!(session.scroll_state().y, 48.0);
    assert!(session.can_undo());
    assert!(session.ime().is_some());
}

#[test]
fn host_stale_snapshot_is_ignored_without_mutating_local_state() {
    let stale = snapshot("abc", 20);
    let mut session = MarkdownDocumentSession::new(stale.clone());
    session
        .execute(EditCommand::Insert(Arc::from("x")), HistoryGroup::named(2))
        .unwrap();
    session.set_scroll_state(ScrollState { x: 1.0, y: 32.0 });
    session.begin_ime().unwrap();
    let current = session.snapshot().clone();
    let selection = session.selections().clone();

    assert_eq!(
        session
            .synchronize_from_host(stale, None, HostSnapshotCause::ExternalReplacement)
            .unwrap(),
        HostSyncOutcome::IgnoredStale
    );
    assert!(Arc::ptr_eq(session.snapshot(), &current));
    assert_eq!(session.selections(), &selection);
    assert_eq!(session.scroll_state().x, 1.0);
    assert_eq!(session.scroll_state().y, 32.0);
    assert!(session.can_undo());
    assert!(session.ime().is_some());
}

#[test]
fn host_application_history_maps_selection_and_clears_local_transients() {
    let mut session = MarkdownDocumentSession::new(snapshot("abc", 30));
    session.set_primary_offset(TextSize::new(2)).unwrap();
    session
        .execute(EditCommand::Insert(Arc::from("x")), HistoryGroup::named(3))
        .unwrap();
    session.undo().unwrap().unwrap();
    assert!(session.can_redo());
    session.set_scroll_state(ScrollState { x: 0.0, y: 24.0 });
    session.begin_ime().unwrap();
    let change = replace(1, 1, "Z");
    let incoming = snapshot("aZbc", 33);

    assert_eq!(
        session
            .synchronize_from_host(
                incoming.clone(),
                Some(std::slice::from_ref(&change)),
                HostSnapshotCause::ApplicationHistory,
            )
            .unwrap(),
        HostSyncOutcome::Installed
    );
    assert!(Arc::ptr_eq(session.snapshot(), &incoming));
    assert_eq!(
        session.selections().primary().cursor.offset,
        TextSize::new(3)
    );
    assert_eq!(session.selections().revision(), DocumentRevision::new(33));
    assert_eq!(session.scroll_state().y, 24.0);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert!(session.ime().is_none());
}

#[test]
fn host_external_replacement_translates_selection_through_supplied_changes() {
    let current = snapshot("abcdef", 35);
    let selections = SelectionSet::caret(&current, TextSize::new(5)).unwrap();
    let mut session = MarkdownDocumentSession::with_selections(current, selections).unwrap();
    session.set_scroll_state(ScrollState { x: 0.0, y: 18.0 });
    let change = replace(1, 3, "XYZ");
    let incoming = snapshot("aXYZdef", 36);

    assert_eq!(
        session
            .synchronize_from_host(
                incoming,
                Some(std::slice::from_ref(&change)),
                HostSnapshotCause::ExternalReplacement,
            )
            .unwrap(),
        HostSyncOutcome::Installed
    );
    assert_eq!(
        session.selections().primary().cursor.offset,
        TextSize::new(6)
    );
    assert_eq!(session.scroll_state().y, 18.0);
}

#[test]
fn host_external_replacement_without_a_map_resets_selection_and_scroll() {
    let mut session = MarkdownDocumentSession::new(snapshot("abc", 40));
    session.set_primary_offset(TextSize::new(2)).unwrap();
    session
        .execute(EditCommand::Insert(Arc::from("x")), HistoryGroup::named(4))
        .unwrap();
    session.set_scroll_state(ScrollState { x: 2.0, y: 64.0 });
    session.begin_ime().unwrap();
    let change = replace(0, 4, "replacement");
    let incoming = snapshot("replacement", 42);

    assert_eq!(
        session
            .synchronize_from_host(
                incoming.clone(),
                Some(std::slice::from_ref(&change)),
                HostSnapshotCause::ExternalReplacement,
            )
            .unwrap(),
        HostSyncOutcome::Installed
    );
    assert!(Arc::ptr_eq(session.snapshot(), &incoming));
    assert_eq!(
        session.selections().primary().cursor.offset,
        TextSize::new(0)
    );
    assert_eq!(session.scroll_state().x, 0.0);
    assert_eq!(session.scroll_state().y, 0.0);
    assert!(!session.can_undo());
    assert!(session.ime().is_none());
}

#[test]
fn host_initial_load_installs_the_supplied_snapshot_arc() {
    let mut session = MarkdownDocumentSession::new(snapshot("", 0));
    let incoming = snapshot("# Loaded\n", 1);

    assert_eq!(
        session
            .synchronize_from_host(incoming.clone(), None, HostSnapshotCause::InitialLoad)
            .unwrap(),
        HostSyncOutcome::Installed
    );
    assert!(Arc::ptr_eq(session.snapshot(), &incoming));
    assert_eq!(session.snapshot().text().shared().as_str(), "# Loaded\n");
}

#[test]
fn host_acknowledgement_rejects_a_different_revision_without_mutation() {
    let current = snapshot("abc", 50);
    let mut session = MarkdownDocumentSession::new(current.clone());
    let incoming = snapshot("abc", 51);

    let error = session
        .synchronize_from_host(incoming, None, HostSnapshotCause::AcknowledgedLocalEdit)
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(HostSnapshotMismatch::AcknowledgementRevision {
            local,
            incoming,
        }) if local == DocumentRevision::new(50) && incoming == DocumentRevision::new(51)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}

#[test]
fn host_equal_revision_with_different_text_is_a_typed_error() {
    let current = snapshot("abc", 60);
    let mut session = MarkdownDocumentSession::new(current.clone());
    let incoming = snapshot("abd", 60);

    let error = session
        .synchronize_from_host(incoming, None, HostSnapshotCause::ExternalReplacement)
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(HostSnapshotMismatch::EqualRevisionText { revision })
            if revision == DocumentRevision::new(60)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}

#[test]
fn host_equal_revision_with_equal_bytes_but_different_text_identity_is_a_typed_error() {
    let current = snapshot("abc", 65);
    let incoming = snapshot("abc", 65);
    let mut session = MarkdownDocumentSession::new(current.clone());

    let error = session
        .synchronize_from_host(incoming, None, HostSnapshotCause::ExternalReplacement)
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(HostSnapshotMismatch::EqualRevisionText { revision })
            if revision == DocumentRevision::new(65)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}

#[test]
fn host_equal_revision_with_different_syntax_identity_is_a_typed_error() {
    let current = snapshot("abc", 70);
    let incoming = snapshot_from_source(current.text().clone(), 70);
    let mut session = MarkdownDocumentSession::new(current.clone());

    let error = session
        .synchronize_from_host(incoming, None, HostSnapshotCause::ApplicationHistory)
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(
            HostSnapshotMismatch::EqualRevisionSyntaxIdentity { revision }
        ) if revision == DocumentRevision::new(70)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}

#[test]
fn host_invalid_changes_are_typed_and_leave_state_unchanged() {
    let current = snapshot("abc", 80);
    let mut session = MarkdownDocumentSession::new(current.clone());
    let incoming = snapshot("xyabc", 81);
    let changes = [replace(0, 0, "x"), replace(0, 0, "y")];

    let error = session
        .synchronize_from_host(
            incoming,
            Some(&changes),
            HostSnapshotCause::ExternalReplacement,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(HostSnapshotMismatch::InvalidChanges {
            base,
            incoming,
            ..
        }) if base == DocumentRevision::new(80) && incoming == DocumentRevision::new(81)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}

#[test]
fn host_changes_must_produce_the_supplied_snapshot() {
    let current = snapshot("abc", 90);
    let mut session = MarkdownDocumentSession::new(current.clone());
    let incoming = snapshot("ayc", 91);
    let wrong_change = replace(1, 2, "x");

    let error = session
        .synchronize_from_host(
            incoming,
            Some(std::slice::from_ref(&wrong_change)),
            HostSnapshotCause::ApplicationHistory,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::HostSnapshot(HostSnapshotMismatch::ChangesDoNotProduceSnapshot {
            base,
            incoming,
        }) if base == DocumentRevision::new(90) && incoming == DocumentRevision::new(91)
    ));
    assert!(Arc::ptr_eq(session.snapshot(), &current));
}
