//! `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-
//! picker + tool dock + selection toolbar). Real behavior lands in Task 3.

use makepad_widgets::*;
use std::collections::HashSet;
use waml::model::Model;

use crate::canvas::ConstraintVisibility;
use crate::diagram_display::resolve_display;
use crate::diagram_properties::{DiagramPropertiesAction, DiagramPropertiesWidgetRefExt};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, PopupRequest,
    ViewData, ViewOutcome,
};
use crate::editor_session::SessionChange;
use crate::icons::Icon;
use crate::inspector::{diagram_elements, subject_from, Subject};
use crate::popup::base::{PopupItem, PopupResult};
use crate::scene::build_scene;
use crate::view_history::ViewAnchor;
use waml::uml::Op;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagramRefresh {
    PreserveCamera,
    None,
}

fn refresh_for(change: SessionChange) -> DiagramRefresh {
    if change.uml_changed {
        DiagramRefresh::PreserveCamera
    } else {
        DiagramRefresh::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ClassDiagramMode {
    #[default]
    Canvas,
    Properties,
}

impl ClassDiagramMode {
    fn toggle_properties(&mut self) {
        *self = match self {
            Self::Canvas => Self::Properties,
            Self::Properties => Self::Canvas,
        };
    }

    fn deactivate(&mut self) {
        *self = Self::Canvas;
    }

    fn properties_visible(self) -> bool {
        self == Self::Properties
    }

    fn accepts_canvas_actions(self) -> bool {
        self == Self::Canvas
    }
}

pub struct ClassDiagramView {
    key: String,
    /// Node keys whose card body is expanded. Per-tab live state, moved off
    /// the shell in Task 3. Cleared when the diagram changes.
    expanded: HashSet<String>,
    mode: ClassDiagramMode,
}

impl ClassDiagramView {
    pub fn new(key: String) -> ClassDiagramView {
        ClassDiagramView {
            key,
            expanded: HashSet::new(),
            mode: ClassDiagramMode::default(),
        }
    }

    fn apply_tool_action(&mut self, action: crate::tool_dock::ToolDockAction) -> bool {
        if matches!(
            action,
            crate::tool_dock::ToolDockAction::Triggered(crate::tool_dock::Tool::DiagramProps)
        ) {
            self.mode.toggle_properties();
            true
        } else {
            false
        }
    }

    fn properties_actions_outcome(
        &mut self,
        actions: impl IntoIterator<Item = DiagramPropertiesAction>,
    ) -> ViewOutcome {
        let mut outcome = ViewOutcome::default();
        let mut ops = Vec::new();
        let mut close = false;
        let mut description_only = true;
        for action in actions {
            if action == DiagramPropertiesAction::Close {
                close = true;
            } else if action == DiagramPropertiesAction::BreakEditMergeGroup {
                outcome.break_merge_group = true;
            } else {
                let (title, description, clear_description, display) = match action {
                    DiagramPropertiesAction::DisplayChanged(display) => {
                        description_only = false;
                        (None, None, false, Some(display))
                    }
                    DiagramPropertiesAction::DescriptionChanged(description) => {
                        let clear_description = description.is_none();
                        (None, description, clear_description, None)
                    }
                    DiagramPropertiesAction::BreakEditMergeGroup
                    | DiagramPropertiesAction::Close => unreachable!(),
                };
                ops.push(Op::DiagramSet {
                    key: self.key.clone(),
                    title,
                    description,
                    clear_description,
                    display,
                });
            }
        }
        if !ops.is_empty() {
            let merge_key =
                (description_only && ops.len() == 1).then(|| crate::editor_history::EditMergeKey {
                    document: crate::view_history::DocumentLocator::primary(self.key.clone()),
                    control: "diagram.description".into(),
                    kind: crate::editor_history::EditMergeKind::Continuous,
                    span: None,
                });
            outcome.edit = Some(crate::document::EditIntent {
                edit: waml::edit::PendingEdit::new(waml::uml::Batch(ops)),
                label: "Change diagram properties".into(),
                merge_key,
                after_location: None,
            });
        }
        if close {
            self.mode.deactivate();
            outcome.break_merge_group = true;
        }
        outcome
    }

    fn show_mode(&self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_diagram_properties_visible(cx, self.mode.properties_visible());
        body.apply_chrome(cx, self.chrome());
    }

    fn clear_properties_focus(cx: &mut Cx) {
        cx.set_key_focus(Area::Empty);
        cx.hide_text_ime();
    }

    fn return_to_canvas(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let leaving_properties = self.mode.properties_visible();
        self.mode.deactivate();
        if leaving_properties {
            Self::clear_properties_focus(cx);
        }
        self.show_mode(cx, body);
    }

    fn sync_properties(&self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        let Some(diagram) = model
            .diagrams
            .iter()
            .find(|diagram| diagram.key == self.key)
        else {
            return;
        };
        body.diagram_properties(cx)
            .as_diagram_properties()
            .set_diagram(
                cx,
                diagram.description.as_deref(),
                &resolve_display(&diagram.display),
            );
    }

    fn update_scene(&self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        self.sync_properties(cx, body, model);
        if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.key) {
            let (scene, diagnostics) = build_scene(
                model,
                diagram,
                resolve_display(&diagram.display),
                &self.expanded,
            );
            let node_keys: Vec<String> = scene.nodes.iter().map(|node| node.key.clone()).collect();
            for diagnostic in &diagnostics {
                log!("diagnostic: {diagnostic:?}");
            }
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            {
                canvas.update_scene(cx, scene);
            }
            self.sync_inspector_elements(cx, body, model, &diagram.key, &diagram.title, &node_keys);
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

    /// Resolve one requested canvas selection and project that resolved state to
    /// every selection consumer. Missing keys deliberately converge on the
    /// diagram subject rather than leaving stale inspector or toolbar state.
    fn sync_selection(
        &self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        requested_key: Option<&str>,
    ) {
        let selected_key = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            .and_then(|mut canvas| {
                if let Some(key) = requested_key {
                    canvas.select_by_key(cx, key);
                } else {
                    canvas.clear_selection(cx);
                }
                canvas.selected_key().map(str::to_owned)
            });
        let subject = selected_key
            .as_ref()
            .map(|key| Subject::Classifier(key.clone()))
            .unwrap_or_else(|| Subject::Diagram(self.key.clone()));
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, subject);
        }
        if let Some(mut toolbar) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        {
            toolbar.set_selection(cx, selected_key.as_ref().map(|_| 1));
        }
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, selected_key.is_some());
        }
    }
}

impl DocView for ClassDiagramView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::ClassDiagram
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_canvas(cx);
        let model = &data.uml_analysis.projection;
        self.sync_properties(cx, body, model);
        let built = model.diagrams.iter().find(|d| d.key == self.key).map(|d| {
            let (scene, diags) = build_scene(model, d, resolve_display(&d.display), &self.expanded);
            (scene, diags, d.title.clone())
        });
        if let Some((scene, diags, title)) = built {
            for d in &diags {
                log!("diagnostic: {d:?}");
            }
            let node_keys: Vec<String> = scene.nodes.iter().map(|n| n.key.clone()).collect();
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            {
                canvas.set_scene(cx, scene);
            }
            self.sync_inspector_elements(cx, body, model, &self.key, &title, &node_keys);
        }
        // A fresh scene starts with no selection. Converge the canvas,
        // inspector, selection toolbar, and fit control through one path.
        self.sync_selection(cx, body, model, None);
        // Re-converge the view bar's lit state onto the canvas. The canvas owns
        // both the veil mode and the hidden-border x-ray; the bar only caches
        // them, and its own click handler is otherwise the sole writer -- so
        // this is the one path that can heal a bar<->canvas disagreement (tab
        // activation, model reload).
        let canvas_state = body
            .canvas(cx)
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .map(|canvas| (canvas.constraint_vis(), canvas.show_hidden_borders()));
        if let Some((vis, hidden)) = canvas_state {
            if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
                bar.set_show_constraints(cx, show_constraints_for(vis));
                bar.set_show_hidden_borders(cx, hidden);
            }
        }
        self.show_mode(cx, body);
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        let model = &data.uml_analysis.projection;
        let mut out = ViewOutcome::default();

        if self.mode.properties_visible() {
            let properties_uid = body.diagram_properties(cx).widget_uid();
            let properties_actions: Vec<DiagramPropertiesAction> = actions
                .filter_widget_actions(properties_uid)
                .filter_map(|item| {
                    item.action
                        .downcast_ref::<DiagramPropertiesAction>()
                        .cloned()
                })
                .collect();
            if !properties_actions.is_empty() {
                let outcome = self.properties_actions_outcome(properties_actions);
                if !self.mode.properties_visible() {
                    Self::clear_properties_focus(cx);
                }
                self.show_mode(cx, body);
                return outcome;
            }
            if let Some((anchor_rect, min_width, items)) = body
                .diagram_properties(cx)
                .as_diagram_properties()
                .max_attributes_open_request(cx, actions)
            {
                out.popup = Some(PopupRequest::Select {
                    tag: live_id!(max_attributes_picker),
                    anchor_rect,
                    min_width,
                    items,
                    compact_frame: true,
                });
                return out;
            }

            // Properties mode owns the center surface. All canvas chrome,
            // including the tool dock, is hidden and its queued actions are
            // deliberately ignored until the panel's Close action restores it.
            return out;
        }
        debug_assert!(self.mode.accepts_canvas_actions());

        // Keep the view bar's fit-to-selection button in step with the canvas
        // selection. `ClassDiagramSurface` mutates `selected_key` in `handle_event`,
        // which runs before `Event::Actions` is dispatched, so this reads the
        // selection as of THIS batch. Cheap: the setter redraws only on change.
        let has_selection = body
            .canvas(cx)
            .borrow::<crate::canvas::ClassDiagramSurface>()
            .is_some_and(|c| c.has_selection());
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, has_selection);
        }

        // Inline-edit commit: inspector emits `Edited(subject_key)`.
        if body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions))
            .is_some()
        {
            out.promote_active = true;
            return out;
        }

        // Element-picker: the SelectBox asked to open its flyout.
        if let Some((anchor_rect, min_width, items)) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.take_open_request(cx, actions))
        {
            out.popup = Some(PopupRequest::Select {
                tag: live_id!(element_picker),
                anchor_rect,
                min_width,
                items,
                compact_frame: false,
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
            let subject = subject_from(&key, kind);
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, model, subject);
            }
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            {
                canvas.select_by_key(cx, &key);
            }
            out.break_merge_group = true;
            return out;
        }

        // Tool dock: mode clicks update their own highlight; ModeChanged
        // re-snaps the statusbar. DiagramProps swaps this tab's center surface;
        // the remaining one-shot action stays a mock `log!` no-op.
        if let Some(action) = body
            .tool_dock(cx)
            .borrow_mut::<crate::tool_dock::ToolDock>()
            .and_then(|dock| dock.dock_action(actions))
        {
            if self.apply_tool_action(action.clone()) {
                self.show_mode(cx, body);
                return out;
            }
            match action {
                crate::tool_dock::ToolDockAction::ModeChanged(_) => out.statusbar_dirty = true,
                other => log!("tool dock: {other:?}"),
            }
            return out;
        }

        // View bar: `ShowConstraints` drives the canvas veil mode,
        // `ShowHiddenBorders` the group x-ray, and the four camera one-shots
        // are thin wrappers over the `Camera` API on `ClassDiagramSurface`.
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
                    if let Some(mut canvas) = body
                        .canvas(cx)
                        .borrow_mut::<crate::canvas::ClassDiagramSurface>()
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
            } else if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
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
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            .and_then(|c| c.surface_action(actions));
        match canvas_action {
            Some(crate::canvas::ClassDiagramSurfaceAction::NodeMenu { abs, key }) => {
                // Select-on-right-click follows the same projection path as a
                // normal node selection.
                self.sync_selection(cx, body, model, Some(&key));
                // Gather the diagram's per-node context items (empty for now).
                let context = body
                    .canvas(cx)
                    .borrow::<crate::canvas::ClassDiagramSurface>()
                    .map(|c| c.context_items(&Subject::Classifier(key.clone())))
                    .unwrap_or_default();
                out.popup = Some(PopupRequest::NodeContextMenu {
                    anchor: abs,
                    key,
                    context,
                });
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::NodeSelect { key }) => {
                self.sync_selection(cx, body, model, Some(&key));
                out.break_merge_group = true;
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::NodeDeselect) => {
                // Deselecting falls back to the diagram, never an empty panel.
                self.sync_selection(cx, body, model, None);
                out.break_merge_group = true;
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::ToggleExpand { key }) => {
                if !self.expanded.remove(&key) {
                    self.expanded.insert(key);
                }
                // Re-solve the current diagram with the updated set; update_scene
                // holds the camera and re-resolves the selection by key.
                if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.key) {
                    let (scene, diags) = build_scene(
                        model,
                        diagram,
                        resolve_display(&diagram.display),
                        &self.expanded,
                    );
                    for d in &diags {
                        log!("diagnostic: {d:?}");
                    }
                    if let Some(mut canvas) = body
                        .canvas(cx)
                        .borrow_mut::<crate::canvas::ClassDiagramSurface>()
                    {
                        canvas.update_scene(cx, scene);
                    }
                }
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::DialDismiss) => {
                out.popup = Some(PopupRequest::Dismiss);
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::CompassArmed {
                subject_key,
                reference_key,
                center,
            }) => {
                // The dial just armed on a (new) target: speculatively solve each
                // zone's placement against the active diagram. The solve yields
                // both the conflict verdict (redden the zones the solver would
                // reject) and the candidate layout itself, which the canvas
                // animates to on hover -- so previewing costs no extra solve.
                if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.key) {
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
                    if let Some(mut canvas) = body
                        .canvas(cx)
                        .borrow_mut::<crate::canvas::ClassDiagramSurface>()
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
        data: ViewData<'_>,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        if tag == live_id!(max_attributes_picker) {
            let action = body
                .diagram_properties(cx)
                .as_diagram_properties()
                .max_attributes_closed(cx, result);
            return action
                .map(|action| self.properties_actions_outcome([action]))
                .unwrap_or_default();
        }
        // Element-picker: any close clears the box's active state; a node
        // commit repoints the inspector (inspector-local -- no tab, no canvas
        // move).
        if tag == live_id!(element_picker) {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.on_picker_closed(cx, data.uml_analysis, result);
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
                    .borrow::<crate::canvas::ClassDiagramSurface>()
                    .and_then(|c| c.placement_for(z))
            });
            if let Some(p) = placement {
                let strip_md = |s: &str| s.strip_suffix(".md").unwrap_or(s).to_string();
                let label = format!("Place {}", p.subject_title);
                out.edit = Some(crate::document::EditIntent {
                    edit: waml::edit::PendingEdit::new(waml::uml::Batch(vec![Op::PlacementSet {
                        diagram: strip_md(&self.key),
                        subject_title: p.subject_title,
                        subject_slug: strip_md(&p.subject_key),
                        reference_title: p.reference_title,
                        reference_slug: strip_md(&p.reference_key),
                        directions: p.directions,
                    }])),
                    label,
                    merge_key: None,
                    after_location: None,
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
        _data: ViewData<'_>,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> ViewOutcome {
        // The dial's armed wedge drives the live layout preview. The candidate
        // layouts were already solved at arm time, so this costs no solve.
        if tag == live_id!(place_dial) {
            let zone = id.and_then(crate::canvas::zone_of_id);
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            {
                canvas.preview_zone(cx, zone);
            }
        }
        ViewOutcome::default()
    }

    fn after_session_change(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        change: SessionChange,
    ) {
        match refresh_for(change) {
            DiagramRefresh::PreserveCamera => {
                self.update_scene(cx, body, &data.uml_analysis.projection);
            }
            DiagramRefresh::None => {}
        }
    }

    fn chrome(&self) -> BodyChrome {
        if self.mode.properties_visible() {
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                },
            }
        } else {
            BodyChrome {
                tool_dock: true,
                view_bar: true,
                canvas_overlays: true,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: Some(Icon::SlidersHorizontal),
                },
            }
        }
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        self.show_mode(cx, body);
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        self.return_to_canvas(cx, body);
    }

    fn on_escape(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        self.return_to_canvas(cx, body);
    }

    fn capture_anchor(&self, body: &BodyWidgets) -> ViewAnchor {
        let Some(canvas) = body
            .canvas_ref()
            .borrow::<crate::canvas::ClassDiagramSurface>()
        else {
            return ViewAnchor::None;
        };
        ViewAnchor::Diagram {
            selected_key: canvas.selected_key().map(str::to_owned),
            camera: canvas.camera_anchor(),
        }
    }

    fn restore_anchor(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        let ViewAnchor::Diagram {
            selected_key,
            camera,
        } = anchor
        else {
            return false;
        };
        let Some(mut canvas) = body
            .canvas_ref()
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        else {
            return false;
        };
        canvas.restore_camera_anchor(cx, *camera);
        drop(canvas);
        self.sync_selection(
            cx,
            body,
            &data.uml_analysis.projection,
            selected_key.as_deref(),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        constraint_vis_for, refresh_for, show_constraints_for, show_hidden_borders_for,
        ClassDiagramMode, ClassDiagramView, DiagramRefresh,
    };
    use crate::canvas::ConstraintVisibility;
    use crate::diagram_properties::DiagramPropertiesAction;
    use crate::doc_view::{BodyChrome, DocView, ViewOutcome};
    use crate::tool_dock::{Tool, ToolDockAction};
    use crate::view_bar::{ViewBarAction, ViewOption};
    use crate::view_history::ViewAnchor;
    use makepad_widgets::{
        live_id, Area, Cx, DrawList, LiveId, RectArea, ScriptApply, ScriptNew, ScriptVm,
        ScriptVmCx, Trigger, Walk, Widget, WidgetNode, WidgetRef, WidgetUid,
    };
    use std::path::Path;
    use waml::model::CardinalityVisibility;
    use waml::ops::DiagramDisplaySet;

    struct TestBody {
        uid: WidgetUid,
        children: Vec<(LiveId, WidgetRef)>,
    }

    impl ScriptApply for TestBody {}

    impl WidgetNode for TestBody {
        fn widget_uid(&self) -> WidgetUid {
            self.uid
        }

        fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
            for (id, child) in &self.children {
                visit(*id, child.clone());
            }
        }

        fn walk(&mut self, _cx: &mut Cx) -> Walk {
            Walk::default()
        }

        fn area(&self) -> Area {
            Area::Empty
        }

        fn redraw(&mut self, _cx: &mut Cx) {}
    }

    impl Widget for TestBody {}

    struct FocusedTextField {
        uid: WidgetUid,
        area: Area,
    }

    impl ScriptApply for FocusedTextField {}

    impl WidgetNode for FocusedTextField {
        fn widget_uid(&self) -> WidgetUid {
            self.uid
        }

        fn walk(&mut self, _cx: &mut Cx) -> Walk {
            Walk::default()
        }

        fn area(&self) -> Area {
            self.area
        }

        fn redraw(&mut self, _cx: &mut Cx) {}
    }

    impl Widget for FocusedTextField {}

    fn test_body(vm: &mut ScriptVm) -> WidgetRef {
        WidgetRef::new_with_inner(Box::new(TestBody {
            uid: WidgetUid::new(),
            children: vec![
                (
                    live_id!(inspector),
                    WidgetRef::new_with_inner(Box::new(
                        crate::inspector_panel::Inspector::script_new(vm),
                    )),
                ),
                (
                    live_id!(diagram_properties),
                    WidgetRef::new_with_inner(Box::new(
                        crate::diagram_properties::DiagramProperties::script_new(vm),
                    )),
                ),
                (
                    live_id!(canvas),
                    WidgetRef::new_with_inner(Box::new(
                        crate::canvas::ClassDiagramSurface::script_new(vm),
                    )),
                ),
                (
                    live_id!(selection_toolbar),
                    WidgetRef::new_with_inner(Box::new(
                        crate::selection_toolbar::SelectionToolbar::script_new(vm),
                    )),
                ),
                (
                    live_id!(view_bar),
                    WidgetRef::new_with_inner(Box::new(crate::view_bar::ViewBar::script_new(vm))),
                ),
            ],
        }))
    }

    fn focused_text_field(cx: &mut Cx) -> WidgetRef {
        let draw_list = DrawList::new(cx);
        let area = Area::Rect(RectArea {
            draw_list_id: draw_list.id(),
            rect_id: 0,
            redraw_id: cx.redraw_id(),
        });
        WidgetRef::new_with_inner(Box::new(FocusedTextField {
            uid: WidgetUid::new(),
            area,
        }))
    }

    fn advance_focus(cx: &mut Cx) {
        cx.send_trigger(Area::Empty, Trigger::default());
        cx.handle_triggers();
    }

    #[allow(dead_code)]
    fn mini_model() -> waml::model::Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        crate::load::load_model(&dir).expect("mini fixture must load")
    }

    fn apply_outcome(outcome: ViewOutcome) -> String {
        use waml::edit::{EditBatch, EditContext};

        let source = waml::source::SourceBundle::try_from_pairs([(
            "orders.md",
            "---\ntype: Diagram\ntitle: Old\nprofile: uml-domain\ndescription: Old description\n---\n# Old\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source.clone(), None, 1).unwrap();
        let intent = outcome.edit.expect("outcome contains an edit");
        assert_eq!(intent.label, "Change diagram properties");
        let changed = intent
            .edit
            .lower(EditContext {
                source: &source,
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            })
            .unwrap();
        changed.documents()[0].text().to_string()
    }

    #[test]
    fn properties_mode_toggles_and_resets_on_deactivation() {
        let mut mode = ClassDiagramMode::Canvas;
        mode.toggle_properties();
        assert_eq!(mode, ClassDiagramMode::Properties);
        mode.toggle_properties();
        assert_eq!(mode, ClassDiagramMode::Canvas);
        mode.toggle_properties();
        mode.deactivate();
        assert_eq!(mode, ClassDiagramMode::Canvas);
    }

    #[test]
    fn properties_mode_hides_canvas_chrome_and_gates_canvas_actions() {
        assert!(ClassDiagramMode::Canvas.accepts_canvas_actions());
        assert!(!ClassDiagramMode::Properties.accepts_canvas_actions());
    }

    #[test]
    fn diagram_properties_tool_toggles_the_view_instead_of_being_a_no_op() {
        let mut view = ClassDiagramView::new("orders".into());

        assert!(view.apply_tool_action(ToolDockAction::Triggered(Tool::DiagramProps)));
        assert_eq!(view.mode, ClassDiagramMode::Properties);
    }

    #[test]
    fn properties_mode_preserves_breadcrumb_while_hiding_diagram_controls() {
        let mut view = ClassDiagramView::new("orders".into());
        view.apply_tool_action(ToolDockAction::Triggered(Tool::DiagramProps));

        assert_eq!(
            view.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: crate::doc_view::DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                },
            }
        );
    }

    #[test]
    fn a_properties_change_returns_exactly_one_diagram_set() {
        let mut view = ClassDiagramView::new("orders".into());
        view.mode = ClassDiagramMode::Properties;
        let chrome_before = view.chrome();
        let display = DiagramDisplaySet {
            show_attributes: true,
            show_type: false,
            show_attribute_visibility: true,
            cardinality: CardinalityVisibility::Explicit,
            max_attributes: Some(4),
            show_roles: true,
            show_cardinality: true,
            show_labels: true,
            show_stereotype: false,
            stereotype_filter: None,
            stereotype_colors: vec![],
        };

        let outcome = view
            .properties_actions_outcome([DiagramPropertiesAction::DisplayChanged(display.clone())]);
        assert!(outcome.edit.as_ref().unwrap().merge_key.is_none());

        let text = apply_outcome(outcome);
        assert!(text.contains("showType: false"), "{text}");
        assert_eq!(text.matches("showType:").count(), 1);
        assert_eq!(view.chrome(), chrome_before);
    }

    #[test]
    fn diagram_view_is_constructed_with_immutable_identity() {
        use super::ClassDiagramView;
        use crate::doc_view::DocView;

        let view = ClassDiagramView::new("orders".into());

        assert_eq!(
            view.chrome(),
            BodyChrome {
                tool_dock: true,
                view_bar: true,
                canvas_overlays: true,
                document_header: crate::doc_view::DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: Some(crate::icons::Icon::SlidersHorizontal),
                },
            }
        );
    }

    #[test]
    fn clearing_description_does_not_write_the_diagram_title() {
        let mut view = ClassDiagramView::new("orders".into());

        let outcome =
            view.properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(None)]);

        let text = apply_outcome(outcome);
        assert!(text.contains("title: Old"), "{text}");
        assert!(!text.contains("description:"), "{text}");
    }

    #[test]
    fn production_description_typing_coalesces_beyond_the_atomic_tail() {
        let mut view = ClassDiagramView::new("orders".into());
        let location = crate::view_history::ViewLocation {
            document: crate::view_history::DocumentLocator::primary("orders"),
            anchor: ViewAnchor::None,
        };
        let mut history = crate::editor_history::EditorHistory::default();

        for index in 0..(crate::editor_history::ATOMIC_TAIL + 3) {
            let outcome =
                view.properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(
                    Some(format!("Customer {index}")),
                )]);
            let intent = outcome.edit.expect("typing emits a model edit");
            let merge_key = intent
                .merge_key
                .as_ref()
                .expect("production typing supplies a merge key");
            assert_eq!(
                merge_key,
                &crate::editor_history::EditMergeKey {
                    document: crate::view_history::DocumentLocator::primary("orders"),
                    control: "diagram.description".into(),
                    kind: crate::editor_history::EditMergeKind::Continuous,
                    span: None,
                }
            );
            history.record_edit(
                intent.edit,
                intent.label,
                intent.merge_key,
                location.clone(),
                location.clone(),
            );
        }

        assert_eq!(
            history.undo_len(),
            crate::editor_history::ATOMIC_TAIL + 2,
            "the first post-save production typing pair coalesces while the savepoint and atomic tail stay distinct",
        );
    }

    #[test]
    fn description_focus_or_selection_change_breaks_the_merge_group() {
        let mut view = ClassDiagramView::new("orders".into());

        let outcome =
            view.properties_actions_outcome([DiagramPropertiesAction::BreakEditMergeGroup]);

        assert!(outcome.break_merge_group);
        assert!(outcome.edit.is_none());
    }

    #[test]
    fn properties_close_separates_aged_production_typing_sessions() {
        let mut view = ClassDiagramView::new("orders".into());
        let location = crate::view_history::ViewLocation {
            document: crate::view_history::DocumentLocator::primary("orders"),
            anchor: ViewAnchor::None,
        };
        let mut history = crate::editor_history::EditorHistory::default();

        let baseline = view
            .properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(Some(
                "Baseline".into(),
            ))])
            .edit
            .unwrap();
        history.record_edit(
            baseline.edit,
            baseline.label,
            None,
            location.clone(),
            location.clone(),
        );

        let before_close = view
            .properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(Some(
                "First session".into(),
            ))])
            .edit
            .unwrap();
        history.record_edit(
            before_close.edit,
            before_close.label,
            before_close.merge_key,
            location.clone(),
            location.clone(),
        );

        let close = view.properties_actions_outcome([DiagramPropertiesAction::Close]);
        assert!(close.break_merge_group);
        history.break_merge_group();
        view.mode = ClassDiagramMode::Properties;

        let after_reopen = view
            .properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(Some(
                "Second session".into(),
            ))])
            .edit
            .unwrap();
        history.record_edit(
            after_reopen.edit,
            after_reopen.label,
            after_reopen.merge_key,
            location.clone(),
            location.clone(),
        );

        for index in 0..(crate::editor_history::ATOMIC_TAIL + 1) {
            let intent = view
                .properties_actions_outcome([DiagramPropertiesAction::DescriptionChanged(Some(
                    format!("Unmergeable {index}"),
                ))])
                .edit
                .unwrap();
            history.record_edit(
                intent.edit,
                intent.label,
                None,
                location.clone(),
                location.clone(),
            );
        }

        assert_eq!(
            history.undo_len(),
            crate::editor_history::ATOMIC_TAIL + 4,
            "typing on opposite sides of close/reopen remains distinct after aging",
        );
    }

    #[test]
    fn model_change_selects_the_camera_preserving_refresh() {
        assert_eq!(
            refresh_for(crate::editor_session::SessionChange {
                revision: 2,
                source_changed: true,
                okf_changed: true,
                uml_changed: true,
                navigation_changed: true,
                conflicts_changed: true,
            }),
            DiagramRefresh::PreserveCamera
        );
        assert_eq!(
            refresh_for(crate::editor_session::SessionChange {
                revision: 3,
                source_changed: true,
                okf_changed: false,
                uml_changed: false,
                navigation_changed: false,
                conflicts_changed: false,
            }),
            DiagramRefresh::None
        );
    }

    #[test]
    fn property_action_batch_keeps_every_edit_before_closing() {
        let mut view = ClassDiagramView::new("orders".into());
        view.mode = ClassDiagramMode::Properties;
        let display = DiagramDisplaySet {
            show_attributes: false,
            show_type: true,
            show_attribute_visibility: true,
            cardinality: CardinalityVisibility::All,
            max_attributes: None,
            show_roles: false,
            show_cardinality: false,
            show_labels: true,
            show_stereotype: true,
            stereotype_filter: None,
            stereotype_colors: vec![],
        };

        let outcome = view.properties_actions_outcome([
            DiagramPropertiesAction::DescriptionChanged(None),
            DiagramPropertiesAction::Close,
            DiagramPropertiesAction::DisplayChanged(display.clone()),
        ]);
        assert!(outcome.break_merge_group);
        assert!(outcome.edit.as_ref().unwrap().merge_key.is_none());

        let text = apply_outcome(outcome);
        assert!(text.contains("title: Old"), "{text}");
        assert!(text.contains("showAttributes: false"), "{text}");
        assert!(!text.contains("description:"), "{text}");
        assert_eq!(view.mode, ClassDiagramMode::Canvas);
    }

    #[test]
    fn leaving_properties_clears_focused_text_field_but_canvas_deactivation_does_not() {
        use crate::doc_view::DocView;

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = crate::doc_view::BodyWidgets::new(cx, &ui);
        let field = focused_text_field(cx);
        let field_area = field.area();
        let mut view = ClassDiagramView::new("orders".into());

        cx.set_key_focus(field_area);
        advance_focus(cx);
        assert_eq!(
            cx.key_focus(),
            field_area,
            "the descendant field is focused"
        );

        view.mode = ClassDiagramMode::Properties;
        view.on_escape(cx, &body);
        advance_focus(cx);
        assert_eq!(
            cx.key_focus(),
            Area::Empty,
            "Escape leaves no hidden field focused"
        );

        cx.set_key_focus(field_area);
        advance_focus(cx);
        view.mode = ClassDiagramMode::Canvas;
        view.on_deactivate(cx, &body);
        advance_focus(cx);
        assert_eq!(
            cx.key_focus(),
            field_area,
            "a Canvas-to-Canvas deactivation must not clear unrelated focus"
        );
    }

    #[test]
    fn renamed_diagram_uses_current_model_title_after_reactivate_and_sync() {
        use crate::doc_view::DocView;

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = crate::doc_view::BodyWidgets::new(cx, &ui);
        let mut view = ClassDiagramView::new("orders-diagram".into());
        let first = waml::analysis::prepare_candidate(
            waml::source::SourceBundle::try_from_pairs([(
                "orders-diagram.md",
                "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n",
            )])
            .unwrap(),
            None,
            1,
        )
        .unwrap();
        let data = crate::doc_view::ViewData {
            source: first.source(),
            okf_analysis: first.okf(),
            uml_analysis: first.uml(),
            revision: 1,
        };

        view.sync(cx, &body, data);
        let second = waml::analysis::prepare_candidate(
            waml::source::SourceBundle::try_from_pairs([(
                "orders-diagram.md",
                "---\ntype: Diagram\ntitle: Order flow\nprofile: uml-domain\n---\n# Order flow\n",
            )])
            .unwrap(),
            None,
            2,
        )
        .unwrap();
        view.on_deactivate(cx, &body);
        view.on_activate(cx, &body);
        view.sync(
            cx,
            &body,
            crate::doc_view::ViewData {
                source: second.source(),
                okf_analysis: second.okf(),
                uml_analysis: second.uml(),
                revision: 2,
            },
        );

        let inspector_widget = body.inspector(cx);
        let inspector = inspector_widget
            .borrow::<crate::inspector_panel::Inspector>()
            .expect("native test body installs an inspector");
        assert_eq!(
            inspector.elements_for_test()[0].label,
            "Order flow",
            "the diagram row must be rebuilt from the renamed model"
        );
    }

    #[test]
    fn anchor_restore_synchronizes_every_selection_consumer_and_clears_unresolved_keys() {
        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = crate::doc_view::BodyWidgets::new(cx, &ui);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let source = crate::load::read_bundle(&dir).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (source, okf_analysis, uml_analysis, revision) = prepared.into_parts();
        let mut view = ClassDiagramView::new("orders-diagram".into());
        view.sync(
            cx,
            &body,
            crate::doc_view::ViewData {
                source: &source,
                okf_analysis: &okf_analysis,
                uml_analysis: &uml_analysis,
                revision,
            },
        );
        let camera = crate::view_history::DiagramCameraAnchor {
            pan_x: 12.0,
            pan_y: 34.0,
            zoom: 1.5,
        };

        assert!(view.restore_anchor(
            cx,
            &body,
            crate::doc_view::ViewData {
                source: &source,
                okf_analysis: &okf_analysis,
                uml_analysis: &uml_analysis,
                revision,
            },
            &ViewAnchor::Diagram {
                selected_key: Some("customer".into()),
                camera,
            },
        ));
        assert_eq!(
            body.canvas(cx)
                .borrow::<crate::canvas::ClassDiagramSurface>()
                .unwrap()
                .selected_key(),
            Some("customer")
        );
        assert_eq!(
            body.inspector(cx)
                .borrow::<crate::inspector_panel::Inspector>()
                .unwrap()
                .subject_key_for_test()
                .as_deref(),
            Some("customer")
        );
        assert_eq!(
            body.selection_toolbar(cx)
                .borrow::<crate::selection_toolbar::SelectionToolbar>()
                .unwrap()
                .selection_count_for_test(),
            Some(1)
        );
        assert!(body
            .view_bar(cx)
            .borrow::<crate::view_bar::ViewBar>()
            .unwrap()
            .fit_to_selection_enabled_for_test());

        assert!(view.restore_anchor(
            cx,
            &body,
            crate::doc_view::ViewData {
                source: &source,
                okf_analysis: &okf_analysis,
                uml_analysis: &uml_analysis,
                revision,
            },
            &ViewAnchor::Diagram {
                selected_key: Some("deleted".into()),
                camera,
            },
        ));
        assert_eq!(
            body.canvas(cx)
                .borrow::<crate::canvas::ClassDiagramSurface>()
                .unwrap()
                .selected_key(),
            None
        );
        assert_eq!(
            body.inspector(cx)
                .borrow::<crate::inspector_panel::Inspector>()
                .unwrap()
                .subject_key_for_test()
                .as_deref(),
            Some("orders-diagram")
        );
        assert_eq!(
            body.selection_toolbar(cx)
                .borrow::<crate::selection_toolbar::SelectionToolbar>()
                .unwrap()
                .selection_count_for_test(),
            None
        );
        assert!(!body
            .view_bar(cx)
            .borrow::<crate::view_bar::ViewBar>()
            .unwrap()
            .fit_to_selection_enabled_for_test());
    }

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
