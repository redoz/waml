use std::sync::Arc;

use makepad_widgets::{dvec2, Rect, ScriptValue, ScriptVm};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::{
        BlockGeometry, CaretStop, EdgeInsets, FontKey, FontWeight, GeometryElementId, GlyphCluster,
        LayoutDocument, LayoutElementId, LayoutSnapshot, TextMetrics, VisualLine,
    },
    motion::{LayoutChangeCause, MotionConfig, MotionController},
    presentation::{
        draw::{
            build_draw_commands, DecorationRole, DrawCommand, InstalledPresentation,
            PresentationFrame, PresentedDiagnostic, PresentedDiagnosticSeverity,
        },
        BlockDecorationKind, BlockDecorationRole, ColorRole, EmbeddedAssetFrame, EmbeddedBlockKind,
        EmbeddedBlockRole, EmbeddedState, PresentationError, PresentationItem, PresentationItemId,
        PresentationPlan, PresentationRole, PresentationStyles, PresentedLink, TextRole,
    },
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
    widget::{
        build_text_paint_operations, derive_motion_scroll_anchor, navigation_position, DrawLayer,
        TextFace, TextPaintOperation,
    },
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxIdentity, TextChange,
    TextRange, TextSize,
};

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

fn position(offset: usize) -> TextPosition {
    TextPosition::new(t(offset), Affinity::Before)
}

fn command_rect_positions(command: &DrawCommand) -> Vec<makepad_widgets::DVec2> {
    match command {
        DrawCommand::BlockBackground { rect, .. }
        | DrawCommand::Selection { rect }
        | DrawCommand::Text { rect, .. }
        | DrawCommand::EmbeddedBlock { rect, .. } => vec![rect.pos],
        DrawCommand::Decoration { rects, .. } => rects.iter().map(|rect| rect.pos).collect(),
        DrawCommand::CaretAndIme { caret, composition } => std::iter::once(caret.pos)
            .chain(composition.iter().map(|rect| rect.pos))
            .collect(),
        DrawCommand::DiagnosticMessage { rect, .. } => vec![rect.pos],
    }
}

fn owner(value: u64) -> SyntaxIdentity {
    SyntaxIdentity::from_raw_for_test(value)
}

fn item_id(owner: u64, role: PresentationRole, fragment_ordinal: u32) -> PresentationItemId {
    PresentationItemId {
        owner: self::owner(owner),
        role,
        fragment_ordinal,
    }
}

fn layout_id(owner: u64, fragment_ordinal: u32) -> LayoutElementId {
    LayoutElementId {
        owner: self::owner(owner),
        fragment_ordinal,
    }
}

fn text_item(
    owner: u64,
    ordinal: u32,
    bounds: std::ops::Range<usize>,
    role: TextRole,
) -> PresentationItem {
    PresentationItem::TextRun {
        id: item_id(owner, PresentationRole::Text(role), ordinal),
        range: range(bounds.start, bounds.end),
        role,
        style: PresentationStyles::balanced().text_style(role),
        hidden: false,
    }
}

fn cluster(
    owner: u64,
    ordinal: u32,
    bounds: std::ops::Range<usize>,
    x: f64,
    role: TextRole,
) -> GlyphCluster {
    let metrics = PresentationStyles::balanced().metrics(role);
    let start = bounds.start;
    let end = bounds.end;
    GlyphCluster::with_metrics(
        GeometryElementId {
            layout: layout_id(owner, ordinal),
            cluster_ordinal: ordinal,
        },
        range(start, end),
        Rect {
            pos: dvec2(x, 20.0),
            size: dvec2((end - start) as f64 * 10.0, 18.0),
        },
        (start..=end)
            .map(|offset| {
                CaretStop::new(
                    position(offset),
                    dvec2(x + (offset - start) as f64 * 10.0, 20.0),
                )
            })
            .collect::<Vec<_>>()
            .into(),
        metrics,
    )
}

fn snapshot(
    source_len: usize,
    clusters: Vec<GlyphCluster>,
    blocks: Vec<BlockGeometry>,
) -> Arc<LayoutSnapshot> {
    Arc::new(LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(600.0, 200.0),
        vec![VisualLine::for_test(range(0, source_len), 20.0, 18.0)],
        clusters,
        blocks,
    ))
}

fn selection(source: &str, anchor: usize, cursor: usize) -> SelectionSet {
    SelectionSet::from_source(
        DocumentRevision::INITIAL,
        &SourceText::new(source.to_owned()).unwrap(),
        vec![Selection::new(position(anchor), position(cursor))],
        0,
    )
    .unwrap()
}

fn empty_layout_document(revision: DocumentRevision) -> Arc<LayoutDocument> {
    Arc::new(LayoutDocument {
        revision,
        content_insets: EdgeInsets::default(),
        blocks: Arc::from([]),
        text_runs: Arc::from([]),
        embedded_blocks: Arc::from([]),
    })
}

fn plan_with_all_layers() -> (
    PresentationPlan,
    PresentationFrame,
    PresentationStyles,
    SelectionSet,
) {
    let source = "text";
    let block_id = item_id(
        2,
        PresentationRole::Block(BlockDecorationRole::InlineCodeFill),
        0,
    );
    let image_id = item_id(3, PresentationRole::Embedded(EmbeddedBlockRole::Image), 0);
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            text_item(1, 0, 0..source.len(), TextRole::LinkLabel),
            PresentationItem::BlockDecoration {
                id: block_id,
                owner: owner(2),
                source_range: range(0, source.len()),
                kind: BlockDecorationKind::InlineCodeFill,
            },
            PresentationItem::EmbeddedBlock {
                id: image_id,
                owner: owner(3),
                source_range: range(0, source.len()),
                kind: EmbeddedBlockKind::Image {
                    destination: Arc::from("image.png"),
                    alt: Arc::from("image"),
                    title: None,
                },
            },
        ]),
        links: Arc::from([PresentedLink {
            owner: owner(1),
            source_range: range(0, source.len()),
            destination: Arc::from("target"),
            title: None,
        }]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let block_rect = Rect {
        pos: dvec2(8.0, 16.0),
        size: dvec2(80.0, 26.0),
    };
    let image_rect = Rect {
        pos: dvec2(100.0, 20.0),
        size: dvec2(40.0, 18.0),
    };
    let layout = snapshot(
        source.len(),
        vec![cluster(1, 0, 0..source.len(), 10.0, TextRole::LinkLabel)],
        vec![
            BlockGeometry::new(layout_id(2, 0), range(0, source.len()), block_rect),
            BlockGeometry::new(layout_id(3, 0), range(0, source.len()), image_rect),
        ],
    );
    let assets = Arc::new(EmbeddedAssetFrame {
        revision: DocumentRevision::INITIAL,
        items: Arc::from([(image_id, EmbeddedState::Loading)]),
    });
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([]),
        diagnostics: Arc::from([]),
        assets,
    };
    (
        plan,
        frame,
        PresentationStyles::balanced(),
        selection(source, 0, source.len()),
    )
}

#[test]
fn commands_follow_the_six_foundation_layers_exactly() {
    let (plan, frame, styles, selection) = plan_with_all_layers();
    let commands = build_draw_commands(&frame, &plan, &styles, &selection, None).unwrap();

    assert_eq!(
        commands.iter().map(DrawCommand::layer).collect::<Vec<_>>(),
        vec![
            DrawLayer::BlockBackground,
            DrawLayer::Selection,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::EmbeddedBlock,
            DrawLayer::CaretAndIme,
        ],
    );
}

#[test]
fn mounted_draw_origin_translates_each_foundation_layer_once() {
    let origin = dvec2(280.0, 40.0);
    let metrics = PresentationStyles::balanced().metrics(TextRole::Body);
    let style = waml_markdown_editor::presentation::ResolvedTextStyle {
        metrics,
        color: ColorRole::Text,
        background: None,
        underline: false,
        strikethrough: false,
    };
    let commands = [
        DrawCommand::BlockBackground {
            id: layout_id(1, 0),
            rect: Rect {
                pos: dvec2(1.0, 2.0),
                size: dvec2(30.0, 10.0),
            },
            role: BlockDecorationRole::InlineCodeFill,
        },
        DrawCommand::Selection {
            rect: Rect {
                pos: dvec2(3.0, 4.0),
                size: dvec2(20.0, 10.0),
            },
        },
        DrawCommand::Text {
            id: GeometryElementId {
                layout: layout_id(2, 0),
                cluster_ordinal: 0,
            },
            range: range(0, 4),
            rect: Rect {
                pos: dvec2(5.0, 6.0),
                size: dvec2(40.0, 18.0),
            },
            style,
        },
        DrawCommand::Decoration {
            range: range(0, 4),
            rects: Arc::from([
                Rect {
                    pos: dvec2(7.0, 8.0),
                    size: dvec2(10.0, 18.0),
                },
                Rect {
                    pos: dvec2(17.0, 8.0),
                    size: dvec2(10.0, 18.0),
                },
            ]),
            role: DecorationRole::LinkUnderline,
        },
        DrawCommand::EmbeddedBlock {
            id: layout_id(3, 0),
            rect: Rect {
                pos: dvec2(9.0, 10.0),
                size: dvec2(50.0, 30.0),
            },
            state: EmbeddedState::Loading,
        },
        DrawCommand::CaretAndIme {
            caret: Rect {
                pos: dvec2(11.0, 12.0),
                size: dvec2(2.0, 18.0),
            },
            composition: Arc::from([Rect {
                pos: dvec2(13.0, 14.0),
                size: dvec2(25.0, 18.0),
            }]),
        },
    ];

    let translated = commands
        .iter()
        .map(|command| command.translated(origin))
        .collect::<Vec<_>>();

    let expected_positions = [
        vec![dvec2(281.0, 42.0)],
        vec![dvec2(283.0, 44.0)],
        vec![dvec2(285.0, 46.0)],
        vec![dvec2(287.0, 48.0), dvec2(297.0, 48.0)],
        vec![dvec2(289.0, 50.0)],
        vec![dvec2(291.0, 52.0), dvec2(293.0, 54.0)],
    ];
    let actual_positions = translated
        .iter()
        .map(command_rect_positions)
        .collect::<Vec<_>>();
    assert_eq!(actual_positions, expected_positions);
}

#[test]
fn zero_draw_origin_preserves_harness_coordinates() {
    let command = DrawCommand::Selection {
        rect: Rect {
            pos: dvec2(3.0, 4.0),
            size: dvec2(20.0, 10.0),
        },
    };

    assert_eq!(command.translated(dvec2(0.0, 0.0)), command);
}

#[test]
fn quote_rule_stays_narrow_and_parent_owned_marker_keeps_a_text_command() {
    let source = "> quote";
    let styles = PresentationStyles::balanced();
    let marker = text_item(2, 0, 0..2, TextRole::QuoteMarker);
    let rule_id = item_id(
        2,
        PresentationRole::Block(BlockDecorationRole::QuoteRule),
        0,
    );
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            marker,
            text_item(3, 0, 2..source.len(), TextRole::Body),
            PresentationItem::BlockDecoration {
                id: rule_id,
                owner: owner(2),
                source_range: range(0, source.len()),
                kind: BlockDecorationKind::QuoteRule,
            },
        ]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let quote_rect = Rect {
        pos: dvec2(24.0, 20.0),
        size: dvec2(400.0, 40.0),
    };
    let marker_layout_id = layout_id(2, u32::MAX - 1);
    let marker_cluster = cluster(2, u32::MAX - 1, 0..2, 36.0, TextRole::QuoteMarker);
    let layout = snapshot(
        source.len(),
        vec![marker_cluster],
        vec![BlockGeometry::new(
            layout_id(2, 0),
            range(0, source.len()),
            quote_rect,
        )],
    );
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([owner(2)]),
        diagnostics: Arc::from([]),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    let commands =
        build_draw_commands(&frame, &plan, &styles, &selection(source, 2, 2), None).unwrap();
    assert!(commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text {
            id,
            range: command_range,
            ..
        } if id.layout == marker_layout_id && *command_range == range(0, 2)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        DrawCommand::BlockBackground {
            rect,
            role: BlockDecorationRole::QuoteRule,
            ..
        } if rect.size.x == styles.spacing().quote_rule
    )));
}

#[test]
fn thematic_rule_is_a_narrow_centered_background_and_keeps_literal_source_text() {
    let source = "---";
    let styles = PresentationStyles::balanced();
    let rule_id = item_id(
        2,
        PresentationRole::Block(BlockDecorationRole::ThematicRule),
        0,
    );
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            text_item(2, 0, 0..source.len(), TextRole::SyntaxMarker),
            PresentationItem::BlockDecoration {
                id: rule_id,
                owner: owner(2),
                source_range: range(0, source.len()),
                kind: BlockDecorationKind::ThematicRule,
            },
        ]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let block_rect = Rect {
        pos: dvec2(24.0, 20.0),
        size: dvec2(400.0, 44.0),
    };
    let layout = snapshot(
        source.len(),
        vec![cluster(2, 0, 0..source.len(), 36.0, TextRole::SyntaxMarker)],
        vec![BlockGeometry::new(
            layout_id(2, 0),
            range(0, source.len()),
            block_rect,
        )],
    );
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([]),
        diagnostics: Arc::from([]),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    let commands =
        build_draw_commands(&frame, &plan, &styles, &selection(source, 0, 0), None).unwrap();
    assert!(commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { range: command_range, .. } if *command_range == range(0, 3)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        DrawCommand::BlockBackground {
            rect,
            role: BlockDecorationRole::ThematicRule,
            ..
        } if rect.size.x == block_rect.size.x
            && rect.size.y == styles.spacing().quote_rule
            && rect.pos.y == block_rect.pos.y + (block_rect.size.y - rect.size.y) * 0.5
    )));
}

#[test]
fn active_markers_change_only_color_and_keep_semantic_content_metrics() {
    let source = "**bold**";
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            text_item(1, 0, 0..2, TextRole::SyntaxMarker),
            text_item(1, 1, 2..6, TextRole::StrongEmphasis),
            text_item(1, 2, 6..8, TextRole::SyntaxMarker),
        ]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let layout = snapshot(
        source.len(),
        vec![
            cluster(1, 0, 0..2, 10.0, TextRole::SyntaxMarker),
            cluster(1, 1, 2..6, 30.0, TextRole::StrongEmphasis),
            cluster(1, 2, 6..8, 70.0, TextRole::SyntaxMarker),
        ],
        Vec::new(),
    );
    let assets = Arc::new(EmbeddedAssetFrame {
        revision: DocumentRevision::INITIAL,
        items: Arc::from([]),
    });
    let make_frame = |active_owners| PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout: layout.clone(),
        active_owners,
        diagnostics: Arc::from([]),
        assets: assets.clone(),
    };
    let styles = PresentationStyles::balanced();
    let caret = selection(source, 3, 3);
    let inactive =
        build_draw_commands(&make_frame(Arc::from([])), &plan, &styles, &caret, None).unwrap();
    let active = build_draw_commands(
        &make_frame(Arc::from([owner(1)])),
        &plan,
        &styles,
        &caret,
        None,
    )
    .unwrap();
    let texts = |commands: &Arc<[DrawCommand]>| {
        commands
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text { rect, style, .. } => Some((*rect, *style)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let inactive = texts(&inactive);
    let active = texts(&active);

    assert_eq!(inactive[0].0, active[0].0);
    assert_eq!(inactive[0].1.metrics, active[0].1.metrics);
    assert_eq!(inactive[0].1.color, ColorRole::Marker);
    assert_eq!(active[0].1.color, ColorRole::ActiveMarker);
    assert_eq!(
        active[1].1.metrics,
        styles.metrics(TextRole::StrongEmphasis)
    );
    assert_eq!(active[1].1.color, ColorRole::Text);
    assert_eq!(inactive[2].0, active[2].0);
    assert_eq!(inactive[2].1.metrics, active[2].1.metrics);
}

#[test]
fn every_interactive_rectangle_comes_from_the_frame_snapshot() {
    let source = "abcdefgh";
    let text = text_item(1, 0, 0..source.len(), TextRole::LinkLabel);
    let image_id = item_id(3, PresentationRole::Embedded(EmbeddedBlockRole::Image), 0);
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            text,
            PresentationItem::EmbeddedBlock {
                id: image_id,
                owner: owner(3),
                source_range: range(4, 8),
                kind: EmbeddedBlockKind::Image {
                    destination: Arc::from("image.png"),
                    alt: Arc::from("image"),
                    title: None,
                },
            },
        ]),
        links: Arc::from([PresentedLink {
            owner: owner(1),
            source_range: range(1, 5),
            destination: Arc::from("target"),
            title: None,
        }]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let image_rect = Rect {
        pos: dvec2(120.0, 20.0),
        size: dvec2(90.0, 18.0),
    };
    let layout = snapshot(
        source.len(),
        vec![cluster(1, 0, 0..source.len(), 10.0, TextRole::LinkLabel)],
        vec![BlockGeometry::new(layout_id(3, 0), range(4, 8), image_rect)],
    );
    let diagnostics = Arc::from([PresentedDiagnostic {
        revision: DocumentRevision::INITIAL,
        range: range(2, 6),
        severity: PresentedDiagnosticSeverity::Warning,
        message: Arc::from("warning"),
    }]);
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout: layout.clone(),
        active_owners: Arc::from([]),
        diagnostics,
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([(image_id, EmbeddedState::Loading)]),
        }),
    };
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let selections = selection(source, 2, 4);
    let mut session = MarkdownDocumentSession::with_selections(
        Arc::new(MarkdownDocumentSnapshot::new(syntax)),
        selections.clone(),
    )
    .unwrap();
    session.begin_ime().unwrap();
    let commands = build_draw_commands(
        &frame,
        &plan,
        &PresentationStyles::balanced(),
        &selections,
        session.ime(),
    )
    .unwrap();

    let selection_rects = layout.selection_rects(selections.primary()).unwrap();
    assert!(commands.iter().any(|command| matches!(command, DrawCommand::Selection { rect } if selection_rects.contains(rect))));
    let link_rects = layout
        .selection_rects(Selection::new(position(1), position(5)))
        .unwrap();
    assert!(commands.iter().any(|command| matches!(command, DrawCommand::Decoration { rects, role: DecorationRole::LinkUnderline, .. } if rects.as_ref() == link_rects)));
    let diagnostic_rects = layout
        .selection_rects(Selection::new(position(2), position(6)))
        .unwrap();
    assert!(commands.iter().any(|command| matches!(command, DrawCommand::Decoration { rects, role: DecorationRole::DiagnosticUnderline(PresentedDiagnosticSeverity::Warning), .. } if rects.as_ref() == diagnostic_rects)));
    assert!(commands.iter().any(
        |command| matches!(command, DrawCommand::EmbeddedBlock { rect, .. } if *rect == image_rect)
    ));
    let expected_caret = layout
        .source_to_point(selections.primary().cursor)
        .unwrap()
        .rect;
    let expected_composition = layout.selection_rects(selections.primary()).unwrap();
    assert!(commands.iter().any(|command| matches!(command, DrawCommand::CaretAndIme { caret, composition } if *caret == expected_caret && composition.as_ref() == expected_composition)));
}

#[test]
fn installed_presentation_rejects_each_partial_revision_bundle() {
    let (plan, _, styles, _) = plan_with_all_layers();
    let plan = Arc::new(plan);
    let stale = DocumentRevision::new(2);
    let diagnostics = Arc::from([PresentedDiagnostic {
        revision: stale,
        range: range(0, 1),
        severity: PresentedDiagnosticSeverity::Information,
        message: Arc::from("stale"),
    }]);
    let assets = Arc::new(EmbeddedAssetFrame {
        revision: stale,
        items: Arc::from([]),
    });

    assert!(matches!(
        InstalledPresentation::new(
            plan.clone(),
            Arc::new(styles),
            empty_layout_document(stale),
            Arc::from([]),
            Arc::new(EmbeddedAssetFrame {
                revision: DocumentRevision::INITIAL,
                items: Arc::from([])
            }),
        ),
        Err(PresentationError::RevisionMismatch {
            component: "layout_document",
            ..
        })
    ));
    assert!(matches!(
        InstalledPresentation::new(
            plan.clone(),
            Arc::new(styles),
            empty_layout_document(DocumentRevision::INITIAL),
            diagnostics,
            Arc::new(EmbeddedAssetFrame {
                revision: DocumentRevision::INITIAL,
                items: Arc::from([])
            }),
        ),
        Err(PresentationError::RevisionMismatch {
            component: "diagnostic",
            ..
        })
    ));
    assert!(matches!(
        InstalledPresentation::new(
            plan,
            Arc::new(styles),
            empty_layout_document(DocumentRevision::INITIAL),
            Arc::from([]),
            assets,
        ),
        Err(PresentationError::RevisionMismatch {
            component: "assets",
            ..
        })
    ));
}

#[test]
fn crate_root_exposes_the_makepad_script_registration_seam() {
    let _: fn(&mut ScriptVm) -> ScriptValue = waml_markdown_editor::script_mod;
}

#[test]
fn motion_scroll_follows_the_interpolated_primary_caret() {
    let previous = snapshot(
        4,
        vec![cluster(1, 0, 0..4, 10.0, TextRole::Body)],
        Vec::new(),
    );
    let mut moved = cluster(1, 0, 0..4, 10.0, TextRole::Body);
    moved.rect.pos.y = 200.0;
    for stop in Arc::make_mut(&mut moved.caret_stops) {
        stop.point.y = 200.0;
    }
    let target = snapshot(4, vec![moved], Vec::new());
    let anchor = derive_motion_scroll_anchor(&previous, &target, position(0), 100.0)
        .expect("both snapshots contain the primary caret");
    assert_eq!(anchor.viewport_y, -80.0);

    let mut motion = MotionController::new(100.0);
    motion.commit(
        10.0,
        Some(previous),
        target,
        LayoutChangeCause::LocalEdit {
            changes: Arc::from([TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("x"),
            }]),
        },
        false,
        Some(anchor),
        MotionConfig::default(),
    );
    let frame = motion.sample(10.050);
    let caret_y = frame
        .layout
        .source_to_point(position(0))
        .unwrap()
        .rect
        .pos
        .y;
    assert_eq!(
        frame.scroll_y,
        (caret_y - anchor.viewport_y).clamp(0.0, frame.layout.max_scroll_y(100.0))
    );
    assert_ne!(frame.scroll_y, 0.0);
}

#[test]
fn renderer_applies_every_resolved_text_attribute_in_its_required_layer() {
    let rect = Rect {
        pos: dvec2(10.0, 20.0),
        size: dvec2(40.0, 18.0),
    };
    let metrics = TextMetrics {
        font: FontKey(1),
        font_size: 14.0,
        line_spacing: 1.35,
        weight: FontWeight(600),
        italic: true,
    };
    let style = waml_markdown_editor::presentation::ResolvedTextStyle {
        metrics,
        color: ColorRole::Link,
        background: Some(ColorRole::CodeSurface),
        underline: true,
        strikethrough: true,
    };

    let operations = build_text_paint_operations(rect, style);
    assert_eq!(
        operations
            .iter()
            .map(TextPaintOperation::layer)
            .collect::<Vec<_>>(),
        vec![
            DrawLayer::BlockBackground,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::Decoration,
        ]
    );
    assert!(matches!(
        operations[0],
        TextPaintOperation::Background {
            rect: actual,
            color: ColorRole::CodeSurface,
        } if actual == rect
    ));
    assert!(matches!(
        operations[1],
        TextPaintOperation::Glyphs {
            face: TextFace::SansSemiboldItalic,
            metrics: actual,
            color: ColorRole::Link,
        } if actual == metrics
    ));
    assert!(matches!(
        operations[2],
        TextPaintOperation::Underline {
            rect: Rect { pos, size },
            color: ColorRole::Link,
        } if pos == dvec2(10.0, 36.0) && size == dvec2(40.0, 2.0)
    ));
    assert!(matches!(
        operations[3],
        TextPaintOperation::Strikethrough {
            rect: Rect { pos, size },
            color: ColorRole::Link,
        } if pos == dvec2(10.0, 28.0) && size == dvec2(40.0, 2.0)
    ));
}

#[test]
fn link_activation_accepts_source_range_start() {
    let (plan, frame, _, _) = plan_with_all_layers();

    assert_eq!(
        navigation_position(&plan, &frame.layout, dvec2(10.0, 20.0)),
        Some(position(0))
    );
}

#[test]
fn link_activation_rejects_source_range_end() {
    let (plan, frame, _, _) = plan_with_all_layers();

    assert_eq!(
        navigation_position(&plan, &frame.layout, dvec2(50.0, 20.0)),
        None
    );
}

#[test]
fn the_editor_never_draws_the_list_bullet_decoration() {
    let source = "- alpha";
    let styles = PresentationStyles::balanced();
    let bullet_id = item_id(
        2,
        PresentationRole::Block(BlockDecorationRole::ListBullet),
        0,
    );
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([
            text_item(2, 0, 0..source.len(), TextRole::ListMarker),
            PresentationItem::BlockDecoration {
                id: bullet_id,
                owner: owner(2),
                source_range: range(0, 2),
                kind: BlockDecorationKind::ListBullet { level: 0 },
            },
        ]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let block_rect = Rect {
        pos: dvec2(24.0, 20.0),
        size: dvec2(400.0, 44.0),
    };
    let layout = snapshot(
        source.len(),
        vec![cluster(2, 0, 0..source.len(), 36.0, TextRole::ListMarker)],
        vec![BlockGeometry::new(
            layout_id(2, 0),
            range(0, source.len()),
            block_rect,
        )],
    );
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([]),
        diagnostics: Arc::from([]),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    let commands =
        build_draw_commands(&frame, &plan, &styles, &selection(source, 0, 0), None).unwrap();
    assert!(
        !commands.iter().any(|command| matches!(
            command,
            DrawCommand::BlockBackground {
                role: BlockDecorationRole::ListBullet,
                ..
            }
        )),
        "the editor shows the raw `-` marker; the ListBullet decoration is \
         plan metadata for the reading view, never an editor draw command"
    );
}

#[test]
fn a_diagnostic_message_command_sits_on_the_decoration_layer_and_translates() {
    let command = DrawCommand::DiagnosticMessage {
        line: range(0, 4),
        rect: Rect {
            pos: dvec2(100.0, 20.0),
            size: dvec2(60.0, 18.0),
        },
        text: Arc::from("unexpected token"),
        severity: PresentedDiagnosticSeverity::Error,
    };
    assert_eq!(command.layer(), DrawLayer::Decoration);
    let moved = command.translated(dvec2(10.0, 5.0));
    let DrawCommand::DiagnosticMessage {
        rect,
        text,
        severity,
        line,
    } = moved
    else {
        panic!("translation must preserve the variant");
    };
    assert_eq!(rect.pos, dvec2(110.0, 25.0));
    assert_eq!(rect.size, dvec2(60.0, 18.0));
    assert_eq!(text.as_ref(), "unexpected token");
    assert_eq!(severity, PresentedDiagnosticSeverity::Error);
    assert_eq!(line, range(0, 4));
}

/// Two visual lines with real clusters, for the emission-rule tests.
/// Line 0 covers 0..6 (clusters end at x=10+6*10=70), line 1 covers 6..12.
fn two_line_snapshot(source_len: usize) -> Arc<LayoutSnapshot> {
    Arc::new(LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(600.0, 200.0),
        vec![
            VisualLine::for_test(range(0, 6), 20.0, 18.0),
            VisualLine::for_test(range(6, source_len), 40.0, 18.0),
        ],
        vec![
            cluster(1, 0, 0..6, 10.0, TextRole::Body),
            cluster(1, 1, 6..source_len, 10.0, TextRole::Body),
        ],
        Vec::new(),
    ))
}

fn diagnostic(
    bounds: std::ops::Range<usize>,
    severity: PresentedDiagnosticSeverity,
    message: &str,
) -> PresentedDiagnostic {
    PresentedDiagnostic {
        revision: DocumentRevision::INITIAL,
        range: range(bounds.start, bounds.end),
        severity,
        message: Arc::from(message),
    }
}

fn message_commands(
    source: &str,
    layout: Arc<LayoutSnapshot>,
    diagnostics: Vec<PresentedDiagnostic>,
) -> Vec<DrawCommand> {
    let plan = PresentationPlan {
        revision: DocumentRevision::INITIAL,
        source_len: t(source.len()),
        items: Arc::from([text_item(1, 0, 0..source.len(), TextRole::Body)]),
        links: Arc::from([]),
        blocks: Arc::from([]),
        diagnostics: Arc::from([]),
    };
    let frame = PresentationFrame {
        revision: DocumentRevision::INITIAL,
        layout,
        active_owners: Arc::from([]),
        diagnostics: diagnostics.into(),
        assets: Arc::new(EmbeddedAssetFrame {
            revision: DocumentRevision::INITIAL,
            items: Arc::from([]),
        }),
    };
    build_draw_commands(
        &frame,
        &plan,
        &PresentationStyles::balanced(),
        &selection(source, 0, 0),
        None,
    )
    .unwrap()
    .iter()
    .filter(|command| matches!(command, DrawCommand::DiagnosticMessage { .. }))
    .cloned()
    .collect()
}

#[test]
fn diagnostics_bucket_onto_the_visual_line_holding_their_end_offset() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            // Starts on line 0 but ENDS on line 1 -> the message rides line 1.
            diagnostic(2..9, PresentedDiagnosticSeverity::Warning, "spans"),
        ],
    );
    assert_eq!(commands.len(), 1);
    let DrawCommand::DiagnosticMessage { line, rect, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(*line, range(6, 12));
    assert_eq!(rect.pos.y, 40.0);
    assert_eq!(rect.size.y, 18.0);
}

#[test]
fn worst_severity_wins_a_contested_line_and_counts_the_losers() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(0..2, PresentedDiagnosticSeverity::Information, "info"),
            diagnostic(3..5, PresentedDiagnosticSeverity::Error, "the error"),
            diagnostic(1..4, PresentedDiagnosticSeverity::Warning, "warn"),
        ],
    );
    assert_eq!(commands.len(), 1, "one message per line");
    let DrawCommand::DiagnosticMessage { text, severity, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(text.as_ref(), "the error +2");
    assert_eq!(*severity, PresentedDiagnosticSeverity::Error);
}

#[test]
fn severity_ties_break_on_the_earliest_start() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(3..5, PresentedDiagnosticSeverity::Error, "later"),
            diagnostic(1..4, PresentedDiagnosticSeverity::Error, "earlier"),
        ],
    );
    let DrawCommand::DiagnosticMessage { text, .. } = &commands[0] else {
        unreachable!()
    };
    assert_eq!(text.as_ref(), "earlier +1");
}

#[test]
fn contested_lines_stay_independent_and_commands_come_in_line_order() {
    let source = "abcdefghijkl";
    let commands = message_commands(
        source,
        two_line_snapshot(source.len()),
        vec![
            diagnostic(7..9, PresentedDiagnosticSeverity::Warning, "second line"),
            diagnostic(0..2, PresentedDiagnosticSeverity::Information, "first line"),
        ],
    );
    assert_eq!(commands.len(), 2);
    let DrawCommand::DiagnosticMessage { text: first, .. } = &commands[0] else {
        unreachable!()
    };
    let DrawCommand::DiagnosticMessage { text: second, .. } = &commands[1] else {
        unreachable!()
    };
    assert_eq!(first.as_ref(), "first line");
    assert_eq!(second.as_ref(), "second line");
}
