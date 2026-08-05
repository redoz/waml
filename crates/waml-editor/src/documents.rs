use crate::document::{DocumentDescriptor, OpenDocument};
use crate::view_history::{DocumentKind, DocumentLocator};

pub fn describe(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    crate::uml_documents::describe(okf, uml, concept_id)
        .or_else(|| crate::okf_documents::describe(okf, concept_id))
}

pub fn open_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    crate::uml_documents::open_with_asset_host(okf, uml, concept_id, assets)
        .or_else(|| crate::okf_documents::open_with_asset_host(okf, concept_id, assets))
}

/// The folder-view provider entry: keyed on a directory address, not a
/// concept id, so it sits beside `open_with_asset_host` rather than inside
/// its `.or_else` chain -- a folder and a concept never share a key. Called
/// from `App::navigate_with`'s `Directory` arm.
pub fn open_folder(
    okf: &waml::analysis::OkfAnalysis,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
) -> Option<OpenDocument> {
    crate::folder_documents::open(okf, directory, limits)
}

/// The raw OKF layer entry (Task D3): opens `directory`'s identity listing,
/// bypassing whatever chain it declares. Called from `App::navigate_with`'s
/// `DirectoryRaw` arm.
pub fn open_folder_raw(okf: &waml::analysis::OkfAnalysis, directory: &str) -> Option<OpenDocument> {
    crate::folder_documents::open_raw(okf, directory)
}

#[cfg(test)]
pub fn open(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_with_asset_host(okf, uml, concept_id, &assets_for_test())
}

pub fn reopen_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    tab: &crate::doc_tabs::DocTab,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    open_locator_with_asset_host(okf, uml, &tab.locator(), assets)
}

pub fn open_locator_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    locator: &DocumentLocator,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    match locator.kind {
        DocumentKind::Primary => open_with_asset_host(okf, uml, &locator.concept_id, assets),
        DocumentKind::Source => {
            crate::okf_documents::open_source_with_asset_host(okf, &locator.concept_id, assets)
        }
    }
}

#[cfg(test)]
fn assets_for_test() -> crate::markdown_hosts::SharedMarkdownAssetHost {
    crate::markdown_hosts::EditorMarkdownAssetHost::shared(
        crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{DocumentCapabilities, DocumentPresentation, NavCategory},
        icons::Icon,
    };
    use waml::source::SourceBundle;

    fn assets() -> crate::markdown_hosts::SharedMarkdownAssetHost {
        crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        )
    }

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
        assert!(
            crate::okf_documents::open_with_asset_host(prepared.okf(), "order", &assets())
                .is_some()
        );
        assert_eq!(
            open_with_asset_host(prepared.okf(), prepared.uml(), "order", &assets())
                .unwrap()
                .tab_id,
            crate::uml_documents::uml_document_tab_id("order")
        );
        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "runbook").is_none());
        assert_eq!(
            open_with_asset_host(prepared.okf(), prepared.uml(), "runbook", &assets())
                .unwrap()
                .tab_id,
            crate::okf_documents::okf_document_tab_id("runbook")
        );

        let (generic_tab, _) =
            open_with_asset_host(prepared.okf(), prepared.uml(), "runbook", &assets())
                .unwrap()
                .into_tab(true);
        assert_eq!(
            reopen_with_asset_host(prepared.okf(), prepared.uml(), &generic_tab, &assets())
                .unwrap()
                .tab_id,
            generic_tab.id
        );

        let (source_tab, _) =
            crate::okf_documents::open_source_with_asset_host(prepared.okf(), "runbook", &assets())
                .unwrap()
                .into_tab(false);
        assert_eq!(
            reopen_with_asset_host(prepared.okf(), prepared.uml(), &source_tab, &assets())
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

        let document =
            open_with_asset_host(prepared.okf(), prepared.uml(), "broken", &assets()).unwrap();
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

        assert!(open_with_asset_host(prepared.okf(), prepared.uml(), "index", &assets()).is_none());
        assert!(open_with_asset_host(prepared.okf(), prepared.uml(), "log", &assets()).is_none());
    }

    #[test]
    fn locator_reopens_the_correct_view_after_transient_tab_identity_is_gone() {
        let source =
            SourceBundle::try_from_pairs([("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 17).unwrap();
        let (mut old_source_tab, _) =
            crate::okf_documents::open_source_with_asset_host(prepared.okf(), "runbook", &assets())
                .unwrap()
                .into_tab(false);
        old_source_tab.id = makepad_widgets::LiveId::from_str("closed-transient-tab");

        let reopened =
            reopen_with_asset_host(prepared.okf(), prepared.uml(), &old_source_tab, &assets())
                .unwrap();

        assert_eq!(reopened.locator(), old_source_tab.locator());
        assert_eq!(reopened.kind, DocumentKind::Source);
        assert_eq!(
            reopened.tab_id,
            crate::okf_documents::source_document_tab_id("runbook")
        );
    }
}
