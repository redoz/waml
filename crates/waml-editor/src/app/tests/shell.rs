use super::super::shell::{
    dock_toggle_icon, panel_body_w, tree_toggle_layout, DEFAULT_TAB_ROW_LEAD_W, TREE_BTN_W,
};
use super::*;

const TREE_W: f64 = crate::tree_panel::PROJECT_TREE_W;
/// What the logo costs before `tab_row` starts. The row sits in the
/// caption, whose left edge the tree column shares, so this plus the slot is
/// where the row's first control lands.
const LEAD: f64 = DEFAULT_TAB_ROW_LEAD_W;

/// With no model open the toggle does not show, and its slot costs the row
/// nothing.
#[test]
fn unmounted_tree_toggle_is_absent() {
    assert_eq!(
        tree_toggle_layout(false, false, TREE_W, TREE_W, LEAD),
        (false, 0.0)
    );
    assert_eq!(
        tree_toggle_layout(false, false, 0.0, TREE_W, LEAD),
        (false, 0.0)
    );
}

/// The two ends of the slot. Open, `LEAD + slot + TREE_BTN_W == TREE_W`: the
/// toggle's right edge is the tree column's right edge, so the burger trailing
/// it starts ON the split and the history pair follows. Collapsed, the slot
/// closes and the two lead the row. Same buttons in both.
#[test]
fn the_toggle_rides_the_column_edge_and_falls_back_to_the_row_head() {
    assert_eq!(
        tree_toggle_layout(true, false, TREE_W, TREE_W, LEAD),
        (true, TREE_W - LEAD - TREE_BTN_W)
    );
    assert_eq!(
        tree_toggle_layout(true, false, 0.0, TREE_W, LEAD),
        (true, 0.0)
    );
}

/// The jerk this replaced: the burger sits at `LEAD + row_slot_w + TREE_BTN_W`
/// (the toggle ahead of it), and that sum has to be continuous across the whole
/// collapse. It runs from `TREE_W` (open, the burger starting on the column's
/// right edge) to `LEAD + TREE_BTN_W` (collapsed) with no step anywhere -- in
/// particular none at the handoff, where the old two-seat arrangement faded a
/// second button in and added `TREE_BTN_W` in a single frame.
#[test]
fn tab_strip_offset_is_continuous_through_the_collapse() {
    let offset =
        |body: f64| LEAD + tree_toggle_layout(true, false, body, TREE_W, LEAD).1 + TREE_BTN_W;
    assert_eq!(offset(TREE_W), TREE_W);
    assert_eq!(offset(0.0), LEAD + TREE_BTN_W);

    let steps = 280;
    let mut prev = offset(TREE_W);
    for i in (0..=steps).rev() {
        let next = offset(TREE_W * i as f64 / steps as f64);
        // Monotonically closing, and never by more than one column's worth of
        // travel per step -- no frame moves the strip by the button's width.
        assert!(next <= prev + 0.001, "offset grew at body {i}");
        assert!(
            prev - next < 1.5,
            "offset jumped {} at body {i}",
            prev - next
        );
        prev = next;
    }
}

/// Narrow docks the panel as a floating overlay, so `left_slot` stays 0 and the
/// column has no edge to seat the button on: the slot closes and the button
/// leads the row throughout, whichever state the panel is in.
#[test]
fn narrow_leaves_the_toggle_leading_the_row() {
    assert_eq!(
        tree_toggle_layout(true, true, TREE_W, TREE_W, LEAD),
        (true, 0.0)
    );
    assert_eq!(
        tree_toggle_layout(true, true, 0.0, TREE_W, LEAD),
        (true, 0.0)
    );
}

#[test]
fn dock_toggle_glyphs_show_the_next_action() {
    assert_eq!(
        dock_toggle_icon(DockEdge::Left, DockState::Flag),
        Icon::PanelLeftOpen
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Left, DockState::Pinned),
        Icon::PanelLeftClose
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Right, DockState::Flag),
        Icon::PanelRightOpen
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Right, DockState::Pinned),
        Icon::PanelRightClose
    );
}

fn mounted_production_shell() -> (Cx, App) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let app = cx.with_vm(|vm| {
        let value = <App as AppMain>::script_mod(vm);
        let mut app = <App as ScriptNew>::script_from_value(vm, value);
        <App as AppMain>::after_new_from_script(vm, &mut app);
        app
    });
    (cx, app)
}

#[derive(Debug)]
struct DockAreas {
    body: Rect,
    left_slot: Rect,
    right_slot: Rect,
    tree_panel: Rect,
    header: Rect,
    center: Rect,
    inspector: Rect,
    inspector_panel: Rect,
}

fn draw_mounted_dock(cx: &mut Cx, app: &App, size: DVec2) -> DockAreas {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, "mounted-dock-test");
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    draw_cx.begin_pass(&pass, None);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(size, Layout::default());
        app.ui.widget(&cx_2d, ids!(dock_body)).draw_walk_all(
            &mut cx_2d,
            &mut Scope::empty(),
            Walk::fill(),
        );
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
    drop(draw_cx);

    let rect = |id_path| app.ui.widget(cx, id_path).area().rect(cx);
    DockAreas {
        body: rect(ids!(dock_body)),
        left_slot: rect(ids!(left_slot)),
        right_slot: rect(ids!(right_slot)),
        tree_panel: rect(ids!(project_tree)),
        header: rect(ids!(document_header)),
        center: rect(ids!(center_stack)),
        inspector: rect(ids!(inspector_host)),
        inspector_panel: rect(ids!(inspector)),
    }
}

fn configure_mounted_dock(
    cx: &mut Cx,
    app: &mut App,
    size: DVec2,
    tree: DockState,
    inspector: DockState,
    header_visible: bool,
) {
    let window_id = app
        .ui
        .window(cx, ids!(main_window))
        .window_id()
        .expect("production shell mounts main_window");
    cx.windows[window_id].window_geom.inner_size = size;
    cx.windows[window_id].window_geom.outer_size = size;
    app.apply_dock_states(cx, tree, inspector);
    app.tree_motion = DockMotion::new(if tree == DockState::Pinned { 1.0 } else { 0.0 });
    app.inspector_motion = DockMotion::new(if inspector == DockState::Pinned {
        1.0
    } else {
        0.0
    });
    let header_widget = app.ui.widget(cx, ids!(document_header));
    let mut header = header_widget
        .borrow_mut::<crate::document_header::DocumentHeader>()
        .expect("production shell mounts document_header");
    if header_visible {
        header.set_segments(
            cx,
            vec![BreadcrumbSegment {
                title: "Order".into(),
                target: NavigationTarget::Document {
                    concept_id: "sales/order".into(),
                    surface: None,
                    fragment: None,
                },
            }],
        );
        header.set_right_dock(cx, Some(Icon::PanelRight));
    } else {
        header.set_segments(cx, Vec::new());
        header.set_right_dock(cx, None);
    }
    drop(header);
    app.sync_dock_slots(cx);
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 0.5,
        "expected {actual} to be within 0.5px of {expected}"
    );
}

fn drawn_header_right_dock_active(cx: &mut Cx, app: &App) -> bool {
    let header = app.ui.widget(cx, ids!(document_header));
    let active = header
        .borrow::<crate::document_header::DocumentHeader>()
        .expect("production shell mounts document_header")
        .test_right_dock_active(cx)
        .expect("production shell draws the visible right button");
    active
}

fn header_right_dock_icon(cx: &mut Cx, app: &App) -> Option<Icon> {
    app.ui
        .widget(cx, ids!(document_header))
        .borrow::<crate::document_header::DocumentHeader>()
        .expect("production shell mounts document_header")
        .test_right_dock()
}

#[test]
fn mounted_dock_close_keeps_presented_geometry_until_motion_completes() {
    let size = dvec2(1_200.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        size,
        DockState::Pinned,
        DockState::Pinned,
        true,
    );

    app.apply_dock_states(&mut cx, DockState::Flag, DockState::Flag);
    app.sync_dock_slots(&mut cx);
    let closing = draw_mounted_dock(&mut cx, &app, size);

    assert_eq!(app.dock_states(&mut cx), (DockState::Flag, DockState::Flag));
    assert_near(app.dock_layout.left_slot, crate::tree_panel::PROJECT_TREE_W);
    assert_near(
        app.dock_layout.right_slot,
        crate::inspector_panel::INSPECTOR_W,
    );
    // The panel BODY is the column minus the splitter strip it shares the host
    // with (see `panel_body_w`); the slot/column width is unchanged.
    assert_near(
        closing.tree_panel.size.x,
        panel_body_w(crate::tree_panel::PROJECT_TREE_W),
    );
    assert_near(
        closing.inspector_panel.size.x,
        panel_body_w(crate::inspector_panel::INSPECTOR_W),
    );
    assert_eq!(
        header_right_dock_icon(&mut cx, &app),
        Some(Icon::PanelRightOpen)
    );
    assert!(!drawn_header_right_dock_active(&mut cx, &app));
    assert_ne!(app.dock_next_frame, NextFrame::default());
}

#[test]
fn mounted_dock_areas_follow_wide_and_narrow_production_layout() {
    let wide_size = dvec2(1_200.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        wide_size,
        DockState::Pinned,
        DockState::Pinned,
        true,
    );
    let wide = draw_mounted_dock(&mut cx, &app, wide_size);
    assert_near(wide.body.size.x, wide_size.x);
    assert_near(wide.left_slot.size.x, crate::tree_panel::PROJECT_TREE_W);
    assert_near(wide.right_slot.size.x, crate::inspector_panel::INSPECTOR_W);
    assert_near(app.dock_layout.left_slot, app.dock_layout.tree_body);
    assert_near(app.dock_layout.right_slot, app.dock_layout.inspector_body);
    assert_near(
        wide.header.pos.x,
        wide.left_slot.pos.x + wide.left_slot.size.x,
    );
    assert_near(
        wide.header.pos.x + wide.header.size.x,
        wide.right_slot.pos.x,
    );
    assert!(drawn_header_right_dock_active(&mut cx, &app));
    // The tab row lives in the caption, so the body's top edge is already below
    // it: tree column and inspector both start flush with it.
    assert_near(wide.tree_panel.pos.y, wide.body.pos.y);
    assert_near(wide.inspector.pos.y, wide.body.pos.y);

    let narrow_size = dvec2(560.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        narrow_size,
        DockState::Flag,
        DockState::Pinned,
        true,
    );
    let narrow_visible = draw_mounted_dock(&mut cx, &app, narrow_size);
    assert_near(
        narrow_visible.header.size.y,
        crate::document_header::DOCUMENT_HEADER_H,
    );
    assert!(
        narrow_visible.inspector.pos.y
            >= narrow_visible.header.pos.y + narrow_visible.header.size.y
    );

    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        narrow_size,
        DockState::Flag,
        DockState::Pinned,
        false,
    );
    let narrow_absent = draw_mounted_dock(&mut cx, &app, narrow_size);
    // With no breadcrumb header there is nothing left to clear: `center_column`
    // has no rows above the canvas stack, and the inspector docks flush.
    assert_near(narrow_absent.inspector.pos.y, narrow_absent.body.pos.y);
    assert_near(narrow_absent.center.pos.y, narrow_absent.body.pos.y);

    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        narrow_size,
        DockState::Flag,
        DockState::Flag,
        true,
    );
    draw_mounted_dock(&mut cx, &app, narrow_size);
    assert!(!drawn_header_right_dock_active(&mut cx, &app));
}

fn draw_tab_row(cx: &mut Cx, app: &App, size: DVec2) {
    let draw_event = DrawEvent {
        redraw_all: true,
        ..DrawEvent::default()
    };
    let pass = DrawPass::new_with_name(cx, "tab-row-test");
    let mut draw_list = DrawList2d::new(cx);
    let mut draw_cx = CxDraw::new(cx, &draw_event);
    draw_cx.begin_pass(&pass, None);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(size, Layout::default());
        app.ui.widget(&cx_2d, ids!(tab_row)).draw_walk_all(
            &mut cx_2d,
            &mut Scope::empty(),
            Walk::fill(),
        );
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
}

/// The history pair belongs to the TAB ROW, not the per-document header, and
/// sits between the tree-column toggle and the tab strip. Drawn in the collapsed
/// arrangement -- the toggle's tab-row seat, its slot opened to full width, as
/// `tree_toggle_layout` sizes it once the column is gone.
#[test]
fn mounted_history_buttons_lead_the_tab_strip_past_the_tree_column() {
    let size = dvec2(600.0, 32.0);
    let (mut cx, app) = mounted_production_shell();
    for id in [
        ids!(tree_btn),
        ids!(history_back_btn),
        ids!(history_forward_btn),
    ] {
        app.ui.widget(&cx, id).set_visible(&mut cx, true);
    }
    // The slot is an empty runtime-sized spacer AHEAD of the button (0 in the
    // DSL, so a collapsed column costs the row nothing). Give it a column's
    // worth here so the row is drawn in its open arrangement.
    if let Some(mut slot) = app
        .ui
        .widget(&cx, ids!(tree_btn_slot))
        .borrow_mut::<makepad_widgets::View>()
    {
        slot.walk.width = Size::Fixed(TREE_BTN_W);
    }

    draw_tab_row(&mut cx, &app, size);

    let tree = app.ui.widget(&cx, ids!(tree_btn)).area().rect(&cx);
    let back = app.ui.widget(&cx, ids!(history_back_btn)).area().rect(&cx);
    let forward = app
        .ui
        .widget(&cx, ids!(history_forward_btn))
        .area()
        .rect(&cx);
    let tabs = app.ui.widget(&cx, ids!(doc_tabs)).area().rect(&cx);

    assert_eq!(back.size.x, 30.0);
    assert_eq!(forward.size.x, 30.0);
    assert!(back.pos.x >= tree.pos.x + tree.size.x);
    assert_eq!(forward.pos.x, back.pos.x + back.size.x);
    assert!(forward.pos.x + forward.size.x <= tabs.pos.x);
    assert_eq!(back.pos.y, tree.pos.y);
}

#[test]
fn visible_mounted_document_header_is_client_area_but_collapsed_header_is_not() {
    let (mut cx, mut app) = navigation_app();
    app.narrow = true;
    let segment = BreadcrumbSegment {
        title: "Order".into(),
        target: NavigationTarget::Document {
            concept_id: "sales/order".into(),
            surface: None,
            fragment: None,
        },
    };
    {
        let header_widget = app.ui.widget(&cx, ids!(document_header));
        let mut header = header_widget
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .expect("test document header is mounted");
        header.set_segments(&mut cx, vec![segment]);
        header.set_right_dock(&mut cx, Some(Icon::PanelRight));
    }
    let header_rect = draw_document_header(&mut cx, &app, dvec2(360.0, 30.0));
    assert_eq!(
        header_rect.size.y,
        crate::document_header::DOCUMENT_HEADER_H
    );
    let response = Rc::new(Cell::new(WindowDragQueryResponse::Caption));
    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::WindowDragQuery(WindowDragQueryEvent {
            window_id: WindowId(0, 0),
            abs: header_rect.pos + header_rect.size * 0.5,
            response: response.clone(),
        }),
    );
    assert!(matches!(response.get(), WindowDragQueryResponse::Client));

    {
        let header_widget = app.ui.widget(&cx, ids!(document_header));
        let mut header = header_widget
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .expect("test document header is mounted");
        header.set_segments(&mut cx, Vec::new());
        header.set_right_dock(&mut cx, None);
    }
    assert_eq!(
        app.ui
            .widget(&cx, ids!(document_header))
            .borrow::<crate::document_header::DocumentHeader>()
            .expect("test document header is mounted")
            .visible_height(),
        0.0
    );
    response.set(WindowDragQueryResponse::Caption);
    AppMain::handle_event(
        &mut app,
        &mut cx,
        &Event::WindowDragQuery(WindowDragQueryEvent {
            window_id: WindowId(0, 0),
            abs: header_rect.pos + header_rect.size * 0.5,
            response: response.clone(),
        }),
    );
    assert!(matches!(response.get(), WindowDragQueryResponse::Caption));
}

/// A synthesized drag of the LEFT (tree) splitter moves the column and its
/// reservation slot together. `apply_splitter_drag` is the seam the widget's
/// `Dragged(pointer_x)` action feeds, so driving it directly is the same code
/// path a real pointer takes, minus the hit test.
#[test]
fn mounted_tree_splitter_drag_widens_the_left_slot() {
    let size = dvec2(1_200.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        size,
        DockState::Pinned,
        DockState::Pinned,
        true,
    );

    app.apply_splitter_drag(&mut cx, DockEdge::Left, 420.0);

    assert_near(app.dock_widths.tree_w, 420.0);
    assert_near(app.dock_layout.left_slot, 420.0);
    let drawn = draw_mounted_dock(&mut cx, &app, size);
    assert_near(drawn.left_slot.size.x, 420.0);
    assert_near(drawn.tree_panel.size.x, panel_body_w(420.0));
    // The inspector is untouched by the other edge's drag.
    assert_near(
        app.dock_widths.inspector_w,
        crate::inspector_panel::INSPECTOR_W,
    );
}

/// The right edge derives its width from the far side of the viewport, so the
/// same pointer x means a different column width.
#[test]
fn mounted_inspector_splitter_drag_narrows_the_right_slot() {
    let size = dvec2(1_200.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        size,
        DockState::Pinned,
        DockState::Pinned,
        true,
    );

    // 1200 - 950 = 250 wide.
    app.apply_splitter_drag(&mut cx, DockEdge::Right, 950.0);

    assert_near(app.dock_widths.inspector_w, 250.0);
    assert_near(app.dock_layout.right_slot, 250.0);
    let drawn = draw_mounted_dock(&mut cx, &app, size);
    assert_near(drawn.right_slot.size.x, 250.0);
    assert_near(drawn.inspector_panel.size.x, panel_body_w(250.0));
}

/// Dragging past `collapse` drives the panel to `Flag` MID-DRAG through the
/// ordinary `DockEvent::Close` transition, so `DockMotion` animates the snap
/// and `DockState` stays the single source of truth. The persisted width is
/// left at the last non-collapsed value.
#[test]
fn mounted_tree_splitter_drag_past_collapse_flags_the_panel() {
    let size = dvec2(1_200.0, 700.0);
    let (mut cx, mut app) = mounted_production_shell();
    configure_mounted_dock(
        &mut cx,
        &mut app,
        size,
        DockState::Pinned,
        DockState::Flag,
        true,
    );

    // Driven off the constants, not literals: these thresholds are a feel to be
    // retuned, and a test written in magic numbers would have to be rewritten
    // (or would quietly stop testing the band) every time they move.
    let l = crate::splitter::DockLimits::TREE;
    let mid_band = (l.collapse + l.min) * 0.5;

    // Inside the sticky band [collapse, min): still open, held at min.
    app.apply_splitter_drag(&mut cx, DockEdge::Left, mid_band);
    assert_eq!(app.dock_states(&mut cx).0, DockState::Pinned);
    assert_near(app.dock_widths.tree_w, l.min);

    // Past it: collapsed.
    app.apply_splitter_drag(&mut cx, DockEdge::Left, l.collapse - 20.0);
    assert_eq!(app.dock_states(&mut cx).0, DockState::Flag);
    // The width the panel will reopen at is the last OPEN one, not the
    // threshold it was dragged through.
    assert_near(app.dock_widths.tree_w, l.min);
    // While shut and still held, the panel shows springy give rather than
    // sitting flush -- and never more than the cap.
    assert!(app.dock_rubber.0 > 0.0);
    assert!(app.dock_rubber.0 <= crate::splitter::RUBBER_MAX_W);

    // Hysteresis: a collapsed panel needs strictly more travel back out than it
    // took to close, and only then reopens.
    app.apply_splitter_drag(&mut cx, DockEdge::Left, l.reopen - 5.0);
    assert_eq!(app.dock_states(&mut cx).0, DockState::Flag);
    app.apply_splitter_drag(&mut cx, DockEdge::Left, 260.0);
    assert_eq!(app.dock_states(&mut cx).0, DockState::Pinned);
    assert_near(app.dock_widths.tree_w, 260.0);
    // Reopening releases the spring.
    assert_eq!(app.dock_rubber.0, 0.0);
}
