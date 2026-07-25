//! `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-
//! picker + tool dock + selection toolbar). Real behavior lands in Task 3.

use makepad_widgets::*;
use std::collections::HashSet;
use waml::model::Model;

use crate::canvas::ConstraintVisibility;
use crate::doc_view::{BodyWidgets, DocView, PopupRequest, ViewOutcome};
use crate::icons::Icon;
use crate::inspector::{diagram_elements, subject_from, Subject};
use crate::popup::base::{PopupItem, PopupResult};
use crate::scene::build_scene;

/// Strip a defensive `.md` tail from a node/diagram key.
fn strip_md_key(s: &str) -> String {
    s.strip_suffix(".md").unwrap_or(s).to_string()
}

/// The canvas veil mode a view-bar action should drive, or `None` when it
/// drives no veil change (the camera one-shots, handled inline in `handle`,
/// and the hidden-borders toggle, which `show_hidden_borders_for` owns). Pure
/// so the on/off mapping is testable.
fn constraint_vis_for(action: &crate::view_bar::ViewBarAction) -> Option<ConstraintVisibility> {
    match action {
        crate::view_bar::ViewBarAction::Toggled(
            crate::view_bar::ViewOption::ShowConstraints,
            on,
        ) => Some(if *on {
            ConstraintVisibility::Selected
        } else {
            ConstraintVisibility::None
        }),
        _ => None,
    }
}

/// The view bar's `ShowConstraints` lit state for a canvas veil mode -- the
/// inverse of `constraint_vis_for`, used to push canvas state *back* into the
/// bar so the two can re-converge if they ever drift.
fn show_constraints_for(vis: ConstraintVisibility) -> bool {
    match vis {
        ConstraintVisibility::None => false,
        ConstraintVisibility::Selected => true,
    }
}

/// The canvas hidden-group-border x-ray state a view-bar action should drive,
/// or `None` when it drives no x-ray change. Sibling of `constraint_vis_for`;
/// pure for the same reason.
fn show_hidden_borders_for(action: &crate::view_bar::ViewBarAction) -> Option<bool> {
    match action {
        crate::view_bar::ViewBarAction::Toggled(
            crate::view_bar::ViewOption::ShowHiddenBorders,
            on,
        ) => Some(*on),
        _ => None,
    }
}

#[derive(Default)]
pub struct ClassDiagramView {
    /// The base tab's current diagram identity, pushed by the shell before
    /// every `sync`/`handle` (see `App::sync_active_tab`'s `set_active` call).
    active_key: String,
    active_title: String,
    /// Node keys whose card body is expanded. Per-tab live state, moved off
    /// the shell in Task 3. Cleared when the diagram changes.
    expanded: HashSet<String>,
}

impl ClassDiagramView {
    pub fn new() -> ClassDiagramView {
        ClassDiagramView::default()
    }

    /// The shell resolves the base tab's key/title and pushes them here before
    /// `sync`/`handle` run -- the view has no other way to know which diagram
    /// it is currently showing.
    pub fn set_active(&mut self, key: String, title: String) {
        self.active_key = key;
        self.active_title = title;
    }

    /// Re-solve the active diagram into the canvas, holding the camera. The
    /// shell calls this after applying an authored layout op (drag-to-place) so
    /// the placed node moves without the view re-framing. Mirrors the
    /// `ToggleExpand` re-solve tail (`update_scene`, not `set_scene`).
    pub fn resolve_active(&self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.active_key) {
            let (scene, diags) = build_scene(model, diagram, &self.expanded);
            for d in &diags {
                log!("diagnostic: {d:?}");
            }
            if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>() {
                canvas.update_scene(cx, scene);
            }
        }
    }

    /// Feed the inspector's element-picker the current diagram's contents.
    fn sync_inspector_elements(
        &self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        diagram_key: &str,
        diagram_title: &str,
        node_keys: &[String],
    ) {
        let rows = diagram_elements(model, diagram_key, diagram_title, node_keys);
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_diagram_elements(cx, model, rows);
        }
    }
}

impl DocView for ClassDiagramView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        let built = model
            .diagrams
            .iter()
            .find(|d| d.key == self.active_key)
            .map(|d| build_scene(model, d, &self.expanded));
        if let Some((scene, diags)) = built {
            for d in &diags {
                log!("diagnostic: {d:?}");
            }
            let node_keys: Vec<String> = scene.nodes.iter().map(|n| n.key.clone()).collect();
            if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>() {
                canvas.set_scene(cx, scene);
            }
            let active_key = self.active_key.clone();
            let active_title = self.active_title.clone();
            self.sync_inspector_elements(cx, body, model, &active_key, &active_title, &node_keys);
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::None);
        }
        if let Some(mut toolbar) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        {
            toolbar.set_selection(cx, None);
        }
        // A fresh scene clears the selection (`set_scene`), so the button starts
        // dim on every diagram activation.
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, false);
        }
        // Re-converge the view bar's lit state onto the canvas. The canvas owns
        // both the veil mode and the hidden-border x-ray; the bar only caches
        // them, and its own click handler is otherwise the sole writer -- so
        // this is the one path that can heal a bar<->canvas disagreement (tab
        // activation, model reload).
        let canvas_state = body
            .canvas(cx)
            .borrow::<crate::canvas::GraphCanvas>()
            .map(|canvas| (canvas.constraint_vis(), canvas.show_hidden_borders()));
        if let Some((vis, hidden)) = canvas_state {
            if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
                bar.set_show_constraints(cx, show_constraints_for(vis));
                bar.set_show_hidden_borders(cx, hidden);
            }
        }
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        model: &Model,
    ) -> ViewOutcome {
        let mut out = ViewOutcome::default();

        // Keep the view bar's fit-to-selection button in step with the canvas
        // selection. `GraphCanvas` mutates `selected_key` in `handle_event`,
        // which runs before `Event::Actions` is dispatched, so this reads the
        // selection as of THIS batch. Cheap: the setter redraws only on change.
        let has_selection = body
            .canvas(cx)
            .borrow::<crate::canvas::GraphCanvas>()
            .is_some_and(|c| c.has_selection());
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, has_selection);
        }

        // Inline-edit commit: inspector emits `Edited(subject_key)`.
        if let Some(key) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions))
        {
            out.promote_subject = Some(key);
            return out;
        }

        // Element-picker: the SelectBox asked to open its flyout.
        if let Some((anchor_rect, min_width, items)) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.take_open_request(cx, actions))
        {
            out.popup = Some(PopupRequest::ElementPicker {
                anchor_rect,
                min_width,
                items,
            });
            return out;
        }

        // Reference-card navigation: a member/association card was clicked.
        // Repoint the inspector AND select the node on the canvas (edge keys
        // repoint only -- no node to select).
        if let Some((key, kind)) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|mut inspector| inspector.navigate(cx, actions))
        {
            if let Some(subject) = subject_from(&key, kind) {
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, subject);
                }
                if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.select_by_key(cx, &key);
                }
            }
            return out;
        }

        // Tool dock: mode clicks update their own highlight; ModeChanged
        // re-snaps the statusbar. Other actions stay mock `log!` no-ops.
        if let Some(action) = body
            .tool_dock(cx)
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .and_then(|dock| dock.dock_action(actions))
        {
            match action {
                crate::tool_dock::ToolDockAction::ModeChanged(_) => out.statusbar_dirty = true,
                other => log!("tool dock: {other:?}"),
            }
            return out;
        }

        // View bar: `ShowConstraints` drives the canvas veil mode,
        // `ShowHiddenBorders` the group x-ray, and the four camera one-shots
        // are thin wrappers over the `Camera` API on `GraphCanvas`.
        if let Some(action) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
            .and_then(|bar| bar.view_bar_action(actions))
        {
            // Both toggles route through their own pure mapper; at most one of
            // them claims a given action, so a single borrow serves both.
            let vis = constraint_vis_for(&action);
            let hidden = show_hidden_borders_for(&action);
            if vis.is_none() && hidden.is_none() {
                if let crate::view_bar::ViewBarAction::Triggered(opt) = action {
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        match opt {
                            crate::view_bar::ViewOption::ZoomIn => {
                                canvas.zoom_step(cx, crate::canvas::ZOOM_STEP)
                            }
                            crate::view_bar::ViewOption::ZoomOut => {
                                canvas.zoom_step(cx, 1.0 / crate::canvas::ZOOM_STEP)
                            }
                            crate::view_bar::ViewOption::FitToSize => canvas.fit_to_scene(cx),
                            crate::view_bar::ViewOption::FitToSelection => {
                                canvas.fit_to_selection(cx)
                            }
                            // The toggles never arrive as `Triggered`.
                            _ => {}
                        }
                    }
                } else {
                    log!("view bar: {action:?}");
                }
            } else if let Some(mut canvas) =
                body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
            {
                if let Some(vis) = vis {
                    canvas.set_constraint_vis(cx, vis);
                }
                if let Some(on) = hidden {
                    canvas.set_show_hidden_borders(cx, on);
                }
            }
            return out;
        }

        // Canvas pointer actions.
        let canvas_action = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .and_then(|c| c.canvas_action(actions));
        match canvas_action {
            Some(crate::canvas::GraphCanvasAction::NodeMenu { abs, key }) => {
                // Select-on-right-click: point the inspector at the node (the
                // same call `NodeSelect` makes).
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, Subject::Classifier(key.clone()));
                }
                // Gather the diagram's per-node context items (empty for now).
                let context = body
                    .canvas(cx)
                    .borrow::<crate::canvas::GraphCanvas>()
                    .map(|c| c.context_items(&Subject::Classifier(key.clone())))
                    .unwrap_or_default();
                out.popup = Some(PopupRequest::NodeContextMenu {
                    anchor: abs,
                    key,
                    context,
                });
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::NodeSelect { key }) => {
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, Subject::Classifier(key));
                }
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::NodeDeselect) => {
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, Subject::None);
                }
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::ToggleExpand { key }) => {
                if !self.expanded.remove(&key) {
                    self.expanded.insert(key);
                }
                // Re-solve the current diagram with the updated set; update_scene
                // holds the camera and re-resolves the selection by key.
                if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.active_key) {
                    let (scene, diags) = build_scene(model, diagram, &self.expanded);
                    for d in &diags {
                        log!("diagnostic: {d:?}");
                    }
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.update_scene(cx, scene);
                    }
                }
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::DialDismiss) => {
                out.popup = Some(PopupRequest::Dismiss);
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::CompassArmed {
                subject_key,
                reference_key,
                center,
            }) => {
                // The dial just armed on a (new) target: speculatively solve each
                // zone's placement against the active diagram. The solve yields
                // both the conflict verdict (redden the zones the solver would
                // reject) and the candidate layout itself, which the canvas
                // animates to on hover -- so previewing costs no extra solve.
                if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.active_key) {
                    let subject = strip_md_key(&subject_key);
                    let reference = strip_md_key(&reference_key);
                    let mut red = Vec::new();
                    let mut layouts = Vec::new();
                    for z in crate::canvas::COMPASS_ZONES {
                        if let Some(dir) = crate::canvas::zone_placed(z).dir {
                            let (conflict, nodes) = crate::scene::placement_preview(
                                model,
                                diagram,
                                &subject,
                                &reference,
                                dir,
                                &self.expanded,
                            );
                            if conflict {
                                red.push(z);
                            }
                            layouts.push((z, nodes));
                        }
                    }
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_conflict_zones(cx, red.clone());
                        canvas.set_zone_layouts(cx, layouts);
                    }
                    // Pop the dial itself: the shared radial, one wedge per
                    // zone in its own clockwise-from-12 order, a zone the
                    // solver would reject drawn as a danger wedge. Each wedge
                    // carries a directional-arrow glyph (`zone_arrow`) pointing
                    // the way it would place the dragged node -- no text label.
                    let items = crate::canvas::DIAL_ZONES
                        .into_iter()
                        .map(|z| PopupItem {
                            id: crate::canvas::zone_id(z),
                            label: String::new(),
                            icon: Some(crate::canvas::zone_arrow(z)),
                            // A direction the solver would reject ships DISABLED:
                            // an inert grey dead-zone that can't preview or commit,
                            // so the diagram is never dropped into conflict. The
                            // disabled grey look overrides `danger`, so the old red
                            // hue is redundant here.
                            danger: false,
                            enabled: !red.contains(&z),
                        })
                        .collect();
                    out.popup = Some(PopupRequest::PlaceDial { center, items });
                }
                return out;
            }
            _ => {}
        }

        // Selection toolbar: Delete only acts on a classifier preview (no-op
        // here); New Diagram is a mock no-op.
        if let Some(action) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
            .and_then(|toolbar| toolbar.toolbar_action(actions))
        {
            match action {
                crate::selection_toolbar::SelectionToolbarAction::Delete => {}
                crate::selection_toolbar::SelectionToolbarAction::NewDiagram => {
                    log!("selection toolbar: New Diagram (mock no-op)");
                }
                _ => {}
            }
            return out;
        }

        out
    }

    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        // Element-picker: any close clears the box's active state; a node
        // commit repoints the inspector (inspector-local -- no tab, no canvas
        // move).
        if tag == live_id!(element_picker) {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.on_picker_closed(cx, model, result);
            }
            return ViewOutcome::default();
        }
        // Drag-to-place dial: a committed wedge authors the `## Layout`
        // placement for the dragged (subject) node relative to the drop target
        // (reference). The view only emits the Op; the shell owns the Model and
        // re-solves. Slugs and the diagram id are bare -- strip a `.md` tail
        // defensively. A dismiss (hub, Escape, out of reach) authors nothing;
        // the canvas puts the committed layout back on its own.
        let mut out = ViewOutcome::default();
        if tag == live_id!(place_dial) {
            let zone = match result {
                PopupResult::Invoked(id) => crate::canvas::zone_of_id(id),
                PopupResult::Dismissed => None,
            };
            let placement = zone.and_then(|z| {
                body.canvas(cx)
                    .borrow::<crate::canvas::GraphCanvas>()
                    .and_then(|c| c.placement_for(z))
            });
            if let Some(p) = placement {
                let strip_md = |s: &str| s.strip_suffix(".md").unwrap_or(s).to_string();
                out.ops.push(waml::ops::Op::PlaceSet {
                    diagram: strip_md(&self.active_key),
                    subject_title: p.subject_title,
                    subject_slug: strip_md(&p.subject_key),
                    reference_title: p.reference_title,
                    reference_slug: strip_md(&p.reference_key),
                    directions: p.directions,
                });
            }
        }
        // node_menu currently only `log!`s on commit -- kept in the shell for
        // now.
        out
    }

    fn on_popup_armed(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        _model: &Model,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> ViewOutcome {
        // The dial's armed wedge drives the live layout preview. The candidate
        // layouts were already solved at arm time, so this costs no solve.
        if tag == live_id!(place_dial) {
            let zone = id.and_then(crate::canvas::zone_of_id);
            if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>() {
                canvas.preview_zone(cx, zone);
            }
        }
        ViewOutcome::default()
    }

    fn wants_tooldock(&self) -> bool {
        true
    }

    fn wants_view_bar(&self) -> bool {
        true
    }

    /// The shared `inspector` widget: this view feeds it the diagram's element
    /// picker, so its caption toggle wears the properties sliders glyph.
    fn right_dock(&self) -> Option<Icon> {
        Some(Icon::SlidersHorizontal)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{constraint_vis_for, show_constraints_for, show_hidden_borders_for};
    use crate::canvas::ConstraintVisibility;
    use crate::view_bar::{ViewBarAction, ViewOption};

    #[test]
    fn constraints_toggle_drives_the_veil_mode() {
        assert_eq!(
            constraint_vis_for(&ViewBarAction::Toggled(ViewOption::ShowConstraints, true)),
            Some(ConstraintVisibility::Selected),
            "toggle ON must light the sticky-selection veil"
        );
        assert_eq!(
            constraint_vis_for(&ViewBarAction::Toggled(ViewOption::ShowConstraints, false)),
            Some(ConstraintVisibility::None),
            "toggle OFF must clear every veil"
        );
    }

    #[test]
    fn other_view_bar_actions_drive_no_veil_change() {
        assert_eq!(
            constraint_vis_for(&ViewBarAction::Toggled(ViewOption::ShowHiddenBorders, true)),
            None
        );
        for opt in ViewOption::ALL.iter().filter(|o| !o.is_toggle()) {
            assert_eq!(
                constraint_vis_for(&ViewBarAction::Triggered(*opt)),
                None,
                "{opt:?} is a camera one-shot, not a veil change"
            );
        }
        assert_eq!(constraint_vis_for(&ViewBarAction::None), None);
    }

    #[test]
    fn hidden_borders_toggle_drives_the_group_xray() {
        assert_eq!(
            show_hidden_borders_for(&ViewBarAction::Toggled(ViewOption::ShowHiddenBorders, true)),
            Some(true),
            "toggle ON must light the x-ray"
        );
        assert_eq!(
            show_hidden_borders_for(&ViewBarAction::Toggled(
                ViewOption::ShowHiddenBorders,
                false
            )),
            Some(false),
            "toggle OFF must clear the x-ray"
        );
    }

    #[test]
    fn other_view_bar_actions_drive_no_xray_change() {
        assert_eq!(
            show_hidden_borders_for(&ViewBarAction::Toggled(ViewOption::ShowConstraints, true)),
            None
        );
        for opt in ViewOption::ALL.iter().filter(|o| !o.is_toggle()) {
            assert_eq!(
                show_hidden_borders_for(&ViewBarAction::Triggered(*opt)),
                None,
                "{opt:?} is a camera one-shot, not an x-ray change"
            );
        }
        assert_eq!(show_hidden_borders_for(&ViewBarAction::None), None);
    }

    #[test]
    fn the_two_toggle_mappers_never_claim_the_same_action() {
        // `handle` applies both mappers to one action and only logs when both
        // decline; if they ever overlapped, one click would drive two pieces of
        // canvas state.
        let mut actions = vec![ViewBarAction::None];
        for opt in ViewOption::ALL {
            actions.push(ViewBarAction::Triggered(opt));
            actions.push(ViewBarAction::Toggled(opt, true));
            actions.push(ViewBarAction::Toggled(opt, false));
        }
        for action in &actions {
            assert!(
                constraint_vis_for(action).is_none() || show_hidden_borders_for(action).is_none(),
                "{action:?} is claimed by both mappers"
            );
        }
    }

    #[test]
    fn veil_mode_round_trips_through_the_bars_lit_state() {
        // `sync` mirrors the canvas mode back onto the bar, so the two maps
        // must be exact inverses or an activation would flip the veil.
        for vis in [ConstraintVisibility::None, ConstraintVisibility::Selected] {
            let on = show_constraints_for(vis);
            assert_eq!(
                constraint_vis_for(&ViewBarAction::Toggled(ViewOption::ShowConstraints, on)),
                Some(vis),
                "{vis:?} must survive the canvas->bar->canvas round trip"
            );
        }
    }
}
