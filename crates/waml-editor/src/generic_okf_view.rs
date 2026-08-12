use makepad_widgets::*;
use waml_markdown_editor::session::HostSnapshotCause;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, HeaderViewAction,
    RevealTarget, ViewData, ViewOutcome, ViewReconcilePolicy,
};
use crate::editor_session::{EditorSessionSnapshot, SessionChange};
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::source_view::SourceView;
use crate::view_history::ViewAnchor;

/// Breathing room left above a revealed match on the reading surface.
const REVEAL_TOP_MARGIN: f64 = 48.0;

pub struct GenericOkfView {
    /// The reading surface. A concept opens here.
    reading: crate::reading_view::ReadingView,
    /// The raw-markdown surface, reached by the explicit source toggle. It is
    /// the SAME editable surface a source tab opens on -- there is one source
    /// view with two doors, and it behaves identically through either.
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
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    pub fn new_with_asset_host(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
    ) -> Self {
        // Opening a concept is a reading action: it renders. Seeing the
        // markdown behind it is a separate, explicit action.
        Self::opened_on(concept_id, assets, emphasis, false)
    }

    /// Same view, opened on the raw-markdown face. This is what a source tab
    /// ("View Source", the tree's source surface) constructs: the destination
    /// differs, the view does not, so the rendered face stays one click away
    /// through the very same toggle a concept tab uses.
    pub fn new_source_with_asset_host(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
    ) -> Self {
        Self::opened_on(concept_id, assets, emphasis, true)
    }

    fn opened_on(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
        showing_source: bool,
    ) -> Self {
        let mut reading = crate::reading_view::ReadingView::new_with_asset_host(
            concept_id.clone(),
            assets.clone(),
        );
        reading.set_showing_source(showing_source);
        Self {
            reading,
            source: SourceView::new_with_asset_host(concept_id, assets, emphasis),
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
            body.markdown_editor().set_read_only(cx, false);
        } else {
            body.show_markdown_viewer(cx);
        }
        body.apply_chrome(cx, self.chrome());
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
            .header_view_action_button(cx)
            .as_icon_button()
            .clicked(actions)
        {
            self.toggle_source();
            self.sync(cx, body, data);
            return ViewOutcome::default();
        }
        // The emphasis button belongs to the source face only; `sync` re-pushes
        // the chrome so the button's lit state follows the new emphasis.
        if self.reading.showing_source() && self.source.handle_emphasis_action(cx, body, actions) {
            self.sync(cx, body, data);
            return ViewOutcome::default();
        }
        self.source.handle(cx, body, actions, data)
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                // Chrome follows the FACE, not the door: the raw-source face
                // keeps the inspector dock it has always had, the reading face
                // keeps having none. Both doors onto a face agree.
                right_dock: self.reading.showing_source().then_some(Icon::PanelRight),
                // Icon shows the surface the toggle LEADS to; `sync` keeps it
                // current when the user flips between them.
                view_toggle: Some(if self.reading.showing_source() {
                    HeaderViewAction::destination(Icon::Eye, "View rendered")
                } else {
                    HeaderViewAction::destination(Icon::Code, "View source")
                }),
                // Emphasis restyles the raw markdown, so it exists only while
                // that face is up.
                emphasis_toggle: self
                    .reading
                    .showing_source()
                    .then(|| self.source.emphasis_action()),
                zoom: Some(if self.reading.showing_source() {
                    crate::zoom::ZoomTarget::Source
                } else {
                    crate::zoom::ZoomTarget::Reading
                }),
            },
        }
    }

    fn tab_accent(&self) -> Option<Vec4> {
        crate::okf_documents::generic_okf_accent()
    }

    /// A search hit's `TextSpan` must land on BOTH of this view's surfaces:
    /// the reading viewer is what a concept opens on (so leaving it to
    /// `SourceView::reveal` alone would highlight a surface nobody is
    /// looking at), and the raw-source surface is one toggle away. A
    /// `ModelElement` target is a canvas concept both text surfaces ignore
    /// (spec §DocView::reveal).
    fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &RevealTarget) {
        let RevealTarget::TextSpan { start, end } = target else {
            return;
        };
        let Ok(range) = waml_markdown_editor::syntax::TextRange::new(
            waml_markdown_editor::syntax::TextSize::new(*start),
            waml_markdown_editor::syntax::TextSize::new(*end),
        ) else {
            return;
        };
        let viewer = body.markdown_viewer();
        viewer.set_search_highlights(cx, vec![range]);
        // Scroll the reading surface to the match, the counterpart of the
        // editor's `reveal_range` below. Measured from the LAST draw, so a
        // hit in a document that has not drawn yet simply stays put.
        if let Some(offset) = viewer.search_highlight_offset(cx) {
            body.scroll_markdown_viewer_to(cx, offset - REVEAL_TOP_MARGIN);
        }
        self.source.reveal(cx, body, target);
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

    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets) {
        waml_markdown_editor::live_design(cx);
        let view_toggle = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        let emphasis_toggle = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        let mut header =
            cx.with_vm(crate::document_header::DocumentHeader::script_new_with_default);
        header.test_mount_view_action_button(view_toggle);
        header.test_mount_emphasis_action_button(emphasis_toggle);
        let header = WidgetRef::new_with_inner(Box::new(header));
        let markdown = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::widget::MarkdownEditor::script_new_with_default),
        ));
        let mut surface = cx.with_vm(View::script_new_with_default);
        surface.children.push((live_id!(editor), markdown));
        let surface = WidgetRef::new_with_inner(Box::new(surface));
        // The reading surface, this view's DEFAULT destination -- a reveal
        // that only reached the raw editor would light a surface the user is
        // not looking at.
        let viewer = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::reading::MarkdownViewer::script_new_with_default),
        ));
        let mut viewer_body = cx.with_vm(View::script_new_with_default);
        viewer_body.children.push((live_id!(viewer), viewer));
        let viewer_body = WidgetRef::new_with_inner(Box::new(viewer_body));
        let mut viewer_surface = cx.with_vm(View::script_new_with_default);
        viewer_surface
            .children
            .push((live_id!(viewer_body), viewer_body));
        let viewer_surface = WidgetRef::new_with_inner(Box::new(viewer_surface));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children.push((live_id!(document_header), header));
        root.children.push((live_id!(markdown_surface), surface));
        root.children
            .push((live_id!(markdown_viewer_surface), viewer_surface));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body)
    }

    fn generic_view() -> GenericOkfView {
        GenericOkfView::new_with_asset_host(
            "runbook".into(),
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    fn source_first_view() -> GenericOkfView {
        GenericOkfView::new_source_with_asset_host(
            "runbook".into(),
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    /// The reported bug: flipping to source handed back a surface that could
    /// not be typed into, because this wrapper pinned the shared widget
    /// read-only and dropped the edit it produced.
    #[test]
    fn the_source_face_accepts_typing() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let session = crate::editor_session::EditorSession::default();
        let snapshot = session.snapshot();
        let mut view = generic_view();

        view.toggle_source();
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        assert!(
            !body.markdown_editor().read_only(),
            "the source face is an editor, not a viewer"
        );
    }

    /// Both doors land on the same face with the same chrome: whichever way
    /// you got here, `view_toggle` leads to the rendered prose.
    #[test]
    fn a_source_first_open_is_the_same_view_on_its_source_face() {
        let mut source_first = source_first_view();
        assert!(source_first.showing_source());
        assert_eq!(source_first.identity(), DocViewIdentity::GenericOkf);

        let mut toggled = generic_view();
        toggled.toggle_source();

        assert_eq!(source_first.chrome(), toggled.chrome());
        assert_eq!(
            source_first.chrome().document_header.view_toggle,
            Some(HeaderViewAction::destination(Icon::Eye, "View rendered"))
        );

        // And the Eye leads OUT of source, not to another source view.
        source_first.toggle_source();
        toggled.toggle_source();
        assert!(!source_first.showing_source());
        assert_eq!(source_first.chrome(), toggled.chrome());
        assert_eq!(
            source_first.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Reading)
        );
    }

    /// The emphasis button restyles the source face WITHOUT leaving it -- the
    /// behaviour that used to be bound to the `Eye`, where it read as "go to
    /// the rendered view" and delivered a differently-styled source view.
    #[test]
    fn the_emphasis_button_restyles_the_source_face_without_leaving_it() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let session = crate::editor_session::EditorSession::default();
        let snapshot = session.snapshot();
        let mut view = source_first_view();
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let emphasis = body.header_emphasis_action_button(&mut cx).as_icon_button();
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(crate::icon_button::IconButtonAction::Clicked),
            widget_uid: emphasis.widget_uid(),
            group: None,
        })];
        view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert!(
            view.showing_source(),
            "emphasis is a restyle, not a navigation"
        );
        assert_eq!(
            body.markdown_editor().emphasis(),
            waml_markdown_editor::EditorEmphasis::Layout
        );
        assert_eq!(
            view.chrome().document_header.emphasis_toggle,
            Some(HeaderViewAction {
                icon: Icon::Type,
                tooltip: "Use code emphasis",
                active: true,
            })
        );
    }

    #[test]
    fn generic_markdown_view_is_retained_by_construction() {
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
    fn chrome_zoom_target_follows_the_source_toggle() {
        let mut view = generic_view();
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Reading)
        );
        view.toggle_source();
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Source)
        );
        view.toggle_source();
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Reading)
        );
    }

    #[test]
    fn sync_projects_each_destination_through_the_shared_header() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (ui, body) = mounted_body(&mut cx);
        let session = crate::editor_session::EditorSession::default();
        let snapshot = session.snapshot();
        let mut view = generic_view();

        view.sync(&mut cx, &body, snapshot.borrowed().into());
        assert_eq!(
            ui.widget(&cx, ids!(document_header))
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("the shared header must be mounted")
                .test_view_toggle(),
            Some(HeaderViewAction::destination(Icon::Code, "View source"))
        );

        view.toggle_source();
        view.sync(&mut cx, &body, snapshot.borrowed().into());
        assert_eq!(
            ui.widget(&cx, ids!(document_header))
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("the shared header must be mounted")
                .test_view_toggle(),
            Some(HeaderViewAction::destination(Icon::Eye, "View rendered"))
        );
    }

    #[test]
    fn source_toggle_action_changes_only_the_generic_surface() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (ui, body) = mounted_body(&mut cx);
        let session = crate::editor_session::EditorSession::default();
        let snapshot = session.snapshot();
        let mut view = generic_view();
        body.markdown_editor()
            .set_emphasis(&mut cx, waml_markdown_editor::EditorEmphasis::Code);
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let toggle = body.header_view_action_button(&mut cx).as_icon_button();
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(crate::icon_button::IconButtonAction::Clicked),
            widget_uid: toggle.widget_uid(),
            group: None,
        })];
        assert!(
            toggle.clicked(&actions),
            "the action must target the toggle"
        );
        view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert!(
            view.showing_source(),
            "the click must reveal markdown source"
        );
        assert_eq!(
            body.markdown_editor().emphasis(),
            waml_markdown_editor::EditorEmphasis::Code,
            "the generic surface toggle must not also toggle emphasis"
        );
        let header = ui.widget(&cx, ids!(document_header));
        let header = header
            .borrow::<crate::document_header::DocumentHeader>()
            .expect("the shared header must be mounted");
        // The source face carries the inspector dock and the emphasis button;
        // the reading face carries neither. Chrome follows the face.
        assert_eq!(header.test_right_dock(), Some(Icon::PanelRight));
        assert_eq!(
            header.test_view_toggle(),
            Some(HeaderViewAction::destination(Icon::Eye, "View rendered"))
        );
        assert_eq!(
            header.test_emphasis_toggle(),
            Some(HeaderViewAction {
                icon: Icon::Type,
                tooltip: "Use layout emphasis",
                active: false,
            })
        );
    }

    #[test]
    fn reveal_lights_the_reading_surface_as_well_as_the_raw_source() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = generic_view();
        assert!(!view.showing_source(), "a concept opens as rendered prose");
        assert!(body.markdown_viewer().test_search_highlights().is_empty());

        view.reveal(&mut cx, &body, &RevealTarget::TextSpan { start: 2, end: 7 });

        let expected = waml_markdown_editor::syntax::TextRange::new(
            waml_markdown_editor::syntax::TextSize::new(2),
            waml_markdown_editor::syntax::TextSize::new(7),
        )
        .unwrap();
        assert_eq!(
            body.markdown_viewer().test_search_highlights(),
            vec![expected],
            "the surface a concept actually opens on must show the match"
        );
        assert_eq!(
            body.markdown_editor().test_search_highlights(),
            vec![expected],
            "and the source surface, one toggle away, must agree"
        );
    }

    #[test]
    fn reveal_ignores_a_model_element_target() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = generic_view();

        view.reveal(
            &mut cx,
            &body,
            &RevealTarget::ModelElement {
                key: "order".into(),
            },
        );

        assert!(body.markdown_viewer().test_search_highlights().is_empty());
        assert!(body.markdown_editor().test_search_highlights().is_empty());
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
                    emphasis_toggle: None,
                    view_toggle: Some(HeaderViewAction::destination(Icon::Code, "View source")),
                    zoom: Some(crate::zoom::ZoomTarget::Reading),
                },
            }
        );
        assert_eq!(
            view.tab_accent(),
            crate::okf_documents::generic_okf_accent()
        );
    }
}
