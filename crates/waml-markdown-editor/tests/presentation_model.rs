use std::{ops::Range, sync::Arc};

use waml_markdown_editor::presentation::{
    compile_presentation, BlockDecorationKind, ColorRole, EmbeddedBlockKind, FontRole,
    FontSizeRole, FontWeightRole, HighlighterRegistry, PresentationBlockKind, PresentationError,
    PresentationItem, PresentationItemId, PresentationPlan, PresentationRole, PresentationStyles,
    TextRole, TextStyle,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxIdentity, TextRange,
    TextSize,
};

fn owner(value: u64) -> SyntaxIdentity {
    SyntaxIdentity::from_raw_for_test(value)
}

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("a test offset fits")
}

fn range(bounds: Range<usize>) -> TextRange {
    TextRange::new(t(bounds.start), t(bounds.end)).expect("a test range is ordered")
}

fn style() -> TextStyle {
    TextStyle {
        font: FontRole::Body,
        size: FontSizeRole::Body,
        weight: FontWeightRole::Regular,
        italic: false,
        color: ColorRole::Text,
        active_color: ColorRole::Text,
        background: None,
        underline: false,
        strikethrough: false,
    }
}

fn run(
    bounds: Range<usize>,
    role: TextRole,
    owner: SyntaxIdentity,
    fragment_ordinal: u32,
) -> PresentationItem {
    PresentationItem::TextRun {
        id: PresentationItemId {
            owner,
            role: PresentationRole::Text(role),
            fragment_ordinal,
        },
        range: range(bounds),
        role,
        style: style(),
        hidden: false,
    }
}

fn plan_for_source(
    source: &str,
    items: impl IntoIterator<Item = PresentationItem>,
) -> PresentationPlan {
    PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: items.into_iter().collect::<Vec<_>>().into(),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    }
}

fn plan_with_ranges(
    source: &str,
    ranges: impl IntoIterator<Item = Range<usize>>,
) -> PresentationPlan {
    plan_for_source(
        source,
        ranges
            .into_iter()
            .enumerate()
            .map(|(ordinal, bounds)| run(bounds, TextRole::Body, owner(1), ordinal as u32))
            .collect::<Vec<_>>(),
    )
}

fn single_range_plan(source: &str, bounds: Range<usize>) -> PresentationPlan {
    plan_for_source(source, [run(bounds, TextRole::Body, owner(1), 0)])
}

fn plan_with_duplicate_ids(source: &str) -> PresentationPlan {
    plan_for_source(
        source,
        [
            run(0..2, TextRole::Body, owner(1), 0),
            run(2..source.len(), TextRole::Body, owner(1), 0),
        ],
    )
}

#[test]
fn text_runs_partition_every_source_byte_once() {
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..2, TextRole::SyntaxMarker, owner(1), 0),
            run(2..3, TextRole::Strong, owner(1), 1),
            run(3..5, TextRole::SyntaxMarker, owner(1), 2),
            run(5..6, TextRole::LineBreak, owner(2), 0),
        ],
    );
    assert_eq!(plan.validate_source_partition(), Ok(()));
}

#[test]
fn partition_rejects_gap_overlap_duplicate_and_out_of_bounds() {
    assert!(matches!(
        plan_with_ranges("abcd", [0..1, 2..4]).validate_source_partition(),
        Err(PresentationError::Gap { .. })
    ));
    assert!(matches!(
        plan_with_ranges("abcd", [0..3, 2..4]).validate_source_partition(),
        Err(PresentationError::Overlap { .. })
    ));
    assert!(matches!(
        single_range_plan("abcd", 0..5).validate_source_partition(),
        Err(PresentationError::OutOfBounds { .. })
    ));
    assert!(matches!(
        plan_with_duplicate_ids("abcd").validate_source_partition(),
        Err(PresentationError::DuplicateId(_))
    ));
}

#[test]
fn a_short_final_run_reports_the_exact_trailing_gap() {
    let plan = single_range_plan("abcd", 0..2);
    assert_eq!(
        plan.validate_source_partition(),
        Err(PresentationError::Gap {
            expected: t(2),
            actual: t(4),
        })
    );
}

#[test]
fn decorations_and_embedded_blocks_stay_out_of_the_text_partition() {
    // Both overlap the owner's source range and neither contributes a byte to
    // the text partition.
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..6, TextRole::Body, owner(1), 0),
            PresentationItem::BlockDecoration {
                id: PresentationItemId {
                    owner: owner(1),
                    role: PresentationRole::Block(
                        waml_markdown_editor::presentation::BlockDecorationRole::InlineCodeFill,
                    ),
                    fragment_ordinal: 0,
                },
                owner: owner(1),
                source_range: range(0..6),
                kind: BlockDecorationKind::InlineCodeFill,
            },
            PresentationItem::EmbeddedBlock {
                id: PresentationItemId {
                    owner: owner(1),
                    role: PresentationRole::Embedded(
                        waml_markdown_editor::presentation::EmbeddedBlockRole::Image,
                    ),
                    fragment_ordinal: 0,
                },
                owner: owner(1),
                source_range: range(2..3),
                kind: EmbeddedBlockKind::Image {
                    destination: Arc::from("a.png"),
                    alt: Arc::from("a"),
                    title: None,
                },
            },
        ],
    );
    assert_eq!(plan.validate_source_partition(), Ok(()));
}

#[test]
fn active_owners_report_every_owner_touching_the_caret() {
    let plan = plan_for_source(
        "**a**\n",
        [
            run(0..2, TextRole::SyntaxMarker, owner(1), 0),
            run(2..3, TextRole::Strong, owner(7), 0),
            run(3..5, TextRole::SyntaxMarker, owner(1), 1),
            run(5..6, TextRole::LineBreak, owner(2), 0),
        ],
    );
    assert_eq!(plan.active_owners(t(2)).as_ref(), &[owner(1), owner(7)]);
    assert_eq!(plan.active_owners(t(6)).as_ref(), &[owner(2)]);
}

#[test]
fn fenced_code_preserves_language_and_content_range() {
    let source = "```MeRmAiD\nflowchart TD\nA-->B\n```";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).expect("the test source is valid"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the test source parses");
    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("the parsed document compiles");

    let code = plan
        .blocks
        .iter()
        .find(|block| matches!(block.kind, PresentationBlockKind::Code { .. }))
        .expect("the document has one code block");
    let PresentationBlockKind::Code { fence: Some(fence) } = &code.kind else {
        panic!("fenced metadata")
    };
    assert_eq!(code.source_range, range(0..source.len()));
    assert_eq!(fence.language.as_deref(), Some("MeRmAiD"));
    assert_eq!(
        &source[fence.content_range.start().to_usize()..fence.content_range.end().to_usize()],
        "flowchart TD\nA-->B\n"
    );
    plan.validate_source_partition().unwrap();
}

#[test]
fn indented_code_has_no_fence_metadata() {
    let source = "    plain indented code\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).expect("the test source is valid"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the test source parses");
    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("the parsed document compiles");

    let code = plan
        .blocks
        .iter()
        .find(|block| matches!(block.kind, PresentationBlockKind::Code { .. }))
        .expect("the document has one code block");
    assert_eq!(code.source_range, range(0..source.len()));
    assert!(matches!(
        code.kind,
        PresentationBlockKind::Code { fence: None }
    ));
    plan.validate_source_partition().unwrap();
}

mod diagnostic_message_emission {
    use std::sync::Arc;

    use makepad_widgets::{dvec2, Rect};
    use waml_markdown_editor::{
        layout::{CaretStop, GlyphCluster, LayoutSnapshot, VisualLine},
        presentation::{
            draw::{
                build_draw_commands, DrawCommand, PresentationFrame, PresentedDiagnostic,
                PresentedDiagnosticSeverity, MESSAGE_GAP,
            },
            EmbeddedAssetFrame, PresentationPlan, PresentationStyles, TextRole,
        },
        selection::{Affinity, Selection, SelectionSet, TextPosition},
    };
    use waml_syntax::{DocumentRevision, SourceText};

    use super::{owner, range, t};

    fn snapshot(viewport_width: f64, cluster_x: f64, cluster_width: f64) -> Arc<LayoutSnapshot> {
        let source_range = range(0..4);
        let cluster = GlyphCluster::for_test(
            source_range,
            Rect {
                pos: dvec2(cluster_x, 20.0),
                size: dvec2(cluster_width, 18.0),
            },
            vec![
                CaretStop::new(
                    TextPosition::new(t(0), Affinity::Before),
                    dvec2(cluster_x, 20.0),
                ),
                CaretStop::new(
                    TextPosition::new(t(4), Affinity::Before),
                    dvec2(cluster_x + cluster_width, 20.0),
                ),
            ],
        );
        Arc::new(LayoutSnapshot::from_parts_for_test(
            DocumentRevision::INITIAL,
            dvec2(viewport_width, 60.0),
            vec![VisualLine::for_test(source_range, 20.0, 18.0)],
            vec![cluster],
            Vec::new(),
        ))
    }

    fn messages(layout: Arc<LayoutSnapshot>, message: &str) -> Vec<DrawCommand> {
        let source = "abcd";
        let plan = PresentationPlan {
            revision: DocumentRevision::INITIAL,
            source_len: t(source.len()),
            items: Arc::from([super::run(0..source.len(), TextRole::Body, owner(1), 0)]),
            links: Arc::from([]),
            blocks: Arc::from([]),
            diagnostics: Arc::from([]),
        };
        let frame = PresentationFrame {
            revision: DocumentRevision::INITIAL,
            layout,
            active_owners: Arc::from([]),
            diagnostics: Arc::from([PresentedDiagnostic {
                revision: DocumentRevision::INITIAL,
                range: range(1..3),
                severity: PresentedDiagnosticSeverity::Error,
                message: Arc::from(message),
            }]),
            assets: Arc::new(EmbeddedAssetFrame {
                revision: DocumentRevision::INITIAL,
                items: Arc::from([]),
            }),
            search_highlights: Arc::from([]),
        };
        let selections = SelectionSet::from_source(
            DocumentRevision::INITIAL,
            &SourceText::new(source.to_owned()).unwrap(),
            vec![Selection::new(
                TextPosition::new(t(0), Affinity::Before),
                TextPosition::new(t(0), Affinity::Before),
            )],
            0,
        )
        .unwrap();
        build_draw_commands(
            &frame,
            &plan,
            &PresentationStyles::balanced(),
            &selections,
            None,
        )
        .unwrap()
        .iter()
        .filter(|command| matches!(command, DrawCommand::DiagnosticMessage { .. }))
        .cloned()
        .collect()
    }

    #[test]
    fn placement_sits_message_gap_past_the_last_cluster_right_edge() {
        let commands = messages(snapshot(600.0, 10.0, 40.0), "boom");
        let DrawCommand::DiagnosticMessage { rect, text, .. } = &commands[0] else {
            unreachable!()
        };
        assert_eq!(rect.pos.x, 10.0 + 40.0 + MESSAGE_GAP);
        assert_eq!(text.as_ref(), "boom");
        let advance = PresentationStyles::balanced().diagnostic_message_advance();
        assert_eq!(rect.size.x, 4.0 * advance);
    }

    #[test]
    fn ellipsize_fires_exactly_at_the_viewport_width_boundary() {
        let advance = PresentationStyles::balanced().diagnostic_message_advance();
        let x = 10.0 + 40.0 + MESSAGE_GAP;
        // Budget for exactly 10 characters past the row text.
        let viewport = x + 10.0 * advance;
        // 10 chars fit untouched.
        let fits = messages(snapshot(viewport, 10.0, 40.0), "0123456789");
        let DrawCommand::DiagnosticMessage { text, .. } = &fits[0] else {
            unreachable!()
        };
        assert_eq!(text.as_ref(), "0123456789");
        // 11 chars ellipsize to 9 + '…' (10 glyphs total).
        let clipped = messages(snapshot(viewport, 10.0, 40.0), "0123456789A");
        let DrawCommand::DiagnosticMessage { text, .. } = &clipped[0] else {
            unreachable!()
        };
        assert_eq!(text.as_ref(), "012345678…");
    }

    #[test]
    fn a_row_with_no_room_left_emits_no_message_at_all() {
        // Viewport ends before even one character fits past the gap.
        let commands = messages(
            snapshot(10.0 + 40.0 + MESSAGE_GAP + 0.5, 10.0, 40.0),
            "boom",
        );
        assert!(
            commands.is_empty(),
            "no wrapping, no row growth, no hard clip"
        );
    }
}
