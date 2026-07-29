use waml_editor::view_history::{
    DiagramCameraAnchor, DocumentKind, DocumentLocator, HistoryDirection, ViewAnchor, ViewHistory,
    ViewLocation, VIEW_HISTORY_LIMIT,
};

fn location(concept_id: &str, scroll_y: f64) -> ViewLocation {
    ViewLocation {
        document: DocumentLocator::primary(concept_id),
        anchor: ViewAnchor::Markdown {
            fragment: None,
            scroll_y,
        },
    }
}

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

#[test]
fn initial_location_and_repeat_current_are_deduplicated() {
    let a = location("a", 0.0);
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));

    history.record_transition(a.clone(), a.clone());

    assert_eq!(history.len(), 1);
    assert_eq!(history.current(), Some(&a));
    assert!(!history.can_traverse(HistoryDirection::Back, |_| true));
    assert!(!history.can_traverse(HistoryDirection::Forward, |_| true));
}

#[test]
fn preview_replacement_and_manual_switching_are_ordinary_transitions() {
    let (a, b, c) = (location("a", 1.0), location("b", 2.0), location("c", 3.0));
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a.clone(), b.clone());
    history.record_transition(b.clone(), c.clone());

    let back = history
        .target(HistoryDirection::Back, |_| true)
        .expect("manual transition records a back target");
    assert_eq!(back.location, b);
    history.commit_traversal(back);
    assert_eq!(
        history
            .target(HistoryDirection::Back, |_| true)
            .unwrap()
            .location,
        a
    );
}

#[test]
fn back_then_new_navigation_clears_forward_without_recording_traversal() {
    let (a, b, c, d) = (
        location("a", 0.0),
        location("b", 0.0),
        location("c", 0.0),
        location("d", 0.0),
    );
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a, b.clone());
    history.record_transition(b.clone(), c.clone());
    let back = history.target(HistoryDirection::Back, |_| true).unwrap();
    history.commit_traversal(back);
    assert_eq!(
        history
            .target(HistoryDirection::Forward, |_| true)
            .unwrap()
            .location,
        c
    );

    history.record_transition(b, d.clone());

    assert_eq!(history.current(), Some(&d));
    assert!(!history.can_traverse(HistoryDirection::Forward, |_| true));
}

#[test]
fn back_then_repeat_current_keeps_forward_history() {
    let (a, b, c) = (location("a", 0.0), location("b", 0.0), location("c", 0.0));
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a, b.clone());
    history.record_transition(b.clone(), c.clone());
    let back = history.target(HistoryDirection::Back, |_| true).unwrap();
    history.commit_traversal(back);

    history.record_transition(b.clone(), b);

    assert_eq!(
        history
            .target(HistoryDirection::Forward, |_| true)
            .unwrap()
            .location,
        c,
        "a no-op navigation at the current entry must preserve Forward"
    );
}

#[test]
fn traversal_moves_only_on_commit_and_refreshes_the_current_anchor() {
    let a = location("a", 0.0);
    let stale_b = location("b", 1.0);
    let fresh_b = location("b", 99.0);
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a, stale_b);
    history.refresh_current(fresh_b.clone());

    let back = history.target(HistoryDirection::Back, |_| true).unwrap();
    assert_eq!(
        history.current(),
        Some(&fresh_b),
        "scanning alone must not move the cursor"
    );
    history.commit_traversal(back);
    let forward = history.target(HistoryDirection::Forward, |_| true).unwrap();
    assert_eq!(forward.location, fresh_b);
    history.commit_traversal(forward);

    assert_eq!(history.len(), 2, "traversal must not append entries");
}

#[test]
fn transition_refreshes_the_departing_anchor() {
    let stale = location("a", 1.0);
    let fresh = location("a", 42.0);
    let b = location("b", 0.0);
    let mut history = ViewHistory::default();
    history.reset(Some(stale));

    history.record_transition(fresh.clone(), b);

    assert_eq!(
        history
            .target(HistoryDirection::Back, |_| true)
            .unwrap()
            .location,
        fresh
    );
}

#[test]
fn unresolved_targets_are_skipped_without_being_removed() {
    let (a, b, c) = (location("a", 0.0), location("b", 0.0), location("c", 0.0));
    let mut history = ViewHistory::default();
    history.reset(Some(a.clone()));
    history.record_transition(a.clone(), b.clone());
    history.record_transition(b.clone(), c);

    let target = history
        .target(HistoryDirection::Back, |candidate| {
            candidate.document.concept_id != "b"
        })
        .unwrap();
    assert_eq!(target.location, a);
    history.commit_traversal(target);

    let restored = history
        .target(HistoryDirection::Forward, |candidate| {
            candidate.document.concept_id == "b"
        })
        .unwrap();
    assert_eq!(restored.location, b);
}

#[test]
fn view_history_evicts_the_oldest_entries_at_the_bound() {
    let mut history = ViewHistory::default();
    let first = location("0", 0.0);
    history.reset(Some(first.clone()));
    let mut departing = first;
    for index in 1..=VIEW_HISTORY_LIMIT + 44 {
        let arriving = location(&index.to_string(), 0.0);
        history.record_transition(departing, arriving.clone());
        departing = arriving;
    }

    assert_eq!(history.len(), VIEW_HISTORY_LIMIT);
    assert_eq!(history.current(), Some(&departing));

    for _ in 1..VIEW_HISTORY_LIMIT {
        let target = history.target(HistoryDirection::Back, |_| true).unwrap();
        history.commit_traversal(target);
    }
    assert_eq!(
        history.current().unwrap().document.concept_id,
        "45",
        "0 through 44 should have been evicted"
    );
    assert!(!history.can_traverse(HistoryDirection::Back, |_| true));
}
