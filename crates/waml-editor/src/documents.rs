use crate::document::{DocumentDescriptor, OpenDocument};

pub fn describe(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    crate::uml_documents::describe(okf, uml, concept_id)
        .or_else(|| crate::okf_documents::describe(okf, concept_id))
}

pub fn open(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    crate::uml_documents::open(okf, uml, concept_id)
        .or_else(|| crate::okf_documents::open(okf, concept_id))
}

pub fn reopen(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    tab: &crate::doc_tabs::DocTab,
) -> Option<OpenDocument> {
    if tab.id == crate::okf_documents::source_document_tab_id(&tab.concept_id) {
        crate::okf_documents::open_source(okf, &tab.concept_id)
    } else {
        open(okf, uml, &tab.concept_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{DocumentCapabilities, DocumentPresentation, NavCategory},
        icons::Icon,
    };
    use waml::source::SourceBundle;

    fn future_sibling_descriptor() -> DocumentDescriptor {
        DocumentDescriptor {
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category: NavCategory::OkfDocument,
            },
            capabilities: DocumentCapabilities::default(),
        }
    }

    #[test]
    fn uml_provider_precedes_generic_okf_provider() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 7).unwrap();

        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "order").is_some());
        assert!(crate::okf_documents::open(prepared.okf(), "order").is_some());
        assert_eq!(
            open(prepared.okf(), prepared.uml(), "order")
                .unwrap()
                .tab_id,
            crate::uml_documents::uml_document_tab_id("order")
        );
        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "runbook").is_none());
        assert_eq!(
            open(prepared.okf(), prepared.uml(), "runbook")
                .unwrap()
                .tab_id,
            crate::okf_documents::okf_document_tab_id("runbook")
        );

        let (generic_tab, _) = open(prepared.okf(), prepared.uml(), "runbook")
            .unwrap()
            .into_tab(true);
        assert_eq!(
            reopen(prepared.okf(), prepared.uml(), &generic_tab)
                .unwrap()
                .tab_id,
            generic_tab.id
        );

        let (source_tab, _) = crate::okf_documents::open_source(prepared.okf(), "runbook")
            .unwrap()
            .into_tab(false);
        assert_eq!(
            reopen(prepared.okf(), prepared.uml(), &source_tab)
                .unwrap()
                .tab_id,
            source_tab.id
        );
    }

    #[test]
    fn sibling_descriptor_stays_outside_static_uml_generic_selection() {
        let descriptor = future_sibling_descriptor();
        assert_eq!(descriptor.presentation.category, NavCategory::OkfDocument);

        let source = SourceBundle::try_from_pairs([(
            "widget.md",
            "---\ntype: future.Widget\n---\n# Widget\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 9).unwrap();
        assert!(crate::uml_documents::describe(prepared.okf(), prepared.uml(), "widget").is_none());
        assert_eq!(
            describe(prepared.okf(), prepared.uml(), "widget").unwrap(),
            crate::okf_documents::describe(prepared.okf(), "widget").unwrap()
        );
    }

    #[test]
    fn invalid_claimed_uml_stays_owned_and_exposes_revision_bound_repairs() {
        let source = SourceBundle::try_from_pairs([(
            "broken.md",
            "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- name String [oops 42]\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 11).unwrap();

        let document = open(prepared.okf(), prepared.uml(), "broken").unwrap();
        assert_eq!(
            document.tab_id,
            crate::uml_documents::uml_document_tab_id("broken")
        );
        let id = prepared
            .okf()
            .catalog
            .id_for_path(&waml::source::BundlePath::parse("broken.md").unwrap())
            .unwrap();
        let context =
            waml::uml::ActionContext::new(prepared.okf(), prepared.uml(), prepared.revision())
                .unwrap();
        let actions = waml::uml::repair_actions(context, id).unwrap();
        assert!(actions
            .iter()
            .any(|action| action.title == "Insert missing `: `"));
        assert!(actions
            .iter()
            .any(|action| action.title == "Replace invalid multiplicity"));
    }

    #[test]
    fn indexes_and_logs_are_not_openable_as_concepts() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Index\n- [Runbook](./runbook.md)\n"),
            ("log.md", "---\ntype: Log\n---\n# Log\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 13).unwrap();

        assert!(open(prepared.okf(), prepared.uml(), "index").is_none());
        assert!(open(prepared.okf(), prepared.uml(), "log").is_none());
    }
}
