//! Mounts `MarkdownViewer` and draws it, to catch drawing regressions no
//! pure-model test can see (a `script_mod!` typo that silently drops the
//! `TextFlow` child, a layout that collapses to zero height, ...).

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{
    build_reading_document, MarkdownViewerWidgetRefExt, RegisteredBlockExtensions,
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

/// Installs `source` on the viewer and runs one full draw pass over it.
fn install_and_draw(cx: &mut Cx, ui: &WidgetRef, source: &str, pass_name: &str) {
    let viewer = ui
        .widget(cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();
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
    viewer.install_document(cx, document, Arc::from(source));

    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, pass_name);
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
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
    let document = Arc::new(
        build_reading_document(&plan, &RegisteredBlockExtensions::default())
            .expect("reading model builds"),
    );
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
    let document = Arc::new(
        build_reading_document(&plan, &RegisteredBlockExtensions::default())
            .expect("reading model builds"),
    );
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

/// A drawn link is clickable: probing the centre of the pixels its text
/// actually occupied resolves to its destination, and a point well outside
/// resolves to nothing.
#[test]
fn a_point_inside_a_drawn_link_resolves_to_its_destination() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    let source = "See [Customer](./customer.md) for more.\n";
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
    let link = document.links[0].clone();
    viewer.install_document(&mut cx, document, Arc::from(source));

    {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(&mut cx, "reading-widget-draw-link-test");
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

    let rects = viewer.test_source_rects(&cx, link.source_range);
    assert!(
        !rects.is_empty(),
        "the link's text must have drawn somewhere"
    );
    let centre = rects[0].pos + rects[0].size * 0.5;
    assert_eq!(
        viewer.test_link_at_point(&cx, centre).as_deref(),
        Some("./customer.md"),
        "the centre of the drawn link resolves to its destination"
    );
    assert!(
        viewer
            .test_link_at_point(&cx, dvec2(-100.0, -100.0))
            .is_none(),
        "a point outside the document resolves to nothing"
    );
}

/// `TextFlow::point_to_index` falls back to the NEAREST character for a point
/// that lands on no run at all, so a click in the reading column's margin --
/// or below the last line -- resolves to whatever character is closest. When
/// that character starts a link (every `## Referenced by` bullet on a
/// generated classifier page does), an unclicked link would navigate. The
/// destination is only real when the point is on the link's own pixels.
#[test]
fn a_point_beside_a_drawn_link_resolves_to_nothing() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    // The link STARTS the line, the worst case: the nearest character to any
    // point in the left gutter is the link's first glyph.
    let source = "[Customer](./customer.md) owns the account.\n";
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
    let link = document.links[0].clone();
    viewer.install_document(&mut cx, document, Arc::from(source));

    {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(&mut cx, "reading-widget-draw-link-margin-test");
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

    let rects = viewer.test_source_rects(&cx, link.source_range);
    assert!(
        !rects.is_empty(),
        "the link's text must have drawn somewhere"
    );
    let rect = rects[0];
    let middle_y = rect.pos.y + rect.size.y * 0.5;
    assert_eq!(
        viewer
            .test_link_at_point(&cx, dvec2(rect.pos.x + rect.size.x * 0.5, middle_y))
            .as_deref(),
        Some("./customer.md"),
        "the drawn pixels themselves still navigate"
    );

    assert!(
        viewer
            .test_link_at_point(&cx, dvec2(rect.pos.x - 20.0, middle_y))
            .is_none(),
        "the gutter to the left of the link is not the link"
    );
    assert!(
        viewer
            .test_link_at_point(&cx, dvec2(rect.pos.x + rect.size.x * 0.5, middle_y + 200.0))
            .is_none(),
        "the empty space below the last line is not the link"
    );
}

/// A link is the only text on a reading surface that DOES something when
/// tapped, and the tap hit-test deliberately never runs on a mouse move (so
/// there is no hover state or pointer cursor to lean on). The affordance has
/// to be in the paint: a link label draws in its own colour, not the body
/// colour, and the style it pushes must not leak into the rest of the page.
#[test]
fn a_link_label_draws_in_its_own_colour() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);

    // Both documents END on the run under test, so `draw_text.color` -- set
    // per run by `TextFlow::draw_text` -- still holds that run's colour.
    install_and_draw(
        &mut cx,
        &ui,
        "Owned by Customer.\n",
        "reading-widget-draw-plain-colour",
    );
    let body_colour = {
        let flow = ui
            .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
            .as_text_flow();
        let flow = flow.borrow().expect("the flow_body child must exist");
        flow.draw_text.color
    };

    install_and_draw(
        &mut cx,
        &ui,
        "Owned by [Customer](./customer.md)\n",
        "reading-widget-draw-link-colour",
    );
    let flow = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer.flow_body))
        .as_text_flow();
    let flow = flow.borrow().expect("the flow_body child must exist");

    assert_ne!(
        flow.draw_text.color, body_colour,
        "a link label must not be painted like ordinary prose"
    );
    assert!(
        flow.draw_text.color.w > 0.0,
        "the link colour must be visible, got {:?}",
        flow.draw_text.color
    );
    assert_eq!(
        flow.underline.value(),
        0,
        "the underline pushed for the link must be popped again"
    );
    assert!(
        flow.font_colors.is_empty(),
        "the link colour must not leak past the run that pushed it"
    );
}
