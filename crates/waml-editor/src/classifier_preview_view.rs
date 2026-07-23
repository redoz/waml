//! `ClassifierPreviewView` — the single-element preview (focus canvas + inspector-
//! without-picker, no tool dock). Real behavior lands in Task 4.

use makepad_widgets::*;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};
use crate::inspector::Subject;
use crate::scene::build_focus_scene;

pub struct ClassifierPreviewView {
    /// The previewed classifier/package key.
    key: String,
}

impl ClassifierPreviewView {
    pub fn new(key: String) -> ClassifierPreviewView {
        ClassifierPreviewView { key }
    }
}

impl DocView for ClassifierPreviewView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        let scene = build_focus_scene(model, &self.key);
        if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>() {
            canvas.set_focus(cx, scene);
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::Classifier(self.key.clone()));
            // Previewing a classifier/package (not a diagram): no picker.
            inspector.set_picker_visible(cx, false);
        }
        if let Some(mut toolbar) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        {
            // Single-classifier focus only in this mock -- always 1.
            toolbar.set_selection(cx, Some(1));
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

        // Inline-edit commit: promote (pin) this preview tab.
        if let Some(key) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions))
        {
            out.promote_subject = Some(key);
            return out;
        }

        // Canvas select/deselect repoints the inspector (inspector-local).
        let canvas_action = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::GraphCanvas>()
            .and_then(|c| c.canvas_action(actions));
        match canvas_action {
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
            _ => {}
        }

        // Selection toolbar: Delete closes this preview tab (in-memory only).
        if let Some(action) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
            .and_then(|toolbar| toolbar.toolbar_action(actions))
        {
            match action {
                crate::selection_toolbar::SelectionToolbarAction::Delete => {
                    out.close_active = true;
                    return out;
                }
                crate::selection_toolbar::SelectionToolbarAction::NewDiagram => {
                    log!("selection toolbar: New Diagram (mock no-op)");
                    return out;
                }
                _ => {}
            }
        }

        out
    }

    fn wants_tooldock(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
