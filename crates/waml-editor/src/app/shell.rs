use super::*;

pub(super) fn open_overlay_contains(
    point: DVec2,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    (tree_state == DockState::Pinned && tree_rect.contains(point))
        || (inspector_state == DockState::Pinned && inspector_rect.contains(point))
}

pub(super) fn dock_toggle_icon(
    edge: crate::dock::DockEdge,
    state: DockState,
) -> crate::icons::Icon {
    use crate::dock::DockEdge;
    use crate::icons::Icon;

    match (edge, state == DockState::Flag) {
        (DockEdge::Left, true) => Icon::PanelLeftOpen,
        (DockEdge::Left, false) => Icon::PanelLeftClose,
        (DockEdge::Right, true) => Icon::PanelRightOpen,
        (DockEdge::Right, false) => Icon::PanelRightClose,
    }
}

/// The tree-column toggle exists twice: `tree_btn_dock` floats over the panel's
/// top-right corner, and `tree_btn` leads the tab row. The docked one has
/// nowhere to sit once the column collapses, so the tab-row twin takes over.
///
/// Returns `(dock_visible, row_slot_w)` -- the tab-row twin's SLOT WIDTH, not a
/// flag, because a flag is what made the handoff jerk. The tab row sits inside
/// `center_column`, whose left edge is the column's right edge, so the strip's
/// offset is `left_slot + row_slot_w`. Toggling the twin on at the end of the
/// collapse added its 32px in one frame and shoved every tab right.
///
/// Sizing the slot as `TREE_BTN_W * (1 - progress)` makes that sum
/// `TREE_BTN_W + (tree_w - TREE_BTN_W) * progress`: continuous the whole way,
/// landing on `tree_w` open (the twin gone, the docked one inside the column)
/// and on `TREE_BTN_W` collapsed (the twin leading the row). Nothing jumps.
///
/// Narrow is the exception: the panel floats instead of reserving, so
/// `left_slot` is always 0 and the strip never moves. The slot stays at full
/// width there and both toggles can be live at once -- the floating panel is
/// drawn over the tab row, so it covers the twin anyway.
pub(super) fn tree_toggle_layout(
    mounted: bool,
    narrow: bool,
    tree_body: f64,
    tree_w: f64,
) -> (bool, f64) {
    if !mounted {
        return (false, 0.0);
    }
    let dock_visible = tree_body > 0.0;
    if narrow {
        return (dock_visible, TREE_BTN_W);
    }
    let progress = if tree_w > 0.0 {
        (tree_body / tree_w).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (dock_visible, TREE_BTN_W * (1.0 - progress))
}

pub(super) fn should_dismiss_narrow_dock(
    point: DVec2,
    canvas_rect: Rect,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    canvas_rect.contains(point)
        && !open_overlay_contains(
            point,
            tree_state,
            tree_rect,
            inspector_state,
            inspector_rect,
        )
}

pub(super) fn project_document_header(
    chrome: crate::doc_view::DocumentHeaderChrome,
    breadcrumb: Option<Vec<crate::navigation::BreadcrumbSegment>>,
) -> (
    Vec<crate::navigation::BreadcrumbSegment>,
    Option<crate::icons::Icon>,
) {
    let segments = if chrome.breadcrumb {
        breadcrumb.unwrap_or_default()
    } else {
        Vec::new()
    };
    (segments, chrome.right_dock)
}

/// Footprint of the tab row's tree-column toggle: the `tree_btn` DSL `width`
/// (30, the caption burger's size) plus its 2px margin. `tree_btn_slot` is sized
/// against this, so the twin's seat and the width it costs the row are one
/// number (see `tree_toggle_layout`).
pub(super) const TREE_BTN_W: f64 = 32.0;

/// Height of `center_column`'s tab row (the `tab_row` DSL `height`). The
/// inspector docks to the right of the same column and must start BELOW this
/// band -- unlike the tree, which deliberately reaches up alongside it.
pub(super) const TAB_ROW_H: f64 = 32.0;
pub(super) const NARROW_ENTER_W: f64 = 640.0;
pub(super) const NARROW_EXIT_W: f64 = 680.0;

/// Width a dock panel's BODY draws at inside a host of `host_w`: the host
/// width less the splitter strip that shares the host with it. Floored at zero
/// so a mid-animation host narrower than the strip cannot go negative.
///
/// This is the spec's stated consequence of mounting the splitter inside the
/// panel host: the stored/persisted width is the whole column, and the body is
/// `SPLITTER_W` narrower than that number.
pub(super) fn panel_body_w(host_w: f64) -> f64 {
    (host_w - crate::panel_splitter::SPLITTER_W).max(0.0)
}

pub(super) fn next_narrow(narrow: bool, viewport_w: f64) -> bool {
    if narrow {
        viewport_w <= NARROW_EXIT_W
    } else {
        viewport_w < NARROW_ENTER_W
    }
}

impl App {
    /// Push the active diagram title into the switcher's trigger chip, falling
    /// back to another open diagram when a classifier is active.
    fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
        let title = self
            .documents
            .active_tab()
            .filter(|tab| tab.presentation.category == NavCategory::Diagram)
            .or_else(|| {
                self.documents
                    .tabs()
                    .iter()
                    .find(|tab| tab.presentation.category == NavCategory::Diagram)
            })
            .map(|t| t.title.clone())
            .unwrap_or_default();
        if let Some(mut switcher) = self
            .ui
            .widget(cx, ids!(diagram_switcher))
            .borrow_mut::<crate::diagram_switcher::DiagramSwitcher>()
        {
            switcher.set_current(cx, &title);
        }
    }

    /// Toggle the keybinding-hint overlay (U8), triggered by the tool
    /// dock's `Shortcuts` button or the `?` hotkey. Opening it closes every
    /// style-guide page first (one-overlay-open-at-a-time invariant).
    pub(super) fn toggle_shortcuts_overlay(&mut self, cx: &mut Cx) {
        let now_visible = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow::<crate::shortcuts_overlay::ShortcutsOverlay>()
            .map(|overlay| overlay.visible())
            .unwrap_or(false);
        let next = !now_visible;
        if next {
            self.close_page_overlays(cx);
        }
        self.set_shortcuts_overlay(cx, next);
    }

    /// Force the overlay's visibility (used by the `Escape` hotkey, which
    /// should only ever close it, never toggle it open).
    fn set_shortcuts_overlay(&mut self, cx: &mut Cx, visible: bool) {
        if let Some(mut overlay) = self
            .ui
            .widget(cx, ids!(shortcuts_overlay))
            .borrow_mut::<crate::shortcuts_overlay::ShortcutsOverlay>()
        {
            overlay.set_visible(cx, visible);
        }
    }

    /// Close the shortcuts overlay AND every style-guide page. Every open path
    /// calls this first, so exactly one overlay is ever visible.
    pub(super) fn close_page_overlays(&mut self, cx: &mut Cx) {
        self.set_shortcuts_overlay(cx, false);
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(fonts_overlay))
            .borrow_mut::<crate::fonts_overlay::FontsOverlay>()
        {
            o.set_visible(cx, false);
        }
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(icons_overlay))
            .borrow_mut::<crate::icons_overlay::IconsOverlay>()
        {
            o.set_visible(cx, false);
        }
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(colors_overlay))
            .borrow_mut::<crate::colors_overlay::ColorsOverlay>()
        {
            o.set_visible(cx, false);
        }
    }

    /// Close every overlay/page, then show the requested style-guide page.
    pub(super) fn open_page_overlay(&mut self, cx: &mut Cx, which: LogoCommand) {
        self.close_page_overlays(cx);
        if which == LogoCommand::Fonts {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(fonts_overlay))
                .borrow_mut::<crate::fonts_overlay::FontsOverlay>()
            {
                o.set_visible(cx, true);
            }
        } else if which == LogoCommand::Icons {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(icons_overlay))
                .borrow_mut::<crate::icons_overlay::IconsOverlay>()
            {
                o.set_visible(cx, true);
            }
        } else if which == LogoCommand::Colors {
            if let Some(mut o) = self
                .ui
                .widget(cx, ids!(colors_overlay))
                .borrow_mut::<crate::colors_overlay::ColorsOverlay>()
            {
                o.set_visible(cx, true);
            }
        }
    }

    pub(super) fn dock_states(&mut self, cx: &mut Cx) -> (DockState, DockState) {
        let tree = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.dock_state())
            .unwrap_or(DockState::Flag);
        let inspector = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .map(|panel| panel.dock_state())
            .unwrap_or(DockState::Flag);
        (tree, inspector)
    }

    pub(super) fn apply_dock_states(&mut self, cx: &mut Cx, tree: DockState, inspector: DockState) {
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            if panel.dock_state() != tree {
                if tree == DockState::Pinned {
                    panel.open_dock(cx);
                } else {
                    panel.close_dock(cx);
                }
            }
        }
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            if panel.dock_state() != inspector {
                if inspector == DockState::Pinned {
                    panel.open_dock(cx);
                } else {
                    panel.close_dock(cx);
                }
            }
        }
    }

    pub(super) fn route_narrow_dock_pointer(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        popup_was_open: bool,
    ) {
        if !self.narrow {
            return;
        }
        let (tree_state, inspector_state) = self.dock_states(cx);
        let tree_rect = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.drawn_rect(cx))
            .unwrap_or_default();
        let inspector_rect = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .map(|panel| panel.drawn_rect(cx))
            .unwrap_or_default();
        let canvas_rect = self.ui.widget(cx, ids!(canvas)).area().rect(cx);
        let contains = |point| {
            open_overlay_contains(
                point,
                tree_state,
                tree_rect,
                inspector_state,
                inspector_rect,
            )
        };
        match event {
            Event::MouseMove(e) => {
                self.pointer_in_narrow_dock = contains(e.abs);
            }
            Event::MouseDown(e) if e.button.is_primary() => {
                let inside = contains(e.abs);
                self.pointer_in_narrow_dock = inside;
                if !popup_was_open
                    && should_dismiss_narrow_dock(
                        e.abs,
                        canvas_rect,
                        tree_state,
                        tree_rect,
                        inspector_state,
                        inspector_rect,
                    )
                {
                    self.apply_dock_states(cx, DockState::Flag, DockState::Flag);
                }
            }
            _ => {}
        }
    }

    /// Reconcile responsive mode and panel state, then update reservation slots
    /// and overlay hosts together so one layout model owns all dock geometry.
    pub(super) fn sync_dock_slots(&mut self, cx: &mut Cx) {
        let viewport_w = self.window_bounds(cx).size.x;
        let next = next_narrow(self.narrow, viewport_w);
        if next != self.narrow {
            if let Some(mut root) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                if root.is_open_for(live_id!(doc_switcher)) {
                    root.close(cx);
                }
            }
            self.narrow = next;
            if self.narrow {
                let (tree, inspector) = self.dock_states(cx);
                let (tree, inspector) = crate::dock::narrow_entry_states(tree, inspector);
                self.apply_dock_states(cx, tree, inspector);
            }
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_narrow(cx, self.narrow);
            }
            cx.redraw_all();
        }

        let (tree_state, inspector_state) = self.dock_states(cx);
        let now = cx.seconds_since_app_start();
        self.tree_motion.request(
            if tree_state == DockState::Pinned {
                1.0
            } else {
                0.0
            },
            now,
        );
        self.inspector_motion.request(
            if inspector_state == DockState::Pinned {
                1.0
            } else {
                0.0
            },
            now,
        );
        let tree_value = self.tree_motion.value();
        let inspector_value = self.inspector_motion.value();
        let mut layout = crate::dock::responsive_layout(
            self.narrow,
            viewport_w,
            tree_value,
            inspector_value,
            self.dock_widths.tree_w,
            self.dock_widths.inspector_w,
        );
        // Springy give for a collapsed panel still under the finger. Applied to
        // the SLOT as well as the body so the sliver pushes the canvas rather
        // than floating over it, keeping the drag physically honest. Narrow
        // mode reserves no slots and hides the splitters, so it has no give.
        if !self.narrow {
            let (tree_rubber, inspector_rubber) = self.dock_rubber;
            layout.tree_body = crate::splitter::with_rubber(layout.tree_body, tree_rubber);
            layout.left_slot = crate::splitter::with_rubber(layout.left_slot, tree_rubber);
            layout.inspector_body =
                crate::splitter::with_rubber(layout.inspector_body, inspector_rubber);
            layout.right_slot = crate::splitter::with_rubber(layout.right_slot, inspector_rubber);
        }
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_presentation_visible(cx, crate::dock::presentation_visible(tree_value));
        }
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            panel.set_presentation_visible(cx, crate::dock::presentation_visible(inspector_value));
        }
        if layout != self.dock_layout {
            self.dock_layout = layout;
            if let Some(mut view) = self.ui.widget(cx, ids!(left_slot)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.left_slot);
            }
            if let Some(mut view) = self.ui.widget(cx, ids!(right_slot)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.right_slot);
            }
            if let Some(mut view) = self.ui.widget(cx, ids!(tree_host)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(layout.tree_body);
            }
            if let Some(mut view) = self
                .ui
                .widget(cx, ids!(inspector_host))
                .borrow_mut::<View>()
            {
                view.walk.width = Size::Fixed(layout.inspector_body);
            }
            // The panel bodies are runtime-Fixed to the host minus the
            // splitter strip -- NOT `Size::Fill`. A `Fill` sibling would be
            // deferred by makepad, leaving the splitter (which trails it in
            // `tree_host`) caching a pre-shift rect and silently unhittable.
            if let Some(mut view) = self.ui.widget(cx, ids!(tree_body)).borrow_mut::<View>() {
                view.walk.width = Size::Fixed(panel_body_w(layout.tree_body));
            }
            if let Some(mut view) = self
                .ui
                .widget(cx, ids!(inspector))
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                view.walk.width = Size::Fixed(panel_body_w(layout.inspector_body));
            }
            cx.redraw_all();
        }
        // Splitters are wide-mode only: in narrow mode the panel floats over
        // the center at a viewport-capped width, so there is no edge to drag.
        let splitters_visible = !self.narrow;
        for id in [ids!(tree_splitter), ids!(inspector_splitter)] {
            if let Some(mut view) = self
                .ui
                .widget(cx, id)
                .borrow_mut::<crate::panel_splitter::PanelSplitter>()
            {
                if view.visible != splitters_visible {
                    view.visible = splitters_visible;
                    cx.redraw_all();
                }
            }
        }
        // Both toggles carry the same glyph and active state -- they are one
        // control that changes seat, not two controls -- so only visibility
        // distinguishes them.
        let (dock_visible, row_slot_w) = tree_toggle_layout(
            self.tree_toggle_mounted,
            self.narrow,
            layout.tree_body,
            crate::tree_panel::PROJECT_TREE_W,
        );
        // The slot is what keeps the handoff smooth, so it is written every
        // frame of the motion -- outside the `dock_layout` change guard above,
        // which only fires when the reservation itself changes.
        if (row_slot_w - self.tree_btn_slot_w).abs() > 0.01 {
            self.tree_btn_slot_w = row_slot_w;
            if let Some(mut slot) = self.ui.widget(cx, ids!(tree_btn_slot)).borrow_mut::<View>() {
                slot.walk.width = Size::Fixed(row_slot_w);
            }
            cx.redraw_all();
        }
        // Visible whenever the slot has opened at all: the button is right-
        // aligned in a clipping slot, so it wipes out from behind the column's
        // edge instead of popping in at full size.
        let row_visible = row_slot_w > 0.5;
        for (id, visible) in [
            (ids!(tree_btn_dock), dock_visible),
            (ids!(tree_btn), row_visible),
        ] {
            let button = self.ui.widget(cx, id);
            button.set_visible(cx, visible);
            let button = button.as_icon_button();
            button.set_icon(
                cx,
                dock_toggle_icon(crate::dock::DockEdge::Left, tree_state),
            );
            button.set_active(cx, tree_state == DockState::Pinned);
        }
        let header_height = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
            .map(|mut header| {
                header.set_right_dock_icon(
                    cx,
                    dock_toggle_icon(crate::dock::DockEdge::Right, inspector_state),
                );
                header.set_right_dock_active(cx, inspector_state == DockState::Pinned);
                header.visible_height()
            })
            .unwrap_or(0.0);
        // The inspector docks against `center_column`, whose first row is the
        // tab strip -- so it always starts `TAB_ROW_H` down, and in narrow mode
        // clears the breadcrumb header below that as well. (The tree column is
        // the other side of this: it sits OUTSIDE `center_column` and
        // deliberately reaches up into the tab row's band.)
        let inspector_top =
            TAB_ROW_H + crate::dock::narrow_inspector_top(self.narrow, header_height);
        if let Some(mut view) = self
            .ui
            .widget(cx, ids!(inspector_host))
            .borrow_mut::<View>()
        {
            if (view.walk.margin.top - inspector_top).abs() > 0.5 {
                view.walk.margin.top = inspector_top;
                cx.redraw_all();
            }
        }
        if self.tree_motion.is_active() || self.inspector_motion.is_active() {
            self.dock_next_frame = cx.new_next_frame();
        }
        self.sync_chrome_seam(cx);
    }

    /// Seed the dock column widths from the project that just opened. Called
    /// once per `open_dir`; a project with no `.waml/settings.json` (or an
    /// unreadable one) lands on the compiled-in defaults.
    pub(super) fn load_dock_widths(&mut self, cx: &mut Cx, project_root: &std::path::Path) {
        self.dock_widths = crate::project_settings::load(project_root).dock;
        self.sync_dock_slots(cx);
    }

    /// Route the two splitters' drag actions. Wide mode only -- in narrow mode
    /// the panels float over the center at a viewport-capped width and the
    /// splitters are hidden.
    pub(super) fn observe_panel_splitters(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.narrow {
            return;
        }
        let tree = self.ui.widget(cx, ids!(tree_splitter)).as_panel_splitter();
        let inspector = self
            .ui
            .widget(cx, ids!(inspector_splitter))
            .as_panel_splitter();

        if let Some(x) = tree.dragged(actions) {
            self.apply_splitter_drag(cx, crate::dock::DockEdge::Left, x);
        }
        if let Some(x) = inspector.dragged(actions) {
            self.apply_splitter_drag(cx, crate::dock::DockEdge::Right, x);
        }
        // Persistence is on RELEASE only -- never per drag frame, which would
        // hammer the disk for the length of a gesture.
        if tree.released(actions) || inspector.released(actions) {
            // The spring lets go with the finger: whatever sliver was being
            // held out springs back flush, animated by the same DockMotion.
            self.dock_rubber = (0.0, 0.0);
            self.sync_dock_slots(cx);
            self.persist_dock_widths();
        }
    }

    /// One drag frame: run the pure decision function over the live viewport
    /// and dock state, then apply its outcome. Collapse and reopen go through
    /// the ordinary `DockEvent::Close`/`Open` transitions so `DockMotion`
    /// animates the snap and `DockState` stays the single source of truth for
    /// open versus closed.
    pub(super) fn apply_splitter_drag(
        &mut self,
        cx: &mut Cx,
        edge: crate::dock::DockEdge,
        pointer_x: f64,
    ) {
        use crate::dock::{DockEdge, DockEvent};
        use crate::splitter::{DockLimits, DragOutcome};

        let viewport_w = self.window_bounds(cx).size.x;
        let (tree_state, inspector_state) = self.dock_states(cx);
        let (limits, state, other_slot_w) = match edge {
            DockEdge::Left => (DockLimits::TREE, tree_state, self.dock_layout.right_slot),
            DockEdge::Right => (
                DockLimits::INSPECTOR,
                inspector_state,
                self.dock_layout.left_slot,
            ),
        };
        let collapsed = state != DockState::Pinned;
        let outcome =
            crate::splitter::drag(edge, limits, pointer_x, viewport_w, other_slot_w, collapsed);

        let set_width = |widths: &mut crate::project_settings::DockWidths, w: f64| match edge {
            DockEdge::Left => widths.tree_w = w,
            DockEdge::Right => widths.inspector_w = w,
        };
        let set_rubber = |rubber: &mut (f64, f64), r: f64| match edge {
            DockEdge::Left => rubber.0 = r,
            DockEdge::Right => rubber.1 = r,
        };
        let event = match outcome {
            DragOutcome::Width(w) => {
                set_width(&mut self.dock_widths, w);
                set_rubber(&mut self.dock_rubber, 0.0);
                None
            }
            DragOutcome::Collapse { rubber } => {
                set_rubber(&mut self.dock_rubber, rubber);
                Some(DockEvent::Close)
            }
            DragOutcome::Reopen(w) => {
                set_width(&mut self.dock_widths, w);
                set_rubber(&mut self.dock_rubber, 0.0);
                Some(DockEvent::Open)
            }
        };
        if let Some(event) = event {
            let (tree, inspector) = match edge {
                DockEdge::Left => (crate::dock::next(tree_state, event), inspector_state),
                DockEdge::Right => (tree_state, crate::dock::next(inspector_state, event)),
            };
            self.apply_dock_states(cx, tree, inspector);
        }
        self.sync_dock_slots(cx);
    }

    /// Write the current column widths to the open project's `.waml/`. No
    /// resolvable project root (an unsaved or browser-decoded model) simply
    /// skips persistence -- the drag still works for the session. A disk
    /// failure is logged and swallowed: losing a column width must never cost
    /// an edit.
    fn persist_dock_widths(&mut self) {
        let Some(root) = self.open_dir.clone() else {
            return;
        };
        let mut settings = crate::project_settings::load(&root);
        settings.dock = self.dock_widths;
        if let Err(error) = crate::project_settings::store(&root, &settings) {
            log!("failed to store dock widths for {root:?}: {error}");
        }
    }

    /// Push the launch-flag marks into `AgentMark`. Called at startup AND from
    /// `rehydrate`: the `T` theme toggle goes through `cx.request_live_edit()`
    /// -> `Apply::Reload`, which resets the widget's `#[rust]` state, so without
    /// the second call both marks vanish the first time an agent toggles the
    /// theme and the window silently becomes indistinguishable again.
    pub(super) fn apply_agent_marks(&mut self, cx: &mut Cx) {
        let badge = self.agent_badge.clone();
        let tint = self.agent_tint;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_marks(cx, badge, tint);
        }
    }

    /// Measure the title row and push its width to `AgentMark`, which draws
    /// across it with `draw_abs` (it is mounted zero-width, so it cannot learn
    /// the row width from its own turtle). Same measure-and-push shape as
    /// `sync_agent_row` feeding `AgentMark::set_row_width`.
    ///
    /// The min/max/close cluster shares this row, and the marker is a
    /// RIGHT-floated pill, so the cluster's width is subtracted -- otherwise the
    /// pill floats to the window edge and lands underneath the buttons.
    pub(super) fn sync_agent_row(&mut self, cx: &mut Cx) {
        if self.agent_badge.is_none() && self.agent_tint.is_none() {
            return;
        }
        let w = (self.ui.widget(cx, ids!(title_row)).area().rect(cx).size.x
            - self
                .ui
                .widget(cx, ids!(windows_buttons))
                .area()
                .rect(cx)
                .size
                .x)
            .max(0.0);
        if (w - self.agent_row_w).abs() <= 0.5 {
            return;
        }
        self.agent_row_w = w;
        if let Some(mut mark) = self
            .ui
            .widget(cx, ids!(agent_mark))
            .borrow_mut::<crate::agent_mark::AgentMark>()
        {
            mark.set_row_width(cx, w);
        }
    }

    /// Tell the chrome/content seam where the active doc tab meets it, so the
    /// hairline breaks there rather than running unbroken over the selected
    /// document (see `ChromeSeam::set_tab_break`).
    ///
    /// Measured off `DocTabs`' last-drawn card rects rather than computed from
    /// tab widths: the strip scrolls, elides and narrows, and only it knows
    /// where a card actually landed. That makes this a one-frame settle after
    /// any tab change, hence the change guard on the span itself.
    fn sync_chrome_seam(&mut self, cx: &mut Cx) {
        let span = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow::<crate::doc_tabs::DocTabs>()
            .and_then(|tabs| tabs.active_card_span());
        let changed = match (span, self.seam_break) {
            (None, None) => false,
            (Some((a0, a1, ac)), Some((b0, b1, bc))) => {
                (a0 - b0).abs() > 0.5 || (a1 - b1).abs() > 0.5 || ac != bc
            }
            _ => true,
        };
        if !changed {
            return;
        }
        self.seam_break = span;
        if let Some(mut seam) = self
            .ui
            .widget(cx, ids!(chrome_seam))
            .borrow_mut::<crate::chrome_seam::ChromeSeam>()
        {
            seam.set_tab_break(cx, span);
        }
    }

    /// Push diagram name / node count / zoom / active tool into the bottom
    /// statusbar. Snapshot values -- called at each sync point (tab switch,
    /// startup, tool-dock mode change), not live during a canvas drag.
    pub(super) fn sync_statusbar(&mut self, cx: &mut Cx) {
        let diagram_name = self
            .documents
            .tabs()
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_default();
        // Read the surface the ACTIVE document actually draws on. Reading only
        // `ClassDiagramSurface` left a behavior document reporting whatever the
        // last class diagram had — an 11-node activity showed "1 node", and the
        // zoom percentage was equally stale. The behavior surface is its own
        // widget (`behavior_canvas`), so the active tab's category selects it.
        let behavior_active = matches!(
            self.documents.active_tab().map(|t| t.presentation.category),
            Some(NavCategory::Behavior | NavCategory::Sequence)
        );
        let (node_count, zoom_pct) = if behavior_active {
            self.ui
                .widget(cx, ids!(behavior_canvas))
                .borrow_mut::<crate::canvas::BehaviorSurface>()
                .map(|b| (b.node_count(), b.zoom_pct()))
                .unwrap_or((0, 100))
        } else {
            self.ui
                .widget(cx, ids!(canvas))
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                .map(|c| (c.node_count(), c.zoom_pct()))
                .unwrap_or((0, 100))
        };
        let tool_label = self
            .ui
            .widget(cx, ids!(tool_dock))
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .map(|d| d.active().label())
            .unwrap_or("Select");
        if let Some(mut statusbar) = self
            .ui
            .widget(cx, ids!(statusbar))
            .borrow_mut::<crate::statusbar::Statusbar>()
        {
            statusbar.set_state(cx, diagram_name, node_count, zoom_pct, tool_label);
        }
    }

    /// Synchronize shell projections after the document host has completed a
    /// transition. Document content and view-specific chrome stay host-owned.
    pub(super) fn sync_document_shell(&mut self, cx: &mut Cx) {
        let active_concept = self
            .documents
            .active_tab()
            .map(|tab| tab.concept_id.clone());
        let chrome = self.documents.active_chrome().document_header;
        let breadcrumb = if chrome.breadcrumb {
            active_concept.as_deref().and_then(|concept_id| {
                crate::navigation::breadcrumb_for(
                    self.session.okf_analysis(),
                    self.session.uml_analysis(),
                    concept_id,
                )
            })
        } else {
            None
        };
        let (segments, right_dock) = project_document_header(chrome, breadcrumb);
        if let Some(mut header) = self
            .ui
            .widget(cx, ids!(document_header))
            .borrow_mut::<crate::document_header::DocumentHeader>()
        {
            header.set_segments(cx, segments);
            header.set_right_dock(cx, right_dock);
        }
        self.sync_history_controls(cx);
        if let Some(mut tree) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            tree.set_selected_key(cx, active_concept);
        }
        self.sync_diagram_switcher_current(cx);
        self.sync_statusbar(cx);
        self.sync_conflict_badge(cx);
    }

    pub(super) fn synchronize_session_change_projections(
        &mut self,
        cx: &mut Cx,
        change: &crate::editor_session::SessionChange,
    ) {
        if change.uml_changed {
            self.sync_document_shell(cx);
        }
        if change.navigation_changed {
            self.refresh_nav(cx, false);
        }
        if change.conflicts_changed {
            self.sync_conflict_badge(cx);
        }
    }

    /// Push the canvas's current conflict count onto the toolbar badge.
    pub(super) fn sync_conflict_badge(&mut self, cx: &mut Cx) {
        let n = self
            .ui
            .widget(cx, ids!(canvas))
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|c| c.conflict_count())
            .unwrap_or(0);
        if let Some(mut badge) = self
            .ui
            .widget(cx, ids!(conflict_badge))
            .borrow_mut::<crate::conflict_badge::ConflictBadge>()
        {
            badge.set_count(cx, n);
        }
    }

    /// Open the grouped, deletable conflict-error-list card, anchored under
    /// the toolbar badge. Shared by the badge click and the delete-refresh
    /// path (which re-anchors the still-open list after a row is removed).
    pub(super) fn open_conflict_list(
        &mut self,
        cx: &mut Cx,
        conflicts: Vec<crate::scene::SceneConflict>,
    ) {
        let btn = self.ui.widget(cx, ids!(conflict_badge)).area().rect(cx);
        let anchor = dvec2(
            btn.pos.x,
            btn.pos.y + btn.size.y + crate::popup::menu::MENU_GAP,
        );
        let bounds = self.window_bounds(cx);
        if let Some(mut pr) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            pr.show_at(
                cx,
                PopupSpec::Conflict {
                    tag: live_id!(conflict_list),
                    anchor,
                    bounds,
                    conflicts,
                },
            );
        }
    }
}
