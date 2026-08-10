use super::*;
use crate::doc_tabs::{DocTab, OpenTabs};
use crate::doc_view::{BodyWidgets, DocView, DocViewIdentity, ViewData};
use crate::dock::{DockEdge, DockMotion, DockState};
use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
use crate::document_host::DocumentCommand;
use crate::icons::{Icon, IconSet};
use crate::nav::NavState;
use crate::navigation::{BreadcrumbSegment, NavigationIntent, NavigationTarget, OpenDisposition};
use crate::tree::TreeKind;
use crate::view_history::{HistoryDirection, ViewAnchor, ViewLocation};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use waml_markdown_editor::layout::LayoutSnapshot;
use waml_markdown_editor::widget::MarkdownEditorWidgetRefExt;
use waml_syntax::{SourceText, TextChange, TextRange, TextSize};

mod completion;
mod menus;
mod navigation;
mod shell;
mod workspace;

fn navigation_app() -> (Cx, App) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    cx.widget_tree_mark_dirty(WidgetUid(0));
    let mut app = cx.with_vm(App::script_new_with_default);
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "# Sales\n\n* [Order](order.md)\n* [Customer](customer.md)\n",
        ),
        (
            "sales/order.md",
            "---\ntype: Runbook\ntitle: Order\n---\n# Order\n",
        ),
        (
            "sales/customer.md",
            "---\ntype: Runbook\ntitle: Customer\n---\n# Customer\n\n## History\nDetails\n",
        ),
        (
            "sales/next.md",
            "---\ntype: Runbook\ntitle: Next\n---\n# Next\n\n## Details\nRecorded\n",
        ),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    let mut project_tree = cx.with_vm(crate::tree_panel::ProjectTree::script_new_with_default);
    project_tree.set_view(
        &mut cx,
        crate::nav::view(
            app.session.okf_analysis(),
            app.session.uml_analysis(),
            &NavState::default(),
            &app.projection_mask,
            app.chain_limits,
        ),
    );
    let project_tree = WidgetRef::new_with_inner(Box::new(project_tree));
    let statusbar = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::statusbar::Statusbar::script_new_with_default),
    ));
    let document_header = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::document_header::DocumentHeader::script_new_with_default),
    ));
    let inspector = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::inspector_panel::Inspector::script_new_with_default),
    ));
    let mut ui = cx.with_vm(View::script_new_with_default);
    ui.children.push((live_id!(project_tree), project_tree));
    ui.children.push((live_id!(statusbar), statusbar));
    ui.children
        .push((live_id!(document_header), document_header));
    ui.children.push((live_id!(inspector), inspector));
    for id in [live_id!(history_back_btn), live_id!(history_forward_btn)] {
        let button = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        ui.children.push((id, button));
    }
    app.ui = WidgetRef::new_with_inner(Box::new(ui));
    (cx, app)
}

fn draw_document_header(cx: &mut Cx, app: &App, size: DVec2) -> Rect {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, "document-header-test");
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    draw_cx.begin_pass(&pass, None);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(size, Layout::default());
        app.ui.widget(&cx_2d, ids!(document_header)).draw_walk_all(
            &mut cx_2d,
            &mut Scope::empty(),
            Walk::fill(),
        );
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
    drop(draw_cx);
    app.ui.widget(cx, ids!(document_header)).area().rect(cx)
}
