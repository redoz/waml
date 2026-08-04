use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::{Icon, IconSet};
use crate::view_history::DocumentKind;
use makepad_widgets::LiveId;

pub fn uml_document_tab_id(concept_id: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_uml__{concept_id}"))
}

fn category(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<NavCategory> {
    if !uml.claims.contains(concept_id) {
        return None;
    }
    let projection = &uml.projection;
    if projection
        .diagrams
        .iter()
        .any(|diagram| diagram.key == concept_id)
    {
        return Some(NavCategory::Diagram);
    }
    if let Some(node) = projection.node(concept_id) {
        return Some(crate::tree::kind_of(&node.ty));
    }
    if projection
        .packages
        .iter()
        .any(|package| package.key == concept_id)
    {
        return Some(NavCategory::Directory);
    }
    if projection
        .interactions
        .iter()
        .any(|interaction| interaction.key == concept_id)
    {
        return Some(NavCategory::Sequence);
    }
    if projection.flows.iter().any(|flow| flow.key == concept_id) {
        return Some(NavCategory::Behavior);
    }
    let concept = okf.bundle.concept(concept_id)?;
    Some(crate::tree::kind_of(&waml::model::ElementType::parse(
        &concept.ty,
    )))
}

pub fn presentation(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<DocumentPresentation> {
    describe(okf, uml, concept_id).map(|descriptor| descriptor.presentation)
}

pub fn describe(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    let category = category(okf, uml, concept_id)?;
    let classifier = matches!(
        category,
        NavCategory::Class | NavCategory::Interface | NavCategory::Enum | NavCategory::DataType
    );
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            icon: IconSet::icon_for(category).unwrap_or(Icon::StickyNote),
            accent: crate::accent::tree_kind_color(category),
            category,
        },
        capabilities: DocumentCapabilities {
            can_edit_classifier: classifier,
            can_delete_classifier: classifier,
        },
    })
}

pub fn open_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    let concept = okf.bundle.concept(concept_id)?;
    let presentation = presentation(okf, uml, concept_id)?;
    let title = concept.title.clone().unwrap_or_else(|| {
        concept_id
            .rsplit('/')
            .next()
            .unwrap_or(concept_id)
            .to_string()
    });
    // Every UML document is markdown underneath, so each primary view carries
    // the breadcrumb header's in-place source toggle.
    use crate::source_toggle_view::SourceToggleView;
    let view: Box<dyn crate::doc_view::DocView> = match presentation.category {
        NavCategory::Diagram => Box::new(SourceToggleView::new(
            crate::class_diagram_view::ClassDiagramView::new(concept_id.to_string()),
            concept_id.to_string(),
            assets.clone(),
        )),
        NavCategory::Behavior => Box::new(SourceToggleView::new(
            crate::behavior_doc_view::BehaviorDocView::flow(concept_id.to_string()),
            concept_id.to_string(),
            assets.clone(),
        )),
        NavCategory::Sequence => Box::new(SourceToggleView::new(
            crate::behavior_doc_view::BehaviorDocView::interaction(concept_id.to_string()),
            concept_id.to_string(),
            assets.clone(),
        )),
        _ => Box::new(SourceToggleView::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(
                concept_id.to_string(),
                presentation.category,
            ),
            concept_id.to_string(),
            assets.clone(),
        )),
    };
    Some(OpenDocument {
        tab_id: uml_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
        kind: DocumentKind::Primary,
        title,
        presentation,
        view,
    })
}

#[cfg(test)]
pub fn open(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_with_asset_host(
        okf,
        uml,
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

    #[test]
    fn claims_only_projected_uml_concepts() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("domain.md", "---\ntype: uml.Package\n---\n# Domain\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert!(open(prepared.okf(), prepared.uml(), "order").is_some());
        let package = describe(prepared.okf(), prepared.uml(), "domain").unwrap();
        assert_eq!(package.presentation.category, NavCategory::Directory);
        assert!(!package.capabilities.can_edit_classifier);
        assert!(open(prepared.okf(), prepared.uml(), "runbook").is_none());
    }

    #[test]
    fn behavior_and_sequence_open_as_behavior_views() {
        let source = SourceBundle::try_from_pairs([
            ("checkout.md", "---\ntype: uml.Activity\n---\n# Checkout\n"),
            ("ordering.md", "---\ntype: uml.Sequence\n---\n# Ordering\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let doc = open(prepared.okf(), prepared.uml(), "checkout").unwrap();
        assert_eq!(doc.presentation.category, NavCategory::Behavior);
        // Downcast-free check: BehaviorDocView reports view_bar-only chrome,
        // unlike ClassifierPreviewView (view_bar: false).
        assert!(doc.view.chrome().view_bar);
        let seq = open(prepared.okf(), prepared.uml(), "ordering").unwrap();
        assert_eq!(seq.presentation.category, NavCategory::Sequence);
        assert!(seq.view.chrome().view_bar);
    }
}
