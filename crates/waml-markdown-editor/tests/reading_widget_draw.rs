//! Mounts `MarkdownViewer` and draws it, to catch drawing regressions no
//! pure-model test can see (a `script_mod!` typo that silently drops the
//! `TextFlow` child, a layout that collapses to zero height, ...).

use std::{cell::Cell, sync::Arc};

use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{
    build_reading_document, BlockExtensionFrame, BlockExtensionState, FencedBlockExtension,
    MarkdownViewerWidgetRefExt, ReadingBlock, ReadingBlockKind, RegisteredBlockExtensions,
    RenderedBlockSvg,
};
use waml_markdown_editor::syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange, TextSize,
};

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn mounted_body(cx: &mut Cx) -> WidgetRef {
    waml_markdown_editor::live_design(cx);
    let viewer = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(waml_markdown_editor::reading::MarkdownViewer::script_new_with_default),
    ));
    let mut surface = cx.with_vm(View::script_new_with_default);
    surface.children.push((live_id!(viewer), viewer));
    let surface = WidgetRef::new_with_inner(Box::new(surface));
    let mut root = cx.with_vm(View::script_new_with_default);
    root.children
        .push((live_id!(markdown_viewer_surface), surface));
    WidgetRef::new_with_inner(Box::new(root))
}

fn empty_frame() -> Arc<BlockExtensionFrame> {
    Arc::new(BlockExtensionFrame {
        revision: DocumentRevision::INITIAL,
        items: Arc::from([]),
    })
}

fn extension_document(
    source: &str,
) -> (
    Arc<waml_markdown_editor::reading::ReadingDocument>,
    FencedBlockExtension,
) {
    let text = SourceText::new(source.to_owned()).expect("valid source text");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = Arc::new(PresentationStyles::balanced());
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    let registered = RegisteredBlockExtensions::from_languages([Arc::from("mermaid")]);
    let document =
        Arc::new(build_reading_document(&plan, &registered).expect("reading model builds"));

    fn find(blocks: &[ReadingBlock]) -> Option<FencedBlockExtension> {
        blocks.iter().find_map(|block| match &block.kind {
            ReadingBlockKind::FencedExtension(extension) => Some(extension.clone()),
            _ => find(&block.children),
        })
    }

    let extension = find(&document.roots).expect("registered fence becomes an extension");
    (document, extension)
}

fn extension_frame(
    extension: &FencedBlockExtension,
    state: BlockExtensionState,
) -> Arc<BlockExtensionFrame> {
    Arc::new(BlockExtensionFrame {
        revision: DocumentRevision::INITIAL,
        items: vec![(extension.id, state)].into(),
    })
}

fn svg(logical_size: (f64, f64)) -> RenderedBlockSvg {
    RenderedBlockSvg {
        data: Arc::from(
            &b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1000 500'><rect width='1000' height='500'/></svg>"[..],
        ),
        logical_size,
    }
}

fn draw_ui(cx: &mut Cx, ui: &WidgetRef, width: f64, height: f64, name: &str) {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, name);
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    draw_cx.begin_pass(&pass, None);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(dvec2(width, height), Layout::default());
        ui.widget(&cx_2d, ids!(markdown_viewer_surface.viewer))
            .draw_walk_all(
                &mut cx_2d,
                &mut Scope::empty(),
                Walk::abs_rect(Rect {
                    pos: dvec2(0.0, 0.0),
                    size: dvec2(width, height),
                }),
            );
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
}

fn mouse_down(abs: DVec2) -> Event {
    Event::MouseDown(MouseDownEvent {
        abs,
        button: MouseButton::PRIMARY,
        window_id: WindowId(0, 0),
        modifiers: KeyModifiers::default(),
        handled: Cell::new(Area::default()),
        time: 0.0,
    })
}

#[test]
fn a_mounted_viewer_paints_the_installed_document() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    let source = "# Title\n\nBody text.\n";
    let text = SourceText::new(source.to_owned()).expect("valid source text");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = Arc::new(PresentationStyles::balanced());
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    let document = Arc::new(
        build_reading_document(&plan, &RegisteredBlockExtensions::default())
            .expect("reading model builds"),
    );
    viewer.install_document(&mut cx, document, Arc::from(source), empty_frame());

    {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(&mut cx, "reading-widget-draw-test");
        let mut draw_list = DrawList2d::new(&mut cx);
        let mut draw_cx = CxDraw::new(&mut cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(dvec2(800.0, 600.0), Layout::default());
            ui.widget(&cx_2d, ids!(markdown_viewer_surface.viewer))
                .draw_walk_all(
                    &mut cx_2d,
                    &mut Scope::empty(),
                    Walk::abs_rect(Rect {
                        pos: dvec2(0.0, 0.0),
                        size: dvec2(800.0, 600.0),
                    }),
                );
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
    }

    let source_span = viewer.selected_source_span(&cx);
    assert_eq!(
        source_span, None,
        "nothing is selected yet, but the call must not panic"
    );
    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    let flow = flow.borrow().expect("the flow_body child must exist");
    assert!(
        flow.text_len() > 0,
        "the installed document must have pushed text into the flow buffer"
    );
}

#[test]
fn setting_and_clearing_search_highlights_updates_the_installed_range_set() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    assert!(
        viewer.test_search_highlights().is_empty(),
        "a fresh viewer starts with no highlights installed"
    );

    let hit = range(2, 7);
    viewer.set_search_highlights(&mut cx, vec![hit]);
    assert_eq!(viewer.test_search_highlights(), vec![hit]);

    viewer.clear_search_highlights(&mut cx);
    assert!(
        viewer.test_search_highlights().is_empty(),
        "clear must drop the installed set"
    );
}

#[test]
fn a_mounted_viewer_paints_a_rect_over_each_matched_run() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    let source = "# Title\n\nBody text.\n";
    let text = SourceText::new(source.to_owned()).expect("valid source text");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = Arc::new(PresentationStyles::balanced());
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    let document = Arc::new(
        build_reading_document(&plan, &RegisteredBlockExtensions::default())
            .expect("reading model builds"),
    );
    viewer.install_document(&mut cx, document, Arc::from(source), empty_frame());
    viewer.set_search_highlights(&mut cx, vec![range(2, 7)]);

    {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(&mut cx, "reading-widget-draw-highlights-test");
        let mut draw_list = DrawList2d::new(&mut cx);
        let mut draw_cx = CxDraw::new(&mut cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(dvec2(800.0, 600.0), Layout::default());
            ui.widget(&cx_2d, ids!(markdown_viewer_surface.viewer))
                .draw_walk_all(
                    &mut cx_2d,
                    &mut Scope::empty(),
                    Walk::abs_rect(Rect {
                        pos: dvec2(0.0, 0.0),
                        size: dvec2(800.0, 600.0),
                    }),
                );
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
    }

    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    {
        let flow = flow.borrow().expect("the flow_body child must exist");
        assert!(
            flow.text_len() > 0,
            "a draw with highlights installed must still paint the document"
        );
    }

    // The whole point of installing a highlight: the reading surface must
    // resolve it to real geometry and paint over it, not silently no-op.
    let rects = viewer.test_highlight_rects(&cx);
    assert!(
        !rects.is_empty(),
        "a highlight over drawn text must resolve to at least one rect"
    );
    assert!(
        rects
            .iter()
            .all(|rect| rect.size.x > 0.0 && rect.size.y > 0.0),
        "every painted highlight rect must have real extent, got {rects:?}"
    );

    // And the reveal path's scroll target is derived from the same geometry.
    assert!(viewer.search_highlight_offset(&cx).is_some());
}

#[test]
fn a_loading_extension_reserves_a_72_pixel_visual_source_unit() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let source = "```mermaid\ngraph TD; A-->B\n```\n";
    let (document, extension) = extension_document(source);
    viewer.install_document(
        &mut cx,
        document,
        Arc::from(source),
        extension_frame(&extension, BlockExtensionState::Loading),
    );

    draw_ui(&mut cx, &ui, 400.0, 600.0, "loading-extension-draw");

    let rects = viewer
        .borrow()
        .expect("viewer is mounted")
        .source_map()
        .visual_rects_for_source(extension.source_range);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].size.y, 72.0);
    assert!(rects[0].size.x > 0.0);
}

#[test]
fn a_search_hit_inside_a_fence_highlights_its_visual_rectangle() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let source = "```mermaid\ngraph TD; A-->B\n```\n";
    let (document, extension) = extension_document(source);
    viewer.install_document(
        &mut cx,
        document,
        Arc::from(source),
        extension_frame(&extension, BlockExtensionState::Loading),
    );
    viewer.set_search_highlights(
        &mut cx,
        vec![range(
            extension.content_range.start().to_usize(),
            extension.content_range.start().to_usize() + 5,
        )],
    );

    draw_ui(&mut cx, &ui, 400.0, 600.0, "visual-highlight-draw");

    assert_eq!(viewer.test_highlight_rects(&cx).len(), 1);
    assert_eq!(
        viewer.test_highlight_rects(&cx),
        viewer
            .borrow()
            .expect("viewer is mounted")
            .source_map()
            .visual_rects_for_source(extension.source_range)
    );
}

#[test]
fn pressing_a_visual_unit_hands_off_its_full_fenced_source_range() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let source = "```mermaid\ngraph TD; A-->B\n```\n";
    let (document, extension) = extension_document(source);
    let frame = extension_frame(&extension, BlockExtensionState::Loading);
    viewer.install_document(&mut cx, document.clone(), Arc::from(source), frame.clone());
    draw_ui(&mut cx, &ui, 400.0, 600.0, "visual-pointer-draw");
    let rect = viewer
        .borrow()
        .expect("viewer is mounted")
        .source_map()
        .visual_rects_for_source(extension.source_range)[0];

    ui.handle_event(
        &mut cx,
        &mouse_down(rect.pos + rect.size * 0.5),
        &mut Scope::empty(),
    );
    assert_eq!(
        viewer.selected_source_span(&cx),
        Some(extension.source_range)
    );

    viewer.install_document(&mut cx, document, Arc::from(source), frame);
    assert_eq!(
        viewer.selected_source_span(&cx),
        None,
        "installing a document clears the remembered visual source"
    );
}

#[test]
fn ready_extensions_scale_down_to_the_column_and_never_scale_up() {
    for (logical_size, expected_size, name) in [
        ((1000.0, 500.0), dvec2(400.0, 200.0), "scaled-down-svg"),
        ((200.0, 100.0), dvec2(200.0, 100.0), "natural-size-svg"),
    ] {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let ui = mounted_body(&mut cx);
        let viewer = ui
            .widget(&cx, ids!(markdown_viewer_surface.viewer))
            .as_markdown_viewer();
        let source = "```mermaid\ngraph TD; A-->B\n```\n";
        let (document, extension) = extension_document(source);
        viewer.install_document(
            &mut cx,
            document,
            Arc::from(source),
            extension_frame(&extension, BlockExtensionState::Ready(svg(logical_size))),
        );

        draw_ui(&mut cx, &ui, 400.0, 600.0, name);

        let rects = viewer
            .borrow()
            .expect("viewer is mounted")
            .source_map()
            .visual_rects_for_source(extension.source_range);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].size, expected_size);
        assert!(rects[0].size.x > 0.0 && rects[0].size.y > 0.0);
        assert_eq!(rects[0].pos.x, (400.0 - expected_size.x) * 0.5);
    }
}

#[test]
fn a_failed_extension_keeps_the_source_and_adds_one_safe_error_line() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let source = "```mermaid\ngraph TD; A-->B\n```\n";
    let (document, extension) = extension_document(source);
    viewer.install_document(
        &mut cx,
        document,
        Arc::from(source),
        extension_frame(
            &extension,
            BlockExtensionState::Failed(Arc::from("diagram syntax is invalid")),
        ),
    );

    draw_ui(&mut cx, &ui, 400.0, 600.0, "failed-extension-draw");

    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    let text = flow
        .borrow()
        .expect("the flow_body child must exist")
        .get_full_text();
    assert!(text.contains("graph TD; A-->B"));
    assert_eq!(
        text.matches("Cannot render Mermaid: diagram syntax is invalid")
            .count(),
        1
    );
}
