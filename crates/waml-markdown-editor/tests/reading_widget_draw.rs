//! Mounts `MarkdownViewer` and draws it, to catch drawing regressions no
//! pure-model test can see (a `script_mod!` typo that silently drops the
//! `TextFlow` child, a layout that collapses to zero height, ...).

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{build_reading_document, MarkdownViewerWidgetRefExt};
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
    let document = Arc::new(build_reading_document(&plan).expect("reading model builds"));
    viewer.install_document(&mut cx, document, Arc::from(source));

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
    let document = Arc::new(build_reading_document(&plan).expect("reading model builds"));
    viewer.install_document(&mut cx, document, Arc::from(source));
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
    let base = flow
        .borrow()
        .expect("the flow_body child must exist")
        .font_size;

    viewer.set_zoom(&mut cx, 1.5);
    assert_eq!(
        flow.borrow().expect("flow still exists").font_size,
        base * 1.5
    );

    viewer.set_zoom(&mut cx, 1.5);
    assert_eq!(
        flow.borrow().expect("flow still exists").font_size,
        base * 1.5,
        "two identical zooms must not compound"
    );

    viewer.set_zoom(&mut cx, 1.0);
    assert_eq!(
        flow.borrow().expect("flow still exists").font_size,
        base,
        "reset returns to the base"
    );
}
