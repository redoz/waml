//! Mounts `MarkdownViewer` and draws it, to catch drawing regressions no
//! pure-model test can see (a `script_mod!` typo that silently drops the
//! `TextFlow` child, a layout that collapses to zero height, ...).

use std::{cell::Cell, sync::Arc};

use makepad_widgets::event::{ScrollEvent, ScrollPhase};
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

fn mounted_scrolling_body(cx: &mut Cx) -> WidgetRef {
    waml_markdown_editor::live_design(cx);
    cx.with_vm(|vm| {
        makepad_widgets::script_mod(vm);
        waml_markdown_editor::script_mod(vm);
        let value = script_eval!(vm, {
            use mod.prelude.widgets_internal.*
            mod.widgets.ScrollYView {
                width: Fill
                height: Fill
                flow: Down
            }
        });
        let mut root = WidgetRef::script_new(vm);
        root.script_apply(vm, &Apply::New, &mut Scope::empty(), value);
        let viewer_value = script_eval!(vm, {
            use mod.prelude.widgets_internal.*
            mod.widgets.MarkdownViewer {
                width: Fill
                height: Fit
            }
        });
        let mut viewer = WidgetRef::script_new(vm);
        viewer.script_apply(vm, &Apply::New, &mut Scope::empty(), viewer_value);
        root.borrow_mut::<View>()
            .expect("ScrollYView is a View")
            .children
            .push((live_id!(viewer), viewer));
        root
    })
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

fn install_and_draw(cx: &mut Cx, ui: &WidgetRef, source: &str, pass_name: &str) {
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
    ui.widget(cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer()
        .install_document(cx, document, Arc::from(source), empty_frame());
    draw_ui(cx, ui, 800.0, 600.0, pass_name);
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

fn draw_scrolling_ui(cx: &mut Cx, ui: &WidgetRef, width: f64, height: f64) {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, "reading-widget-scroll-test");
    pass.set_size(cx, dvec2(width, height));
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    let mut cx_2d = Cx2d::new(&mut draw_cx);
    cx_2d.begin_pass(&pass, None);
    draw_list.begin_always(&mut cx_2d);
    let size = cx_2d.current_pass_size();
    cx_2d.begin_root_turtle(size, Layout::flow_down());
    ui.draw_walk_all(&mut cx_2d, &mut Scope::empty(), Walk::fill());
    cx_2d.end_pass_sized_turtle();
    draw_list.end(&mut cx_2d);
    cx_2d.end_pass(&pass);
}

#[test]
fn wheel_input_scrolls_the_parent_around_a_mounted_viewer() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_scrolling_body(&mut cx);
    let viewer_widget = ui.widget(&cx, ids!(viewer));
    assert!(
        viewer_widget
            .borrow::<waml_markdown_editor::reading::MarkdownViewer>()
            .is_some(),
        "the scripted scroller must contain the MarkdownViewer"
    );
    assert!(
        viewer_widget.visible(),
        "the MarkdownViewer must be visible"
    );
    let viewer_walk = viewer_widget.walk(&mut cx);
    assert!(
        viewer_walk.height.is_fit(),
        "the MarkdownViewer must use Fit height: {viewer_walk:?}"
    );
    let viewer = viewer_widget.as_markdown_viewer();

    let source = (0..80)
        .map(|line| format!("Paragraph {line} has enough text to draw.\n\n"))
        .collect::<String>();
    let text = SourceText::new(source.clone()).expect("valid source text");
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

    draw_scrolling_ui(&mut cx, &ui, 400.0, 120.0);
    let flow = ui.widget(&cx, ids!(viewer.flow_body));
    let before = flow.area().rect(&cx);
    assert!(
        before.size.y > 120.0,
        "the viewer text must overflow its parent: {before:?}"
    );

    let event = Event::Scroll(ScrollEvent {
        window_id: WindowId(0, 0),
        scroll: dvec2(0.0, 80.0),
        abs: dvec2(100.0, 80.0),
        modifiers: KeyModifiers::default(),
        handled_x: Cell::new(false),
        handled_y: Cell::new(false),
        is_mouse: true,
        time: 0.0,
        phase: ScrollPhase::Changed,
    });
    ui.handle_event(&mut cx, &event, &mut Scope::empty());
    let Event::Scroll(scroll) = &event else {
        unreachable!();
    };
    assert!(
        scroll.handled_y.get(),
        "the parent must claim vertical wheel input"
    );

    draw_scrolling_ui(&mut cx, &ui, 400.0, 120.0);
    let after = flow.area().rect(&cx);
    assert!(
        after.pos.y < before.pos.y,
        "the parent must move the tall viewer after wheel input: before={before:?}, after={after:?}"
    );
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

#[test]
fn set_zoom_scales_the_flow_from_a_stable_base_without_compounding() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    let base = flow.borrow().expect("the flow exists").font_size;

    viewer.set_zoom(&mut cx, 1.5);
    assert_eq!(flow.borrow().unwrap().font_size, base * 1.5);
    viewer.set_zoom(&mut cx, 1.5);
    assert_eq!(flow.borrow().unwrap().font_size, base * 1.5);
    viewer.set_zoom(&mut cx, 1.0);
    assert_eq!(flow.borrow().unwrap().font_size, base);
}

fn drawn_link_fixture(source: &str, pass_name: &str) -> (Cx, WidgetRef, TextRange) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    install_and_draw(&mut cx, &ui, source, pass_name);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let text = SourceText::new(source.to_owned()).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let plan = compile_presentation(
        &syntax,
        &Arc::new(PresentationStyles::balanced()),
        &HighlighterRegistry::default(),
    )
    .unwrap();
    let document = build_reading_document(&plan, &RegisteredBlockExtensions::default()).unwrap();
    let range = document.links[0].source_range;
    assert!(!viewer.test_source_rects(&cx, range).is_empty());
    (cx, ui, range)
}

#[test]
fn a_point_inside_a_drawn_link_resolves_to_its_destination() {
    let (cx, ui, range) = drawn_link_fixture(
        "See [Customer](./customer.md) for more.\n",
        "reading-widget-draw-link-test",
    );
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let rect = viewer.test_source_rects(&cx, range)[0];
    assert_eq!(
        viewer
            .test_link_at_point(&cx, rect.pos + rect.size * 0.5)
            .as_deref(),
        Some("./customer.md")
    );
    assert!(viewer
        .test_link_at_point(&cx, dvec2(-100.0, -100.0))
        .is_none());
}

#[test]
fn a_point_beside_a_drawn_link_resolves_to_nothing() {
    let (cx, ui, range) = drawn_link_fixture(
        "[Customer](./customer.md) owns the account.\n",
        "reading-widget-draw-link-margin-test",
    );
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
    let rect = viewer.test_source_rects(&cx, range)[0];
    let middle_y = rect.pos.y + rect.size.y * 0.5;
    assert!(viewer
        .test_link_at_point(&cx, dvec2(rect.pos.x - 20.0, middle_y))
        .is_none());
    assert!(viewer
        .test_link_at_point(&cx, dvec2(rect.pos.x + rect.size.x * 0.5, middle_y + 200.0))
        .is_none());
}

#[test]
fn a_link_label_draws_in_its_own_colour() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    install_and_draw(
        &mut cx,
        &ui,
        "Owned by Customer.\n",
        "reading-widget-draw-plain-colour",
    );
    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    let body_colour = flow.borrow().unwrap().draw_text.color;

    install_and_draw(
        &mut cx,
        &ui,
        "Owned by [Customer](./customer.md)\n",
        "reading-widget-draw-link-colour",
    );
    let flow = flow.borrow().unwrap();
    assert_ne!(flow.draw_text.color, body_colour);
    assert!(flow.draw_text.color.w > 0.0);
    assert_eq!(flow.underline.value(), 0);
    assert!(flow.font_colors.is_empty());
}
