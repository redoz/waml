//! The `waml-editor` side of the book surface. Mirrors the split
//! `ReadingView` uses: the model is compiled from the session snapshot
//! (revision-guarded; a failed rebuild keeps the previous revision and says
//! so), and `sync` only pushes already-built state at the shared body.

use std::rc::Rc;

use makepad_widgets::*;

use crate::book_model::{build_book, BookModel};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, HeaderViewAction,
    ViewData, ViewOutcome,
};
use crate::editor_session::EditorSessionSnapshot;
use crate::icons::Icon;

pub struct BookView {
    directory: String,
    limits: waml::view::chain::ChainLimits,
    /// The mask the book was opened under -- the session's one mask, carried
    /// like `OpenCtx.mask` so the book lists what the tree lists.
    mask: waml::view::mask::ProjectionMask,
    model: Option<Rc<BookModel>>,
    revision: Option<u64>,
}

impl BookView {
    pub fn new(
        directory: String,
        limits: waml::view::chain::ChainLimits,
        mask: waml::view::mask::ProjectionMask,
    ) -> BookView {
        BookView {
            directory,
            limits,
            mask,
            model: None,
            revision: None,
        }
    }

    // Consumed by book_surface.rs in Task 5, which removes this allow --
    // clippy's lib target does not count the unit test below as a caller.
    #[allow(dead_code)]
    pub(crate) fn model(&self) -> Option<&Rc<BookModel>> {
        self.model.as_ref()
    }

    fn install_snapshot(&mut self, snapshot: &EditorSessionSnapshot) {
        let revision = snapshot.borrowed().revision;
        if self.revision == Some(revision) {
            return;
        }
        // On failure keep the previous model -- the same posture as
        // ReadingView: a stale book beats a blank one, and the next good
        // snapshot heals it.
        match build_book(snapshot, &self.directory, self.limits, &self.mask) {
            Some(model) => {
                self.revision = Some(revision);
                self.model = Some(Rc::new(model));
            }
            None => log!(
                "book view {}: directory left the bundle, keeping the previous sections",
                self.directory
            ),
        }
    }
}

impl DocView for BookView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::Book
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_book_view(cx);
        // Task 5 pushes self.model at the BookSurface widget here.
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.install_snapshot(snapshot);
        self.sync(cx, body, snapshot.borrowed().into());
    }

    fn after_session_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
        _change: crate::editor_session::SessionChange,
    ) {
        self.install_snapshot(snapshot);
        self.sync(cx, body, snapshot.borrowed().into());
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        // Task 4: header toggle. Task 6: link/open-full clicks. Task 8: fold marking.
        ViewOutcome::default()
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: None,
                // The destination the toggle LEADS to: the plain listing of
                // the same directory -- the same shape as the source toggle.
                view_toggle: Some(HeaderViewAction {
                    icon: Icon::Folder,
                    tooltip: "View folder listing",
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    /// Same fixture as `book_model.rs`'s `book_source` -- duplicated rather
    /// than imported, since test modules may not import each other's.
    fn book_source() -> SourceBundle {
        SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Guide](guide/)\n* [Loose](loose.md)\n"),
            (
                "guide/index.md",
                "---\nview: book\n---\n# Guide\n\n* [Intro](intro.md)\n* [Flow](flow.md)\n* [Deep](deep/)\n* [Inner](inner/)\n",
            ),
            ("guide/intro.md", "# Intro\n\nSome prose.\n"),
            (
                "guide/flow.md",
                "---\ntype: uml.ClassDiagram\ntitle: Flow\n---\n# Flow\n",
            ),
            ("guide/deep/index.md", "# Deep\n\n* [Leaf](leaf.md)\n"),
            ("guide/deep/leaf.md", "# Leaf\n"),
            (
                "guide/inner/index.md",
                "---\nview: book\n---\n# Inner\n\n* [Nested](nested.md)\n",
            ),
            ("guide/inner/nested.md", "# Nested\n"),
            ("loose.md", "# Loose\n"),
        ])
        .unwrap()
    }

    #[test]
    fn install_snapshot_rebuilds_only_when_the_revision_moves() {
        let mut session = crate::editor_session::EditorSession::default();
        session.replace(book_source()).unwrap();
        let snapshot = session.snapshot();
        let mut view = BookView::new(
            "/guide".to_string(),
            waml::view::chain::ChainLimits::default(),
            waml::view::mask::ProjectionMask::default(),
        );
        view.install_snapshot(&snapshot);
        let first = Rc::as_ptr(view.model().unwrap());
        view.install_snapshot(&snapshot);
        assert_eq!(
            first,
            Rc::as_ptr(view.model().unwrap()),
            "same revision, same model"
        );
    }
}
