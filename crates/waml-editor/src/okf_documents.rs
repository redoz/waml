use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::Icon;
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

pub fn presentation(bundle: &waml::okf::Bundle, concept_id: &str) -> Option<DocumentPresentation> {
    describe(bundle, concept_id).map(|descriptor| descriptor.presentation)
}

pub fn describe(bundle: &waml::okf::Bundle, concept_id: &str) -> Option<DocumentDescriptor> {
    bundle.concept(concept_id)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            icon: Icon::StickyNote,
            accent: generic_okf_accent(),
            category: NavCategory::OkfDocument,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open(bundle: &waml::okf::Bundle, concept_id: &str) -> Option<OpenDocument> {
    let concept = bundle.concept(concept_id)?;
    let presentation = presentation(bundle, concept_id)?;
    Some(OpenDocument {
        tab_id: okf_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
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

pub fn open_source(bundle: &waml::okf::Bundle, concept_id: &str) -> Option<OpenDocument> {
    let concept = bundle.concept(concept_id)?;
    let presentation = presentation(bundle, concept_id)?;
    Some(OpenDocument {
        tab_id: source_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
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
        let bundle = waml::okf::Bundle::parse(&source).unwrap();
        assert!(open(&bundle, "runbook").is_some());
        assert!(open(&bundle, "index").is_none());
        assert!(open(&bundle, "log").is_none());
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
}
