use std::sync::Arc;

use makepad_widgets::{dvec2, Rect, ScriptValue, ScriptVm};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    layout::{
        BlockGeometry, CaretStop, EdgeInsets, GeometryElementId, GlyphCluster, LayoutDocument,
        LayoutElementId, LayoutSnapshot, VisualLine,
    },
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
    widget::DrawLayer,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxIdentity, TextRange,
    TextSize,
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
