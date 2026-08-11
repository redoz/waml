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
            crate::markdown_extensions::EditorMarkdownExtensionHost::shared(),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    pub fn new_with_asset_host(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
        extensions: crate::markdown_extensions::SharedMarkdownExtensionHost,
        emphasis: waml_markdown_editor::EditorEmphasis,
    ) -> Self {
        // Opening a concept is a reading action: it renders. Seeing the
        // markdown behind it is a separate, explicit action.
        Self {
            reading: crate::reading_view::ReadingView::new_with_extension_host(
                concept_id.clone(),
                assets.clone(),
                extensions,
            ),
            source: SourceView::new_read_only(concept_id, assets, emphasis),
        }
    }

    #[cfg(test)]
    pub(crate) fn showing_source(&self) -> bool {
        self.reading.showing_source()
    }

    #[cfg(test)]
    pub(crate) fn extensions_pending_for_test(&self) -> usize {
        self.reading.extensions_pending_for_test()
    }

    pub(crate) fn toggle_source(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let showing = self.reading.showing_source();
        if !showing {
            let offset = self.reading.caret_for_handoff(cx, body);
            let offset = u32::try_from(offset.to_usize())
                .expect("Markdown source offsets fit the editor's u32 reveal boundary");
            self.source.reveal(
                cx,
                body,
                &RevealTarget::TextSpan {
                    start: offset,
                    end: offset,
                },
            );
        }
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
        let body = BodyWidgets::new(cx, ui);
        self.reading.handle_event(cx, &body, event);
        self.source.route_editor_event(cx, ui, event);
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = body;
        #[cfg(target_arch = "wasm32")]
        self.reading.on_activate(cx);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = cx;
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
            self.toggle_source(cx, body);
            self.sync(cx, body, data);
            return ViewOutcome::default();
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
    use std::{cell::Cell, time::Duration};

    use super::*;
    use waml::source::SourceBundle;

    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets) {
        waml_markdown_editor::live_design(cx);
        let view_toggle = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        let mut header =
            cx.with_vm(crate::document_header::DocumentHeader::script_new_with_default);
        header.test_mount_view_action_button(view_toggle);
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
            crate::markdown_extensions::EditorMarkdownExtensionHost::shared(),
            waml_markdown_editor::EditorEmphasis::Code,
        )
    }

    fn draw_viewer(cx: &mut Cx, ui: &WidgetRef) {
        let draw_event = DrawEvent {
            redraw_all: true,
            ..DrawEvent::default()
        };
        let pass = DrawPass::new_with_name(cx, "generic-source-handoff");
        let mut draw_list = DrawList2d::new(cx);
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        draw_cx.begin_pass(&pass, None);
        draw_list.begin_always(&mut draw_cx);
        {
            let mut cx_2d = Cx2d::new(&mut draw_cx);
            cx_2d.begin_root_turtle(dvec2(500.0, 700.0), Layout::default());
            ui.widget(&cx_2d, ids!(markdown_viewer_surface.viewer_body.viewer))
                .draw_walk_all(
                    &mut cx_2d,
                    &mut Scope::empty(),
                    Walk::abs_rect(Rect {
                        pos: dvec2(0.0, 0.0),
                        size: dvec2(500.0, 700.0),
                    }),
                );
            cx_2d.end_turtle();
            draw_list.end(&mut cx_2d);
        }
        draw_cx.end_pass(&pass);
    }

    fn mouse_down(abs: DVec2) -> Event {
        Event::MouseDown(MouseDownEvent {
            abs,
            button: MouseButton::PRIMARY,
            window_id: WindowId(0, 0),
            modifiers: KeyModifiers::default(),
            handled: Cell::new(Area::default()),
            time: 0.0,
        })
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
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let (_ui, body) = mounted_body(&mut cx);
        let mut view = generic_view();
        view.toggle_source(&mut cx, &body);
        assert!(
            view.showing_source(),
            "the toggle reveals the markdown source"
        );
        view.toggle_source(&mut cx, &body);
        assert!(!view.showing_source(), "and puts it back");
    }

    #[test]
    fn selecting_a_mermaid_diagram_hands_its_fence_start_to_the_source_caret() {
        let source = "---\ntype: Runbook\n---\n# Runbook\n\nBefore.\n\n```mermaid\nflowchart LR\n    A --> B\n```\n\nAfter.\n";
        let fence_start = source.find("```mermaid").unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("runbook.md", source)]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (ui, body) = mounted_body(&mut cx);
        let mut view = generic_view();
        view.sync_from_session(&mut cx, &body, &snapshot);

        draw_viewer(&mut cx, &ui);
        let viewer = body.markdown_viewer();
        let whole_source = waml_markdown_editor::syntax::TextRange::new(
            waml_markdown_editor::syntax::TextSize::try_from_usize(0).unwrap(),
            waml_markdown_editor::syntax::TextSize::try_from_usize(source.len()).unwrap(),
        )
        .unwrap();
        let rect = viewer
            .borrow()
            .expect("the viewer is mounted")
            .source_map()
            .visual_rects_for_source(whole_source)[0];
        view.route_ui_event(&mut cx, &ui, &mouse_down(rect.pos + rect.size * 0.5));
        assert_eq!(
            view.reading.caret_for_handoff(&cx, &body).to_usize(),
            fence_start,
            "the reading view retains the full fence start before it drains completions"
        );

        for _ in 0..400 {
            view.route_ui_event(&mut cx, &ui, &Event::Signal);
            if view.extensions_pending_for_test() == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            view.extensions_pending_for_test(),
            0,
            "the completion must be installed after the visual selection"
        );

        view.toggle_source(&mut cx, &body);

        let caret = waml_markdown_editor::syntax::TextSize::try_from_usize(fence_start).unwrap();
        let expected = waml_markdown_editor::syntax::TextRange::new(caret, caret).unwrap();
        assert_eq!(
            body.markdown_editor().test_search_highlights(),
            vec![expected],
            "the read-only source editor receives the reading selection caret"
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
            Some(HeaderViewAction {
                icon: Icon::Code,
                tooltip: "View source",
            })
        );

        view.toggle_source(&mut cx, &body);
        view.sync(&mut cx, &body, snapshot.borrowed().into());
        assert_eq!(
            ui.widget(&cx, ids!(document_header))
                .borrow::<crate::document_header::DocumentHeader>()
                .expect("the shared header must be mounted")
                .test_view_toggle(),
            Some(HeaderViewAction {
                icon: Icon::Eye,
                tooltip: "View rendered",
            })
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
        assert_eq!(header.test_right_dock(), None);
        assert_eq!(
            header.test_view_toggle(),
            Some(HeaderViewAction {
                icon: Icon::Eye,
                tooltip: "View rendered",
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
