use makepad_widgets::{dvec2, Rect};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use unicode_bidi::BidiInfo;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::{
        Affinity, BlockFlow, BlockGeometry, BlockLayoutData, BlockLayoutSpec, CaretStop,
        ColumnAlignment, ColumnConstraint, EdgeInsets, FontKey, FontWeight, GeometryElementId,
        GlyphCluster, LayoutBlock, LayoutBudget, LayoutDocument, LayoutElementId, LayoutEngine,
        LayoutError, LayoutInvalidation, LayoutSnapshot, LayoutTextRun, LayoutViewport,
        LayoutWorkPhase, MeasuredBlock, ParagraphIntrinsic, ParagraphIntrinsicRequest,
        ParagraphShapeRequest, ShapeSpan, ShapedCluster, ShapedFragment, ShapedGlyph,
        ShapedParagraph, ShapedRow, ShapedRun, TextMetrics, TextShaper, VisualLine,
    },
    presentation::{
        build_draw_commands, build_layout_document, compile_presentation, EmbeddedAssetFrame,
        EmbeddedMeasurements, HighlighterRegistry, PresentationFrame, PresentationStyles,
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

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn closing_fenced_code_marker_has_exact_geometry_and_a_text_paint_command() {
    let source = "```waml\numl.class Foo {}\n```\n";
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(syntax.clone()));
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default()).unwrap();
    let document = build_layout_document(&plan, &styles, &EmbeddedMeasurements::default()).unwrap();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(640.0, 480.0, 0.0, 480.0),
            LayoutInvalidation::Document,
            &mut FakeShaper::default(),
        )
        .unwrap();
    let closing_start = source.rfind("```").unwrap();
    let closing = range(closing_start, closing_start + 3);
    let geometry_ranges = layout
        .glyph_clusters()
        .iter()
        .filter(|cluster| {
            closing.start() < cluster.source_range.end()
                && cluster.source_range.start() < closing.end()
        })
        .map(|cluster| cluster.source_range)
        .collect::<Vec<_>>();
    assert_eq!(
        geometry_ranges
            .iter()
            .flat_map(|range| range.start().to_usize()..range.end().to_usize())
            .collect::<Vec<_>>(),
        (closing_start..closing_start + 3).collect::<Vec<_>>()
    );

    let selection = SelectionSet::caret(presentation.as_ref(), t(closing_start)).unwrap();
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout: Arc::new(layout),
        active_owners: Arc::from([]),
        diagnostics: Arc::from([]),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    let commands = build_draw_commands(&frame, &plan, &styles, &selection, None).unwrap();
    let painted = commands
        .iter()
        .filter_map(|command| match command {
            waml_markdown_editor::presentation::DrawCommand::Text { range, .. }
                if closing.start() < range.end() && range.start() < closing.end() =>
            {
                Some(range.start().to_usize()..range.end().to_usize())
            }
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(
        painted,
        (closing_start..closing_start + 3).collect::<Vec<_>>(),
        "the closing fence remains literal in the paint plan"
    );
}

#[test]
fn unpositioned_blocks_use_an_optional_document_index() {
    let (document, _, _) = fixtures::paragraph();
    let block = &document.blocks[0];
    let geometry = BlockGeometry::new(
        block.id,
        block.source_range,
        Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(100.0, 20.0),
        },
    );
    let document_index: Option<usize> = geometry.document_index();
    assert_eq!(document_index, None);
}

#[test]
fn snapshots_and_retained_layout_payloads_are_send_sync() {
    assert_send_sync::<LayoutSnapshot>();
    assert_send_sync::<Arc<BlockLayoutData>>();
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
fn glyph_clusters_carry_the_metrics_used_for_measurement_and_hit_testing() {
    let (document, presentation, mut shaper) = fixtures::mixed_heading_and_body(80.0);
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 60.0, 0.0, 24.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    let first = layout.glyph_clusters().first().unwrap();
    assert_eq!(first.metrics, document.text_runs[0].metrics);
    assert_eq!(first.metrics.font, FontKey(12));
    assert_eq!(first.metrics.weight, FontWeight(400));
}

#[test]
fn renderer_ready_glyph_payload_survives_complex_clusters() {
    let (document, presentation, _) = fixtures::paragraph();
    let run = &document.text_runs[0];
    let start = run.range.start().to_usize();
    let glyph = |glyph_id, origin_x, advance, baseline| ShapedGlyph {
        glyph_id,
        origin: dvec2(origin_x, 1.5),
        advance,
        paint_scale: 1.0,
        font: None,
        font_key: FontKey(77),
        font_size: 19.0,
        ascender: 14.0,
        descender: -5.0,
        line_gap: 2.0,
        baseline,
        offset: 0.25,
        color: None,
    };
    let mut shaper = FixedShaper(ShapedRun {
        clusters: Arc::from([
            ShapedCluster {
                id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                span_id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                source_range: range(start, start + 3),
                metrics: run.metrics,
                advance: 17.0,
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([t(start), t(start + 3)]),
                glyphs: Arc::from([glyph(501, 0.0, 17.0, 14.0)]),
            },
            ShapedCluster {
                id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                span_id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                source_range: range(start + 3, start + 6),
                metrics: run.metrics,
                advance: 12.0,
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([t(start + 3), t(start + 6)]),
                glyphs: Arc::from([glyph(601, 0.0, 12.0, 14.0), glyph(602, 4.0, 0.0, 14.0)]),
            },
            ShapedCluster {
                id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                span_id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                source_range: range(start + 6, start + 14),
                metrics: run.metrics,
                advance: 24.0,
                bidi_level: 1,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([t(start + 6), t(start + 14)]),
                glyphs: Arc::from([
                    glyph(701, 0.0, 8.0, 14.0),
                    glyph(702, 8.0, 8.0, 14.0),
                    glyph(703, 16.0, 8.0, 14.0),
                ]),
            },
        ]),
        ascender: 14.0,
        descender: -5.0,
        line_gap: 2.0,
    });

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    let clusters = layout.glyph_clusters();
    assert_eq!(
        clusters[0].glyphs.len(),
        1,
        "ligature glyphs must not be expanded"
    );
    assert_eq!(clusters[0].glyphs[0].glyph_id, 501);
    assert_eq!(
        clusters[1].glyphs.len(),
        2,
        "combining glyphs must not be dropped"
    );
    assert_eq!(clusters[1].glyphs[1].origin, dvec2(21.0, 15.5));
    assert_eq!(
        clusters[2]
            .glyphs
            .iter()
            .map(|glyph| glyph.glyph_id)
            .collect::<Vec<_>>(),
        vec![701, 702, 703]
    );
    assert_eq!(clusters[0].glyphs[0].font_key, FontKey(77));
    assert_eq!(clusters[0].glyphs[0].font_size, 19.0);
    assert_eq!(clusters[0].glyphs[0].ascender, 14.0);
    assert_eq!(clusters[0].glyphs[0].descender, -5.0);
    assert_eq!(clusters[0].glyphs[0].line_gap, 2.0);
    assert_eq!(clusters[0].glyphs[0].baseline, 14.0);
    assert_eq!(clusters[0].glyphs[0].advance, 17.0);
}

#[test]
fn snapshot_keeps_only_visible_blocks_plus_overscan() {
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
    assert!(layout.visible_blocks().len() < 40);
    assert_eq!(layout.block_summaries().len(), 100);
    assert!(layout.content_size().y >= 2_000.0);
}

#[test]
fn hierarchy_rejects_duplicate_missing_self_and_cyclic_parents() {
    fn layout_error(mut blocks: Vec<LayoutBlock>) -> LayoutError {
        let (mut document, presentation, mut shaper) = fixtures::one_hundred_blocks();
        blocks.truncate(2);
        let live_ids = blocks.iter().map(|block| block.id).collect::<HashSet<_>>();
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| live_ids.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::default_overscan(400.0, 100.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap_err()
    }

    let (document, _, _) = fixtures::one_hundred_blocks();
    let original = document.blocks[..2].to_vec();

    let mut duplicate = original.clone();
    duplicate[1].id = duplicate[0].id;
    assert!(matches!(
        layout_error(duplicate),
        LayoutError::DuplicateBlockId { id } if id == original[0].id
    ));

    let mut missing = original.clone();
    let missing_parent = LayoutElementId {
        owner: missing[0].id.owner,
        fragment_ordinal: u32::MAX,
    };
    missing[1].parent = Some(missing_parent);
    assert!(matches!(
        layout_error(missing),
        LayoutError::MissingParent { block, parent }
            if block == original[1].id && parent == missing_parent
    ));

    let mut self_parent = original.clone();
    self_parent[0].parent = Some(self_parent[0].id);
    assert!(matches!(
        layout_error(self_parent),
        LayoutError::HierarchyCycle { block } if block == original[0].id
    ));

    let mut cycle = original.clone();
    cycle[0].parent = Some(cycle[1].id);
    cycle[1].parent = Some(cycle[0].id);
    assert!(matches!(
        layout_error(cycle),
        LayoutError::HierarchyCycle { .. }
    ));
}

#[test]
fn child_before_parent_keeps_the_same_subtree_fingerprint() {
    fn nested_fixture(
        child_first: bool,
    ) -> (
        LayoutDocument,
        Arc<MarkdownDocumentSnapshot>,
        FakeShaper,
        LayoutElementId,
    ) {
        let (mut document, presentation, shaper) = fixtures::one_hundred_blocks();
        let mut blocks = document.blocks[..3].to_vec();
        let root = blocks[0].id;
        blocks[1].parent = Some(root);
        blocks[2].parent = Some(blocks[1].id);
        let live_ids = blocks.iter().map(|block| block.id).collect::<HashSet<_>>();
        if child_first {
            blocks.reverse();
        }
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| live_ids.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        (document, presentation, shaper, root)
    }

    fn fingerprint(child_first: bool) -> u64 {
        let (document, presentation, mut shaper, root) = nested_fixture(child_first);
        let mut engine = LayoutEngine::default();
        engine
            .layout(
                &document,
                &presentation,
                LayoutViewport::default_overscan(400.0, 200.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        engine.subtree_fingerprint_for_test(root).unwrap()
    }

    assert_eq!(fingerprint(false), fingerprint(true));
}

#[test]
fn index_hashing_visits_source_and_records_once() {
    let (mut document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    document.blocks = document.blocks[..3].to_vec().into();
    let live_ids = document
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<HashSet<_>>();
    document.text_runs = document
        .text_runs
        .iter()
        .filter(|run| live_ids.contains(&run.id))
        .cloned()
        .collect::<Vec<_>>()
        .into();
    let mut engine = LayoutEngine::default();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::default_overscan(400.0, 200.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    let stats = engine.last_index_build_stats_for_test();
    assert_eq!(stats.source_bytes, presentation.text().len().to_usize());
    assert_eq!(stats.run_visits, document.text_runs.len());
    assert_eq!(stats.embedded_visits, document.embedded_blocks.len());
    assert_eq!(stats.block_visits, document.blocks.len());
}

#[test]
fn index_rejects_overlapping_direct_text_runs() {
    let (mut document, presentation, mut shaper) = fixtures::paragraph();
    let run = document.text_runs[0].clone();
    document.text_runs = Arc::from([run.clone(), run.clone()]);

    let error = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::default_overscan(400.0, 200.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        LayoutError::OverlappingTextRuns { first, second }
            if first == run.range && second == run.range
    ));
}

#[test]
fn deep_hierarchy_validation_and_indexing_are_iterative_and_linear() {
    let (mut document, presentation, _) = fixtures::ten_thousand_blocks();
    let mut blocks = document.blocks.to_vec();
    for index in 1..blocks.len() {
        blocks[index].parent = Some(blocks[index - 1].id);
    }
    document.blocks = blocks.into();

    let stats = LayoutEngine::build_index_stats_for_test(&document, &presentation).unwrap();

    assert_eq!(stats.hierarchy_node_visits, 10_000);
    assert_eq!(stats.block_visits, 10_000);
    assert_eq!(stats.run_visits, 10_000);
}

#[test]
fn default_overscan_owns_exact_320_pixel_boundaries() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::default_overscan(400.0, 100.0, 1_000.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    assert_eq!(
        layout.visible_blocks().first().unwrap().document_index(),
        Some(28)
    );
    assert_eq!(
        layout
            .visible_blocks()
            .last()
            .unwrap()
            .document_index()
            .unwrap()
            + 1,
        60
    );
    assert_eq!(layout.visible_block_local_range(), 0..32);
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
    assert!(layout.visible_blocks()[1].is_plain_text_fallback());
    let source = layout.visible_blocks()[1].source_range();
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
            shaped_cluster(run.id, start, start + 1, 0),
            shaped_cluster(run.id, start + 1, start + 2, 1),
            shaped_cluster(run.id, start + 2, start + 3, 1),
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
    assert!(layout.visible_blocks().first().unwrap().document_index() > Some(0));
    assert!(layout.visible_blocks().last().unwrap().document_index() < Some(99));
    assert!(layout.visible_source_range().start() > t(0));
    assert!(layout.visible_blocks().len() < 20);
}

#[test]
fn scrolled_snapshot_separates_local_geometry_from_document_block_indexes() {
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

    let local_range = layout.visible_block_local_range();
    assert_eq!(local_range, 0..layout.visible_blocks().len());
    let first_document_index = layout.visible_blocks()[0].document_index().unwrap();
    let last_document_index = layout
        .visible_blocks()
        .last()
        .unwrap()
        .document_index()
        .unwrap();
    assert!(first_document_index > 0);
    assert_eq!(
        layout.visible_blocks()[0].id,
        document.blocks[first_document_index].id
    );
    assert_eq!(layout.document_block_index(0), Some(first_document_index));
    assert_eq!(
        layout.document_block_index(local_range.end - 1),
        Some(last_document_index)
    );
    assert_eq!(layout.document_block_index(local_range.end), None);
}

#[test]
fn sparse_nested_visible_blocks_carry_their_exact_document_indexes() {
    let (mut document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let mut blocks = document.blocks.to_vec();
    let root = blocks[0].id;
    blocks[0].spec.flow = BlockFlow::Quote;
    blocks[0].spec.space_after = 0.0;
    for block in blocks.iter_mut().skip(1) {
        block.parent = Some(root);
    }
    document.blocks = blocks.into();
    document.text_runs = document
        .text_runs
        .iter()
        .filter(|run| run.id != root)
        .cloned()
        .collect::<Vec<_>>()
        .into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 80.0, 1_200.0, 20.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let document_indexes = layout
        .visible_blocks()
        .iter()
        .map(|block| block.document_index().unwrap())
        .collect::<Vec<_>>();
    let consumer_slice = &layout.blocks()[layout.visible_block_range()];
    assert_eq!(consumer_slice, layout.visible_blocks());
    assert_eq!(
        document_indexes[0], 0,
        "the visible quote root is document block zero"
    );
    assert!(document_indexes
        .windows(2)
        .any(|pair| pair[1] > pair[0] + 1));
    for (local_index, visible) in layout.visible_blocks().iter().enumerate() {
        let document_index = visible.document_index().unwrap();
        assert_eq!(document.blocks[document_index].id, visible.id);
        assert_eq!(
            layout.document_block_index(local_index),
            Some(document_index)
        );
    }
}

#[test]
fn cold_large_document_and_scroll_only_layout_shape_only_the_overscanned_window() {
    let (document, presentation, mut shaper) = fixtures::ten_thousand_blocks();
    let mut engine = LayoutEngine::default();
    let first = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::default_overscan(400.0, 100.0, 100_000.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(first.block_summaries().len(), 10_000);
    assert!(first.content_size().y >= 200_000.0);
    assert!(
        shaper.shaped.len() <= 50,
        "cold shaped {} blocks",
        shaper.shaped.len()
    );
    assert!(first.visible_block_layouts().len() <= 50);

    let cold_calls = shaper.shaped.len();
    let second = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::default_overscan(400.0, 100.0, 100_200.0),
            LayoutInvalidation::Viewport,
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped.len() <= cold_calls + 12);
    assert!(second.visible_block_layouts().len() <= 50);
}

#[test]
fn scrolled_visible_window_is_recomputed_after_an_earlier_block_wraps() {
    let (document, presentation, mut shaper) = fixtures::fixture(
        &[16.0, 16.0, 16.0, 16.0, 16.0, 16.0],
        &[60, 1, 1, 1, 1, 1],
        None,
    );

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 40.0, 80.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    // The first block grows from its one-line estimate to six wrapped rows, so
    // the scrolled viewport is inside that block, not the initially estimated
    // fifth block.
    assert_eq!(
        layout.visible_blocks().first().unwrap().document_index(),
        Some(0)
    );
    assert_eq!(
        layout.visible_source_range().start(),
        document.text_runs[0].range.start()
    );
}

#[test]
fn repeated_far_scrolls_keep_all_summaries_but_bound_full_layout_payloads() {
    let (document, presentation, mut shaper) = fixtures::ten_thousand_blocks();
    let mut engine = LayoutEngine::default();
    let scroll_positions = [0.0, 60_000.0, 120_000.0, 200_000.0, 0.0];
    let mut initial_heights = None;

    for (turn, scroll_y) in scroll_positions.into_iter().enumerate() {
        shaper.shaped.clear();
        let layout = engine
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(400.0, 80.0, scroll_y, 0.0),
                if turn == 0 {
                    LayoutInvalidation::Document
                } else {
                    LayoutInvalidation::Viewport
                },
                &mut shaper,
            )
            .unwrap();

        assert_eq!(layout.block_summaries().len(), 10_000);
        assert_eq!(engine.cached_summary_count_for_test(), 10_000);
        assert!(engine.retained_layout_payload_count_for_test() <= 40);
        assert!(shaper.shaped.len() <= 40);

        let heights = layout
            .block_summaries()
            .iter()
            .map(|summary| summary.height)
            .collect::<Vec<_>>();
        if let Some(initial_heights) = &initial_heights {
            if turn == scroll_positions.len() - 1 {
                assert_eq!(&heights, initial_heights);
            }
        } else {
            initial_heights = Some(heights);
        }
    }
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
    assert_eq!(shaper.shaped, HashSet::from([target]));
}

#[test]
fn block_measurement_reuses_exact_unchanged_block_layout_data() {
    let (document, presentation, mut shaper) =
        fixtures::fixture(&[16.0, 16.0, 16.0], &[8, 8, 8], None);
    let mut engine = LayoutEngine::default();
    let first = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let first_data = first.visible_block_layouts().to_vec();
    shaper.shaped.clear();
    let target = document.blocks[1].id;

    let second = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::BlockMeasurement(target),
            &mut shaper,
        )
        .unwrap();

    assert_eq!(shaper.shaped, HashSet::from([target]));
    assert_eq!(second.dirty_block_document_range(), 1..2);
    assert!(Arc::ptr_eq(
        &first_data[0],
        &second.visible_block_layouts()[0]
    ));
    assert!(!Arc::ptr_eq(
        &first_data[1],
        &second.visible_block_layouts()[1]
    ));
    assert!(Arc::ptr_eq(
        &first_data[2],
        &second.visible_block_layouts()[2]
    ));
}

#[test]
fn measurement_growth_and_shrink_converge_before_visible_selection() {
    let (document, presentation, mut shaper) = fixtures::fixture(
        &[16.0, 16.0, 16.0, 16.0, 16.0, 16.0],
        &[6, 6, 6, 6, 6, 6],
        None,
    );
    let mut engine = LayoutEngine::default();
    let viewport = LayoutViewport::new(80.0, 40.0, 80.0, 0.0);
    let initial = engine
        .layout(
            &document,
            &presentation,
            viewport,
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(
        initial.visible_blocks().first().unwrap().document_index(),
        Some(4)
    );
    let target = document.blocks[0].id;

    shaper.shaped.clear();
    shaper.advance_override.insert(0, 40.0);
    let grown = engine
        .layout(
            &document,
            &presentation,
            viewport,
            LayoutInvalidation::BlockMeasurement(target),
            &mut shaper,
        )
        .unwrap();
    assert_eq!(shaper.shaped, HashSet::from([target]));
    assert_eq!(grown.block_summaries()[0].height, 48.0);
    assert_eq!(
        grown.visible_blocks().first().unwrap().document_index(),
        Some(2)
    );
    assert_eq!(grown.dirty_block_document_range(), 0..6);

    shaper.shaped.clear();
    shaper.advance_override.insert(0, 8.0);
    let shrunk = engine
        .layout(
            &document,
            &presentation,
            viewport,
            LayoutInvalidation::BlockMeasurement(target),
            &mut shaper,
        )
        .unwrap();
    assert_eq!(shaper.shaped, HashSet::from([target]));
    assert_eq!(shrunk.block_summaries()[0].height, 16.0);
    assert_eq!(
        shrunk.visible_blocks().first().unwrap().document_index(),
        Some(4)
    );
    assert_eq!(shrunk.dirty_block_document_range(), 0..6);
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

#[test]
fn quote_hanging_tree_aggregates_children_without_phantom_height() {
    let (mut document, presentation, mut shaper) =
        fixtures::fixture(&[16.0, 16.0, 16.0], &[1, 4, 4], None);
    let mut blocks = document.blocks.to_vec();
    let quote = blocks[0].id;
    blocks[0].spec.flow = BlockFlow::Quote;
    blocks[0].spec.insets = EdgeInsets {
        top: 2.0,
        right: 0.0,
        bottom: 4.0,
        left: 10.0,
    };
    blocks[0].spec.space_after = 0.0;
    blocks[1].parent = Some(quote);
    let original = document.text_runs[1].clone();
    let marker = range(
        original.range.start().to_usize(),
        original.range.start().to_usize() + 1,
    );
    blocks[1].spec.flow = BlockFlow::Hanging {
        marker_range: marker,
        content_indent: 20.0,
    };
    blocks[1].spec.space_after = 3.0;
    blocks[2].parent = Some(quote);
    blocks[2].spec.space_before = 5.0;
    blocks[2].spec.space_after = 0.0;
    let mut runs = document.text_runs.to_vec();
    runs.remove(0);
    runs[0].range = marker;
    runs.insert(
        1,
        LayoutTextRun {
            id: original.id,
            range: range(marker.end().to_usize(), original.range.end().to_usize()),
            metrics: original.metrics,
            hidden: false,
        },
    );
    document.blocks = blocks.into();
    document.text_runs = runs.into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(200.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let geometry = |id| {
        layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == id)
            .unwrap()
    };
    let quote_geometry = geometry(quote);
    let first_child = geometry(document.blocks[1].id);
    let second_child = geometry(document.blocks[2].id);

    assert_eq!(quote_geometry.rect.pos, dvec2(0.0, 0.0));
    assert_eq!(quote_geometry.rect.size.y, 46.0);
    assert_eq!(first_child.rect.pos, dvec2(10.0, 2.0));
    assert_eq!(second_child.rect.pos, dvec2(10.0, 26.0));
    assert_eq!(layout.content_size().y, 46.0);
    let first_lines = layout
        .visual_lines()
        .iter()
        .filter(|line| {
            line.source_range.start() >= original.range.start()
                && line.source_range.end() <= original.range.end()
        })
        .collect::<Vec<_>>();
    assert_eq!(first_lines.len(), 2);
    assert_eq!(first_lines[0].rect.pos, dvec2(10.0, 2.0));
    assert_eq!(first_lines[1].rect.pos, dvec2(30.0, 2.0));
}

#[test]
fn hanging_parent_places_nested_children_at_its_content_indent() {
    let (mut document, presentation, mut shaper) = fixtures::fixture(&[16.0, 16.0], &[4, 4], None);
    let mut blocks = document.blocks.to_vec();
    let parent = blocks[0].id;
    let marker_start = document.text_runs[0].range.start().to_usize();
    blocks[0].spec.flow = BlockFlow::Hanging {
        marker_range: range(marker_start, marker_start + 1),
        content_indent: 20.0,
    };
    blocks[0].spec.space_after = 0.0;
    blocks[1].parent = Some(parent);
    document.blocks = blocks.into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(200.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let child = layout
        .visible_blocks()
        .iter()
        .find(|block| block.id == document.blocks[1].id)
        .expect("the nested child is visible");
    assert_eq!(child.rect.pos.x, 20.0);
    assert_eq!(child.rect.size.x, 180.0);
}

#[test]
fn hanging_wrapped_multi_run_clusters_have_unique_block_ordinals() {
    let (mut document, presentation, mut shaper) = fixtures::paragraph();
    let mut blocks = document.blocks.to_vec();
    let original = document.text_runs[0].clone();
    let marker_end = original.range.start().to_usize() + 1;
    blocks[0].spec.flow = BlockFlow::Hanging {
        marker_range: range(original.range.start().to_usize(), marker_end),
        content_indent: 20.0,
    };
    document.blocks = blocks.into();
    document.text_runs = Arc::from([
        LayoutTextRun {
            id: original.id,
            range: range(original.range.start().to_usize(), marker_end),
            metrics: original.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(marker_end, original.range.end().to_usize()),
            metrics: original.metrics,
            hidden: false,
        },
    ]);

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(72.0, 500.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let ids = layout
        .glyph_clusters()
        .iter()
        .map(|cluster| cluster.id)
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), layout.glyph_clusters().len());
    assert!(layout.visual_lines().len() > 2);
}

#[test]
fn table_rows_share_column_origins_and_aggregate_cell_heights() {
    let (mut document, presentation, mut shaper) = fixtures::fixture(
        &[16.0, 16.0, 16.0, 16.0, 16.0, 16.0, 16.0],
        &[1, 1, 8, 4, 1, 2, 2],
        None,
    );
    let mut blocks = document.blocks.to_vec();
    let table = blocks[0].id;
    let row_one = blocks[1].id;
    let row_two = blocks[4].id;
    blocks[0].spec.flow = BlockFlow::Table;
    blocks[0].spec.insets.left = 5.0;
    blocks[0].spec.space_after = 0.0;
    blocks[0].spec.columns = Arc::from([
        ColumnConstraint {
            min_width: 40.0,
            max_width: Some(40.0),
            alignment: ColumnAlignment::Start,
        },
        ColumnConstraint {
            min_width: 60.0,
            max_width: Some(60.0),
            alignment: ColumnAlignment::Start,
        },
    ]);
    for row_index in [1, 4] {
        blocks[row_index].parent = Some(table);
        blocks[row_index].spec.flow = BlockFlow::TableRow;
        blocks[row_index].spec.space_after = 0.0;
    }
    for (index, parent, column) in [
        (2, row_one, 0),
        (3, row_one, 1),
        (5, row_two, 0),
        (6, row_two, 1),
    ] {
        blocks[index].parent = Some(parent);
        blocks[index].spec.flow = BlockFlow::TableCell { column };
        blocks[index].spec.space_after = 0.0;
    }
    let structural = HashSet::from([table, row_one, row_two]);
    document.blocks = blocks.into();
    document.text_runs = document
        .text_runs
        .iter()
        .filter(|run| !structural.contains(&run.id))
        .cloned()
        .collect::<Vec<_>>()
        .into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(200.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let geometry = |index: usize| {
        layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == document.blocks[index].id)
            .unwrap()
    };

    assert_eq!(geometry(0).rect.size.y, 48.0);
    assert_eq!(
        geometry(1).rect,
        Rect {
            pos: dvec2(5.0, 0.0),
            size: dvec2(100.0, 32.0)
        }
    );
    assert_eq!(
        geometry(2).rect,
        Rect {
            pos: dvec2(5.0, 0.0),
            size: dvec2(40.0, 32.0)
        }
    );
    assert_eq!(
        geometry(3).rect,
        Rect {
            pos: dvec2(45.0, 0.0),
            size: dvec2(60.0, 16.0)
        }
    );
    assert_eq!(
        geometry(4).rect,
        Rect {
            pos: dvec2(5.0, 32.0),
            size: dvec2(100.0, 16.0)
        }
    );
    assert_eq!(geometry(5).rect.pos, dvec2(5.0, 32.0));
    assert_eq!(geometry(6).rect.pos, dvec2(45.0, 32.0));
    assert_eq!(layout.content_size().y, 48.0);
    assert_eq!(layout.visual_lines().len(), 5);
}

#[test]
fn table_uses_measured_min_content_proportions_and_column_alignment() {
    let (mut document, presentation, mut shaper) =
        fixtures::fixture(&[16.0; 5], &[1, 1, 1, 2, 4], None);
    let mut blocks = document.blocks.to_vec();
    let table = blocks[0].id;
    let row = blocks[1].id;
    blocks[0].spec.flow = BlockFlow::Table;
    blocks[0].spec.space_after = 0.0;
    blocks[0].spec.columns = Arc::from([
        ColumnConstraint {
            min_width: 40.0,
            max_width: Some(40.0),
            alignment: ColumnAlignment::Start,
        },
        ColumnConstraint {
            min_width: 0.0,
            max_width: None,
            alignment: ColumnAlignment::Center,
        },
        ColumnConstraint {
            min_width: 0.0,
            max_width: None,
            alignment: ColumnAlignment::End,
        },
    ]);
    blocks[1].parent = Some(table);
    blocks[1].spec.flow = BlockFlow::TableRow;
    blocks[1].spec.space_after = 0.0;
    for (index, column) in [(2, 0), (3, 1), (4, 2)] {
        blocks[index].parent = Some(row);
        blocks[index].spec.flow = BlockFlow::TableCell { column };
        blocks[index].spec.space_after = 0.0;
    }
    let structural = HashSet::from([table, row]);
    document.blocks = blocks.into();
    document.text_runs = document
        .text_runs
        .iter()
        .filter(|run| !structural.contains(&run.id))
        .cloned()
        .collect::<Vec<_>>()
        .into();

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(120.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let geometry = |id| {
        layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == id)
            .unwrap()
    };
    let second = geometry(document.blocks[3].id).rect;
    let third = geometry(document.blocks[4].id).rect;
    assert!((second.size.x - 80.0 / 3.0).abs() < 0.001, "{second:?}");
    assert!((third.size.x - 160.0 / 3.0).abs() < 0.001, "{third:?}");
    let second_line = layout
        .visual_lines()
        .iter()
        .find(|line| line.source_range == document.text_runs[1].range)
        .unwrap();
    let third_line = layout
        .visual_lines()
        .iter()
        .find(|line| line.source_range == document.text_runs[2].range)
        .unwrap();
    assert!((second_line.rect.pos.x - (40.0 + (80.0 / 3.0 - 16.0) / 2.0)).abs() < 0.001);
    assert!((third_line.rect.pos.x - (120.0 - 32.0)).abs() < 0.001);
}

#[test]
fn table_intrinsic_crosses_styles_and_includes_nested_embedded_width() {
    let (mut document, presentation, mut shaper) =
        fixtures::fixture(&[16.0; 4], &[1, 1, 2, 4], None);
    let mut blocks = document.blocks.to_vec();
    let table = blocks[0].id;
    let row = blocks[1].id;
    let cell = blocks[2].id;
    let nested = blocks[3].id;
    blocks[0].spec.flow = BlockFlow::Table;
    blocks[0].spec.space_after = 0.0;
    blocks[0].spec.columns = Arc::from([ColumnConstraint {
        min_width: 0.0,
        max_width: None,
        alignment: ColumnAlignment::Start,
    }]);
    blocks[1].parent = Some(table);
    blocks[1].spec.flow = BlockFlow::TableRow;
    blocks[1].spec.space_after = 0.0;
    blocks[2].parent = Some(row);
    blocks[2].spec.flow = BlockFlow::TableCell { column: 0 };
    blocks[2].spec.space_after = 0.0;
    blocks[3].parent = Some(cell);
    blocks[3].spec.space_after = 0.0;
    document.blocks = blocks.into();

    let cell_run = document
        .text_runs
        .iter()
        .find(|run| run.id == cell)
        .unwrap()
        .clone();
    let split = cell_run.range.start().to_usize() + 1;
    let nested_run = document
        .text_runs
        .iter()
        .find(|run| run.id == nested)
        .unwrap()
        .clone();
    document.text_runs = Arc::from([
        LayoutTextRun {
            id: cell,
            range: range(cell_run.range.start().to_usize(), split),
            metrics: cell_run.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: cell,
            range: range(split, cell_run.range.end().to_usize()),
            metrics: TextMetrics {
                italic: true,
                ..cell_run.metrics
            },
            hidden: false,
        },
        nested_run.clone(),
    ]);
    document.embedded_blocks = Arc::from([MeasuredBlock {
        id: nested_run.id,
        source_range: nested_run.range,
        size: dvec2(90.0, 12.0),
        baseline: Some(10.0),
    }]);

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(60.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let cell_geometry = layout
        .visible_blocks()
        .iter()
        .find(|block| block.id == cell)
        .unwrap();
    assert_eq!(cell_geometry.rect.size.x, 90.0);
}

#[test]
fn embedded_block_geometry_uses_measured_size_instead_of_filling_available_width() {
    let (mut document, presentation, mut shaper) = fixtures::paragraph();
    let id = document.blocks[0].id;
    let source_range = document.blocks[0].source_range;
    let mut blocks = document.blocks.to_vec();
    blocks[0].spec.flow = BlockFlow::Embedded;
    document.blocks = blocks.into();
    document.text_runs = Arc::from([]);
    document.embedded_blocks = Arc::from([MeasuredBlock {
        id,
        source_range,
        size: dvec2(96.0, 48.0),
        baseline: None,
    }]);

    for (viewport_width, expected_width) in [(640.0, 96.0), (64.0, 64.0)] {
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(viewport_width, 480.0, 0.0, 480.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        let rect = layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == id)
            .unwrap()
            .rect;
        assert_eq!(rect.size, dvec2(expected_width, 48.0));
    }
}

#[test]
fn ten_thousand_row_table_reuses_intrinsics_and_bounds_full_shaping() {
    let (document, presentation, mut shaper) = fixtures::ten_thousand_row_table();
    let mut engine = LayoutEngine::default();
    let initial = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(60.0, 80.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let first_cell = document.blocks[10_001].id;
    assert_eq!(
        initial
            .visible_blocks()
            .iter()
            .find(|block| block.id == first_cell)
            .unwrap()
            .rect
            .size
            .x,
        120.0
    );
    assert!(shaper.shaped.len() <= 50);
    assert_eq!(shaper.intrinsic_measured, 10_000);
    let intrinsic_measured = shaper.intrinsic_measured;

    shaper.shaped.clear();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(60.0, 80.0, 120_000.0, 0.0),
            LayoutInvalidation::Viewport,
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped.len() <= 50);
    assert_eq!(shaper.intrinsic_measured, intrinsic_measured);
}

#[test]
fn center_and_end_alignment_shift_each_wrapped_line_payload() {
    let (mut document, presentation, _) = fixtures::fixture(&[16.0; 4], &[1, 1, 5, 5], None);
    let mut blocks = document.blocks.to_vec();
    let table = blocks[0].id;
    let row = blocks[1].id;
    blocks[0].spec.flow = BlockFlow::Table;
    blocks[0].spec.space_after = 0.0;
    blocks[0].spec.columns = Arc::from([
        ColumnConstraint {
            min_width: 30.0,
            max_width: Some(30.0),
            alignment: ColumnAlignment::Center,
        },
        ColumnConstraint {
            min_width: 30.0,
            max_width: Some(30.0),
            alignment: ColumnAlignment::End,
        },
    ]);
    blocks[1].parent = Some(table);
    blocks[1].spec.flow = BlockFlow::TableRow;
    blocks[1].spec.space_after = 0.0;
    for (index, column) in [(2, 0), (3, 1)] {
        blocks[index].parent = Some(row);
        blocks[index].spec.flow = BlockFlow::TableCell { column };
        blocks[index].spec.space_after = 0.0;
    }
    let structural = HashSet::from([table, row]);
    document.blocks = blocks.into();
    document.text_runs = document
        .text_runs
        .iter()
        .filter(|run| !structural.contains(&run.id))
        .cloned()
        .collect::<Vec<_>>()
        .into();
    let mut shaper = GlyphCharacterShaper;
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(60.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    for (run, expected_x) in document.text_runs.iter().zip([5.0, 40.0]) {
        let second_start = t(run.range.start().to_usize() + 3);
        let line = layout
            .visual_lines()
            .iter()
            .find(|line| line.source_range.start() == second_start)
            .unwrap();
        let cluster = layout
            .glyph_clusters()
            .iter()
            .find(|cluster| cluster.source_range.start() == second_start)
            .unwrap();
        assert_eq!(line.rect.pos.x, expected_x);
        assert_eq!(cluster.rect.pos.x, expected_x);
        assert_eq!(cluster.caret_stops[0].point.x, expected_x);
        assert_eq!(cluster.glyphs[0].origin.x, expected_x);
    }
}

#[test]
fn hanging_splits_spanning_marker_run_and_aligns_mixed_metrics_baseline() {
    let (mut document, presentation, _) = fixtures::paragraph();
    let original = document.text_runs[0].clone();
    let start = original.range.start().to_usize();
    let middle = start + 4;
    let mut blocks = document.blocks.to_vec();
    blocks[0].spec.flow = BlockFlow::Hanging {
        marker_range: range(start, start + 1),
        content_indent: 20.0,
    };
    document.blocks = blocks.into();
    document.text_runs = Arc::from([
        LayoutTextRun {
            id: original.id,
            range: range(start, middle),
            metrics: TextMetrics {
                font_size: 30.0,
                ..original.metrics
            },
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(middle, original.range.end().to_usize()),
            metrics: original.metrics,
            hidden: false,
        },
    ]);
    let mut shaper = MetricGlyphShaper;
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let marker = layout
        .glyph_clusters()
        .iter()
        .find(|cluster| cluster.source_range == range(start, start + 1))
        .unwrap();
    let small_content = layout
        .glyph_clusters()
        .iter()
        .find(|cluster| cluster.source_range == range(middle, original.range.end().to_usize()))
        .unwrap();
    assert_eq!(marker.rect.pos.x, 0.0);
    assert!(small_content.rect.pos.x >= 20.0);
    assert!((marker.glyphs[0].baseline - small_content.glyphs[0].baseline).abs() < 0.001);
}

#[test]
fn adjacent_styled_paragraph_runs_share_one_inline_line() {
    let (mut document, presentation, mut shaper) = fixtures::paragraph();
    let original = document.text_runs[0].clone();
    let start = original.range.start().to_usize();
    document.text_runs = Arc::from([
        LayoutTextRun {
            id: original.id,
            range: range(start, start + 2),
            metrics: original.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(start + 2, start + 4),
            metrics: TextMetrics {
                italic: true,
                ..original.metrics
            },
            hidden: false,
        },
    ]);

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(100.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    assert_eq!(layout.visual_lines().len(), 1);
    let positions = layout
        .glyph_clusters()
        .iter()
        .map(|cluster| cluster.rect.pos)
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![
            dvec2(0.0, 0.0),
            dvec2(8.0, 0.0),
            dvec2(16.0, 0.0),
            dvec2(24.0, 0.0),
        ]
    );
}

#[test]
fn styled_unbreakable_word_does_not_break_at_span_boundary() {
    let (document, presentation, mut shaper) = fixtures::styled_paragraph("international", 5);

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(72.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    assert_eq!(shaper.paragraph_requests.len(), 1);
    assert_eq!(shaper.paragraph_requests[0].spans, 2);
    assert_eq!(layout.visual_lines().len(), 1);
    assert!(layout.visual_lines()[0].rect.size.x > 72.0);
}

#[test]
fn styled_bidi_uses_one_paragraph_context_and_exact_span_payloads() {
    let (document, presentation, mut shaper) = fixtures::styled_paragraph("abc אבג", 4);
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(300.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    assert_eq!(shaper.paragraph_requests.len(), 1);
    assert_eq!(shaper.paragraph_requests[0].spans, 2);
    let mut logical = layout.glyph_clusters().iter().collect::<Vec<_>>();
    logical.sort_by_key(|cluster| cluster.source_range.start());
    let split = document.text_runs[0].range.end();
    assert!(logical
        .iter()
        .filter(|cluster| cluster.source_range.end() <= split)
        .all(|cluster| cluster.metrics == document.text_runs[0].metrics));
    assert!(logical
        .iter()
        .filter(|cluster| cluster.source_range.start() >= split)
        .all(|cluster| cluster.metrics == document.text_runs[1].metrics));
    assert_eq!(
        logical
            .iter()
            .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.glyph_id))
            .collect::<Vec<_>>(),
        "abc אבג"
            .chars()
            .map(|character| character as u16)
            .collect::<Vec<_>>()
    );
    let rtl = logical
        .iter()
        .filter(|cluster| cluster.source_range.start() >= split)
        .collect::<Vec<_>>();
    assert!(rtl
        .windows(2)
        .all(|pair| pair[0].rect.pos.x > pair[1].rect.pos.x));
}

#[test]
fn empty_continuation_row_and_caret_boundary_survive_shaping() {
    let (document, presentation, mut shaper) = fixtures::styled_paragraph("line\n", 5);
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(200.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();

    assert_eq!(layout.visual_lines().len(), 2);
    let empty = &layout.visual_lines()[1];
    assert_eq!(empty.source_range.start(), empty.source_range.end());
    assert_eq!(
        empty.source_range.start(),
        document.text_runs[0].range.end()
    );
}

#[test]
fn hanging_runs_stay_clamped_and_content_advances_across_styles() {
    let (mut document, presentation, mut shaper) = fixtures::paragraph();
    let original = document.text_runs[0].clone();
    let start = original.range.start().to_usize();
    let mut blocks = document.blocks.to_vec();
    blocks[0].spec.flow = BlockFlow::Hanging {
        marker_range: range(start + 2, start + 3),
        content_indent: 20.0,
    };
    document.blocks = blocks.into();
    document.text_runs = Arc::from([
        LayoutTextRun {
            id: original.id,
            range: range(start, start + 1),
            metrics: original.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(start + 1, start + 2),
            metrics: original.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(start + 2, start + 3),
            metrics: original.metrics,
            hidden: false,
        },
        LayoutTextRun {
            id: original.id,
            range: range(start + 3, start + 5),
            metrics: TextMetrics {
                italic: true,
                ..original.metrics
            },
            hidden: false,
        },
    ]);

    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(100.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let mut clusters = layout.glyph_clusters().iter().collect::<Vec<_>>();
    clusters.sort_by_key(|cluster| cluster.source_range.start());
    assert_eq!(
        clusters
            .iter()
            .map(|cluster| cluster.source_range)
            .collect::<Vec<_>>(),
        vec![
            range(start, start + 1),
            range(start + 1, start + 2),
            range(start + 2, start + 3),
            range(start + 3, start + 4),
            range(start + 4, start + 5),
        ]
    );
    let content = clusters
        .into_iter()
        .filter(|cluster| cluster.source_range != range(start + 2, start + 3))
        .collect::<Vec<_>>();
    assert!(content
        .windows(2)
        .all(|pair| pair[0].rect.pos.x < pair[1].rect.pos.x));
    assert!(content.iter().all(|cluster| cluster.rect.pos.x >= 20.0));
}

#[test]
fn logical_cluster_ids_survive_bidi_reorder_and_width_changes() {
    let (document, presentation, _) = fixtures::paragraph();
    let start = document.text_runs[0].range.start().to_usize();
    let shaped = ShapedRun {
        clusters: Arc::from([
            shaped_cluster(document.text_runs[0].id, start, start + 1, 0),
            shaped_cluster(document.text_runs[0].id, start + 1, start + 2, 1),
            shaped_cluster(document.text_runs[0].id, start + 2, start + 3, 1),
            shaped_cluster(document.text_runs[0].id, start + 3, start + 4, 0),
        ]),
        ascender: 12.0,
        descender: 4.0,
        line_gap: 0.0,
    };
    let layout_at = |width| {
        LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(width, 100.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut FixedShaper(shaped.clone()),
            )
            .unwrap()
    };
    let ids_by_source = |layout: &LayoutSnapshot| {
        let mut clusters = layout.glyph_clusters().iter().collect::<Vec<_>>();
        clusters.sort_by_key(|cluster| cluster.source_range.start());
        clusters
            .into_iter()
            .map(|cluster| (cluster.source_range, cluster.id))
            .collect::<Vec<_>>()
    };

    let wide = layout_at(100.0);
    let narrow = layout_at(20.0);
    assert_eq!(ids_by_source(&wide), ids_by_source(&narrow));
}

fn test_paragraph(
    request: ParagraphShapeRequest<'_>,
    shaped_runs: Vec<(ShapeSpan, ShapedRun)>,
) -> ShapedParagraph {
    let mut clusters = Vec::new();
    for (span, shaped) in &shaped_runs {
        for cluster in shaped.clusters.iter() {
            let mut cluster = cluster.clone();
            cluster.span_id = span.id;
            cluster.metrics = span.metrics;
            clusters.push(cluster);
        }
    }
    clusters.sort_by_key(|cluster| cluster.source_range.start());
    let paragraph_text = request.source.slice(request.paragraph_range).unwrap();
    let bidi = BidiInfo::new(paragraph_text, None);
    for (ordinal, cluster) in clusters.iter_mut().enumerate() {
        cluster.id = GeometryElementId {
            layout: request.paragraph_id.layout,
            cluster_ordinal: 0x8000_0000
                | ((request.paragraph_id.cluster_ordinal & 0x0f) << 24)
                | ordinal as u32,
        };
        let relative =
            cluster.source_range.start().to_usize() - request.paragraph_range.start().to_usize();
        cluster.bidi_level = bidi.levels.get(relative).map_or(0, |level| level.number());
    }

    let mut legal_breaks = paragraph_text
        .char_indices()
        .filter(|(_, character)| character.is_whitespace() || request.spans.len() == 1)
        .map(|(relative, character)| {
            t(request.paragraph_range.start().to_usize() + relative + character.len_utf8())
        })
        .collect::<Vec<_>>();
    legal_breaks.push(request.paragraph_range.end());
    legal_breaks.sort_unstable();
    legal_breaks.dedup();

    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut segment_start = 0;
    let mut row_width = 0.0;
    let mut row_top = 0.0;
    for index in 0..clusters.len() {
        let boundary = clusters[index].source_range.end();
        if !legal_breaks.contains(&boundary) && index + 1 < clusters.len() {
            continue;
        }
        let segment_end = index + 1;
        let segment_width = clusters[segment_start..segment_end]
            .iter()
            .map(|cluster| cluster.advance)
            .sum::<f64>();
        let available = if rows.is_empty() {
            request.first_row_width
        } else {
            request.full_width
        };
        if segment_start > row_start && row_width + segment_width > available.max(1.0) {
            let row = test_row(
                request.paragraph_id,
                &clusters,
                row_start,
                segment_start,
                row_top,
            );
            row_top += row.ascender + row.descender + row.line_gap;
            rows.push(row);
            row_start = segment_start;
            row_width = 0.0;
        }
        row_width += segment_width;
        segment_start = segment_end;
    }
    if row_start < clusters.len() || rows.is_empty() {
        let row = test_row(
            request.paragraph_id,
            &clusters,
            row_start,
            clusters.len(),
            row_top,
        );
        row_top += row.ascender + row.descender + row.line_gap;
        rows.push(row);
    }
    if paragraph_text.ends_with('\n') {
        let boundary = request.paragraph_range.end();
        let metrics = request.spans.last().unwrap().metrics;
        rows.push(ShapedRow {
            id: GeometryElementId {
                layout: request.paragraph_id.layout,
                cluster_ordinal: 0xd000_0000
                    | ((request.paragraph_id.cluster_ordinal & 0x0f) << 24),
            },
            source_range: range(boundary.to_usize(), boundary.to_usize()),
            cluster_range: clusters.len()..clusters.len(),
            caret_offsets: Arc::from([boundary]),
            ascender: metrics.font_size as f64 * 0.8,
            descender: metrics.font_size as f64 * 0.2,
            line_gap: 0.0,
            line_spacing_scale: 1.0,
            row_top,
        });
    }
    ShapedParagraph {
        rows: rows.into(),
        fragments: request
            .spans
            .iter()
            .map(|span| ShapedFragment {
                id: GeometryElementId {
                    layout: request.paragraph_id.layout,
                    cluster_ordinal: 0x6000_0000
                        | ((request.paragraph_id.cluster_ordinal & 0x0f) << 24)
                        | span.stable_ordinal,
                },
                span_id: span.id,
                stable_ordinal: span.stable_ordinal,
                source_range: span.source_range,
                metrics: span.metrics,
            })
            .collect::<Vec<_>>()
            .into(),
        clusters: clusters.into(),
        bidi_levels: bidi
            .levels
            .iter()
            .map(|level| level.number())
            .collect::<Vec<_>>()
            .into(),
        legal_breaks: legal_breaks.into(),
    }
}

fn test_row(
    paragraph_id: GeometryElementId,
    clusters: &[ShapedCluster],
    start: usize,
    end: usize,
    row_top: f64,
) -> ShapedRow {
    let source_start = clusters[start].source_range.start();
    let source_end = clusters[end - 1].source_range.end();
    let ascender = clusters[start..end]
        .iter()
        .map(|cluster| cluster.metrics.font_size as f64 * 0.8)
        .fold(0.0, f64::max);
    let descender = clusters[start..end]
        .iter()
        .map(|cluster| cluster.metrics.font_size as f64 * 0.2)
        .fold(0.0, f64::max);
    ShapedRow {
        id: GeometryElementId {
            layout: paragraph_id.layout,
            cluster_ordinal: 0xc000_0000
                | ((paragraph_id.cluster_ordinal & 0x0f) << 24)
                | (source_start.to_usize() as u32 & 0x00ff_ffff),
        },
        source_range: TextRange::new(source_start, source_end).unwrap(),
        cluster_range: start..end,
        caret_offsets: Arc::from([source_start, source_end]),
        ascender,
        descender,
        line_gap: 0.0,
        line_spacing_scale: 1.0,
        row_top,
    }
}

fn test_intrinsic(
    request: ParagraphIntrinsicRequest<'_>,
    advance: impl Fn(&ShapeSpan) -> f64,
) -> ParagraphIntrinsic {
    let mut min_content = 0.0_f64;
    let mut max_content = 0.0_f64;
    let mut word = 0.0_f64;
    for span in request.spans {
        let text = request.source.slice(span.source_range).unwrap();
        for character in text.chars() {
            let width = advance(span);
            max_content += width;
            if character.is_whitespace() {
                min_content = min_content.max(word);
                word = 0.0;
            } else {
                word += width;
            }
        }
    }
    ParagraphIntrinsic {
        min_content: min_content.max(word),
        max_content,
    }
}

struct GlyphCharacterShaper;

impl GlyphCharacterShaper {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        _max_width: f64,
    ) -> Result<ShapedRun, LayoutError> {
        let text = source.slice(run.range).unwrap();
        let clusters = text
            .char_indices()
            .map(|(relative, character)| {
                let start = run.range.start().to_usize() + relative;
                let end = start + character.len_utf8();
                ShapedCluster {
                    id: GeometryElementId {
                        layout: run.id,
                        cluster_ordinal: 0,
                    },
                    span_id: GeometryElementId {
                        layout: run.id,
                        cluster_ordinal: 0,
                    },
                    source_range: range(start, end),
                    metrics: run.metrics,
                    advance: 10.0,
                    bidi_level: 0,
                    row_ordinal: 0,
                    row_top: 0.0,
                    caret_offsets: Arc::from([t(start), t(end)]),
                    glyphs: Arc::from([ShapedGlyph {
                        glyph_id: 1,
                        origin: dvec2(0.0, 0.0),
                        advance: 10.0,
                        paint_scale: 1.0,
                        font: None,
                        font_key: run.metrics.font,
                        font_size: run.metrics.font_size,
                        ascender: 12.8,
                        descender: -3.2,
                        line_gap: 0.0,
                        baseline: 12.8,
                        offset: 0.0,
                        color: None,
                    }]),
                }
            })
            .collect::<Vec<_>>();
        Ok(ShapedRun {
            clusters: clusters.into(),
            ascender: 12.8,
            descender: 3.2,
            line_gap: 0.0,
        })
    }
}

impl TextShaper for GlyphCharacterShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        let runs = request
            .spans
            .iter()
            .map(|span| {
                let run = LayoutTextRun {
                    id: span.run_id,
                    range: span.source_range,
                    metrics: span.metrics,
                    hidden: false,
                };
                self.shape(request.source, &run, request.full_width)
                    .map(|shaped| (span.clone(), shaped))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(test_paragraph(request, runs))
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        Ok(test_intrinsic(request, |_| 10.0))
    }
}

struct FixedShaper(ShapedRun);

impl TextShaper for FixedShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        let mut paragraph =
            test_paragraph(request, vec![(request.spans[0].clone(), self.0.clone())]);
        let mut clusters = paragraph.clusters.to_vec();
        for cluster in &mut clusters {
            if let Some(original) = self
                .0
                .clusters
                .iter()
                .find(|item| item.source_range == cluster.source_range)
            {
                cluster.bidi_level = original.bidi_level;
            }
        }
        paragraph.clusters = clusters.into();
        Ok(paragraph)
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        Ok(test_intrinsic(request, |_| 10.0))
    }
}

struct MetricGlyphShaper;

impl MetricGlyphShaper {
    fn shape(
        &mut self,
        _source: &SourceText,
        run: &LayoutTextRun,
        _max_width: f64,
    ) -> Result<ShapedRun, LayoutError> {
        let ascender = run.metrics.font_size as f64 * 0.8;
        let descender = -(run.metrics.font_size as f64 * 0.2);
        Ok(ShapedRun {
            clusters: Arc::from([ShapedCluster {
                id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                span_id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                source_range: run.range,
                metrics: run.metrics,
                advance: 10.0,
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([run.range.start(), run.range.end()]),
                glyphs: Arc::from([ShapedGlyph {
                    glyph_id: 1,
                    origin: dvec2(0.0, 0.0),
                    advance: 10.0,
                    paint_scale: 1.0,
                    font: None,
                    font_key: run.metrics.font,
                    font_size: run.metrics.font_size,
                    ascender,
                    descender,
                    line_gap: 0.0,
                    baseline: ascender,
                    offset: 0.0,
                    color: None,
                }]),
            }]),
            ascender,
            descender,
            line_gap: 0.0,
        })
    }
}

impl TextShaper for MetricGlyphShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        let runs = request
            .spans
            .iter()
            .map(|span| {
                let run = LayoutTextRun {
                    id: span.run_id,
                    range: span.source_range,
                    metrics: span.metrics,
                    hidden: false,
                };
                self.shape(request.source, &run, request.full_width)
                    .map(|shaped| (span.clone(), shaped))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(test_paragraph(request, runs))
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        Ok(test_intrinsic(request, |_| 10.0))
    }
}

fn shaped_cluster(
    layout_id: LayoutElementId,
    start: usize,
    end: usize,
    bidi_level: u8,
) -> ShapedCluster {
    ShapedCluster {
        id: GeometryElementId {
            layout: layout_id,
            cluster_ordinal: 0,
        },
        span_id: GeometryElementId {
            layout: layout_id,
            cluster_ordinal: 0,
        },
        source_range: range(start, end),
        metrics: TextMetrics {
            font: FontKey(1),
            font_size: 16.0,
            line_spacing: 0.0,
            weight: FontWeight(400),
            italic: false,
        },
        advance: 10.0,
        bidi_level,
        row_ordinal: 0,
        row_top: 0.0,
        caret_offsets: Arc::from([t(start), t(end)]),
        glyphs: Arc::from([]),
    }
}

#[derive(Default)]
struct FakeShaper {
    shaped: HashSet<LayoutElementId>,
    fail_fragment: Option<u32>,
    advance_override: HashMap<u32, f64>,
    intrinsic_measured: usize,
    paragraph_requests: Vec<RecordedParagraphRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RecordedParagraphRequest {
    full_width: f64,
    first_row_width: f64,
    spans: usize,
}

impl FakeShaper {
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
                id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                span_id: GeometryElementId {
                    layout: run.id,
                    cluster_ordinal: 0,
                },
                source_range: range(start, end),
                metrics: run.metrics,
                advance: self
                    .advance_override
                    .get(&run.id.fragment_ordinal)
                    .copied()
                    .unwrap_or(run.metrics.font.0 as f64),
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([t(start), t(end)]),
                glyphs: Arc::from([ShapedGlyph {
                    glyph_id: u16::try_from(character as u32).unwrap_or(0),
                    origin: dvec2(0.0, 0.0),
                    advance: self
                        .advance_override
                        .get(&run.id.fragment_ordinal)
                        .copied()
                        .unwrap_or(run.metrics.font.0 as f64),
                    paint_scale: 1.0,
                    font: None,
                    font_key: run.metrics.font,
                    font_size: run.metrics.font_size,
                    ascender: run.metrics.font_size as f64 * 0.8,
                    descender: -(run.metrics.font_size as f64 * 0.2),
                    line_gap: 0.0,
                    baseline: run.metrics.font_size as f64 * 0.8,
                    offset: 0.0,
                    color: None,
                }]),
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

impl TextShaper for FakeShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        self.paragraph_requests.push(RecordedParagraphRequest {
            full_width: request.full_width,
            first_row_width: request.first_row_width,
            spans: request.spans.len(),
        });
        let runs = request
            .spans
            .iter()
            .map(|span| {
                let run = LayoutTextRun {
                    id: span.run_id,
                    range: span.source_range,
                    metrics: span.metrics,
                    hidden: false,
                };
                self.shape(request.source, &run, request.full_width)
                    .map(|shaped| (span.clone(), shaped))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(test_paragraph(request, runs))
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        self.intrinsic_measured += 1;
        Ok(test_intrinsic(request, |span| {
            self.advance_override
                .get(&span.run_id.fragment_ordinal)
                .copied()
                .unwrap_or(span.metrics.font.0 as f64)
        }))
    }
}

#[test]
fn same_y_table_lanes_keep_hit_testing_vertical_motion_and_selection_independent() {
    let (document, layout) = fixtures::two_by_two_table();
    let run_range = |index: usize| {
        document
            .text_runs
            .iter()
            .find(|run| run.id == document.blocks[index].id)
            .unwrap()
            .range
    };
    let owns = |index: usize, position: TextPosition| {
        let range = run_range(index);
        range.start() <= position.offset && position.offset <= range.end()
    };

    // The two first-row cells share a Y band but own their own X lanes.
    assert!(owns(2, layout.point_to_source(dvec2(10.0, 8.0))));
    assert!(owns(3, layout.point_to_source(dvec2(50.0, 8.0))));

    // Vertical motion leaves the current visual row instead of stepping into a
    // sibling lane that happens to share the row's Y band.
    let (from_right_cell, _) = layout
        .move_vertical(
            TextPosition::new(run_range(3).start(), Affinity::Before),
            None,
            1,
        )
        .unwrap();
    assert!(owns(2, from_right_cell));
    let (from_continuation, _) = layout
        .move_vertical(
            TextPosition::new(run_range(2).end(), Affinity::After),
            Some(6.0),
            1,
        )
        .unwrap();
    assert!(owns(5, from_continuation));

    // Selection intersects one lane at a time and never bridges two columns.
    let rects = layout
        .selection_rects(Selection::new(
            TextPosition::new(run_range(2).start(), Affinity::Before),
            TextPosition::new(run_range(3).end(), Affinity::After),
        ))
        .unwrap();
    let first_row = rects
        .iter()
        .filter(|rect| rect.pos.y == 0.0)
        .collect::<Vec<_>>();
    assert_eq!(first_row.len(), 2);
    assert!(first_row
        .iter()
        .any(|rect| rect.pos.x >= 5.0 && rect.pos.x + rect.size.x <= 45.0));
    assert!(first_row.iter().any(|rect| rect.pos.x >= 45.0));
}

#[test]
fn hanging_marker_and_content_share_a_row_without_sharing_stops() {
    let (document, layout) = fixtures::wrapped_hanging_item();
    let marker = document.blocks[0].spec.clone();
    let BlockFlow::Hanging { marker_range, .. } = marker.flow else {
        panic!("the hanging fixture keeps its hanging flow");
    };
    let rows = layout.visual_rows();
    assert_eq!(layout.visual_lanes()[rows[0].lanes.clone()].len(), 2);
    let marker_stop = layout.point_to_source(dvec2(1.0, 4.0));
    let content_stop = layout.point_to_source(dvec2(60.0, 4.0));
    assert!(marker_stop.offset <= marker_range.end());
    assert!(content_stop.offset >= marker_range.end());
}

#[test]
fn visible_source_range_uses_all_lane_min_and_max_boundaries() {
    let (document, layout) = fixtures::hanging_with_content_prefix();
    let prefix_start = document.text_runs[0].range.start();
    let last_end = document.text_runs[0].range.end();
    assert_eq!(layout.visible_source_range().start(), prefix_start);
    assert_eq!(layout.visible_source_range().end(), last_end);
}

#[test]
fn cluster_alignment_offset_lookup_is_constant_time() {
    let (document, presentation, mut shaper) = fixtures::fixture(&[16.0], &[2000], None);
    let mut engine = LayoutEngine::default();
    let layout = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100_000.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let stats = engine.last_lane_offset_stats_for_test();
    assert_eq!(
        stats.direct_lane_offset_lookups,
        layout.glyph_clusters().len()
    );
    assert_eq!(stats.linear_lane_offset_scans, 0);
}

#[test]
fn unconstrained_table_uses_intrinsic_column_bases() {
    let (document, layout) = fixtures::unconstrained_unequal_table();
    let width = |index: usize| {
        layout
            .visible_blocks()
            .iter()
            .find(|block| block.id == document.blocks[index].id)
            .unwrap()
            .rect
            .size
            .x
    };
    assert!(width(3) > width(2) * 2.0, "{} vs {}", width(3), width(2));
}

#[test]
fn styled_unbreakable_min_content_overflows_narrow_table() {
    let (document, layout) = fixtures::styled_unbreakable_table();
    let table = layout
        .visible_blocks()
        .iter()
        .find(|block| block.id == document.blocks[0].id)
        .unwrap();
    assert!(table.rect.size.x > 80.0, "{:?}", table.rect);
}

#[test]
fn nested_tables_measure_each_intrinsic_paragraph_once() {
    let (document, presentation, mut shaper) = fixtures::nested_table();
    LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(300.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(shaper.intrinsic_measured, 1);
}

#[test]
fn removed_tables_prune_intrinsic_cache_entries() {
    let mut engine = LayoutEngine::default();
    let (document, presentation, mut shaper) = fixtures::styled_unbreakable_table_document();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(engine.cached_table_intrinsic_count_for_test(), 1);
    let (document, presentation, mut shaper) = fixtures::paragraph();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(200.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(engine.cached_table_intrinsic_count_for_test(), 0);
}

#[test]
fn adversarial_height_changes_stop_at_structural_hydration_bound() {
    let (document, presentation, _) = fixtures::one_hundred_blocks();
    let mut shaper = AdversarialHeightShaper::default();
    let mut engine = LayoutEngine::default();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let stats = engine.last_shape_call_stats_for_test();
    assert!(stats.full_shape <= 100, "{stats:?}");
    assert!(stats.hydration_passes <= 101, "{stats:?}");
}

#[test]
fn exhausted_shape_budget_returns_typed_error_before_backend_call() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let error = LayoutEngine::default()
        .layout_with_budget_for_test(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
            LayoutBudget::for_test(0, 0, 1),
        )
        .unwrap_err();
    assert!(
        matches!(
            error,
            LayoutError::BudgetExceeded {
                phase: LayoutWorkPhase::FullShape,
                limit: 0,
                observed: 0,
            }
        ),
        "{error:?}"
    );
    assert!(shaper.shaped.is_empty());
}

#[test]
fn exhausted_hydration_budget_returns_typed_error() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let error = LayoutEngine::default()
        .layout_with_budget_for_test(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
            LayoutBudget::for_test(4, 4, 0),
        )
        .unwrap_err();
    assert!(
        matches!(
            error,
            LayoutError::BudgetExceeded {
                phase: LayoutWorkPhase::Hydration,
                limit: 0,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn total_shape_calls_equal_unique_visible_paragraph_keys() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let mut engine = LayoutEngine::default();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let stats = engine.last_shape_call_stats_for_test();
    assert_eq!(stats.full_shape, shaper.paragraph_requests.len());
    assert!(stats.full_shape < 100, "{stats:?}");
    assert_eq!(stats.intrinsic, 0);
}

#[test]
fn table_intrinsic_calls_are_counted_apart_from_full_shaping() {
    let (document, presentation, mut shaper) = fixtures::nested_table();
    let mut engine = LayoutEngine::default();
    engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(300.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let stats = engine.last_shape_call_stats_for_test();
    assert_eq!(stats.intrinsic, 1);
    assert!(stats.full_shape >= 1, "{stats:?}");
}

/// Reports a different height on every paragraph, so each hydration pass moves
/// the measurement window.
#[derive(Default)]
struct AdversarialHeightShaper {
    inner: FakeShaper,
    calls: usize,
}

impl TextShaper for AdversarialHeightShaper {
    fn shape_paragraph(
        &mut self,
        request: ParagraphShapeRequest<'_>,
    ) -> Result<ShapedParagraph, LayoutError> {
        self.calls += 1;
        let scale = 1.0 + (self.calls % 5) as f64;
        let mut paragraph = self.inner.shape_paragraph(request)?;
        let clusters = paragraph
            .clusters
            .iter()
            .cloned()
            .map(|mut cluster| {
                cluster.glyphs = cluster
                    .glyphs
                    .iter()
                    .cloned()
                    .map(|mut glyph| {
                        glyph.ascender *= scale;
                        glyph.descender *= scale;
                        glyph
                    })
                    .collect::<Vec<_>>()
                    .into();
                cluster
            })
            .collect::<Vec<_>>();
        paragraph.clusters = clusters.into();
        Ok(paragraph)
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: ParagraphIntrinsicRequest<'_>,
    ) -> Result<ParagraphIntrinsic, LayoutError> {
        self.inner.measure_paragraph_intrinsic(request)
    }
}

mod fixtures {
    use super::*;

    /// A table with no column constraints whose second cell holds four times
    /// the content of the first.
    pub fn unconstrained_unequal_table() -> (LayoutDocument, LayoutSnapshot) {
        let (document, presentation, mut shaper) = table_document(&[2, 8], &[]);
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(300.0, 200.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        (document, layout)
    }

    pub fn styled_unbreakable_table() -> (LayoutDocument, LayoutSnapshot) {
        let (document, presentation, mut shaper) = styled_unbreakable_table_document();
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(80.0, 200.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        (document, layout)
    }

    /// One cell holding a single unbreakable 20-character word.
    pub fn styled_unbreakable_table_document(
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        table_document(&[20], &[])
    }

    /// An outer table whose only cell holds a nested table.
    pub fn nested_table() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        let (mut document, presentation, shaper) = fixture(&[16.0; 6], &[1, 1, 1, 1, 1, 4], None);
        let mut blocks = document.blocks.to_vec();
        let outer = blocks[0].id;
        let outer_row = blocks[1].id;
        let outer_cell = blocks[2].id;
        let inner = blocks[3].id;
        let inner_row = blocks[4].id;
        for index in [0, 3] {
            blocks[index].spec.flow = BlockFlow::Table;
            blocks[index].spec.space_after = 0.0;
            blocks[index].spec.columns = Arc::from([]);
        }
        for (index, parent) in [(1, outer), (4, inner)] {
            blocks[index].parent = Some(parent);
            blocks[index].spec.flow = BlockFlow::TableRow;
            blocks[index].spec.space_after = 0.0;
        }
        for (index, parent) in [(2, outer_row), (5, inner_row)] {
            blocks[index].parent = Some(parent);
            blocks[index].spec.flow = BlockFlow::TableCell { column: 0 };
            blocks[index].spec.space_after = 0.0;
        }
        blocks[3].parent = Some(outer_cell);
        let structural = HashSet::from([outer, outer_row, outer_cell, inner, inner_row]);
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| !structural.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        (document, presentation, shaper)
    }

    /// One table row with one cell per entry in `cells`, each holding that many
    /// unbreakable characters.
    pub(super) fn table_document(
        cells: &[usize],
        constraints: &[ColumnConstraint],
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        let mut characters = vec![1, 1];
        characters.extend_from_slice(cells);
        let (mut document, presentation, shaper) =
            fixture(&vec![16.0; characters.len()], &characters, None);
        let mut blocks = document.blocks.to_vec();
        let table = blocks[0].id;
        let row = blocks[1].id;
        blocks[0].spec.flow = BlockFlow::Table;
        blocks[0].spec.space_after = 0.0;
        blocks[0].spec.columns = constraints.to_vec().into();
        blocks[1].parent = Some(table);
        blocks[1].spec.flow = BlockFlow::TableRow;
        blocks[1].spec.space_after = 0.0;
        for column in 0..cells.len() {
            let index = 2 + column;
            blocks[index].parent = Some(row);
            blocks[index].spec.flow = BlockFlow::TableCell {
                column: column as u32,
            };
            blocks[index].spec.space_after = 0.0;
        }
        let structural = HashSet::from([table, row]);
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| !structural.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        (document, presentation, shaper)
    }

    pub fn two_by_two_table() -> (LayoutDocument, LayoutSnapshot) {
        let (mut document, presentation, mut shaper) = fixture(
            &[16.0, 16.0, 16.0, 16.0, 16.0, 16.0, 16.0],
            &[1, 1, 8, 4, 1, 2, 2],
            None,
        );
        let mut blocks = document.blocks.to_vec();
        let table = blocks[0].id;
        let row_one = blocks[1].id;
        let row_two = blocks[4].id;
        blocks[0].spec.flow = BlockFlow::Table;
        blocks[0].spec.insets.left = 5.0;
        blocks[0].spec.space_after = 0.0;
        blocks[0].spec.columns = Arc::from([
            ColumnConstraint {
                min_width: 40.0,
                max_width: Some(40.0),
                alignment: ColumnAlignment::Start,
            },
            ColumnConstraint {
                min_width: 60.0,
                max_width: Some(60.0),
                alignment: ColumnAlignment::Start,
            },
        ]);
        for row_index in [1, 4] {
            blocks[row_index].parent = Some(table);
            blocks[row_index].spec.flow = BlockFlow::TableRow;
            blocks[row_index].spec.space_after = 0.0;
        }
        for (index, parent, column) in [
            (2, row_one, 0),
            (3, row_one, 1),
            (5, row_two, 0),
            (6, row_two, 1),
        ] {
            blocks[index].parent = Some(parent);
            blocks[index].spec.flow = BlockFlow::TableCell { column };
            blocks[index].spec.space_after = 0.0;
        }
        let structural = HashSet::from([table, row_one, row_two]);
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| !structural.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(200.0, 100.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        (document, layout)
    }

    /// A hanging item whose content wraps under a shared first row.
    pub fn wrapped_hanging_item() -> (LayoutDocument, LayoutSnapshot) {
        let (mut document, presentation, mut shaper) = fixture(&[16.0], &[12], None);
        let content = document.text_runs[0].range;
        let start = content.start().to_usize();
        let mut blocks = document.blocks.to_vec();
        blocks[0].spec.flow = BlockFlow::Hanging {
            marker_range: range(start, start + 1),
            content_indent: 20.0,
        };
        document.blocks = blocks.into();
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(96.0, 200.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        (document, layout)
    }

    /// A hanging item whose marker starts after some content, so the lane that
    /// holds the earliest source offset is stored after later lanes.
    pub fn hanging_with_content_prefix() -> (LayoutDocument, LayoutSnapshot) {
        let (mut document, presentation, mut shaper) = fixture(&[16.0], &[12], None);
        let content = document.text_runs[0].range;
        let start = content.start().to_usize();
        let mut blocks = document.blocks.to_vec();
        blocks[0].spec.flow = BlockFlow::Hanging {
            marker_range: range(start + 2, start + 3),
            content_indent: 20.0,
        };
        document.blocks = blocks.into();
        let layout = LayoutEngine::default()
            .layout(
                &document,
                &presentation,
                LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .unwrap();
        (document, layout)
    }

    pub fn mixed_heading_and_body(
        _width: f64,
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[30.0, 16.0], &[4, 18], None)
    }

    pub fn one_hundred_blocks() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&vec![20.0; 100], &vec![6; 100], None)
    }

    pub fn ten_thousand_blocks() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&vec![20.0; 10_000], &vec![6; 10_000], None)
    }

    pub fn ten_thousand_row_table() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        const ROWS: usize = 10_000;
        let block_count = 1 + ROWS * 2;
        let (mut document, presentation, mut shaper) =
            fixture(&vec![16.0; block_count], &vec![1; block_count], None);
        let mut blocks = document.blocks.to_vec();
        let table = blocks[0].id;
        blocks[0].spec.flow = BlockFlow::Table;
        blocks[0].spec.space_after = 0.0;
        blocks[0].spec.columns = Arc::from([ColumnConstraint {
            min_width: 0.0,
            max_width: None,
            alignment: ColumnAlignment::Start,
        }]);
        let mut structural = HashSet::from([table]);
        for row_offset in 0..ROWS {
            let row_index = 1 + row_offset;
            let cell_index = 1 + ROWS + row_offset;
            let row = blocks[row_index].id;
            structural.insert(row);
            blocks[row_index].parent = Some(table);
            blocks[row_index].spec.flow = BlockFlow::TableRow;
            blocks[row_index].spec.space_after = 0.0;
            blocks[cell_index].parent = Some(row);
            blocks[cell_index].spec.flow = BlockFlow::TableCell { column: 0 };
            blocks[cell_index].spec.space_after = 0.0;
        }
        shaper
            .advance_override
            .insert(blocks[block_count - 1].id.fragment_ordinal, 120.0);
        document.blocks = blocks.into();
        document.text_runs = document
            .text_runs
            .iter()
            .filter(|run| !structural.contains(&run.id))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        (document, presentation, shaper)
    }

    pub fn paragraph() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[16.0], &[60], None)
    }

    pub fn styled_paragraph(
        text: &str,
        split: usize,
    ) -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        let source = SourceText::new(format!("# {text}")).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::new(18),
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let heading = syntax.queries().headings().next().unwrap().clone();
        let id = LayoutElementId {
            owner: heading.owner,
            fragment_ordinal: 0,
        };
        let content_start = heading.content_range.start().to_usize();
        let content_end = content_start + text.len();
        let base_metrics = TextMetrics {
            font: FontKey(8),
            font_size: 16.0,
            line_spacing: 0.0,
            weight: FontWeight(400),
            italic: false,
        };
        let mut runs = vec![LayoutTextRun {
            id,
            range: range(content_start, content_start + split),
            metrics: base_metrics,
            hidden: false,
        }];
        if split < text.len() {
            runs.push(LayoutTextRun {
                id,
                range: range(content_start + split, content_end),
                metrics: TextMetrics {
                    font: FontKey(13),
                    italic: true,
                    ..base_metrics
                },
                hidden: false,
            });
        }
        let presentation = Arc::new(MarkdownDocumentSnapshot::new(syntax));
        (
            LayoutDocument {
                revision: presentation.revision(),
                content_insets: EdgeInsets::default(),
                blocks: Arc::from([LayoutBlock {
                    id,
                    source_range: range(0, content_end),
                    parent: None,
                    spec: BlockLayoutSpec {
                        flow: BlockFlow::Paragraph,
                        insets: EdgeInsets::default(),
                        space_before: 0.0,
                        space_after: 0.0,
                        columns: Arc::from([]),
                    },
                }]),
                text_runs: runs.into(),
                embedded_blocks: Arc::from([]),
            },
            presentation,
            FakeShaper::default(),
        )
    }

    pub fn failing_second_block() -> (LayoutDocument, Arc<MarkdownDocumentSnapshot>, FakeShaper) {
        fixture(&[16.0, 16.0], &[8, 8], Some(1))
    }

    pub(super) fn fixture(
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
                hidden: false,
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
                advance_override: HashMap::new(),
                intrinsic_measured: 0,
                paragraph_requests: Vec::new(),
            },
        )
    }
}
