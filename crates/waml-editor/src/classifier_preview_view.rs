//! `ClassifierPreviewView` — the single-element preview: the classifier's own
//! generated documentation page on the reading surface, plus the
//! inspector-without-picker. No canvas, no tool dock.

use std::sync::Arc;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, RevealTarget,
    ViewData, ViewOutcome,
};
use crate::document::NavCategory;
use crate::icons::Icon;
use crate::inspector::Subject;
use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{build_reading_document, ReadingDocument};
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

pub struct ClassifierPreviewView {
    key: String,
    category: NavCategory,
    /// The markdown this tab owns, kept so `handle` can resolve a clicked
    /// link and so a re-sync at the same revision needs no regeneration.
    page: Option<Arc<str>>,
    /// The compiled form of `page`, kept for the same reason: the reading
    /// viewer is SHARED between tabs, so re-activating this one has to be
    /// able to put its own document back without recompiling it.
    document: Option<Arc<ReadingDocument>>,
    /// The session revision `page` was generated from.
    installed_revision: Option<u64>,
}

impl ClassifierPreviewView {
    pub fn new(key: String, category: NavCategory) -> ClassifierPreviewView {
        ClassifierPreviewView {
            key,
            category,
            page: None,
            document: None,
            installed_revision: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_page(&self) -> Option<&str> {
        self.page.as_deref()
    }

    /// Generate the page when the model moved, then put this tab's own page
    /// on the reading surface.
    ///
    /// The reconcile is unconditional because `body.markdown_viewer()` is ONE
    /// widget shared by every reading tab: gating on the revision alone would
    /// early-return when this tab is re-activated at an unchanged revision,
    /// leaving whatever the previously active tab installed on screen,
    /// captioned as this classifier.
    fn install_page(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        if self.installed_revision != Some(data.revision) {
            self.installed_revision = Some(data.revision);
            self.rebuild_page(data);
        }
        let viewer = body.markdown_viewer();
        match (self.page.clone(), self.document.clone()) {
            (Some(source), Some(document)) => {
                let mine_is_up = viewer
                    .installed_source()
                    .is_some_and(|installed| Arc::ptr_eq(&installed, &source));
                if !mine_is_up {
                    viewer.install_document(cx, document, source);
                }
            }
            // No page of our own: a package preview, an unknown key, or a
            // classifier deleted from the model. An empty surface is the
            // honest answer -- the alternative is another tab's document
            // standing in for this one.
            _ => viewer.clear_document(cx),
        }
    }

    /// Regenerate `page`/`document` from the current model. A failure to
    /// parse or compile keeps the previous pair and says so: that page is at
    /// least this classifier's own, where a cleared surface would look like
    /// the classifier had vanished.
    fn rebuild_page(&mut self, data: ViewData<'_>) {
        let Some(markdown) =
            waml::classifier_page::classifier_page(&data.uml_analysis.projection, &self.key)
        else {
            self.page = None;
            self.document = None;
            return;
        };
        let Ok(text) = SourceText::new(markdown.clone()) else {
            log!(
                "classifier preview {}: generated page is not valid source text",
                self.key
            );
            return;
        };
        let syntax = match parse_markdown(
            DocumentRevision::INITIAL,
            text,
            MarkdownDialect::WAML_DEFAULT,
        ) {
            Ok(syntax) => syntax,
            Err(error) => {
                log!(
                    "classifier preview {}: generated page did not parse: {error:?}",
                    self.key
                );
                return;
            }
        };
        let styles = Arc::new(PresentationStyles::balanced());
        let plan = match compile_presentation(&syntax, &styles, &HighlighterRegistry::default()) {
            Ok(plan) => plan,
            Err(error) => {
                log!(
                    "classifier preview {}: presentation compile failed: {error:?}",
                    self.key
                );
                return;
            }
        };
        let document = match build_reading_document(&plan) {
            Ok(document) => document,
            Err(error) => {
                log!(
                    "classifier preview {}: reading model build failed: {error:?}",
                    self.key
                );
                return;
            }
        };
        self.page = Some(Arc::from(markdown.as_str()));
        self.document = Some(Arc::new(document));
    }
}

impl DocView for ClassifierPreviewView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::ClassifierPreview(self.category)
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_markdown_viewer(cx);
        self.install_page(cx, body, data);
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

        // A tap on a classifier name in the page. Resolved here, against this
        // classifier's own concept id, because a node key IS its concept id.
        // An unresolvable href is handed to the app instead: only its
        // `handle_navigation_intent` puts `NavigationError::status_message`
        // on the status bar, and a silent drop reads as a dead click.
        if let Some(href) = body.markdown_viewer().link_clicked(actions) {
            let href = href.to_string();
            out.navigation = Some(
                match crate::navigation::resolve_link(&data.okf_analysis.bundle, &self.key, &href) {
                    Ok(target) => crate::navigation::NavigationIntent::Resolved {
                        target,
                        disposition: crate::navigation::OpenDisposition::Preview,
                    },
                    Err(_) => crate::navigation::NavigationIntent::MarkdownLink {
                        current_concept_id: self.key.clone(),
                        href,
                    },
                },
            );
            return out;
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

        let _ = data;
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
                // The body IS the shared reading viewer, so it zooms with the
                // reading ladder. Declaring `None` hid the `[-][%][+]` control
                // and left the widget at whatever scale the last reading tab
                // set on it, with no way to correct it.
                zoom: Some(crate::zoom::ZoomTarget::Reading),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_session::EditorSession;

    /// The two surfaces this view arbitrates between, plus the inspector whose
    /// subject must survive the swap.
    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets) {
        waml_markdown_editor::live_design(cx);
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
        let inspector = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::inspector_panel::Inspector::script_new_with_default),
        ));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children
            .push((live_id!(markdown_viewer_surface), viewer_surface));
        root.children.push((live_id!(inspector), inspector));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body)
    }

    fn session_with_order() -> EditorSession {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
        ])
        .unwrap();
        let mut session = EditorSession::default();
        session.replace(source).unwrap();
        session
    }

    #[test]
    fn sync_installs_the_generated_page_and_keeps_the_inspector_subject() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("order".into(), NavCategory::Class);

        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let page = view.test_page().expect("sync must generate a page");
        assert!(page.starts_with("# Order\n"), "page was:\n{page}");
        assert!(
            page.contains("- Associated with one [Customer](/customer.md)."),
            "page was:\n{page}"
        );
        let inspector = ui.widget(&cx, ids!(inspector));
        let inspector = inspector
            .borrow::<crate::inspector_panel::Inspector>()
            .expect("the inspector is mounted");
        assert_eq!(inspector.subject_key_for_test().as_deref(), Some("order"));
    }

    use crate::navigation::{NavigationIntent, NavigationTarget, OpenDisposition};

    /// Synthesize the action the viewer posts on a tap, without a live pointer.
    fn link_click(body: &BodyWidgets, href: &str) -> ActionsBuf {
        let viewer = body.markdown_viewer();
        vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(
                waml_markdown_editor::reading::MarkdownViewerAction::LinkClicked {
                    destination: Arc::from(href),
                },
            ),
            widget_uid: viewer.widget_uid(),
            group: None,
        })]
    }

    #[test]
    fn a_link_click_resolves_to_the_target_document() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let actions = link_click(&body, "/customer.md");
        let outcome = view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Document {
                    concept_id: "customer".into(),
                    surface: None,
                    fragment: None,
                },
                disposition: OpenDisposition::Preview,
            })
        );
    }

    #[test]
    fn an_unresolvable_link_defers_to_the_app_so_the_reader_is_told() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let actions = link_click(&body, "/missing.md");
        let outcome = view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::MarkdownLink {
                current_concept_id: "order".into(),
                href: "/missing.md".into(),
            })
        );
    }

    #[test]
    fn a_key_that_names_no_classifier_installs_nothing() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("nope".into(), NavCategory::Class);

        view.sync(&mut cx, &body, snapshot.borrowed().into());

        assert!(view.test_page().is_none());
    }

    /// The viewer is ONE widget shared by every reading tab, so a tab with no
    /// page of its own must clear it -- not leave the previously active tab's
    /// document up, captioned as this classifier.
    #[test]
    fn a_key_that_names_no_classifier_clears_the_shared_surface() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut order = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        let mut missing = ClassifierPreviewView::new("nope".into(), NavCategory::Class);

        order.sync(&mut cx, &body, snapshot.borrowed().into());
        assert!(body.markdown_viewer().installed_source().is_some());

        missing.sync(&mut cx, &body, snapshot.borrowed().into());

        assert_eq!(body.markdown_viewer().installed_source(), None);
    }

    /// Two preview tabs at the SAME revision: re-activating the first must put
    /// its own page back. A revision-only gate early-returns here and leaves
    /// the second tab's page on the shared viewer.
    #[test]
    fn re_activating_a_tab_at_an_unchanged_revision_reinstalls_its_own_page() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut order = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        let mut customer = ClassifierPreviewView::new("customer".into(), NavCategory::Class);

        order.sync(&mut cx, &body, snapshot.borrowed().into());
        customer.sync(&mut cx, &body, snapshot.borrowed().into());
        assert!(
            body.markdown_viewer()
                .installed_source()
                .expect("customer installed its page")
                .starts_with("# Customer\n"),
            "the second tab owns the surface"
        );

        order.sync(&mut cx, &body, snapshot.borrowed().into());

        assert!(
            body.markdown_viewer()
                .installed_source()
                .expect("order reinstalled its page")
                .starts_with("# Order\n"),
            "re-activating the first tab must put ITS page back"
        );
    }

    #[test]
    fn chrome_is_unchanged_by_the_surface_swap() {
        let view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        let chrome = view.chrome();
        assert!(!chrome.tool_dock);
        assert!(!chrome.view_bar);
        assert!(chrome.document_header.breadcrumb);
        assert_eq!(chrome.document_header.right_dock, Some(Icon::PanelRight));
    }

    /// The preview's body is the SHARED reading viewer. Declaring no zoom
    /// target hides the `[-][%][+]` control and leaves the widget at whatever
    /// scale the last reading tab left on it, with the persisted reading zoom
    /// never applied.
    #[test]
    fn the_preview_zooms_with_the_reading_surface_it_draws_on() {
        let view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        assert_eq!(
            view.chrome().document_header.zoom,
            Some(crate::zoom::ZoomTarget::Reading)
        );
    }
}
