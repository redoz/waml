use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::{Icon, IconSet};
use makepad_widgets::LiveId;

pub fn uml_document_tab_id(concept_id: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_uml__{concept_id}"))
}

fn category(projection: &waml::uml::Projection, concept_id: &str) -> Option<NavCategory> {
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
    None
}

pub fn presentation(
    projection: &waml::uml::Projection,
    concept_id: &str,
) -> Option<DocumentPresentation> {
    describe(projection, concept_id).map(|descriptor| descriptor.presentation)
}

pub fn describe(
    projection: &waml::uml::Projection,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    let category = category(projection, concept_id)?;
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

pub fn open(
    bundle: &waml::okf::Bundle,
    projection: &waml::uml::Projection,
    concept_id: &str,
) -> Option<OpenDocument> {
    let concept = bundle.concept(concept_id)?;
    let presentation = presentation(projection, concept_id)?;
    let title = concept.title.clone().unwrap_or_else(|| {
        concept_id
            .rsplit('/')
            .next()
            .unwrap_or(concept_id)
            .to_string()
    });
    let view: Box<dyn crate::doc_view::DocView> = if presentation.category == NavCategory::Diagram {
        Box::new(crate::class_diagram_view::ClassDiagramView::new(
            concept_id.to_string(),
        ))
    } else {
        Box::new(crate::classifier_preview_view::ClassifierPreviewView::new(
            concept_id.to_string(),
            presentation.category,
        ))
    };
    Some(OpenDocument {
        tab_id: uml_document_tab_id(concept_id),
        concept_id: concept_id.to_string(),
        title,
        presentation,
        view,
    })
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
        let bundle = waml::okf::Bundle::parse(&source).unwrap();
        let projection = waml::uml::project(&bundle);
        assert!(open(&bundle, &projection, "order").is_some());
        let package = describe(&projection, "domain").unwrap();
        assert_eq!(package.presentation.category, NavCategory::Directory);
        assert!(!package.capabilities.can_edit_classifier);
        assert!(open(&bundle, &projection, "runbook").is_none());
    }
}
