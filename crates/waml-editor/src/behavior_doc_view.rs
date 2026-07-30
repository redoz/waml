//! `BehaviorDocView` — the kind-agnostic doc tab for activity, state-machine,
//! and sequence documents (spec §1.2-1.3). Task 6 wires only the empty-state
//! seam: `Flow`/`Interaction` scenes and their solve-driven `sync` land in
//! Tasks 7-8.

use makepad_widgets::*;

use crate::canvas::BehaviorScene;
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::icons::Icon;

/// Which behavior family this tab renders. Both kinds share one widget/view
/// (spec §1.2: the surface is kind-agnostic); the kind only picks the solver
/// this doc's `sync` will call in Tasks 7-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BehaviorKind {
    Flow,
    Interaction,
}

pub struct BehaviorDocView {
    #[allow(dead_code)]
    key: String,
    kind: BehaviorKind,
}

impl BehaviorDocView {
    pub fn flow(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Flow,
        }
    }

    pub fn interaction(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Interaction,
        }
    }
}

impl DocView for BehaviorDocView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.set_behavior_canvas_visible(cx, true);
        let message = match self.kind {
            BehaviorKind::Flow => "No renderable elements".to_string(),
            BehaviorKind::Interaction => "No renderable elements".to_string(),
        };
        if let Some(mut canvas) = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
        {
            canvas.set_scene(cx, BehaviorScene::Empty { message });
        }
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let out = ViewOutcome::default();

        // The four camera one-shots are thin wrappers over the `Camera` API
        // on `BehaviorSurface` (mirrors `ClassDiagramView`'s view-bar wiring).
        // The veil/x-ray toggles and fit-to-* one-shots have no behavior-canvas
        // equivalent yet -- Tasks 7-9 add selection and a real scene to fit.
        if let Some(crate::view_bar::ViewBarAction::Triggered(opt)) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
            .and_then(|bar| bar.view_bar_action(actions))
        {
            if let Some(mut canvas) = body
                .behavior_canvas(cx)
                .borrow_mut::<crate::canvas::BehaviorSurface>()
            {
                match opt {
                    crate::view_bar::ViewOption::ZoomIn => {
                        canvas.zoom_step(cx, crate::canvas::ZOOM_STEP)
                    }
                    crate::view_bar::ViewOption::ZoomOut => {
                        canvas.zoom_step(cx, 1.0 / crate::canvas::ZOOM_STEP)
                    }
                    _ => {}
                }
            }
        }

        // No hit-testable target exists yet (the scene is always `Empty`),
        // but reading the action keeps the seam exercised for Tasks 7-9.
        let _ = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
            .and_then(|canvas| canvas.surface_action(actions));

        out
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: true,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::SlidersHorizontal),
            },
        }
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_behavior_canvas_visible(cx, true);
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_behavior_canvas_visible(cx, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_and_interaction_constructors_pick_distinct_kinds() {
        assert_eq!(BehaviorDocView::flow("a".into()).kind, BehaviorKind::Flow);
        assert_eq!(
            BehaviorDocView::interaction("a".into()).kind,
            BehaviorKind::Interaction
        );
    }
}
