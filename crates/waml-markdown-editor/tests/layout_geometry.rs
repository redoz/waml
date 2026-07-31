use makepad_widgets::{dvec2, Rect};
use std::{collections::HashSet, sync::Arc};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::{
        Affinity, BlockFlow, BlockGeometry, BlockLayoutSpec, CaretStop, EdgeInsets, FontKey,
        FontWeight, GlyphCluster, LayoutBlock, LayoutDocument, LayoutElementId, LayoutEngine,
        LayoutError, LayoutInvalidation, LayoutSnapshot, LayoutTextRun, LayoutViewport,
        ShapedCluster, ShapedRun, TextMetrics, TextShaper, VisualLine,
    },
    selection::{Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
};
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange,
    TextSize,
};

fn t(n: usize) -> TextSize {
    TextSize::try_from_usize(n).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

#[test]
fn source_point_round_trip_handles_proportional_clusters_and_affinity() {
    let snapshot = LayoutSnapshot::from_parts_for_test(
        DocumentRevision::new(3),
        dvec2(120.0, 24.0),
        vec![VisualLine::for_test(range(0, 3), 0.0, 24.0)],
        vec![GlyphCluster::for_test(
            range(0, 3),
            Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(30.0, 24.0),
            },
            vec![
                CaretStop::new(TextPosition::new(t(0), Affinity::Before), dvec2(0.0, 0.0)),
                CaretStop::new(TextPosition::new(t(1), Affinity::After), dvec2(9.0, 0.0)),
                CaretStop::new(TextPosition::new(t(3), Affinity::After), dvec2(30.0, 0.0)),
            ],
        )],
        Vec::<BlockGeometry>::new(),
    );
    for position in [
        TextPosition::new(t(0), Affinity::Before),
        TextPosition::new(t(1), Affinity::After),
        TextPosition::new(t(3), Affinity::After),
    ] {
        let point = snapshot.source_to_point(position).unwrap().rect.pos;
        assert_eq!(snapshot.point_to_source(point), position);
    }
}

#[test]
fn selection_rects_split_across_wrapped_mixed_height_lines() {
    let snapshot = LayoutSnapshot::wrapped_fixture_for_test();
    let selection = Selection::new(
        TextPosition::new(t(1), Affinity::Before),
        TextPosition::new(t(8), Affinity::After),
    );
    let rects = snapshot.selection_rects(selection).unwrap();
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].size.y, 18.0);
    assert_eq!(rects[1].size.y, 30.0);
}

#[test]
fn vertical_motion_uses_preferred_pixels_not_character_columns() {
    let snapshot = LayoutSnapshot::proportional_fixture_for_test();
    let start = TextPosition::new(t(2), Affinity::After);
    let (down, preferred_x) = snapshot.move_vertical(start, None, 1).unwrap();
    assert_eq!(preferred_x, 26.0);
    let (up, _) = snapshot.move_vertical(down, Some(preferred_x), -1).unwrap();
    assert_eq!(up, start);
}

#[test]
fn session_vertical_motion_reuses_and_resets_preferred_pixels() {
    let source = SourceText::new("abcdef".to_owned()).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let document = Arc::new(MarkdownDocumentSnapshot::new(syntax));
    let selections = SelectionSet::single(
        document.as_ref(),
        Selection::caret(TextPosition::new(t(2), Affinity::After)),
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(document, selections).unwrap();
    let layout = LayoutSnapshot::proportional_fixture_for_test();

    session.move_vertical(&layout, 1, false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset, t(5));
    session.move_right(false).unwrap();
    session.move_vertical(&layout, -1, false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset, t(3));
}

#[test]
fn session_vertical_motion_rejects_a_stale_layout_revision() {
    let source = SourceText::new("abcdef".to_owned()).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::new(1),
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)));
    let layout = LayoutSnapshot::proportional_fixture_for_test();

    assert!(matches!(
        session.move_vertical(&layout, 1, false),
        Err(LayoutError::RevisionMismatch { document, layout })
            if document == DocumentRevision::new(1) && layout == DocumentRevision::INITIAL
    ));
}

#[test]
fn mixed_metrics_wrap_without_a_cell_width() {
    let (document, presentation, mut shaper) = fixtures::mixed_heading_and_body(80.0);
    let mut engine = LayoutEngine::default();
    let layout = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 60.0, 0.0, 24.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(layout.visual_lines()[0].height(), 30.0);
    assert_eq!(layout.visual_lines()[1].height(), 16.0);
    assert!(layout.visual_lines().len() > 2);
}

#[test]
fn viewport_shapes_only_visible_blocks_plus_overscan() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 800.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped_block_count() < 20);
    assert_eq!(layout.block_summaries().len(), 100);
    assert!(layout.content_size().y >= 2_000.0);
}

#[test]
fn width_change_rewraps_without_changing_document_revision() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let mut engine = LayoutEngine::default();
    let wide = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let narrow = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(120.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::ViewportWidth,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(wide.revision(), narrow.revision());
    assert!(narrow.visual_lines().len() > wide.visual_lines().len());
}

#[test]
fn failed_block_uses_editable_plain_text_fallback() {
    let (document, presentation, mut shaper) = fixtures::failing_second_block();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(layout.blocks()[1].is_plain_text_fallback());
    let source = layout.blocks()[1].source_range();
    assert_eq!(
        layout.point_to_source(
            layout
                .source_to_point(TextPosition::new(source.start(), Affinity::Before))
                .unwrap()
                .rect
                .pos
        ),
        TextPosition::new(source.start(), Affinity::Before)
    );
}

#[test]
fn nested_left_insets_offset_lines_carets_and_selections() {
    let (mut document, presentation, mut shaper) = fixtures::failing_second_block();
    shaper.fail_fragment = None;
    let mut blocks = document.blocks.to_vec();
    let parent_id = blocks[0].id;
    blocks[0].spec.insets.left = 5.0;
    blocks[1].parent = Some(parent_id);
    blocks[1].spec.insets.left = 7.0;
    let child_range = document.text_runs[1].range;
    document.content_insets.left = 10.0;
    document.blocks = blocks.into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let child_line = layout.visual_lines().last().unwrap();
    assert_eq!(child_line.rect.pos.x, 22.0);
    let start = TextPosition::new(child_range.start(), Affinity::Before);
    assert_eq!(layout.source_to_point(start).unwrap().rect.pos.x, 22.0);
    let selection = Selection::new(start, TextPosition::new(child_range.end(), Affinity::After));
    assert_eq!(
        layout
            .selection_rects(selection)
            .unwrap()
            .last()
            .unwrap()
            .pos
            .x,
        22.0
    );
}

#[test]
fn syntax_invalidation_rejects_update_snapshot_revision_mismatch() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let update = reparse_markdown(
        presentation.syntax(),
        DocumentRevision::new(9),
        presentation.text().clone(),
        &[],
    )
    .unwrap();
    let error = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::SyntaxUpdate(update),
            &mut shaper,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::RevisionMismatch { document, layout }
            if document == DocumentRevision::new(8) && layout == DocumentRevision::new(9)
    ));
}

#[test]
fn bidi_levels_reorder_clusters_and_keep_boundary_affinities_distinct() {
    let (document, presentation, _) = fixtures::paragraph();
    let run = &document.text_runs[0];
    let start = run.range.start().to_usize();
    let mut shaper = FixedShaper(ShapedRun {
        clusters: Arc::from([
            shaped_cluster(start, start + 1, 0),
            shaped_cluster(start + 1, start + 2, 1),
            shaped_cluster(start + 2, start + 3, 1),
        ]),
        ascender: 12.0,
        descender: 4.0,
        line_gap: 0.0,
    });
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let boundary = t(start + 1);
    assert_eq!(
        layout
            .source_to_point(TextPosition::new(boundary, Affinity::After))
            .unwrap()
            .rect
            .pos
            .x,
        10.0
    );
    assert_eq!(
        layout
            .source_to_point(TextPosition::new(boundary, Affinity::Before))
            .unwrap()
            .rect
            .pos
            .x,
        30.0
    );
}

#[test]
fn content_extent_and_offscreen_virtualization_remain_document_wide() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 80.0, 1_200.0, 20.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(layout.block_summaries().len(), 100);
    assert!(layout.content_size().y >= 2_000.0);
    assert!(layout.visible_block_range().start > 0);
    assert!(layout.visible_block_range().end < 100);
    assert!(layout.visible_source_range().start() > t(0));
    assert!(shaper.shaped_block_count() < 20);
}

#[test]
fn embedded_measurement_invalidation_reshapes_only_the_stable_block_id() {
    let (document, presentation, mut shaper) = fixtures::failing_second_block();
    shaper.fail_fragment = None;
    let mut engine = LayoutEngine::default();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    shaper.shaped.clear();
    let target = document.blocks[1].id;
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::BlockMeasurement(target),
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped.contains(&target));
    assert_eq!(shaper.shaped.len(), document.text_runs.len());
}

#[test]
fn selection_across_blocks_uses_layout_geometry_and_reaches_eof() {
    let (document, presentation, mut shaper) = fixtures::failing_second_block();
    shaper.fail_fragment = None;
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let start = document.text_runs[0].range.start();
    let end = document.text_runs[1].range.end();
    let rects = layout
        .selection_rects(Selection::new(
            TextPosition::new(start, Affinity::Before),
            TextPosition::new(end, Affinity::After),
        ))
        .unwrap();
    assert!(rects.len() >= 2);
    let eof = TextPosition::new(end, Affinity::After);
    let point = layout.source_to_point(eof).unwrap().rect.pos;
    assert_eq!(layout.point_to_source(point), eof);
}

struct FixedShaper(ShapedRun);

impl TextShaper for FixedShaper {
    fn shape(
        &mut self,
        _source: &SourceText,
        _run: &LayoutTextRun,
        _max_width: f64,
    ) -> Result<ShapedRun, LayoutError> {
        Ok(self.0.clone())
    }
}

fn shaped_cluster(start: usize, end: usize, bidi_level: u8) -> ShapedCluster {
    ShapedCluster {
        source_range: range(start, end),
        advance: 10.0,
        bidi_level,
        caret_offsets: Arc::from([t(start), t(end)]),
    }
}

#[derive(Default)]
struct FakeShaper {
    shaped: HashSet<LayoutElementId>,
    fail_fragment: Option<u32>,
}

impl FakeShaper {
    fn shaped_block_count(&self) -> usize {
        self.shaped.len()
    }
}

impl TextShaper for FakeShaper {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        _max_width: f64,
    ) -> Result<ShapedRun, LayoutError> {
        self.shaped.insert(run.id);
        if self.fail_fragment == Some(run.id.fragment_ordinal) {
            return Err(LayoutError::ShapingFailed { run: run.id });
        }
        let text = source.slice(run.range).unwrap();
        let mut clusters = Vec::new();
        for (relative, character) in text.char_indices() {
            let start = run.range.start().to_usize() + relative;
            let end = start + character.len_utf8();
            clusters.push(ShapedCluster {
                source_range: range(start, end),
                advance: run.metrics.font.0 as f64,
                bidi_level: 0,
                caret_offsets: Arc::from([t(start), t(end)]),
            });
        }
        Ok(ShapedRun {
            clusters: clusters.into(),
            ascender: run.metrics.font_size as f64 * 0.8,
            descender: run.metrics.font_size as f64 * 0.2,
            line_gap: 0.0,
        })
    }
}

mod fixtures {
    use super::*;

    pub fn mixed_heading_and_body(
        _width: f64,
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[30.0, 16.0], &[4, 18], None)
    }

    pub fn one_hundred_blocks() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&vec![20.0; 100], &vec![6; 100], None)
    }

    pub fn paragraph() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[16.0], &[60], None)
    }

    pub fn failing_second_block() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[16.0, 16.0], &[8, 8], Some(1))
    }

    fn fixture(
        sizes: &[f32],
        content_characters: &[usize],
        fail_fragment: Option<u32>,
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        let source = SourceText::new(
            content_characters
                .iter()
                .map(|characters| format!("# {}\n", "x".repeat(*characters)))
                .collect::<String>(),
        )
        .unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(8),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let headings: Vec<_> = syntax.queries().headings().cloned().collect();
        let mut blocks = Vec::new();
        let mut runs = Vec::new();
        for (index, (heading, font_size)) in headings.iter().zip(sizes).enumerate() {
            let id = LayoutElementId {
                owner: heading.owner,
                fragment_ordinal: index as u32,
            };
            blocks.push(LayoutBlock {
                id,
                source_range: heading.range,
                parent: None,
                spec: BlockLayoutSpec {
                    flow: BlockFlow::Paragraph,
                    insets: EdgeInsets::default(),
                    space_before: 0.0,
                    space_after: 4.0,
                    columns: Arc::from([]),
                },
            });
            runs.push(LayoutTextRun {
                id,
                range: heading.content_range,
                metrics: TextMetrics {
                    font: FontKey(if *font_size == 30.0 { 12 } else { 8 }),
                    font_size: *font_size,
                    line_spacing: 0.0,
                    weight: FontWeight(400),
                    italic: false,
                },
            });
        }
        let presentation = Arc::new(MarkdownDocumentSnapshot::new(syntax));
        (
            LayoutDocument {
                revision: presentation.revision(),
                content_insets: EdgeInsets::default(),
                blocks: blocks.into(),
                text_runs: runs.into(),
                embedded_blocks: Arc::from([]),
            },
            presentation,
            FakeShaper {
                shaped: HashSet::new(),
                fail_fragment,
            },
        )
    }
}
