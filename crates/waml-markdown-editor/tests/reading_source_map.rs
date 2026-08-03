//! `SourceMap` maps `TextFlow`'s selection-buffer indices back to source.
//!
//! TextFlow's selection index space is its own accumulated `SelectionTracker`
//! text buffer, NOT source offsets: it holds only the runs that were drawn,
//! plus structural newlines it injects itself. Anything that carries a viewer
//! selection back to the editor has to translate.

use waml_markdown_editor::reading::SourceMap;
use waml_syntax::{TextRange, TextSize};

fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("in range")
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).expect("ordered")
}

fn map() -> SourceMap {
    // Renders "# Title\n\nBody\n" as "Title" + "\n" + "Body": the `# ` marker
    // and the blank line never reach the flow buffer.
    let mut map = SourceMap::default();
    map.push(0..5, Some(range(2, 7))); // "Title"
    map.push(5..6, None); // structural newline
    map.push(6..10, Some(range(9, 13))); // "Body"
    map
}

#[test]
fn an_empty_map_reports_itself_empty() {
    assert!(SourceMap::default().is_empty());
    assert_eq!(SourceMap::default().source_offset(0), None);
}

#[test]
fn a_flow_index_inside_a_piece_maps_to_the_matching_source_offset() {
    let map = map();
    assert_eq!(map.source_offset(0), Some(size(2)), "start of the run");
    assert_eq!(map.source_offset(3), Some(size(5)), "offset within the run");
    assert_eq!(map.source_offset(6), Some(size(9)), "start of the next run");
}

#[test]
fn an_index_in_a_structural_gap_falls_forward_to_the_next_real_piece() {
    let map = map();
    assert_eq!(
        map.source_offset(5),
        Some(size(9)),
        "a newline TextFlow injected has no source of its own; the caret belongs \
         to the next drawn run"
    );
}

#[test]
fn an_index_past_the_end_maps_to_the_end_of_the_last_real_piece() {
    let map = map();
    assert_eq!(map.source_offset(10), Some(size(13)));
    assert_eq!(map.source_offset(999), Some(size(13)));
}

#[test]
fn a_flow_span_becomes_the_enclosing_source_span() {
    let map = map();
    assert_eq!(
        map.source_span(0..10),
        Some(range(2, 13)),
        "a selection over both runs spans the source between them, including \
         the punctuation it skipped over"
    );
    assert_eq!(map.source_span(1..4), Some(range(3, 6)));
}

#[test]
fn a_span_entirely_inside_a_gap_has_no_source_span() {
    let mut map = SourceMap::default();
    map.push(0..1, None);
    assert_eq!(map.source_span(0..1), None);
}

#[test]
fn clear_resets_the_map() {
    let mut map = map();
    map.clear();
    assert!(map.is_empty());
}
