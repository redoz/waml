use makepad_widgets::*;
use waml_markdown_editor::session::HostSnapshotCause;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
    ViewReconcilePolicy,
};
use crate::editor_session::{EditorSessionSnapshot, SessionChange};
use crate::source_view::SourceView;
use crate::view_history::ViewAnchor;

pub struct GenericOkfView {
    source: SourceView,
}

impl GenericOkfView {
    pub fn new(concept_id: String) -> Self {
        Self {
            source: SourceView::new_read_only(concept_id),
        }
    }
}

impl DocView for GenericOkfView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::GenericOkf
    }

    fn reconcile_policy(&self) -> ViewReconcilePolicy {
        ViewReconcilePolicy::RetainLiveState
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_markdown_editor(cx);
        body.markdown_editor().set_read_only(cx, true);
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.sync(cx, body, snapshot.borrowed().into());
        self.source
            .install_snapshot(cx, body, snapshot, HostSnapshotCause::InitialLoad);
    }

    fn after_session_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
        change: SessionChange,
    ) {
        let cause = if change.source_changed {
            HostSnapshotCause::ApplicationHistory
        } else {
            HostSnapshotCause::AcknowledgedLocalEdit
        };
        self.source.install_snapshot(cx, body, snapshot, cause);
    }

    fn route_ui_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        self.source.route_editor_event(cx, ui, event);
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        let mut outcome = self.source.handle(cx, body, actions, data);
        outcome.source_edit = None;
        outcome
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: None,
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        crate::okf_documents::generic_okf_accent()
    }

    fn capture_anchor(&self, body: &BodyWidgets) -> ViewAnchor {
        self.source.capture_anchor(body)
    }

    fn restore_anchor(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        self.source.restore_anchor(cx, body, data, anchor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_markdown_view_is_retained_and_read_only_by_construction() {
        let view = GenericOkfView::new("runbook".into());
        assert_eq!(
            view.reconcile_policy(),
            ViewReconcilePolicy::RetainLiveState
        );
        assert_eq!(view.identity(), DocViewIdentity::GenericOkf);
    }

    #[test]
    fn generic_document_hides_all_diagram_chrome_and_has_stable_accent() {
        let view = GenericOkfView::new("runbook".into());
        assert_eq!(
            view.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                },
            }
        );
        assert_eq!(
            view.tab_accent(),
            crate::okf_documents::generic_okf_accent()
        );
    }
}
