use waml_markdown_editor::{
    gutter::{gutter_rows, gutter_width, LineNumberMode},
    layout::{LayoutSnapshot, VisualLine},
    syntax::{DocumentRevision, SourceText, TextRange, TextSize},
};

use makepad_widgets::dvec2;
use waml_syntax::LineIndex;

const SOURCE: &str = "alpha\nbeta beta\ngamma\n";

fn t(offset: usize) -> TextSize {
    TextSize::try_from_usize(offset).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

/// Three source lines where the second wraps across two visual rows.
fn layout() -> LayoutSnapshot {
    LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(80.0, 80.0),
        vec![
            VisualLine::for_test(range(0, 6), 0.0, 20.0),
            VisualLine::for_test(range(6, 11), 20.0, 20.0),
            VisualLine::for_test(range(11, 16), 40.0, 20.0),
            VisualLine::for_test(range(16, 22), 60.0, 20.0),
        ],
        Vec::new(),
        Vec::new(),
    )
}

fn rows(mode: LineNumberMode, cursor: usize) -> Vec<(usize, String, bool)> {
    let text = SourceText::new(SOURCE.to_owned()).unwrap();
    let lines = LineIndex::new(&text);
    gutter_rows(&layout(), &text, &lines, t(cursor), mode)
        .into_iter()
        .map(|row| (row.line, row.label, row.current))
        .collect()
}

#[test]
fn absolute_numbers_count_source_lines_and_skip_wrapped_continuations() {
    assert_eq!(
        rows(LineNumberMode::Absolute, 0),
        vec![
            (1, "1".to_owned(), true),
            (2, "2".to_owned(), false),
            (3, "3".to_owned(), false),
        ]
    );
}

#[test]
fn relative_numbers_keep_the_cursor_line_absolute_and_count_outward() {
    // Cursor inside line 2; its neighbours are one line away in both
    // directions, which is exactly the count a motion would take.
    assert_eq!(
        rows(LineNumberMode::Relative, 7),
        vec![
            (1, "1".to_owned(), false),
            (2, "2".to_owned(), true),
            (3, "1".to_owned(), false),
        ]
    );
}

#[test]
fn a_line_is_numbered_on_the_row_that_holds_its_glyphs() {
    // A heading's block emits an empty placement row at the same offset as the
    // row carrying its text; numbering the first would float the digit a row
    // above the words it counts.
    let layout = LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(80.0, 80.0),
        vec![
            VisualLine::for_test(range(0, 0), 0.0, 6.0),
            VisualLine::for_test(range(0, 6), 6.0, 20.0),
        ],
        Vec::new(),
        Vec::new(),
    );
    let text = SourceText::new(SOURCE.to_owned()).unwrap();
    let lines = LineIndex::new(&text);
    let rows = gutter_rows(&layout, &text, &lines, t(0), LineNumberMode::Absolute);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].line, 1);
    assert_eq!(rows[0].y, 6.0);
    assert!(rows[0].has_text);
}

#[test]
fn a_blank_line_keeps_its_own_empty_row() {
    let layout = LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(80.0, 80.0),
        vec![
            VisualLine::for_test(range(0, 6), 0.0, 20.0),
            VisualLine::for_test(range(6, 6), 20.0, 20.0),
        ],
        Vec::new(),
        Vec::new(),
    );
    let text = SourceText::new(SOURCE.to_owned()).unwrap();
    let lines = LineIndex::new(&text);
    let rows = gutter_rows(&layout, &text, &lines, t(0), LineNumberMode::Absolute);
    assert_eq!(
        rows.iter().map(|row| row.line).collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn the_off_mode_reserves_nothing() {
    assert!(rows(LineNumberMode::Off, 0).is_empty());
}

#[test]
fn the_reserved_width_follows_the_document_not_the_visible_rows() {
    let two = gutter_width(9, 6.0, 10.0);
    let three = gutter_width(100, 6.0, 10.0);
    // Single-digit documents still reserve two digits, so the first wrap past
    // line 9 cannot shift the text sideways.
    assert_eq!(gutter_width(1, 6.0, 10.0), two);
    assert_eq!(two, 22.0);
    assert_eq!(three, 28.0);
}
