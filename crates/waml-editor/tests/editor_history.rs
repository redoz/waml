use std::ops::Range;

use waml::edit::{EditBatch, EditContext, PendingEdit};
use waml_editor::editor_history::{
    EditMergeKey, EditMergeKind, EditorHistory, ATOMIC_TAIL, HISTORY_LIMIT,
};
use waml_editor::view_history::{DocumentLocator, ViewAnchor, ViewLocation};

fn fake_edit() -> PendingEdit {
    PendingEdit::sequence(Vec::new())
}

fn location(document: &str, scroll_y: f64) -> ViewLocation {
    ViewLocation {
        document: DocumentLocator::primary(document),
        anchor: ViewAnchor::Markdown {
            fragment: None,
            scroll_y,
        },
    }
}

fn key(
    document: &str,
    control: &str,
    kind: EditMergeKind,
    span: Option<Range<usize>>,
) -> EditMergeKey {
    EditMergeKey {
        document: DocumentLocator::primary(document),
        control: control.into(),
        kind,
        span,
    }
}

fn record(history: &mut EditorHistory, index: usize, merge_key: Option<EditMergeKey>) {
    history.record_edit(
        fake_edit(),
        format!("Edit {index}"),
        merge_key,
        location("document", index as f64),
        location("document", index as f64 + 1.0),
    );
}

#[test]
fn one_edit_produces_undo_and_undo_redo_swap_reciprocals() {
    let mut history = EditorHistory::default();
    let saved = history.saved_state();
    let after = history.record_edit(
        fake_edit(),
        "Type customer",
        None,
        location("customer", 10.0),
        location("customer", 20.0),
    );

    assert!(history.can_undo());
    assert!(!history.can_redo());
    assert_eq!(history.current_state(), after);
    assert_ne!(history.current_state(), saved);

    let undo = history.prepare_undo().unwrap();
    assert_eq!(undo.label(), "Type customer");
    assert_eq!(undo.target_location(), &location("customer", 10.0));
    assert!(history.commit_undo(undo, fake_edit()));
    assert_eq!(history.current_state(), saved);
    assert!(history.can_redo());

    let redo = history.prepare_redo().unwrap();
    assert_eq!(redo.target_location(), &location("customer", 20.0));
    assert!(history.commit_redo(redo, fake_edit()));
    assert_eq!(history.current_state(), after);
    assert!(history.can_undo());
}

#[test]
fn new_edit_after_undo_clears_redo() {
    let mut history = EditorHistory::default();
    record(&mut history, 0, None);
    let undo = history.prepare_undo().unwrap();
    assert!(history.commit_undo(undo, fake_edit()));
    assert!(history.can_redo());

    record(&mut history, 1, None);

    assert!(!history.can_redo());
    assert!(history.can_undo());
}

#[test]
fn failed_undo_and_redo_leave_the_prepared_entry_unchanged() {
    let mut history = EditorHistory::default();
    record(&mut history, 0, None);
    let failed_undo = history.prepare_undo().unwrap();
    let undo_state = failed_undo.target_state();
    history.abort_undo(failed_undo);
    assert_eq!(history.prepare_undo().unwrap().target_state(), undo_state);

    let undo = history.prepare_undo().unwrap();
    assert!(history.commit_undo(undo, fake_edit()));
    let failed_redo = history.prepare_redo().unwrap();
    let redo_state = failed_redo.target_state();
    history.abort_redo(failed_redo);
    assert_eq!(history.prepare_redo().unwrap().target_state(), redo_state);
}

#[test]
fn savepoint_identity_tracks_undo_back_to_saved_state() {
    let mut history = EditorHistory::default();
    record(&mut history, 0, None);
    history.mark_saved();
    let saved = history.saved_state();
    assert!(history.is_saved());
    record(&mut history, 1, None);
    assert!(!history.is_saved());

    let undo = history.prepare_undo().unwrap();
    assert!(history.commit_undo(undo, fake_edit()));

    assert_eq!(history.current_state(), saved);
    assert!(history.is_saved());
}

#[test]
fn newest_atomic_tail_never_coalesces() {
    let mut history = EditorHistory::default();
    let merge_key = key("document", "body", EditMergeKind::Insert, Some(0..1));
    for index in 0..ATOMIC_TAIL {
        let mut key = merge_key.clone();
        key.span = Some(index..index + 1);
        record(&mut history, index, Some(key));
    }

    assert_eq!(history.undo_len(), ATOMIC_TAIL);
}

#[test]
fn only_compatible_older_contiguous_edits_coalesce() {
    let mut history = EditorHistory::default();
    record(&mut history, 999, None);
    for index in 0..2 {
        record(
            &mut history,
            index,
            Some(key(
                "document",
                "body",
                EditMergeKind::Insert,
                Some(index..index + 1),
            )),
        );
    }
    for index in 0..ATOMIC_TAIL {
        record(&mut history, index + 2, None);
    }

    assert_eq!(history.undo_len(), ATOMIC_TAIL + 2);
}

fn history_with_old_pair(
    first: EditMergeKey,
    boundary: impl FnOnce(&mut EditorHistory),
    second: EditMergeKey,
) -> EditorHistory {
    let mut history = EditorHistory::default();
    record(&mut history, 999, None);
    record(&mut history, 0, Some(first));
    boundary(&mut history);
    record(&mut history, 1, Some(second));
    for index in 0..ATOMIC_TAIL {
        record(&mut history, index + 2, None);
    }
    history
}

#[test]
fn explicit_focus_selection_and_navigation_boundaries_prevent_coalescing() {
    for boundary in ["focus", "selection", "navigation"] {
        let history = history_with_old_pair(
            key("document", "body", EditMergeKind::Insert, Some(0..1)),
            EditorHistory::break_merge_group,
            key("document", "body", EditMergeKind::Insert, Some(1..2)),
        );
        assert_eq!(history.undo_len(), ATOMIC_TAIL + 3, "{boundary}");
    }
}

#[test]
fn savepoint_document_control_kind_span_and_structural_boundaries_are_preserved() {
    let cases = [
        (
            key("a", "body", EditMergeKind::Insert, Some(0..1)),
            key("b", "body", EditMergeKind::Insert, Some(1..2)),
        ),
        (
            key("a", "title", EditMergeKind::Insert, Some(0..1)),
            key("a", "body", EditMergeKind::Insert, Some(1..2)),
        ),
        (
            key("a", "body", EditMergeKind::Insert, Some(0..1)),
            key("a", "body", EditMergeKind::Delete, Some(1..2)),
        ),
        (
            key("a", "body", EditMergeKind::Insert, Some(0..1)),
            key("a", "body", EditMergeKind::Insert, Some(3..4)),
        ),
        (
            key("a", "body", EditMergeKind::Structural, None),
            key("a", "body", EditMergeKind::Structural, None),
        ),
    ];
    for (first, second) in cases {
        let history = history_with_old_pair(first, |_| {}, second);
        assert_eq!(history.undo_len(), ATOMIC_TAIL + 3);
    }

    let savepoint = history_with_old_pair(
        key("a", "body", EditMergeKind::Insert, Some(0..1)),
        EditorHistory::mark_saved,
        key("a", "body", EditMergeKind::Insert, Some(1..2)),
    );
    assert_eq!(savepoint.undo_len(), ATOMIC_TAIL + 3);
}

#[test]
fn history_keeps_only_the_newest_reachable_entries_at_the_bound() {
    let mut history = EditorHistory::default();
    for index in 0..HISTORY_LIMIT + 77 {
        record(&mut history, index, None);
    }
    assert_eq!(history.undo_len() + history.redo_len(), HISTORY_LIMIT);

    for _ in 0..HISTORY_LIMIT {
        let undo = history.prepare_undo().unwrap();
        assert!(history.commit_undo(undo, fake_edit()));
    }
    assert!(!history.can_undo());
    assert_ne!(
        history.current_state(),
        history.saved_state(),
        "evicted states are no longer reachable"
    );
}

#[test]
fn coalesced_inverse_commands_preserve_reverse_application_order() {
    let before = waml::source::SourceBundle::try_from_pairs([
        ("sales/index.md", "# Sales\n\n* [Order](order.md)\n"),
        ("sales/order.md", "# Order\n"),
    ])
    .unwrap();
    let directory = waml::okf::DirectoryAddress::parse("/sales").unwrap();
    let apply = |edit: &PendingEdit, source: &waml::source::SourceBundle| {
        let okf = waml::okf::Bundle::parse(source).unwrap();
        let uml = waml::uml::project(&okf);
        edit.apply_reversible(EditContext {
            source,
            okf: &okf,
            uml: &uml,
        })
        .unwrap()
    };
    let first = PendingEdit::new(waml::okf::Batch(vec![waml::okf::Op::IndexRetitle {
        directory: directory.clone(),
        title: "Commerce".into(),
    }]));
    let first_applied = apply(&first, &before);
    let second = PendingEdit::new(waml::okf::Batch(vec![waml::okf::Op::IndexRetitle {
        directory,
        title: "Retail".into(),
    }]));
    let second_applied = apply(&second, &first_applied.source);

    let mut history = EditorHistory::default();
    record(&mut history, 999, None);
    history.record_edit(
        first_applied.inverse,
        "Retitle",
        Some(key("sales", "title", EditMergeKind::Insert, Some(0..1))),
        location("sales", 0.0),
        location("sales", 1.0),
    );
    history.record_edit(
        second_applied.inverse,
        "Retitle",
        Some(key("sales", "title", EditMergeKind::Insert, Some(1..2))),
        location("sales", 1.0),
        location("sales", 2.0),
    );
    for index in 0..ATOMIC_TAIL {
        record(&mut history, index + 2, None);
    }
    for _ in 0..ATOMIC_TAIL {
        let filler = history.prepare_undo().unwrap();
        assert!(history.commit_undo(filler, fake_edit()));
    }

    let coalesced = history.prepare_undo().unwrap();
    let restored = apply(coalesced.edit(), &second_applied.source);

    assert_eq!(restored.source, before);
}
