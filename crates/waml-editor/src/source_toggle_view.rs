//! `SourceToggleView` — wraps any primary document view with the breadcrumb
//! header's view-source toggle, flipping in place between the wrapped surface
//! and the editable raw-markdown view of the same concept. This is the same
//! affordance the generic OKF viewer offers, made uniform for the diagram,
//! behavior, and classifier-preview tabs.

use makepad_widgets::*;
use waml_markdown_editor::session::HostSnapshotCause;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, HeaderViewAction, RevealTarget, ViewData,
    ViewOutcome, ViewReconcilePolicy,
};
use crate::editor_session::{EditorSessionSnapshot, SessionChange};
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::popup::base::PopupResult;
use crate::source_view::SourceView;
use crate::view_history::ViewAnchor;

pub struct SourceToggleView<V: DocView> {
    inner: V,
    /// The raw-markdown surface behind the toggle -- the same editable source
    /// view every other door opens onto.
    source: SourceView,
    showing_source: bool,
}

impl<V: DocView> SourceToggleView<V> {
    pub fn new(
        inner: V,
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
    ) -> Self {
        Self {
            inner,
            source: SourceView::new_with_asset_host(concept_id, assets, emphasis),
            showing_source: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn showing_source(&self) -> bool {
        self.showing_source
    }

    #[cfg(test)]
    pub(crate) fn toggle_for_test(&mut self) {
        self.showing_source = !self.showing_source;
    }

    /// Force the raw-markdown surface on. The behavior canvas is a sibling of
    /// the canvas/markdown swap, so it must be dismissed explicitly first —
    /// `show_markdown_editor` alone would leave it drawn on top.
    fn assert_source_surface(&self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_behavior_canvas_visible(cx, false);
        body.show_markdown_editor(cx);
        body.markdown_editor().set_read_only(cx, false);
    }

    /// Hand the surface back to the wrapped view: generic canvas visibility
    /// first, then the view's own activation (the behavior view swaps to its
    /// canvas, the class diagram re-applies its canvas/properties mode).
    fn restore_inner_surface(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        body.show_canvas(cx);
        self.inner.on_activate(cx, body);
    }
}

impl<V: DocView> DocView for SourceToggleView<V> {
    fn identity(&self) -> DocViewIdentity {
        self.inner.identity()
    }

    fn reconcile_policy(&self) -> ViewReconcilePolicy {
        self.inner.reconcile_policy()
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        if self.showing_source {
            self.assert_source_surface(cx, body);
        } else {
            self.inner.sync(cx, body, data);
        }
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.inner.sync_from_session(cx, body, snapshot);
        self.source
            .install_snapshot(cx, body, snapshot, HostSnapshotCause::InitialLoad);
        // `SourceView::install_snapshot` unconditionally shows the raw-markdown
        // surface, and the inner sync shows its own — whichever surface this
        // wrapper has chosen must be applied LAST.
        if self.showing_source {
            self.assert_source_surface(cx, body);
        } else {
            self.sync(cx, body, snapshot.borrowed().into());
        }
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
        self.inner
            .after_session_snapshot(cx, body, snapshot, change);
        self.source.install_snapshot(cx, body, snapshot, cause);
        if self.showing_source {
            self.assert_source_surface(cx, body);
        } else {
            self.sync(cx, body, snapshot.borrowed().into());
        }
    }

    fn route_ui_event(&mut self, cx: &mut Cx, ui: &WidgetRef, event: &Event) {
        if self.showing_source {
            self.source.route_editor_event(cx, ui, event);
        } else {
            self.inner.route_ui_event(cx, ui, event);
        }
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        if body
            .header_view_action_button(cx)
            .as_icon_button()
            .clicked(actions)
        {
            self.showing_source = !self.showing_source;
            if self.showing_source {
                self.inner.on_deactivate(cx, body);
                self.assert_source_surface(cx, body);
            } else {
                self.restore_inner_surface(cx, body);
                self.inner.sync(cx, body, data);
            }
            body.apply_chrome(cx, self.chrome());
            return ViewOutcome::default();
        }
        if self.showing_source {
            if self.source.handle_emphasis_action(cx, body, actions) {
                body.apply_chrome(cx, self.chrome());
                return ViewOutcome::default();
            }
            self.source.handle(cx, body, actions, data)
        } else {
            self.inner.handle(cx, body, actions, data)
        }
    }

    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        self.inner.on_popup_result(cx, body, data, tag, result)
    }

    fn on_popup_armed(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        tag: LiveId,
        id: Option<LiveId>,
    ) -> ViewOutcome {
        self.inner.on_popup_armed(cx, body, data, tag, id)
    }

    fn chrome(&self) -> BodyChrome {
        let mut chrome = if self.showing_source {
            let inner = self.inner.chrome();
            BodyChrome {
                tool_dock: false,
                view_bar: false,
                canvas_overlays: false,
                document_header: inner.document_header,
            }
        } else {
            self.inner.chrome()
        };
        // Icon shows the surface the toggle LEADS to.
        chrome.document_header.view_toggle = Some(if self.showing_source {
            HeaderViewAction::destination(Icon::Eye, "View rendered")
        } else {
            HeaderViewAction::destination(Icon::Code, "View source")
        });
        // Emphasis restyles the raw markdown, so it exists only on that face.
        chrome.document_header.emphasis_toggle =
            self.showing_source.then(|| self.source.emphasis_action());
        // The source face is the shared markdown editor, same as SourceView.
        // The rendered face keeps whatever the WRAPPED view declares: a canvas
        // has no zoom of its own (spec §Scope), but the classifier preview
        // draws on the shared reading viewer and zooms with it -- and previews
        // are only ever constructed wrapped, so overwriting here is the only
        // value production would ever see.
        if self.showing_source {
            chrome.document_header.zoom = Some(crate::zoom::ZoomTarget::Source);
        }
        chrome
    }

    fn tab_accent(&self) -> Option<Vec4> {
        self.inner.tab_accent()
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        if self.showing_source {
            self.assert_source_surface(cx, body);
        } else {
            self.inner.on_activate(cx, body);
        }
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        self.inner.on_deactivate(cx, body);
    }

    fn on_escape(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        if self.showing_source {
            self.showing_source = false;
            self.restore_inner_surface(cx, body);
            body.apply_chrome(cx, self.chrome());
        } else {
            self.inner.on_escape(cx, body);
        }
    }

    /// Delegates to whichever surface is currently showing (spec
    /// §DocView::reveal) -- the raw-markdown surface while toggled to
    /// source, the wrapped view otherwise.
    fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &RevealTarget) {
        if self.showing_source {
            self.source.reveal(cx, body, target);
        } else {
            self.inner.reveal(cx, body, target);
        }
    }

    fn capture_anchor(&self, body: &BodyWidgets) -> ViewAnchor {
        self.inner.capture_anchor(body)
    }

    fn restore_anchor(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        data: ViewData<'_>,
        anchor: &ViewAnchor,
    ) -> bool {
        self.inner.restore_anchor(cx, body, data, anchor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_view::{DocumentHeaderChrome, HeaderViewAction};
    use waml_markdown_editor::syntax::{TextRange, TextSize};

    fn mounted_body(cx: &mut Cx) -> WidgetRef {
        waml_markdown_editor::live_design(cx);
        let markdown = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::widget::MarkdownEditor::script_new_with_default),
        ));
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(editor), markdown));
        let surface = WidgetRef::new_with_inner(Box::new(surface));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children.push((live_id!(markdown_surface), surface));
        WidgetRef::new_with_inner(Box::new(root))
    }

    fn wrapped_preview() -> SourceToggleView<crate::classifier_preview_view::ClassifierPreviewView>
    {
        SourceToggleView::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(
                "shop/order".into(),
                crate::document::NavCategory::Class,
            ),
            "shop/order".into(),
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    fn wrapped_diagram() -> SourceToggleView<crate::class_diagram_view::ClassDiagramView> {
        SourceToggleView::new(
            crate::class_diagram_view::ClassDiagramView::new(
                "orders".into(),
                crate::StructuralVisualKind::Class,
            ),
            "orders".into(),
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    #[test]
    fn a_wrapped_document_opens_on_its_own_surface() {
        let view = wrapped_preview();
        assert!(!view.showing_source());
        assert_eq!(
            view.identity(),
            DocViewIdentity::ClassifierPreview(crate::document::NavCategory::Class)
        );
    }

    #[test]
    fn the_toggle_decorates_the_inner_chrome_with_the_source_icon() {
        let view = wrapped_diagram();
        let chrome = view.chrome();
        // Everything except the toggle is the inner view's own chrome.
        assert!(chrome.tool_dock);
        assert!(chrome.view_bar);
        assert!(chrome.canvas_overlays);
        assert_eq!(
            chrome.document_header,
            DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::PanelRight),
                emphasis_toggle: None,
                view_toggle: Some(HeaderViewAction::destination(Icon::Code, "View source")),
                zoom: None,
            }
        );
    }

    #[test]
    fn showing_source_hides_the_diagram_chrome_and_flips_the_icon() {
        let mut view = wrapped_diagram();
        view.toggle_for_test();
        let chrome = view.chrome();
        assert!(!chrome.tool_dock);
        assert!(!chrome.view_bar);
        assert!(!chrome.canvas_overlays);
        assert_eq!(
            chrome.document_header,
            DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::PanelRight),
                view_toggle: Some(HeaderViewAction::destination(Icon::Eye, "View rendered")),
                // The emphasis button rides along with the source face only.
                emphasis_toggle: Some(HeaderViewAction {
                    icon: Icon::Type,
                    tooltip: "Use layout emphasis",
                    active: false,
                }),
                zoom: Some(crate::zoom::ZoomTarget::Source),
            }
        );
        view.toggle_for_test();
        assert_eq!(
            view.chrome().document_header.view_toggle,
            Some(HeaderViewAction::destination(Icon::Code, "View source"))
        );
        assert_eq!(view.chrome().document_header.emphasis_toggle, None);
        assert_eq!(view.chrome().document_header.zoom, None);
    }

    /// Every classifier preview is constructed WRAPPED, so the wrapper's
    /// rendered face is the only face production ever shows. Its body is the
    /// shared reading viewer, and overwriting the inner view's zoom target hid
    /// the `[-][%][+]` control and left the page at whatever scale the last
    /// reading tab set.
    #[test]
    fn the_rendered_face_keeps_the_wrapped_views_own_zoom_target() {
        let mut view = wrapped_preview();
        assert!(!view.showing_source());
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Reading)
        );
        // The source face is the shared markdown editor either way.
        view.toggle_for_test();
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Source)
        );
    }

    #[test]
    fn reveal_forwards_to_the_source_surface_while_toggled_to_source() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let ui = mounted_body(&mut cx);
        let body = BodyWidgets::new(&mut cx, &ui);
        let mut view = wrapped_diagram();
        view.toggle_for_test();
        assert!(view.showing_source());

        let target = RevealTarget::TextSpan { start: 1, end: 4 };
        view.reveal(&mut cx, &body, &target);

        let expected = TextRange::new(TextSize::new(1), TextSize::new(4)).unwrap();
        assert_eq!(
            body.markdown_editor().test_search_highlights(),
            vec![expected]
        );
    }

    #[test]
    fn identity_and_policy_are_the_wrapped_views_own() {
        let view = wrapped_diagram();
        assert_eq!(
            view.identity(),
            DocViewIdentity::StructuralDiagram(crate::StructuralVisualKind::Class)
        );
        assert_eq!(view.reconcile_policy(), view.inner.reconcile_policy());
    }
}
