use std::sync::Arc;

use waml_markdown_editor::{
    layout::{BlockFlow, ColumnAlignment, LayoutDocument},
    presentation::{
        build_layout_document, compile_presentation, style::WEIGHT_SEMIBOLD, EditorEmphasis,
        EmbeddedMeasurements, HighlighterRegistry, PresentationBlockKind, PresentationPlan,
        PresentationStyles,
    },
};
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSnapshot,
    SourceText, TextChange, TextRange, TextSize,
};

fn document_for(source: &str) -> (Arc<MarkdownSyntaxSnapshot>, LayoutDocument) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).expect("valid source"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the source parses");
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&snapshot, &styles, &HighlighterRegistry::default())
        .expect("the plan compiles");
    let document = build_layout_document(&plan, &styles, &EmbeddedMeasurements::default())
        .expect("the layout document builds");
    (snapshot, document)
}

fn document_with_emphasis(source: &str, emphasis: EditorEmphasis) -> LayoutDocument {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).expect("valid source"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the source parses");
    let styles = PresentationStyles::for_emphasis(emphasis);
    let plan = compile_presentation(&snapshot, &styles, &HighlighterRegistry::default())
        .expect("the plan compiles");
    build_layout_document(&plan, &styles, &EmbeddedMeasurements::default())
        .expect("the layout document builds")
}

#[test]
fn emphasis_profiles_change_base_spacing_without_changing_document_insets() {
    let code = document_with_emphasis("Body\n", EditorEmphasis::Code);
    let layout = document_with_emphasis("Body\n", EditorEmphasis::Layout);
    let paragraph_after = |document: &LayoutDocument| {
        document
            .blocks
            .iter()
            .find(|block| block.parent.is_some() && matches!(block.spec.flow, BlockFlow::Paragraph))
            .expect("one paragraph block")
            .spec
            .space_after
    };

    assert_eq!(paragraph_after(&code), 0.0);
    assert_eq!(paragraph_after(&layout), 6.0);
    assert_eq!(code.content_insets.left, layout.content_insets.left);
    assert_eq!(code.content_insets.right, layout.content_insets.right);
}

#[test]
fn emphasis_profiles_keep_construct_insets() {
    let code = document_with_emphasis(
        "> quote\n\n```text\ncode\n```\n\n| left | right |\n| --- | --- |\n| a | b |\n",
        EditorEmphasis::Code,
    );
    let layout = document_with_emphasis(
        "> quote\n\n```text\ncode\n```\n\n| left | right |\n| --- | --- |\n| a | b |\n",
        EditorEmphasis::Layout,
    );
    let geometry = |document: &LayoutDocument| {
        document
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.spec.flow,
                    BlockFlow::Quote | BlockFlow::Code | BlockFlow::TableCell { .. }
                )
            })
            .map(|block| (block.spec.flow.clone(), block.spec.insets))
            .collect::<Vec<_>>()
    };

    assert_eq!(geometry(&code), geometry(&layout));
}

#[test]
fn code_profile_keeps_heading_size_and_strong_run_weight() {
    let source = "# Heading **strong**\n";
    let document = document_with_emphasis(source, EditorEmphasis::Code);
    let run_at = |offset: usize| {
        document
            .text_runs
            .iter()
            .find(|run| {
                run.range.start().to_usize() <= offset && offset < run.range.end().to_usize()
            })
            .expect("text offset has a layout run")
    };

    let heading = run_at(source.find("Heading").expect("heading text"));
    let strong = run_at(source.find("strong").expect("strong text"));
    assert!(heading.metrics.font_size > strong.metrics.font_size);
    assert_eq!(strong.metrics.weight, WEIGHT_SEMIBOLD);
}

fn document_from_snapshot(snapshot: &MarkdownSyntaxSnapshot) -> LayoutDocument {
    let plan = plan_from_snapshot(snapshot);
    let styles = PresentationStyles::balanced();
    build_layout_document(&plan, &styles, &EmbeddedMeasurements::default())
        .expect("the layout document builds")
}

fn plan_from_snapshot(snapshot: &MarkdownSyntaxSnapshot) -> Arc<PresentationPlan> {
    let styles = PresentationStyles::balanced();
    compile_presentation(snapshot, &styles, &HighlighterRegistry::default())
        .expect("the plan compiles")
}

fn flows(document: &LayoutDocument) -> Vec<BlockFlow> {
    document
        .blocks
        .iter()
        .map(|block| block.spec.flow.clone())
        .collect()
}

#[test]
fn the_layout_document_carries_the_plan_revision_and_document_inset() {
    let (_, document) = document_for("# Title\n\nBody\n");
    assert_eq!(document.revision, DocumentRevision::INITIAL);
    assert_eq!(document.content_insets.left, 24.0);
    assert_eq!(document.content_insets.top, 24.0);
    assert!(!document.text_runs.is_empty());
}

#[test]
fn measurements_from_another_revision_are_rejected() {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new("Body\n".to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&snapshot, &styles, &HighlighterRegistry::default()).unwrap();
    let stale = EmbeddedMeasurements {
        revision: Some(DocumentRevision::new(99)),
        blocks: Arc::from([]),
    };
    assert!(build_layout_document(&plan, &styles, &stale).is_err());
}

#[test]
fn headings_and_paragraphs_use_the_balanced_spacing_table() {
    let (_, document) = document_for("# Title\n\nBody\n");
    let spacing = PresentationStyles::balanced().spacing();
    let heading = document
        .blocks
        .iter()
        .find(|block| block.spec.space_before == spacing.heading_margins(1).0)
        .expect("the heading block carries its before margin");
    assert_eq!(heading.spec.space_after, spacing.heading_margins(1).1);
    assert!(document
        .blocks
        .iter()
        .any(|block| block.spec.space_after == spacing.paragraph_after));
}

#[test]
fn nested_lists_use_hanging_flow_with_a_parsed_marker_range() {
    let source = "- bullet\n  1. ordered\n";
    let (snapshot, document) = document_for(source);
    let mut hanging = document
        .blocks
        .iter()
        .filter_map(|block| match &block.spec.flow {
            BlockFlow::Hanging {
                marker_range,
                content_indent,
            } => Some((*marker_range, *content_indent)),
            _ => None,
        })
        .collect::<Vec<_>>();
    hanging.sort_by_key(|(range, _)| (range.start(), range.end()));
    hanging.dedup_by_key(|(range, _)| (range.start(), range.end()));
    assert_eq!(hanging.len(), 2, "one hanging block per list item");
    for (marker_range, content_indent) in hanging {
        let marker = snapshot.text().slice(marker_range).unwrap_or_default();
        assert!(marker == "-" || marker == "1.", "marker was {marker:?}");
        assert!(content_indent > PresentationStyles::balanced().spacing().list_marker_gap);
    }
}

#[test]
fn nested_quotes_parent_their_inner_blocks() {
    let (_, document) = document_for("> outer\n>\n> > inner\n");
    let quotes = document
        .blocks
        .iter()
        .filter(|block| matches!(block.spec.flow, BlockFlow::Quote))
        .collect::<Vec<_>>();
    assert!(quotes.len() >= 2, "{:?}", flows(&document));
    // The inner quote is parented, not a second root.
    assert!(quotes.iter().any(|block| block.parent.is_some()));
    assert!(quotes.iter().all(
        |block| block.spec.insets.left == PresentationStyles::balanced().spacing().quote_inset
    ));
}

#[test]
fn a_table_creates_parented_rows_and_aligned_columns() {
    let source = "| left | center | right |\n| :--- | :----: | ----: |\n| a | b | c |\n";
    let (_, document) = document_for(source);
    let table = document
        .blocks
        .iter()
        .find(|block| matches!(block.spec.flow, BlockFlow::Table))
        .expect("one table block");
    assert_eq!(table.spec.columns.len(), 3);
    assert_eq!(
        table
            .spec
            .columns
            .iter()
            .map(|column| column.alignment)
            .collect::<Vec<_>>(),
        vec![
            ColumnAlignment::Start,
            ColumnAlignment::Center,
            ColumnAlignment::End
        ]
    );
    let rows = document
        .blocks
        .iter()
        .filter(|block| matches!(block.spec.flow, BlockFlow::TableRow))
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|row| row.parent.is_some()));
    let cells = document
        .blocks
        .iter()
        .filter(|block| matches!(block.spec.flow, BlockFlow::TableCell { .. }))
        .collect::<Vec<_>>();
    assert!(cells.len() >= 3);
    assert!(cells.iter().all(|cell| cell.parent.is_some()));
    assert!(cells.iter().all(|cell| {
        cell.parent
            .and_then(|parent| document.blocks.iter().find(|block| block.id == parent))
            .is_some_and(|parent| matches!(parent.spec.flow, BlockFlow::TableRow))
    }));
}

#[test]
fn fenced_code_uses_code_flow_with_padding() {
    let (_, document) = document_for("```waml\ntype: uml.class\n```\n");
    let code = document
        .blocks
        .iter()
        .find(|block| matches!(block.spec.flow, BlockFlow::Code))
        .expect("one code block");
    let padding = PresentationStyles::balanced().spacing().code_padding;
    assert_eq!(code.spec.insets.left, padding);
    assert_eq!(code.spec.insets.top, padding);
}

#[test]
fn an_image_keeps_its_literal_source_line_and_measured_embed() {
    let source = "![checker](checker.svg)\n";
    let (snapshot, document) = document_for(source);
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&snapshot, &styles, &HighlighterRegistry::default()).unwrap();
    // The literal source stays in text runs.
    let covered = plan
        .items
        .iter()
        .filter_map(|item| match item {
            waml_markdown_editor::presentation::PresentationItem::TextRun { range, .. } => {
                Some(range.end().to_usize() - range.start().to_usize())
            }
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(covered, source.len());
    assert!(plan
        .blocks
        .iter()
        .any(|block| block.kind == PresentationBlockKind::Image));
    assert!(!document.text_runs.is_empty());
}

#[test]
fn measured_image_block_starts_below_its_literal_source_geometry() {
    use makepad_widgets::dvec2;
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{
            LayoutElementId, LayoutEngine, LayoutInvalidation, LayoutViewport, MeasuredBlock,
        },
        presentation::PresentationItem,
    };
    let source = "![checker](checker.svg)\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&snapshot, &styles, &HighlighterRegistry::default()).unwrap();
    let (item, source_range) = plan
        .items
        .iter()
        .find_map(|item| match item {
            PresentationItem::EmbeddedBlock {
                id, source_range, ..
            } => Some((*id, *source_range)),
            _ => None,
        })
        .expect("the image has an embedded presentation item");
    let embedded_id = LayoutElementId {
        owner: item.owner,
        fragment_ordinal: item.fragment_ordinal,
    };
    let measurements = EmbeddedMeasurements {
        revision: Some(DocumentRevision::INITIAL),
        blocks: Arc::from([MeasuredBlock {
            id: embedded_id,
            source_range,
            size: dvec2(96.0, 48.0),
            baseline: None,
        }]),
    };
    let document = build_layout_document(&plan, &styles, &measurements).unwrap();
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .unwrap();
    let literal_bottom = layout
        .glyph_clusters()
        .iter()
        .filter(|cluster| {
            source_range.start() <= cluster.source_range.start()
                && cluster.source_range.end() <= source_range.end()
        })
        .map(|cluster| cluster.rect.pos.y + cluster.rect.size.y)
        .reduce(f64::max)
        .expect("the literal image source has geometry");
    let embedded = layout
        .visible_blocks()
        .iter()
        .find(|block| block.id == embedded_id)
        .expect("the measured image has block geometry");
    assert!(
        embedded.rect.pos.y >= literal_bottom,
        "literal_bottom={literal_bottom:?}, embedded={:?}, blocks={:?}",
        embedded.rect,
        layout.visible_blocks()
    );
}

#[test]
fn every_text_run_belongs_to_a_declared_block() {
    let source = std::fs::read_to_string("tests/fixtures/presentation-all.md").unwrap();
    let (_, document) = document_for(&source);
    for run in document.text_runs.iter() {
        assert!(
            document.blocks.iter().any(|block| block.id == run.id),
            "run {:?} has no block",
            run.range
        );
    }
}

#[test]
fn parent_owned_text_is_fragmented_between_its_child_blocks() {
    let presentation_all = std::fs::read_to_string("tests/fixtures/presentation-all.md").unwrap();
    for source in [presentation_all.as_str(), "> outer\n>\n> > inner\n"] {
        let (_, document) = document_for(source);
        for run in document.text_runs.iter() {
            assert!(
                !document
                    .blocks
                    .iter()
                    .any(|block| block.parent == Some(run.id)),
                "run {:?} is laid before child blocks of {:?}",
                run.range,
                run.id
            );
        }
    }
}

#[test]
fn inserting_an_early_quote_fragment_preserves_the_later_fragment_identity() {
    let before_source = "> > nested zero\n>\n> > nested one\n>\n> > nested two\n";
    let insertion = "> inserted\n>\n";
    let insertion_offset = before_source.find("> > nested one").unwrap();
    let mut after_source = before_source.to_owned();
    after_source.insert_str(insertion_offset, insertion);
    let (before_snapshot, before_document) = document_for(before_source);
    let before_fragment = before_document
        .blocks
        .iter()
        .filter(|block| {
            block.parent.is_some()
                && block.id.fragment_ordinal != 0
                && block.id.fragment_ordinal != u32::MAX
        })
        .max_by_key(|block| block.source_range.start())
        .expect("the nested quote has a later parent-owned fragment");
    let before_anchor = before_document
        .blocks
        .iter()
        .filter(|block| block.parent == before_fragment.parent)
        .filter(|block| block.source_range.start() >= before_fragment.source_range.end())
        .min_by_key(|block| block.source_range.start())
        .expect("the fragment is anchored before an unchanged child block")
        .id;
    assert!(
        before_fragment.source_range.start().to_usize() >= insertion_offset,
        "the selected survivor must follow the insertion"
    );
    let shift = TextSize::try_from_usize(insertion.len()).unwrap();
    let shifted_range = TextRange::new(
        (before_fragment.source_range.start() + shift).unwrap(),
        (before_fragment.source_range.end() + shift).unwrap(),
    )
    .unwrap();

    let update = reparse_markdown(
        &before_snapshot,
        DocumentRevision::new(2),
        SourceText::new(after_source).unwrap(),
        &[TextChange {
            old_range: TextRange::new(
                TextSize::try_from_usize(insertion_offset).unwrap(),
                TextSize::try_from_usize(insertion_offset).unwrap(),
            )
            .unwrap(),
            replacement: insertion.into(),
        }],
    )
    .expect("the quote insertion reparses");
    let after_document = document_from_snapshot(&update.snapshot);
    let after_fragment = after_document
        .blocks
        .iter()
        .find(|block| block.source_range == shifted_range)
        .expect("the unchanged later parent-owned fragment survives at its shifted range");
    let after_anchor = after_document
        .blocks
        .iter()
        .filter(|block| block.parent == after_fragment.parent)
        .filter(|block| block.source_range.start() >= after_fragment.source_range.end())
        .min_by_key(|block| block.source_range.start())
        .expect("the shifted fragment remains before the same child block")
        .id;

    assert_eq!(
        after_anchor, before_anchor,
        "the child syntax anchor must survive"
    );
    assert_eq!(after_fragment.id, before_fragment.id);
}

/// A minimal shaper: the layout engine only has to accept the document's
/// hierarchy and runs, so one cluster per run is enough.
struct CountingShaper;

impl waml_markdown_editor::layout::TextShaper for CountingShaper {
    fn shape_paragraph(
        &mut self,
        request: waml_markdown_editor::layout::ParagraphShapeRequest<'_>,
    ) -> Result<
        waml_markdown_editor::layout::ShapedParagraph,
        waml_markdown_editor::layout::LayoutError,
    > {
        use waml_markdown_editor::layout::{
            ShapedCluster, ShapedFragment, ShapedParagraph, ShapedRow,
        };
        let covered_bytes = request
            .spans
            .iter()
            .map(|span| span.source_range.end().to_usize() - span.source_range.start().to_usize())
            .sum::<usize>();
        let envelope_bytes =
            request.paragraph_range.end().to_usize() - request.paragraph_range.start().to_usize();
        // Model a renderer that scans the paragraph envelope for line breaks.
        // A discontiguous set of spans must not create a large hidden envelope.
        let envelope_gap = envelope_bytes.saturating_sub(covered_bytes) as f64;
        let mut clusters = Vec::new();
        for (ordinal, span) in request.spans.iter().enumerate() {
            clusters.push(ShapedCluster {
                hidden: false,
                id: waml_markdown_editor::layout::GeometryElementId {
                    layout: request.paragraph_id.layout,
                    cluster_ordinal: 0x8000_0000 | ordinal as u32,
                },
                span_id: span.id,
                source_range: span.source_range,
                metrics: span.metrics,
                advance: 8.0,
                bidi_level: 0,
                row_ordinal: 0,
                row_top: 0.0,
                caret_offsets: Arc::from([span.source_range.start(), span.source_range.end()]),
                glyphs: Arc::from([]),
            });
        }
        let rows = if clusters.is_empty() {
            Vec::new()
        } else {
            vec![ShapedRow {
                id: waml_markdown_editor::layout::GeometryElementId {
                    layout: request.paragraph_id.layout,
                    cluster_ordinal: 0xc000_0000,
                },
                source_range: request.paragraph_range,
                cluster_range: 0..clusters.len(),
                caret_offsets: Arc::from([
                    request.paragraph_range.start(),
                    request.paragraph_range.end(),
                ]),
                ascender: 12.0 + envelope_gap,
                descender: 3.0,
                line_gap: 0.0,
                line_spacing_scale: 1.0,
                row_top: 0.0,
            }]
        };
        Ok(ShapedParagraph {
            rows: rows.into(),
            fragments: request
                .spans
                .iter()
                .map(|span| ShapedFragment {
                    id: span.id,
                    span_id: span.id,
                    stable_ordinal: span.stable_ordinal,
                    source_range: span.source_range,
                    metrics: span.metrics,
                })
                .collect::<Vec<_>>()
                .into(),
            clusters: clusters.into(),
            bidi_levels: Arc::from([]),
            legal_breaks: Arc::from([request.paragraph_range.end()]),
        })
    }

    fn measure_paragraph_intrinsic(
        &mut self,
        request: waml_markdown_editor::layout::ParagraphIntrinsicRequest<'_>,
    ) -> Result<
        waml_markdown_editor::layout::ParagraphIntrinsic,
        waml_markdown_editor::layout::LayoutError,
    > {
        let width = request.spans.len() as f64 * 8.0;
        Ok(waml_markdown_editor::layout::ParagraphIntrinsic {
            min_content: width,
            max_content: width,
        })
    }
}

#[test]
fn the_foundation_engine_accepts_the_built_document() {
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{LayoutEngine, LayoutInvalidation, LayoutViewport},
    };
    let source = std::fs::read_to_string("tests/fixtures/presentation-all.md").unwrap();
    let (snapshot, document) = document_for(&source);
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot.clone()));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(800.0, 600.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .expect("the engine accepts the presentation-built hierarchy");
    assert!(layout.content_size().y > 0.0);
    assert!(!layout.visible_blocks().is_empty());
}

#[test]
fn the_first_heading_starts_near_the_document_top_inset() {
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{LayoutEngine, LayoutInvalidation, LayoutViewport},
    };
    let source = std::fs::read_to_string("tests/fixtures/presentation-all.md").unwrap();
    let (snapshot, document) = document_for(&source);
    let top_inset = document.content_insets.top;
    let root = document
        .blocks
        .iter()
        .find(|block| block.parent.is_none())
        .expect("the layout document has a structural root");
    assert!(
        document.text_runs.iter().all(|run| run.id != root.id),
        "the structural root must not shape source-wide gap runs before its children"
    );
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(1280.0, 871.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .expect("the presentation fixture lays out");
    let first_heading_y = layout
        .glyph_clusters()
        .iter()
        .filter(|cluster| cluster.source_range.start().to_usize() < 30)
        .map(|cluster| cluster.rect.pos.y)
        .reduce(f64::min)
        .expect("the first heading has visible text geometry");
    assert!(
        first_heading_y < top_inset + 100.0,
        "first heading y={first_heading_y}, top inset={top_inset}"
    );
}

#[test]
fn presentation_nested_list_marker_is_indented_from_outer_marker() {
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{LayoutEngine, LayoutInvalidation, LayoutViewport},
    };
    let source = "- bullet\n  1. ordered\n";
    let (snapshot, document) = document_for(source);
    let mut marker_ranges = document
        .blocks
        .iter()
        .filter_map(|block| match block.spec.flow {
            BlockFlow::Hanging { marker_range, .. } => Some(marker_range),
            _ => None,
        })
        .collect::<Vec<_>>();
    marker_ranges.sort_by_key(|range| (range.start(), range.end()));
    marker_ranges.dedup();
    assert_eq!(marker_ranges.len(), 2);
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .unwrap();
    let marker_x = |marker: TextRange| {
        layout
            .glyph_clusters()
            .iter()
            .find(|cluster| {
                cluster.source_range.start() <= marker.start()
                    && marker.end() <= cluster.source_range.end()
            })
            .expect("marker has glyph geometry")
            .rect
            .pos
            .x
    };
    assert!(marker_x(marker_ranges[1]) > marker_x(marker_ranges[0]));
}

#[test]
fn quote_marker_and_first_content_share_one_source_row() {
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{LayoutEngine, LayoutInvalidation, LayoutViewport},
    };
    let source = "> quoted **text**\n";
    let (snapshot, mut document) = document_for(source);
    let mut runs = document.text_runs.to_vec();
    runs.iter_mut()
        .find(|run| run.range.start().to_usize() == 0)
        .expect("quote marker has a layout run")
        .metrics
        .italic = true;
    document.text_runs = runs.into();
    let mut ordered_ranges = document
        .text_runs
        .iter()
        .map(|run| run.range)
        .collect::<Vec<_>>();
    ordered_ranges.sort_by_key(|range| (range.start(), range.end()));
    let reconstructed = ordered_ranges
        .iter()
        .map(|range| snapshot.text().slice(*range).unwrap())
        .collect::<String>();
    assert_eq!(reconstructed, source);
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .unwrap();
    let cluster_at = |offset: usize| {
        layout
            .glyph_clusters()
            .iter()
            .find(|cluster| {
                cluster.source_range.start().to_usize() <= offset
                    && offset < cluster.source_range.end().to_usize()
            })
            .expect("source offset has glyph geometry")
    };
    let marker = cluster_at(0);
    let content = cluster_at(2);
    assert_eq!(marker.rect.pos.y, content.rect.pos.y);
    assert!(marker.rect.pos.x < content.rect.pos.x);
}

#[test]
fn table_header_cells_share_a_row_with_increasing_column_origins() {
    use waml_markdown_editor::{
        document::MarkdownDocumentSnapshot,
        layout::{LayoutEngine, LayoutInvalidation, LayoutViewport},
    };
    let source = "| left | center | right |\n| :--- | :----: | ----: |\n| a | b | c |\n";
    let (snapshot, document) = document_for(source);
    let presentation = Arc::new(MarkdownDocumentSnapshot::new(snapshot));
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(800.0, 400.0, 0.0, 0.0),
            LayoutInvalidation::Document,
            &mut CountingShaper,
        )
        .unwrap();
    let cluster_for = |needle: &str| {
        let offset = source.find(needle).unwrap();
        layout
            .glyph_clusters()
            .iter()
            .find(|cluster| {
                cluster.source_range.start().to_usize() <= offset
                    && offset < cluster.source_range.end().to_usize()
            })
            .expect("header text has glyph geometry")
    };
    let left = cluster_for("left");
    let center = cluster_for("center");
    let right = cluster_for("right");
    assert_eq!(left.rect.pos.y, center.rect.pos.y);
    assert_eq!(center.rect.pos.y, right.rect.pos.y);
    assert!(left.rect.pos.x < center.rect.pos.x);
    assert!(center.rect.pos.x < right.rect.pos.x);
}

#[test]
fn each_blank_source_line_gets_its_own_gap_fragment() {
    // Merging a multi-line gap into one fragment gives two blank lines a single
    // visual row: the second line loses its height and its gutter number.
    let (_, document) = document_for("# Head\n\npara one\n\n\npara two\n");
    let gaps = document
        .blocks
        .iter()
        .filter(|block| block.source_range.start().to_usize() >= 17)
        .filter(|block| block.source_range.end().to_usize() <= 19)
        .map(|block| {
            (
                block.source_range.start().to_usize(),
                block.source_range.end().to_usize(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(gaps, vec![(17, 18), (18, 19)]);
}
