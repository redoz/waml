use makepad_widgets::*;
use waml_markdown_editor::session::HostSnapshotCause;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, HeaderViewAction,
    ViewData, ViewOutcome, ViewReconcilePolicy,
};
use crate::editor_session::{EditorSessionSnapshot, SessionChange};
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::source_view::SourceView;
use crate::view_history::ViewAnchor;

pub struct GenericOkfView {
    /// The reading surface. A concept opens here.
    reading: crate::reading_view::ReadingView,
    /// The raw-markdown surface, reached by the explicit source toggle. It
    /// stays read-only: this view discards the edit outcome, so a writable
    /// editor would silently drop what the user typed.
    source: SourceView,
}

impl GenericOkfView {
    #[cfg(test)]
    pub fn new(concept_id: String) -> Self {
        Self::new_with_asset_host(
            concept_id,
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
        )
    }

    pub fn new_with_asset_host(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
    ) -> Self {
        // Opening a concept is a reading action: it renders. Seeing the
        // markdown behind it is a separate, explicit action.
        Self {
            reading: crate::reading_view::ReadingView::new_with_asset_host(
                concept_id.clone(),
                assets.clone(),
            ),
            source: SourceView::new_read_only(concept_id, assets),
        }
    }

    #[cfg(test)]
    pub(crate) fn showing_source(&self) -> bool {
        self.reading.showing_source()
    }

    pub(crate) fn toggle_source(&mut self) {
        let showing = self.reading.showing_source();
        self.reading.set_showing_source(!showing);
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
        if self.reading.showing_source() {
            body.show_markdown_editor(cx);
            body.markdown_editor().set_read_only(cx, true);
            body.markdown_viewer_source_toggle(cx)
                .as_icon_button()
                .set_icon(cx, Icon::Eye);
        } else {
            body.show_markdown_viewer(cx);
            body.markdown_viewer_source_toggle(cx)
                .as_icon_button()
                .set_icon(cx, Icon::FileCode);
        }
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.reading.install_snapshot(cx, body, snapshot);
        self.source
            .install_snapshot(cx, body, snapshot, HostSnapshotCause::InitialLoad);
        // `SourceView::install_snapshot` unconditionally shows the
        // raw-markdown surface (it has no notion of the reading toggle), so
        // this view's own surface choice must be applied LAST, after both
        // installs, or a reading-mode open would flash to the editor.
        self.sync(cx, body, snapshot.borrowed().into());
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
        self.reading.install_snapshot(cx, body, snapshot);
        self.source.install_snapshot(cx, body, snapshot, cause);
        self.sync(cx, body, snapshot.borrowed().into());
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
        if body
            .markdown_viewer_source_toggle(cx)
            .as_icon_button()
            .clicked(actions)
        {
            self.toggle_source();
            self.sync(cx, body, data);
        }
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
                // Icon shows the surface the toggle LEADS to; `sync` keeps it
                // current when the user flips between them.
                view_toggle: Some(if self.reading.showing_source() {
                    HeaderViewAction {
                        icon: Icon::Eye,
                        tooltip: "View rendered",
                    }
                } else {
                    HeaderViewAction {
                        icon: Icon::Code,
                        tooltip: "View source",
                    }
                }),
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

    fn generic_view() -> GenericOkfView {
        GenericOkfView::new_with_asset_host(
            "runbook".into(),
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
        )
    }

    #[test]
    fn generic_markdown_view_is_retained_and_read_only_by_construction() {
        let view = generic_view();
        assert_eq!(
            view.reconcile_policy(),
            ViewReconcilePolicy::RetainLiveState
        );
        assert_eq!(view.identity(), DocViewIdentity::GenericOkf);
    }

    #[test]
    fn a_concept_opens_in_the_reading_view() {
        let view = generic_view();
        assert!(
            !view.showing_source(),
            "a concept opens as rendered prose, not as markdown to edit"
        );
    }

    #[test]
    fn the_source_toggle_switches_between_the_viewer_and_the_editor() {
        let mut view = generic_view();
        view.toggle_source();
        assert!(
            view.showing_source(),
            "the toggle reveals the markdown source"
        );
        view.toggle_source();
        assert!(!view.showing_source(), "and puts it back");
    }

    #[test]
    fn generic_document_hides_all_diagram_chrome_and_has_stable_accent() {
        let view = generic_view();
        assert_eq!(
            view.chrome(),
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: DocumentHeaderChrome {
                    breadcrumb: true,
                    right_dock: None,
                    view_toggle: Some(HeaderViewAction {
                        icon: Icon::Code,
                        tooltip: "View source",
                    }),
                },
            }
        );
        assert_eq!(
            view.tab_accent(),
            crate::okf_documents::generic_okf_accent()
        );
    }
}
