use std::sync::Arc;

use waml_markdown_editor::{
    layout::{BlockFlow, ColumnAlignment, LayoutDocument},
    presentation::{
        build_layout_document, compile_presentation, EmbeddedMeasurements, PresentationBlockKind,
        PresentationStyles,
    },
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSnapshot, SourceText,
};

fn document_for(source: &str) -> (Arc<MarkdownSyntaxSnapshot>, LayoutDocument) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).expect("valid source"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the source parses");
    let styles = PresentationStyles::balanced();
    let plan = compile_presentation(&snapshot, &styles).expect("the plan compiles");
    let document = build_layout_document(&plan, &styles, &EmbeddedMeasurements::default())
        .expect("the layout document builds");
    (snapshot, document)
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
    let plan = compile_presentation(&snapshot, &styles).unwrap();
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
    let hanging = document
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
    let plan = compile_presentation(&snapshot, &styles).unwrap();
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
        let mut clusters = Vec::new();
        for (ordinal, span) in request.spans.iter().enumerate() {
            clusters.push(ShapedCluster {
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
                ascender: 12.0,
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
