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
use waml_markdown_editor::reading::build_reading_document;
use waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

pub struct ClassifierPreviewView {
    key: String,
    category: NavCategory,
    /// The markdown installed on the viewer, kept so `handle` can resolve a
    /// clicked link and so a re-sync at the same revision is a no-op.
    page: Option<Arc<str>>,
    /// The session revision `page` was generated from.
    installed_revision: Option<u64>,
}

impl ClassifierPreviewView {
    pub fn new(key: String, category: NavCategory) -> ClassifierPreviewView {
        ClassifierPreviewView {
            key,
            category,
            page: None,
            installed_revision: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_page(&self) -> Option<&str> {
        self.page.as_deref()
    }

    /// Generate the page and install it on the reading surface. A failure to
    /// parse or compile leaves the previous page up and says so: a stale
    /// surface is otherwise indistinguishable from a current one.
    fn install_page(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        if self.installed_revision == Some(data.revision) {
            return;
        }
        self.installed_revision = Some(data.revision);
        let Some(markdown) =
            waml::classifier_page::classifier_page(&data.uml_analysis.projection, &self.key)
        else {
            self.page = None;
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
        let source: Arc<str> = Arc::from(markdown.as_str());
        self.page = Some(source.clone());
        body.markdown_viewer()
            .install_document(cx, Arc::new(document), source);
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

    #[test]
    fn chrome_is_unchanged_by_the_surface_swap() {
        let view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        let chrome = view.chrome();
        assert!(!chrome.tool_dock);
        assert!(!chrome.view_bar);
        assert!(chrome.document_header.breadcrumb);
        assert_eq!(chrome.document_header.right_dock, Some(Icon::PanelRight));
    }
}
