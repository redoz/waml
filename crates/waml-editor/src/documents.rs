use crate::document::OpenDocument;

pub fn open(
    bundle: &waml::okf::Bundle,
    uml: &waml::uml::Projection,
    concept_id: &str,
) -> Option<OpenDocument> {
    crate::uml_documents::open(bundle, uml, concept_id)
        .or_else(|| crate::okf_documents::open(bundle, concept_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    #[test]
    fn uml_provider_precedes_generic_okf_provider() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let bundle = waml::okf::Bundle::parse(&source).unwrap();
        let projection = waml::uml::project(&bundle);

        assert!(crate::uml_documents::open(&bundle, &projection, "order").is_some());
        assert!(crate::okf_documents::open(&bundle, "order").is_some());
        assert_eq!(
            open(&bundle, &projection, "order").unwrap().tab_id,
            crate::uml_documents::uml_document_tab_id("order")
        );
        assert!(crate::uml_documents::open(&bundle, &projection, "runbook").is_none());
        assert_eq!(
            open(&bundle, &projection, "runbook").unwrap().tab_id,
            crate::okf_documents::okf_document_tab_id("runbook")
        );
    }
}
