use std::sync::Arc;

use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    edit::{EditCommand, HistoryGroup},
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
    unicode::{offset_to_utf16, utf16_to_offset, Utf16Position},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
};

fn session(text: &str) -> MarkdownDocumentSession {
    let source = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
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
            &[change.clone()],
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
            &[change.clone()],
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
