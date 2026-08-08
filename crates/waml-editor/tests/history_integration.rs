use std::collections::HashSet;

use waml_editor::view_history::{
    DiagramCameraAnchor, DocumentLocator, HistoryDirection, ViewAnchor, ViewHistory, ViewLocation,
};
use waml_markdown_editor::input::ScrollState;
use waml_syntax::DocumentRevision;

fn location(document: &str, anchor: ViewAnchor) -> ViewLocation {
    ViewLocation {
        document: DocumentLocator::concept(document, waml::view::surface::SurfaceId::markdown()),
        anchor,
    }
}

#[test]
fn deleted_target_is_skipped_retained_and_reached_after_model_restore() {
    let a = location("a", ViewAnchor::None);
    let b = location("b", ViewAnchor::None);
    let c = location("c", ViewAnchor::None);
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a.clone(), b.clone());
    history.record_transition(b.clone(), c);
    let mut resolvable = HashSet::from(["a", "c"]);

    let skipped = history
        .target(HistoryDirection::Back, |candidate| {
            candidate
                .document
                .concept_id()
                .is_some_and(|id| resolvable.contains(id))
        })
        .unwrap();
    assert_eq!(skipped.location, a);
    history.commit_traversal(skipped);

    resolvable.insert("b");
    let restored = history
        .target(HistoryDirection::Forward, |candidate| {
            candidate
                .document
                .concept_id()
                .is_some_and(|id| resolvable.contains(id))
        })
        .unwrap();
    assert_eq!(restored.location, b);
}

#[test]
fn back_and_forward_restore_view_anchors_without_tab_metadata() {
    let diagram = location(
        "orders",
        ViewAnchor::Diagram {
            selected_key: Some("customer".into()),
            camera: DiagramCameraAnchor {
                pan_x: 40.0,
                pan_y: 80.0,
                zoom: 1.75,
            },
        },
    );
    let source = ViewLocation {
        document: DocumentLocator::source("orders"),
        anchor: ViewAnchor::markdown_start(
            DocumentRevision::INITIAL,
            Some("layout".into()),
            ScrollState { x: 0.0, y: 320.0 },
        ),
    };
    let mut history = ViewHistory::default();
    history.reset(Some(diagram.clone()));
    history.record_transition(diagram.clone(), source.clone());

    let back = history.target(HistoryDirection::Back, |_| true).unwrap();
    assert_eq!(back.location, diagram);
    history.commit_traversal(back);
    let forward = history.target(HistoryDirection::Forward, |_| true).unwrap();
    assert_eq!(forward.location, source);
}

#[test]
fn passive_refresh_changes_anchor_without_growing_the_logical_timeline() {
    let initial = location(
        "orders",
        ViewAnchor::markdown_start(DocumentRevision::INITIAL, None, ScrollState::default()),
    );
    let refreshed = location(
        "orders",
        ViewAnchor::markdown_start(
            DocumentRevision::INITIAL,
            Some("details".into()),
            ScrollState { x: 0.0, y: 144.0 },
        ),
    );
    let mut history = ViewHistory::default();
    history.reset(Some(initial));

    history.refresh_current(refreshed.clone());

    assert_eq!(history.len(), 1);
    assert_eq!(history.current(), Some(&refreshed));
}
