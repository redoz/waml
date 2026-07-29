use waml_editor::view_history::{
    DiagramCameraAnchor, DocumentKind, DocumentLocator, ViewAnchor, ViewLocation,
};

#[test]
fn locator_distinguishes_primary_and_source_views_of_the_same_concept() {
    let primary = DocumentLocator::primary("sales/order");
    let source = DocumentLocator::source("sales/order");

    assert_eq!(primary.concept_id, source.concept_id);
    assert_eq!(primary.kind, DocumentKind::Primary);
    assert_eq!(source.kind, DocumentKind::Source);
    assert_ne!(primary, source);
}

#[test]
fn anchors_are_value_only_and_preserve_markdown_and_diagram_state() {
    let markdown = ViewLocation {
        document: DocumentLocator::primary("runbook"),
        anchor: ViewAnchor::Markdown {
            fragment: Some("recovery".into()),
            scroll_y: 384.5,
        },
    };
    assert_eq!(
        markdown.anchor,
        ViewAnchor::Markdown {
            fragment: Some("recovery".into()),
            scroll_y: 384.5,
        }
    );

    let diagram = ViewAnchor::Diagram {
        selected_key: Some("sales/customer".into()),
        camera: DiagramCameraAnchor {
            pan_x: 12.0,
            pan_y: 34.0,
            zoom: 1.5,
        },
    };
    assert!(matches!(diagram, ViewAnchor::Diagram { .. }));
}
