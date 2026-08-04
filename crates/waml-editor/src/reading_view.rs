//! The `waml-editor` side of the markdown reading view.
//!
//! Mirrors `SourceView`, but installs a `ReadingDocument` on the
//! `MarkdownViewer` instead of an `InstalledPresentation` on the editor. The
//! two surfaces share the parse, the compile and the styles; they share no
//! layout engine.

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{compile_presentation, PresentationStyles};
use waml_markdown_editor::reading::{build_reading_document, ReadingDocument};

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
            revision: None,
        }
    }

    pub fn showing_source(&self) -> bool {
        self.showing_source
    }

    pub fn set_showing_source(&mut self, showing: bool) {
        self.showing_source = showing;
    }

    pub fn install_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        let Some((_document, syntax)) = SourceView::resolve_document(snapshot, &self.key) else {
            return;
        };
        if self.revision == Some(syntax.revision()) {
            return;
        }
        let styles = Arc::new(PresentationStyles::balanced());
        let highlighters = WamlCodeHighlightHost::registry(Arc::new(snapshot.clone()));
        let Ok(plan) = compile_presentation(&syntax, &styles, &highlighters) else {
            return;
        };
        let Ok(document) = build_reading_document(&plan) else {
            return;
        };
        let source: Arc<str> = Arc::from(syntax.text().shared().as_str());
        self.revision = Some(syntax.revision());
        self.document = Some(Arc::new(document));
        body.markdown_viewer().install_document(
            cx,
            self.document.clone().expect("just installed"),
            source,
        );
    }
}
