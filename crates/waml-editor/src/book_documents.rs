//! Document provider for a folder's BOOK surface (spec
//! 2026-08-11-book-mode-design). Keyed on a directory address like
//! `folder_documents`, but a different surface: the same directory's book
//! tab and listing tab are two tabs (`tab_id_for` bakes the surface in).
//!
//! Deliberately opens ANY directory in the bundle, declared or not: a stored
//! book locator (history, reopened tab) must keep opening after the folder's
//! declaration is edited away -- the `view: book` declaration is a CLICK
//! routing decision (`App::primary_folder_locator`), not a capability gate.

use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::Icon;
use crate::navigation::DocumentLocator;

pub fn describe(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
) -> Option<DocumentDescriptor> {
    analysis.bundle.directory(directory)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            icon: Icon::Book,
            accent: None,
            category: NavCategory::Directory,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    let presentation = describe(analysis, directory)?.presentation;
    let title = crate::folder_documents::title_for(analysis, directory);
    let locator = DocumentLocator::new(
        waml::view::row::RowTarget::Folder(directory.to_string()),
        waml::view::surface::SurfaceId::book(),
    );
    Some(OpenDocument {
        tab_id: crate::documents::tab_id_for(&locator),
        locator,
        title,
        presentation,
        view: Box::new(crate::book_view::BookView::new(
            directory.to_string(),
            limits,
            mask.clone(),
        )),
    })
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
    fn opening_a_book_folder_yields_a_book_presentation_and_tab_identity() {
        let prepared = waml::analysis::prepare_candidate(book_source(), None, 1).unwrap();
        let document = open(
            prepared.okf(),
            "/guide",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap();
        assert_eq!(document.presentation.category, NavCategory::Directory);
        assert_eq!(document.presentation.icon, Icon::Book);
        assert_eq!(document.title, "Guide");
        assert_eq!(
            document.locator,
            DocumentLocator::new(
                waml::view::row::RowTarget::Folder("/guide".to_string()),
                waml::view::surface::SurfaceId::book(),
            )
        );
        assert_eq!(
            document.tab_id,
            crate::documents::tab_id_for(&document.locator)
        );
        // A book tab and a folder-listing tab of the SAME directory are two tabs.
        assert_ne!(
            document.tab_id,
            crate::documents::tab_id_for(&DocumentLocator::folder("/guide"))
        );
        assert!(open(
            prepared.okf(),
            "/missing",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .is_none());
    }

    /// Read-as-scroll's load-bearing check (spec
    /// 2026-08-11-read-as-scroll-design §5): `describe` gates on
    /// `bundle.directory`, and the menu targets exactly the folders that
    /// never declared anything -- so a directory with files but NO
    /// `index.md` must resolve, or the entry would appear and silently do
    /// nothing. Directory addresses are derived from every document path
    /// (`okf/shell.rs`'s `project`), not from index files, so it does.
    #[test]
    fn a_directory_without_an_index_still_describes_and_opens_as_a_book() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [First](notes/first.md)\n"),
            ("notes/first.md", "# First\n\nSome prose.\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert!(
            describe(prepared.okf(), "/notes").is_some(),
            "an index-less directory must resolve in the bundle"
        );
        let document = open(
            prepared.okf(),
            "/notes",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap();
        assert_eq!(
            document.title, "notes",
            "title falls back to the folder name"
        );
        assert_eq!(
            document.locator,
            DocumentLocator::new(
                waml::view::row::RowTarget::Folder("/notes".to_string()),
                waml::view::surface::SurfaceId::book(),
            )
        );
    }
}
