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
            icon: Icon::StickyNote,
            accent: generic_okf_accent(),
            category: NavCategory::OkfDocument,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open(analysis: &waml::analysis::OkfAnalysis, concept_id: &str) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let mut presentation = presentation(analysis, concept_id)?;
    presentation.icon = Icon::FileText;
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
        view: Box::new(crate::generic_okf_view::GenericOkfView::new(
            concept_id.to_string(),
        )),
    })
}

pub fn open_source(
    analysis: &waml::analysis::OkfAnalysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    let concept = analysis.bundle.concept(concept_id)?;
    let mut presentation = presentation(analysis, concept_id)?;
    presentation.icon = Icon::FileBraces;
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
        view: Box::new(crate::source_view::SourceView::new(concept_id.to_string())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

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
            open(prepared.okf(), "runbook").unwrap().presentation.icon,
            Icon::FileText
        );
        assert!(open(prepared.okf(), "index").is_none());
        assert!(open(prepared.okf(), "log").is_none());
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

        let source_document = open_source(prepared.okf(), "runbook").unwrap();

        assert_eq!(source_document.presentation.icon, Icon::FileBraces);
    }
}
