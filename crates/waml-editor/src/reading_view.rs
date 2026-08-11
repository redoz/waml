//! The `waml-editor` side of the markdown reading view.
//!
//! Mirrors `SourceView`, but installs a `ReadingDocument` on the
//! `MarkdownViewer` instead of an `InstalledPresentation` on the editor. The
//! two surfaces share the parse, the compile and the styles; they share no
//! layout engine.

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{compile_presentation, PresentationStyles};
use waml_markdown_editor::reading::{
    build_reading_document, BlockExtensionFrame, ReadingDocument, RegisteredBlockExtensions,
};

use crate::doc_view::BodyWidgets;
use crate::editor_session::EditorSessionSnapshot;
use crate::markdown_hosts::{SharedMarkdownAssetHost, WamlCodeHighlightHost};
use crate::source_view::SourceView;

pub struct ReadingView {
    key: String,
    /// `true` once the reader has asked to see the markdown source. The
    /// editor side stays read-only: this toggles RENDERING, not writability.
    showing_source: bool,
    document: Option<Arc<ReadingDocument>>,
    /// The source `document` was compiled from, kept so a re-activation can
    /// tell whether the SHARED viewer is still showing this view's own page
    /// (`Arc::ptr_eq`) and put it back when it is not.
    source: Option<Arc<str>>,
    revision: Option<waml_markdown_editor::syntax::DocumentRevision>,
}

impl ReadingView {
    /// `assets` is accepted for symmetry with `SourceView::new_with_asset_host`
    /// and because a future task wires embedded-image assets into the reading
    /// view; this task's viewer does not resolve embedded images yet, so no
    /// lease is opened against it.
    pub fn new_with_asset_host(key: String, _assets: SharedMarkdownAssetHost) -> ReadingView {
        ReadingView {
            key,
            showing_source: false,
            document: None,
            source: None,
            revision: None,
        }
    }

    /// The concept this view renders. A link tapped on the reading surface is
    /// resolved RELATIVE to it.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn showing_source(&self) -> bool {
        self.showing_source
    }

    pub fn set_showing_source(&mut self, showing: bool) {
        self.showing_source = showing;
    }

    /// Rebuild this view's document when the source moved, then put it back on
    /// the reading surface.
    ///
    /// The reconcile is unconditional because `body.markdown_viewer()` is ONE
    /// widget shared by every reading tab (a concept tab, a classifier
    /// preview): gating on the revision alone early-returns when this tab is
    /// re-activated at an unchanged revision, leaving whatever the previously
    /// active tab installed on screen, captioned as this document.
    pub fn install_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.rebuild_document(snapshot);
        let (Some(document), Some(source)) = (self.document.clone(), self.source.clone()) else {
            // Nothing of our own to show yet -- the surface is left alone
            // rather than cleared, so a compile failure keeps the last good
            // revision up (see the logs below).
            return;
        };
        let viewer = body.markdown_viewer();
        let mine_is_up = viewer
            .installed_source()
            .is_some_and(|installed| Arc::ptr_eq(&installed, &source));
        if !mine_is_up {
            viewer.install_document(
                cx,
                document,
                source,
                Arc::new(BlockExtensionFrame {
                    revision: self.revision.expect("document has a revision"),
                    items: Arc::from([]),
                }),
            );
        }
    }

    /// Recompile `document`/`source` from the snapshot when the revision moved.
    /// A failure keeps the previous pair, which is at least this document's own.
    fn rebuild_document(&mut self, snapshot: &EditorSessionSnapshot) {
        let Some((_document, syntax)) = SourceView::resolve_document(snapshot, &self.key) else {
            return;
        };
        let revision = syntax.revision();
        if self.revision == Some(revision) {
            return;
        }
        let styles = Arc::new(PresentationStyles::balanced());
        let highlighters = WamlCodeHighlightHost::registry(Arc::new(snapshot.clone()));
        // On failure the viewer keeps showing the previous revision; say so,
        // or the stale surface is indistinguishable from a current one.
        let plan = match compile_presentation(&syntax, &styles, &highlighters) {
            Ok(plan) => plan,
            Err(error) => {
                log!(
                    "reading view {}: presentation compile failed, keeping the previous revision: {error:?}",
                    self.key
                );
                return;
            }
        };
        let document = match build_reading_document(&plan, &RegisteredBlockExtensions::default()) {
            Ok(document) => document,
            Err(error) => {
                log!(
                    "reading view {}: reading model build failed, keeping the previous revision: {error:?}",
                    self.key
                );
                return;
            }
        };
        let source: Arc<str> = Arc::from(syntax.text().shared().as_str());
        self.revision = Some(revision);
        self.document = Some(Arc::new(document));
        self.source = Some(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_session::EditorSession;
    use crate::markdown_hosts::{EditorMarkdownAssetHost, MarkdownAssetPolicy};

    /// Only the shared reading surface: this view installs nothing else.
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
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children
            .push((live_id!(markdown_viewer_surface), viewer_surface));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body)
    }

    fn session_with_two_concepts() -> EditorSession {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("runbook.md", "# Runbook\n\nStep one.\n"),
            ("glossary.md", "# Glossary\n\nTerms.\n"),
        ])
        .unwrap();
        let mut session = EditorSession::default();
        session.replace(source).unwrap();
        session
    }

    fn reading_view(key: &str) -> ReadingView {
        ReadingView::new_with_asset_host(
            key.into(),
            EditorMarkdownAssetHost::shared(MarkdownAssetPolicy::BrowserBundle),
        )
    }

    /// Two reading tabs at the SAME revision share ONE viewer widget:
    /// re-activating the first must put its own document back. A
    /// revision-only gate early-returns here and leaves the second tab's page
    /// on screen, captioned as the first document.
    #[test]
    fn re_activating_a_tab_at_an_unchanged_revision_reinstalls_its_own_document() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_two_concepts();
        let snapshot = session.snapshot();
        let mut runbook = reading_view("runbook");
        let mut glossary = reading_view("glossary");

        runbook.install_snapshot(&mut cx, &body, &snapshot);
        assert!(body
            .markdown_viewer()
            .installed_source()
            .expect("the first tab installed its document")
            .starts_with("# Runbook\n"),);

        glossary.install_snapshot(&mut cx, &body, &snapshot);
        assert!(body
            .markdown_viewer()
            .installed_source()
            .expect("the second tab took the shared surface")
            .starts_with("# Glossary\n"),);

        runbook.install_snapshot(&mut cx, &body, &snapshot);

        assert!(
            body.markdown_viewer()
                .installed_source()
                .expect("the first tab reinstalled its document")
                .starts_with("# Runbook\n"),
            "re-activating the first tab must put ITS document back"
        );
    }

    /// A classifier preview clears the shared viewer when it has no page of
    /// its own; the reading tab behind it must come back, not stay blank.
    #[test]
    fn a_cleared_surface_is_repopulated_on_re_activation() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_two_concepts();
        let snapshot = session.snapshot();
        let mut runbook = reading_view("runbook");

        runbook.install_snapshot(&mut cx, &body, &snapshot);
        body.markdown_viewer().clear_document(&mut cx);

        runbook.install_snapshot(&mut cx, &body, &snapshot);

        assert!(body
            .markdown_viewer()
            .installed_source()
            .expect("the reading tab must restore its own document")
            .starts_with("# Runbook\n"),);
    }
}
