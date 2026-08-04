//! Mounts `MarkdownViewer` and draws it, to catch drawing regressions no
//! pure-model test can see (a `script_mod!` typo that silently drops the
//! `TextFlow` child, a layout that collapses to zero height, ...).

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{build_reading_document, MarkdownViewerWidgetRefExt};
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

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
