use std::sync::Arc;

use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    edit::{EditCommand, HistoryGroup, MarkdownEditError},
    ime::ImeError,
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
    unicode::{offset_to_utf16, utf16_to_offset, PositionError, Utf16Position},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
};

fn session(text: &str) -> MarkdownDocumentSession {
    session_at_revision(text, DocumentRevision::INITIAL)
}

fn session_at_revision(text: &str, revision: DocumentRevision) -> MarkdownDocumentSession {
    let source = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    let syntax = parse_markdown(revision, source, MarkdownDialect::WAML_DEFAULT).unwrap();
    MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)))
}

#[test]
fn horizontal_navigation_moves_by_extended_grapheme_cluster() {
    let mut session = session("a👩‍💻e\u{301}z");
    session
        .set_primary_offset(TextSize::try_from_usize(1).unwrap())
        .unwrap();
    session.move_right(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 12);
    session.move_right(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 15);
    session.move_left(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 12);
}

#[test]
fn utf8_and_utf16_positions_round_trip_across_crlf_and_emoji() {
    let session = session("a\r\n😀b\n");
    let offset = TextSize::try_from_usize(7).unwrap();
    let position = offset_to_utf16(session.snapshot(), offset).unwrap();
    assert_eq!(
        position,
        Utf16Position {
            line: 1,
            character: 2
        }
    );
    assert_eq!(
        utf16_to_offset(session.snapshot(), position).unwrap(),
        offset
    );
}

#[test]
fn insertion_affinity_translates_equal_boundaries_differently() {
    let old = session("ab");
    let at = TextSize::try_from_usize(1).unwrap();
    let before = TextPosition::new(at, Affinity::Before);
    let after = TextPosition::new(at, Affinity::After);
    let change = TextChange {
        old_range: TextRange::new(at, at).unwrap(),
        replacement: Arc::from("X"),
    };
    assert_eq!(
        waml_markdown_editor::selection::translate_position(
            old.snapshot(),
            before,
            std::slice::from_ref(&change),
        )
        .unwrap()
        .offset
        .to_usize(),
        1
    );
    assert_eq!(
        waml_markdown_editor::selection::translate_position(old.snapshot(), after, &[change])
            .unwrap()
            .offset
            .to_usize(),
        2
    );
}

#[test]
fn insertion_affinity_translation_matches_session_indent_mapping() {
    let old = session("ab");
    let at = TextSize::try_from_usize(0).unwrap();
    let change = TextChange {
        old_range: TextRange::new(at, at).unwrap(),
        replacement: Arc::from(" "),
    };
    for affinity in [Affinity::Before, Affinity::After] {
        let position = TextPosition::new(at, affinity);
        let selections = SelectionSet::single(old.snapshot(), Selection::caret(position)).unwrap();
        let expected = waml_markdown_editor::selection::translate_position(
            old.snapshot(),
            position,
            std::slice::from_ref(&change),
        )
        .unwrap();

        let mut session =
            MarkdownDocumentSession::with_selections(old.snapshot().clone(), selections).unwrap();
        session
            .execute(EditCommand::Indent { spaces: 1 }, HistoryGroup::isolated())
            .unwrap();

        assert_eq!(session.selections().primary().cursor, expected);
    }
}

#[test]
fn triple_click_selects_one_logical_crlf_source_line() {
    let mut session = session("first\r\nsecond\r\n");
    let selection = session
        .select_line_at(TextSize::try_from_usize(9).unwrap())
        .unwrap();
    assert_eq!(
        session.snapshot().text().slice(selection.range()).unwrap(),
        "second\r\n"
    );
}

// Scenario: NATIVE-026
#[test]
fn ime_preedit_is_visible_state_but_not_a_published_revision() {
    let mut session = session("ab");
    session
        .set_primary_offset(TextSize::try_from_usize(1).unwrap())
        .unwrap();
    session.begin_ime().unwrap();
    session.update_ime("に", 0..1).unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
    assert_eq!(session.local_revision(), DocumentRevision::INITIAL);
    assert_eq!(session.ime().unwrap().preedit(), "に");

    let proposal = session
        .commit_ime(HistoryGroup::isolated())
        .unwrap()
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aにb");
    assert_eq!(session.local_revision().get(), 1);
    assert_eq!(proposal.edit.changes.len(), 1);
    assert!(session.ime().is_none());
}

// Scenario: NATIVE-026
#[test]
fn ime_cancel_restores_the_last_committed_snapshot_and_selection() {
    let mut session = session("a😀b");
    session
        .set_primary_offset(TextSize::try_from_usize(1).unwrap())
        .unwrap();
    let committed = session.snapshot().clone();
    let selection = session.selections().clone();
    session.begin_ime().unwrap();
    session.update_ime("漢字", 0..2).unwrap();
    session.cancel_ime();
    assert!(Arc::ptr_eq(session.snapshot(), &committed));
    assert_eq!(session.selections(), &selection);
    assert!(session.ime().is_none());
}

#[test]
fn ime_commit_revision_overflow_does_not_panic_or_discard_composition() {
    let mut session = session_at_revision("ab", DocumentRevision::new(u64::MAX));
    session
        .set_primary_offset(TextSize::try_from_usize(1).unwrap())
        .unwrap();
    session.begin_ime().unwrap();
    session.update_ime("に", 0..1).unwrap();

    let error = session.commit_ime(HistoryGroup::isolated()).unwrap_err();

    assert!(matches!(
        error,
        MarkdownEditError::RevisionOverflow { current }
            if current == DocumentRevision::new(u64::MAX)
    ));
    assert_eq!(session.ime().unwrap().preedit(), "に");
    assert_eq!(session.local_revision(), DocumentRevision::new(u64::MAX));
}

#[test]
fn lf_crlf_and_mixed_line_endings_round_trip_every_boundary() {
    for text in ["a\nb\n", "a\r\nb\r\n", "a\r\nb\nc\r\n"] {
        let session = session(text);
        for line in 0..text.lines().count() as u32 {
            for character in [0, 1] {
                let position = Utf16Position { line, character };
                if let Ok(offset) = utf16_to_offset(session.snapshot(), position) {
                    assert_eq!(
                        offset_to_utf16(session.snapshot(), offset).unwrap(),
                        position
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_utf16_columns_return_concrete_position_errors() {
    let session = session("😀\n");
    assert_eq!(
        utf16_to_offset(
            session.snapshot(),
            Utf16Position {
                line: 0,
                character: 1,
            },
        ),
        Err(PositionError::SplitUtf16Scalar {
            line: 0,
            character: 1,
        })
    );
    assert_eq!(
        utf16_to_offset(
            session.snapshot(),
            Utf16Position {
                line: 0,
                character: 3,
            },
        ),
        Err(PositionError::Utf16ColumnOutOfBounds {
            line: 0,
            character: 3,
        })
    );
    assert_eq!(
        utf16_to_offset(
            session.snapshot(),
            Utf16Position {
                line: 9,
                character: 0,
            },
        ),
        Err(PositionError::LineOutOfBounds { line: 9 })
    );
}

#[test]
fn combining_zwj_flags_and_non_latin_words_use_unicode_boundaries() {
    for text in ["e\u{301}", "👩‍💻", "🇳🇱"] {
        let mut session = session(text);
        session
            .set_primary_offset(TextSize::try_from_usize(0).unwrap())
            .unwrap();
        session.move_right(false).unwrap();
        assert_eq!(
            session.selections().primary().cursor.offset.to_usize(),
            text.len()
        );
    }
    let mut session = session("γειά 世界");
    let selection = session
        .select_word_at(TextSize::try_from_usize(1).unwrap())
        .unwrap();
    assert_eq!(
        session.snapshot().text().slice(selection.range()).unwrap(),
        "γειά"
    );
}

// Scenario: NATIVE-026
#[test]
fn ime_replaces_nonempty_selection_and_cancel_models_focus_loss() {
    let source = session("abc");
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selected = SelectionSet::single(source.snapshot(), Selection::new(p(1), p(2))).unwrap();
    let mut session =
        MarkdownDocumentSession::with_selections(source.snapshot().clone(), selected).unwrap();
    session.begin_ime().unwrap();
    session.update_ime("候", 0..1).unwrap();
    session.commit_ime(HistoryGroup::isolated()).unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "a候c");

    let committed = session.snapshot().clone();
    session.begin_ime().unwrap();
    session.update_ime("補", 0..1).unwrap();
    session.cancel_ime();
    assert!(Arc::ptr_eq(session.snapshot(), &committed));
    assert!(session.ime().is_none());
}

#[test]
fn accepted_edit_cancels_composition_and_later_update_and_commit_are_typed() {
    let mut session = session("ab");
    session.begin_ime().unwrap();
    session.update_ime("x", 0..1).unwrap();
    session
        .execute(
            EditCommand::Insert(Arc::from("y")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    let update = session.update_ime("z", 0..1).unwrap_err();
    assert_eq!(update, ImeError::NotActive);
    let commit = session.commit_ime(HistoryGroup::isolated()).unwrap_err();
    assert!(matches!(
        commit,
        MarkdownEditError::Ime(ImeError::NotActive)
    ));
}

#[test]
fn stale_edit_failure_preserves_active_ime_composition() {
    let mut session = session("ab");
    session.begin_ime().unwrap();
    session.update_ime("候", 0..1).unwrap();
    let next_text = SourceText::new("xab".to_owned()).unwrap();
    let selection_after = SelectionSet::caret_in_text(
        DocumentRevision::new(1),
        &next_text,
        TextSize::try_from_usize(1).unwrap(),
    )
    .unwrap();
    let error = session
        .apply_edit(waml_markdown_editor::edit::MarkdownEdit {
            base_revision: DocumentRevision::new(9),
            changes: vec![TextChange {
                old_range: TextRange::new(
                    TextSize::try_from_usize(0).unwrap(),
                    TextSize::try_from_usize(0).unwrap(),
                )
                .unwrap(),
                replacement: Arc::from("x"),
            }],
            selection_after,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::StaleRevision { base, current }
            if base == DocumentRevision::new(9) && current == DocumentRevision::INITIAL
    ));
    assert_eq!(session.ime().unwrap().preedit(), "候");
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
}

#[test]
fn revision_overflow_failure_preserves_active_ime_composition() {
    let mut session = session_at_revision("ab", DocumentRevision::new(u64::MAX));
    session.begin_ime().unwrap();
    session.update_ime("候", 0..1).unwrap();
    let error = session
        .execute(
            EditCommand::Insert(Arc::from("x")),
            HistoryGroup::isolated(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::RevisionOverflow { current }
            if current == DocumentRevision::new(u64::MAX)
    ));
    assert_eq!(session.ime().unwrap().preedit(), "候");
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
}

#[test]
fn selection_validation_failure_preserves_active_ime_composition() {
    let mut session = session("ab");
    session.begin_ime().unwrap();
    session.update_ime("候", 0..1).unwrap();
    let wrong_revision = SelectionSet::caret_in_text(
        DocumentRevision::new(7),
        session.snapshot().text(),
        TextSize::try_from_usize(0).unwrap(),
    )
    .unwrap();
    let error = session
        .apply_edit(waml_markdown_editor::edit::MarkdownEdit {
            base_revision: DocumentRevision::INITIAL,
            changes: vec![TextChange {
                old_range: TextRange::new(
                    TextSize::try_from_usize(0).unwrap(),
                    TextSize::try_from_usize(0).unwrap(),
                )
                .unwrap(),
                replacement: Arc::from("x"),
            }],
            selection_after: wrong_revision,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::SelectionRevision { selection, expected }
            if selection == DocumentRevision::new(7) && expected == DocumentRevision::new(1)
    ));
    assert_eq!(session.ime().unwrap().preedit(), "候");
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
}
