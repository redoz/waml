//! `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-
//! picker + tool dock + selection toolbar). Real behavior lands in Task 3.

use makepad_widgets::*;
use std::collections::HashSet;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, PopupRequest, ViewOutcome};
use crate::inspector::{diagram_elements, Subject};
use crate::popup::base::PopupResult;
use crate::scene::build_scene;

/// Strip a defensive `.md` tail from a node/diagram key.
fn strip_md_key(s: &str) -> String {
    s.strip_suffix(".md").unwrap_or(s).to_string()
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
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        model: &Model,
    ) -> ViewOutcome {
        let mut out = ViewOutcome::default();

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
            Some(crate::canvas::GraphCanvasAction::AuthorPlacement {
                subject_key,
                subject_title,
                reference_key,
                reference_title,
                directions,
            }) => {
                // Drag-to-place: author a `## Layout` placement for the dragged
                // (subject) node relative to the drop-target (reference). The
                // shell owns the Model, so the view only emits the Op; the shell
                // applies it against the bundle and re-solves (see App). Slugs and
                // the diagram id are bare -- strip a `.md` tail defensively.
                let strip_md = |s: &str| s.strip_suffix(".md").unwrap_or(s).to_string();
                out.ops.push(waml::ops::Op::PlaceSet {
                    diagram: strip_md(&self.active_key),
                    subject_title,
                    subject_slug: strip_md(&subject_key),
                    reference_title,
                    reference_slug: strip_md(&reference_key),
                    directions,
                });
                return out;
            }
            Some(crate::canvas::GraphCanvasAction::CompassArmed {
                subject_key,
                reference_key,
            }) => {
                // The compass just armed on a (new) target: speculatively solve
                // each zone's placement against the active diagram and push the
                // per-zone conflict verdict back to the canvas so it can redden
                // the zones the solver would reject.
                if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.active_key) {
                    let subject = strip_md_key(&subject_key);
                    let reference = strip_md_key(&reference_key);
                    let mut red = Vec::new();
                    for z in crate::canvas::COMPASS_ZONES {
                        if let Some(dir) = crate::canvas::zone_placed(z).dir {
                            if crate::scene::placement_would_conflict(
                                model,
                                diagram,
                                &subject,
                                &reference,
                                dir,
                                &self.expanded,
                            ) {
                                red.push(z);
                            }
                        }
                    }
                    if let Some(mut canvas) =
                        body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>()
                    {
                        canvas.set_conflict_zones(cx, red);
                    }
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
        }
        // node_menu currently only `log!`s on commit -- kept in the shell for
        // now.
        ViewOutcome::default()
    }

    fn wants_tooldock(&self) -> bool {
        true
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
