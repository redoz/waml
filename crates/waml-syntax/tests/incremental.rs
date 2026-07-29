use std::sync::Arc;

use waml_syntax::{
    parse_okf_markdown, reparse_okf_markdown, ChangeMap, FullReparseReason, MarkdownDialect,
    ReparseOutcome, SourceText, TextChange, TextRange, TextSize,
};

fn text(value: &str) -> SourceText {
    SourceText::from_shared(Arc::new(value.to_owned())).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::try_from(start).unwrap(), TextSize::try_from(end).unwrap()).unwrap()
}

fn size(value: usize) -> TextSize {
    TextSize::try_from(value).unwrap()
}

fn oracle(previous: &str, next: &str, changes: &[TextChange]) {
    let previous = parse_okf_markdown(text(previous), MarkdownDialect::CommonMarkCurrent).unwrap();
    let full = parse_okf_markdown(text(next), MarkdownDialect::CommonMarkCurrent).unwrap();
    let outcome = reparse_okf_markdown(&previous.tree, text(next), changes).unwrap();
    let incremental = match outcome {
        ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
    };
    assert_eq!(incremental.write_to_string(), next);
    assert_eq!(incremental.write_to_string(), full.tree.write_to_string());
    assert_eq!(incremental.diagnostics().len(), full.tree.diagnostics().len());
}

#[test]
fn reparse_matches_full_oracle_for_safe_edits_and_fallback_boundaries() {
    for (previous, next, changes) in [
        ("# One\nbody\n", "# Two\nbody\n", vec![TextChange { old_range: range(2, 5), replacement: Arc::from("Two") }]),
        ("# One\nbody\n", "# One\nbody!\n", vec![TextChange { old_range: range(10, 10), replacement: Arc::from("!") }]),
        ("# Café\nbody\n", "# Café\nbody!\n", vec![TextChange { old_range: range(12, 12), replacement: Arc::from("!") }]),
        ("---\ntype: uml.Class\n---\n# One\n", "---\ntype: uml.Interface\n---\n# One\n", vec![TextChange { old_range: range(10, 19), replacement: Arc::from("uml.Interface") }]),
        ("# One\nbody\n", "## One\nbody\n", vec![TextChange { old_range: range(1, 1), replacement: Arc::from("#") }]),
        ("# One\n  body\n", "# One\n    body\n", vec![TextChange { old_range: range(6, 8), replacement: Arc::from("    ") }]),
        ("# One\na: [b, c]\n", "# One\na: [b, d]\n", vec![TextChange { old_range: range(13, 14), replacement: Arc::from("d") }]),
        ("# One\na\nb\n", "# Uno\na\nbee\n", vec![TextChange { old_range: range(2, 5), replacement: Arc::from("Uno") }, TextChange { old_range: range(8, 9), replacement: Arc::from("bee") }]),
    ] {
        oracle(previous, next, &changes);
    }
}

#[test]
fn change_map_rejects_unsorted_overlapping_and_non_utf8_changes() {
    let source = text("# Café\n");
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[TextChange { old_range: range(5, 6), replacement: Arc::from("x") }],
        )
        .unwrap_err(),
        FullReparseReason::InvalidUtf8Boundary,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[
                TextChange { old_range: range(2, 3), replacement: Arc::from("x") },
                TextChange { old_range: range(1, 2), replacement: Arc::from("y") },
            ],
        )
        .unwrap_err(),
        FullReparseReason::OverlappingChanges,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[
                TextChange { old_range: range(2, 4), replacement: Arc::from("x") },
                TextChange { old_range: range(3, 5), replacement: Arc::from("y") },
            ],
        )
        .unwrap_err(),
        FullReparseReason::OverlappingChanges,
    );
    assert!(matches!(
        reparse_okf_markdown(&parse_okf_markdown(source, MarkdownDialect::CommonMarkCurrent).unwrap().tree, text("# Café\n"), &[]).unwrap(),
        ReparseOutcome::Full { reason: FullReparseReason::NoPreviousSnapshot, .. } | ReparseOutcome::Incremental { .. }
    ));
}

#[test]
fn change_map_translates_only_unchanged_occurrences_and_surviving_boundaries() {
    let source = text("zero one two");
    let map = ChangeMap::checked(
        &source,
        &[TextChange { old_range: range(5, 8), replacement: Arc::from("ONE!") }],
    )
    .unwrap();

    assert_eq!(map.old_len(), size(12));
    assert_eq!(map.new_len(), size(13));
    assert_eq!(map.translate_unchanged(range(0, 4)), Some(range(0, 4)));
    assert_eq!(map.translate_unchanged(range(9, 12)), Some(range(10, 13)));
    assert_eq!(map.translate_unchanged(range(5, 8)), None);
    assert_eq!(map.translate_start_boundary(size(5)), Some(size(5)));
    assert_eq!(map.translate_end_boundary(size(8)), Some(size(9)));
    assert_eq!(map.translate_start_boundary(size(6)), None);
}

#[test]
fn change_map_side_biases_zero_width_insertion_boundaries() {
    let source = text("# H\nbody\n");
    let map = ChangeMap::checked(
        &source,
        &[TextChange { old_range: range(4, 4), replacement: Arc::from("x") }],
    )
    .unwrap();

    assert_eq!(map.translate_end_boundary(size(4)), Some(size(4)));
    assert_eq!(map.translate_start_boundary(size(4)), Some(size(5)));
    assert_eq!(map.translate_unchanged(range(0, 4)), Some(range(0, 4)));
    assert_eq!(map.translate_unchanged(range(4, 9)), Some(range(5, 10)));
}
