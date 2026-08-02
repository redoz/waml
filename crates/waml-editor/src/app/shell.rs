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

/// Footprint of the caption's tree-column toggle: the `tree_btn` DSL `width`
/// (30, the burger's size) plus its 2px left margin, which seats it on the
/// burger's centreline one row above (see the `tree_btn` comment). Kept here
/// because `sync_tree_gap` has to subtract it from the tree column's width: the
/// button leads the row, so the spacer after it is short by exactly the button's
/// own footprint.
pub(super) const TREE_BTN_W: f64 = 32.0;
pub(super) const NARROW_ENTER_W: f64 = 640.0;
pub(super) const NARROW_EXIT_W: f64 = 680.0;

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
    pub(super) fn sync_diagram_switcher_current(&mut self, cx: &mut Cx) {
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
    pub(super) fn set_shortcuts_overlay(&mut self, cx: &mut Cx, visible: bool) {
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
        let layout = crate::dock::responsive_layout(
            self.narrow,
            viewport_w,
            tree_value,
            inspector_value,
            crate::tree_panel::PROJECT_TREE_W,
            crate::inspector_panel::INSPECTOR_W,
        );
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
            cx.redraw_all();
        }
        let tree_button = self.ui.widget(cx, ids!(tree_btn)).as_icon_button();
        tree_button.set_icon(
            cx,
            dock_toggle_icon(crate::dock::DockEdge::Left, tree_state),
        );
        tree_button.set_active(cx, tree_state == DockState::Pinned);
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
        let inspector_top = crate::dock::narrow_inspector_top(self.narrow, header_height);
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
        self.sync_tree_gap(cx, layout.left_slot);
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
    /// `sync_tree_gap` feeding `DocTabs::set_left_overshoot`.
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

    /// Size the caption's `tree_gap` so the tab STRIP's left edge lands on the
    /// tree column's right edge -- the invariant that makes the two-row caption
    /// read as one chrome mass: the tabs start exactly where `field_bg` steps in
    /// from full-width to column-width (the first card sits `TAB_LEFT_INSET`
    /// inside that). `[T]` itself leads the row and never moves; only the cards
    /// travel, because the control that moves them must not move itself.
    ///
    /// `tree_w` is the column's width in window coordinates (the body starts at
    /// x=0, so it is also the column's right edge). `tab_row` starts at x=0 and
    /// `[T]` occupies the first `TREE_BTN_W` of it, so the gap is what is left
    /// after the button; clamped at 0 so the collapsed
    /// state collapses the spacer instead of going negative and the strip sits
    /// immediately right of `[T]` (`TAB_LEFT_INSET` supplies the breathing gap).
    ///
    /// The row offset is read from the last-drawn `tab_row` rect rather than
    /// hardcoded, so a reshaped caption can't silently desync the cards from the
    /// column. That makes this a two-frame settle on the very first draw (the
    /// rect is zero until `tab_row` has been laid out once), hence the guard is
    /// on the computed gap rather than on `tree_w` alone.
    ///
    /// Same measured rects also drive the tab strip's top-rule left overshoot:
    /// `doc_tabs` no longer begins at the window's left edge, and the rule must
    /// reach back to `tab_row`'s left edge, which is the window edge in this
    /// full-width caption hierarchy.
    pub(super) fn sync_tree_gap(&mut self, cx: &mut Cx, tree_w: f64) {
        let row_x = self.ui.widget(cx, ids!(tab_row)).area().rect(cx).pos.x;
        let tabs_x = self.ui.widget(cx, ids!(doc_tabs)).area().rect(cx).pos.x;
        let overshoot = (tabs_x - row_x).max(0.0);
        if (overshoot - self.rule_overshoot).abs() > 0.5 {
            self.rule_overshoot = overshoot;
            if let Some(mut tabs) = self
                .ui
                .widget(cx, ids!(doc_tabs))
                .borrow_mut::<crate::doc_tabs::DocTabs>()
            {
                tabs.set_left_overshoot(cx, overshoot);
            }
        }
        let gap = (tree_w - TREE_BTN_W).max(0.0);
        if (gap - self.tree_gap_w).abs() <= 0.5 {
            return;
        }
        self.tree_gap_w = gap;
        // The first card's lead-in is a pure function of the gap, so it rides
        // the same change guard. Open column (gap > 0): flush, so the card's
        // left flank lands on the tree column's right edge -- the canvas's left
        // edge -- and the chrome has no step in it. Collapsed (gap == 0): the
        // strip butts against `[T]`, which needs the breathing room back.
        if let Some(mut tabs) = self
            .ui
            .widget(cx, ids!(doc_tabs))
            .borrow_mut::<crate::doc_tabs::DocTabs>()
        {
            let inset = if gap > 0.5 {
                0.0
            } else {
                crate::doc_tabs::TAB_LEFT_INSET
            };
            tabs.set_lead_inset(cx, inset);
        }
        // Same seam as the dock slots: no live-DSL setter in this fork, so
        // mutate the public `walk` field and force a full relayout (the
        // `flow: Right` row must reflow, not just this child).
        if let Some(mut spacer) = self.ui.widget(cx, ids!(tree_gap)).borrow_mut::<View>() {
            spacer.walk.width = Size::Fixed(gap);
        }
        cx.redraw_all();
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
            self.nav_kinds = crate::nav::kinds_in_model(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
            );
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
