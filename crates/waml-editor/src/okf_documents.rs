use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::Icon;
use crate::view_history::DocumentKind;
use makepad_widgets::{LiveId, Vec4};

pub fn generic_okf_accent() -> Option<Vec4> {
    Some(crate::accent::bucket_color(
        crate::node_style::AccentBucket::None,
    ))
}

pub fn okf_document_tab_id(concept_id: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_okf__{concept_id}"))
}

pub fn source_document_tab_id(concept_id: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_source__{concept_id}"))
}

pub fn presentation(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<DocumentPresentation> {
    describe(analysis, concept_id).map(|descriptor| descriptor.presentation)
}

pub fn describe(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    analysis.bundle.concept(concept_id)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            // The generic-document glyph the tree rows, doc tabs, and the
            // inspector's node lead all share; a describe/open split here
            // makes the tree row disagree with the tab for the same concept.
            icon: Icon::FileText,
            accent: generic_okf_accent(),
            category: NavCategory::OkfDocument,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open_with_asset_host(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let presentation = presentation(analysis, concept_id)?;
    Some(OpenDocument {
        tab_id: okf_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
        kind: DocumentKind::Primary,
        title: concept.title.clone().unwrap_or_else(|| {
            concept_id
                .rsplit('/')
                .next()
                .unwrap_or(concept_id)
                .to_string()
        }),
        presentation,
        view: Box::new(
            crate::generic_okf_view::GenericOkfView::new_with_asset_host(
                concept_id.to_string(),
                assets.clone(),
            ),
        ),
    })
}

#[cfg(test)]
pub fn open(analysis: &waml::analysis::OkfAnalysis, concept_id: &str) -> Option<OpenDocument> {
    open_with_asset_host(
        analysis,
        concept_id,
        &crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        ),
    )
}

pub fn open_source_with_asset_host(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let mut presentation = presentation(analysis, concept_id)?;
    presentation.icon = Icon::FileCode;
    Some(OpenDocument {
        tab_id: source_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
        kind: DocumentKind::Source,
        title: concept.title.clone().unwrap_or_else(|| {
            concept_id
                .rsplit('/')
                .next()
                .unwrap_or(concept_id)
                .to_string()
        }),
        presentation,
        view: Box::new(crate::source_view::SourceView::new_with_asset_host(
            concept_id.to_string(),
            assets.clone(),
        )),
    })
}

#[cfg(test)]
pub fn open_source(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_source_with_asset_host(
        analysis,
        concept_id,
        &crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn assets() -> crate::markdown_hosts::SharedMarkdownAssetHost {
        crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        )
    }

    #[test]
    fn generic_provider_excludes_reserved_index_and_log_documents() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n"),
            ("log.md", "# Log\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert_eq!(
            open_with_asset_host(prepared.okf(), "runbook", &assets())
                .unwrap()
                .presentation
                .icon,
            Icon::FileText
        );
        assert!(open_with_asset_host(prepared.okf(), "index", &assets()).is_none());
        assert!(open_with_asset_host(prepared.okf(), "log", &assets()).is_none());
    }

    #[test]
    fn generic_okf_identity_is_stable_and_distinct_from_uml_and_source() {
        let generic = okf_document_tab_id("runbook");
        assert_ne!(
            generic,
            crate::uml_documents::uml_document_tab_id("runbook")
        );
        assert_ne!(generic, source_document_tab_id("runbook"));
        assert_eq!(generic, okf_document_tab_id("runbook"));
    }

    #[test]
    fn source_documents_use_the_source_file_icon() {
        let source =
            SourceBundle::try_from_pairs([("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();

        let source_document =
            open_source_with_asset_host(prepared.okf(), "runbook", &assets()).unwrap();

        assert_eq!(source_document.presentation.icon, Icon::FileCode);
    }
}
