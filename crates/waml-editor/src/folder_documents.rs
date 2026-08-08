//! Document provider for a folder's own view. The sibling of
//! `okf_documents.rs`, keyed on a directory address instead of a concept id.

use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::folder_view::{folder_document_tab_id, FolderView};
use crate::icons::Icon;
use crate::navigation::DocumentKind;

pub fn describe(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
) -> Option<DocumentDescriptor> {
    analysis.bundle.directory(directory)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            icon: Icon::Folder,
            accent: None,
            category: NavCategory::Directory,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

fn title_for(analysis: &waml::analysis::OkfAnalysis, directory: &str) -> String {
    analysis
        .bundle
        .index(directory)
        .and_then(|index| index.title.clone())
        .unwrap_or_else(|| {
            directory
                .rsplit('/')
                .next()
                .filter(|last| !last.is_empty())
                .unwrap_or(directory)
                .to_string()
        })
}

pub fn open(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    let presentation = describe(analysis, directory)?.presentation;
    let view = FolderView::build(analysis, directory, limits, mask)?;
    let title = title_for(analysis, directory);
    Some(OpenDocument {
        tab_id: folder_document_tab_id(directory),
        concept_id: directory.to_string(),
        kind: DocumentKind::Primary,
        title,
        presentation,
        view: Box::new(view),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    #[test]
    fn opening_a_folder_yields_a_folder_presentation_and_tab_identity() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Orders](orders.md)\n"),
            ("sales/orders.md", "# Orders\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();

        let document = open(
            prepared.okf(),
            "/sales",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap();
        assert_eq!(document.presentation.category, NavCategory::Directory);
        assert_eq!(document.tab_id, folder_document_tab_id("/sales"));
        assert_eq!(document.title, "Sales");

        assert!(open(
            prepared.okf(),
            "/missing",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .is_none());
    }
}
