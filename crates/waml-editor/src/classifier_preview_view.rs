//! `ClassifierPreviewView` — the single-element preview (focus canvas + inspector-
//! without-picker, no tool dock). Real behavior lands in Task 4.

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, RevealTarget,
    ViewData, ViewOutcome,
};
use crate::document::NavCategory;
use crate::icons::Icon;
use crate::inspector::Subject;
use crate::scene::build_focus_scene;
use makepad_widgets::*;

pub struct ClassifierPreviewView {
    key: String,
    category: NavCategory,
}

impl ClassifierPreviewView {
    pub fn new(key: String, category: NavCategory) -> ClassifierPreviewView {
        ClassifierPreviewView { key, category }
    }
}

impl DocView for ClassifierPreviewView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::ClassifierPreview(self.category)
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_canvas(cx);
        let model = &data.uml_analysis.projection;
        let scene = build_focus_scene(model, &self.key);
        if let Some(mut canvas) = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
        {
            canvas.set_focus(cx, scene);
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject_analysis(
                cx,
                data.uml_analysis,
                Subject::Classifier(self.key.clone()),
            );
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
        // The preview tab focuses one classifier but never selects a canvas
        // node, so fit-to-selection has no target here.
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, false);
        }
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

        // Inline-edit commit: promote (pin) this preview tab.
        if body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
            .and_then(|inspector| inspector.edited(actions))
            .is_some()
        {
            out.promote_active = true;
            return out;
        }

        // Canvas select/deselect repoints the inspector (inspector-local).
        let canvas_action = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::ClassDiagramSurface>()
            .and_then(|c| c.surface_action(actions));
        match canvas_action {
            Some(crate::canvas::ClassDiagramSurfaceAction::NodeSelect { key }) => {
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject_analysis(cx, data.uml_analysis, Subject::Classifier(key));
                }
                return out;
            }
            Some(crate::canvas::ClassDiagramSurfaceAction::NodeDeselect) => {
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

    /// The subject's own node-kind swatch -- the colour its card already wears
    /// on the canvas -- so the tab's accent names *what* is open, not just that
    /// something is. A plain class and an unresolved type have no swatch of
    /// their own, so those keep the theme accent.
    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::PanelRight),
                emphasis_toggle: None,
                view_toggle: None,
                zoom: None,
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        crate::accent::tree_kind_color(self.category)
    }

    /// This tab already IS a fixed focus on `self.key` (spec
    /// §DocView::reveal): a matching hit needs no further action, and a
    /// `TextSpan`/mismatched key doesn't apply to a canvas-only preview.
    fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &RevealTarget) {
        let _ = (cx, body, target);
    }
}
