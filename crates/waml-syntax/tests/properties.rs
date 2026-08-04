use std::{collections::BTreeSet, sync::Arc};

use proptest::prelude::*;
use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, FullReparseReason, MarkdownDialect,
    MarkdownReparseOutcome, SourceText, SyntaxElement, SyntaxNode, TextChange, TextRange, TextSize,
};

const BASE: &str = "---\ntitle: test\n---\n\n# Model\n\n[id]: /one\n\n- item\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\n<div>html</div>\n\nuse [x][id]\n\n## Attributes\nname: String\n";

fn source(value: &str) -> SourceText {
    SourceText::new(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn fingerprint(node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>, out: &mut Vec<String>) {
    out.push(format!("node:{:?}:{:?}", node.kind(), node.range()));
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => fingerprint(child, out),
            SyntaxElement::Token(token) => out.push(format!(
                "token:{:?}:{:?}:{}",
                token.kind(),
                token.range(),
                token.text().write_to_string()
            )),
        }
    }
}

fn structural_fingerprint(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    let mut output = Vec::new();
    fingerprint(snapshot.tree().root(), &mut output);
    output
}

fn diagnostic_fingerprint(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    let mut diagnostics: Vec<_> = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{}",
                diagnostic.code, diagnostic.range, diagnostic.message
            )
        })
        .collect();
    diagnostics.sort_unstable();
    diagnostics
}

fn query_fingerprint(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
    candidate: &str,
) -> Vec<String> {
    let queries = snapshot.queries();
    let whole = range(0, candidate.len());
    let spans: Vec<_> = queries.spans(whole).collect();
    let mut output = BTreeSet::new();

    for span in &spans {
        output.insert(format!(
            "span:{:?}:{:?}:{:?}",
            span.range, span.source_role, span.semantic_role
        ));
        if let Some(heading) = queries.heading(span.owner) {
            output.insert(format!(
                "heading:{:?}:{:?}:{}",
                heading.range, heading.content_range, heading.level
            ));
        }
        if let Some(list) = queries.list(span.owner) {
            output.insert(format!(
                "list:{:?}:{:?}:{:?}",
                list.range, list.kind, list.task
            ));
        }
        if let Some(cell) = queries.table_cell(span.owner) {
            output.insert(format!("cell:{:?}:{:?}", cell.range, cell.alignment));
        }
        if let Some(html) = queries.raw_html(span.owner) {
            output.insert(format!(
                "html:{:?}:{:?}:{:?}",
                html.range, html.filter, html.filtered_ranges
            ));
        }
        if let Some(fenced) = queries.fenced_code(span.owner) {
            output.insert(format!(
                "fenced:{:?}:{:?}:{:?}:{:?}:{:?}:{}:{:?}",
                fenced.source_range,
                fenced.fence_range,
                fenced.info_range,
                fenced.content_range,
                fenced.info,
                fenced.language.is_some(),
                fenced.language
            ));
        }
        if let Some(island) = queries.island(span.owner) {
            output.insert(format!(
                "island-query:{:?}:{:?}:{:?}",
                island.kind, island.heading_range, island.content_range
            ));
        }
    }

    for link in queries.links() {
        output.insert(format!(
            "link:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            link.source_range,
            link.content_range,
            link.destination,
            link.destination_range,
            link.title,
            link.kind
        ));
    }
    for image in queries.images() {
        output.insert(format!(
            "image:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            image.source_range,
            image.alt_range,
            image.source,
            image.source_definition_range,
            image.title,
            image.kind
        ));
    }
    for entity in queries.entities() {
        output.insert(format!("entity:{:?}:{}", entity.source_range, entity.value));
    }
    for diagnostic in queries.diagnostics(whole) {
        output.insert(format!(
            "diagnostic:{:?}:{:?}:{:?}:{}",
            diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
        ));
    }
    for island in snapshot.structure().islands.iter() {
        output.insert(format!(
            "island:{:?}:{:?}:{:?}",
            island.kind, island.heading_range, island.content_range
        ));
    }

    let mut labels = BTreeSet::from(["id".to_owned(), " ID ".to_owned(), "missing".to_owned()]);
    let mut rest = candidate;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        labels.insert(rest[..close].to_owned());
        rest = &rest[close + 1..];
    }
    for label in labels {
        let mut ranges: Vec<_> = queries
            .reference_backlinks(&label)
            .iter()
            .filter_map(|identity| {
                queries
                    .links()
                    .find(|link| link.identity == *identity)
                    .map(|link| link.source_range)
            })
            .collect();
        ranges.sort_unstable_by_key(|value| (value.start(), value.end()));
        output.insert(format!(
            "backlinks:{}:{ranges:?}",
            label.trim().to_lowercase()
        ));
    }

    output.into_iter().collect()
}

fn boundaries(value: &str) -> Vec<usize> {
    value
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(value.len()))
        .collect()
}

fn assert_full_oracle(snapshot: &waml_syntax::MarkdownSyntaxSnapshot, candidate: &str) {
    let full = parse_markdown(
        DocumentRevision::new(snapshot.revision().get() + 1),
        source(candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(snapshot.text().shared().as_str(), candidate);
    assert_eq!(snapshot.tree().write_to_string(), candidate);
    assert_eq!(
        structural_fingerprint(snapshot),
        structural_fingerprint(&full)
    );
    assert_eq!(
        diagnostic_fingerprint(snapshot),
        diagnostic_fingerprint(&full)
    );
    assert_eq!(
        query_fingerprint(snapshot, candidate),
        query_fingerprint(&full, candidate)
    );
    assert_eq!(
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            snapshot.structure().headings,
            snapshot.structure().nested_headings,
            snapshot.structure().protected_ranges,
            snapshot.structure().list_item_lines,
            snapshot.structure().tab_indented_item_lines,
            snapshot.structure().opaque_ranges,
        ),
        format!(
            "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
            full.structure().headings,
            full.structure().nested_headings,
            full.structure().protected_ranges,
            full.structure().list_item_lines,
            full.structure().tab_indented_item_lines,
            full.structure().opaque_ranges,
        ),
        "structure metadata agrees without identity IDs"
    );
    assert_eq!(
        snapshot.queries().links().count(),
        full.queries().links().count(),
        "reference resolution and query roles agree"
    );
    assert_eq!(
        snapshot.structure().islands.len(),
        full.structure().islands.len()
    );
}

#[test]
fn reference_paste_into_heading_matches_clean_parse() {
    // This fails if a local heading reparse does not invalidate inline reference parsing.
    let mut candidate = BASE.to_owned();
    candidate.replace_range(23..24, "[n][id]");
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        source(BASE),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(2),
        source(&candidate),
        &[TextChange {
            old_range: range(23, 24),
            replacement: Arc::from("[n][id]"),
        }],
    )
    .unwrap();

    assert_eq!(candidate[21..35].to_owned(), "# [n][id]odel\n");
    assert_eq!(update.snapshot.queries().links().count(), 2);
    assert_full_oracle(&update.snapshot, &candidate);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::UnsafeSynchronization
        }
    ));
}

#[test]
fn heading_edit_leaving_trailing_eof_whitespace_matches_clean_parse() {
    // This fails if a heading window reparse keeps end-of-document spaces inside the
    // heading; a full parse hands them to end-of-file trivia instead.
    let before = "# Modx x";
    let candidate = "# Modx ";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        source(before),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(2),
        source(candidate),
        &[TextChange {
            old_range: range(7, 8),
            replacement: Arc::from(""),
        }],
    )
    .unwrap();
    assert_full_oracle(&update.snapshot, candidate);
}

#[test]
fn width_changes_before_reference_definition_update_destination_ranges() {
    // This fails if reused reference annotations retain pre-edit definition offsets.
    let mut candidate = BASE.to_owned();
    let mut snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut revision = DocumentRevision::INITIAL;
    for (first, second, replacement_kind) in [
        (36_u8, 103_u8, 1_u8),
        (36, 108, 52),
        (115, 54, 71),
        (140, 4, 69),
    ] {
        let points = boundaries(&candidate);
        let left = usize::from(first) % points.len();
        let right = usize::from(second) % points.len();
        let (start, end) = if left <= right {
            (points[left], points[right])
        } else {
            (points[right], points[left])
        };
        let replacement: Arc<str> = match replacement_kind % 4 {
            0 => Arc::from(""),
            1 => Arc::from("x"),
            2 => Arc::from("é"),
            _ => Arc::from("[n][id]"),
        };
        candidate.replace_range(start..end, &replacement);
        revision = revision.checked_next().unwrap();
        let update = reparse_markdown(
            &snapshot,
            revision,
            source(&candidate),
            &[TextChange {
                old_range: range(start, end),
                replacement,
            }],
        )
        .unwrap();
        assert_full_oracle(&update.snapshot, &candidate);
        snapshot = update.snapshot;
    }
}

#[test]
fn final_heading_edit_reassigns_trailing_whitespace_to_eof() {
    let previous_text = "---\ntitle: test\n---\n\n# Modeldiv>\n\nuse [x][id]\n\n## xame: String\n";
    let candidate = "---\ntitle: test\n---\n\n# Modeldiv>\n\nuse [x][id]\n\n## xame: ";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        source(previous_text),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(2),
        source(candidate),
        &[TextChange {
            old_range: range(56, 63),
            replacement: Arc::from(""),
        }],
    )
    .unwrap();

    assert_full_oracle(&update.snapshot, candidate);
}

#[test]
fn minimized_edit_sequence_recovers_invalid_block_ranges() {
    let mut candidate = BASE.to_owned();
    let mut snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut revision = DocumentRevision::INITIAL;
    let mut recovered_with_full_fallback = false;
    for (first, second, replacement_kind) in [
        (181_u8, 33_u8, 89_u8),
        (20, 94, 235),
        (138, 211, 105),
        (153, 210, 52),
        (54, 31, 192),
    ] {
        let points = boundaries(&candidate);
        let left = usize::from(first) % points.len();
        let right = usize::from(second) % points.len();
        let (start, end) = if left <= right {
            (points[left], points[right])
        } else {
            (points[right], points[left])
        };
        let replacement: Arc<str> = match replacement_kind % 4 {
            0 => Arc::from(""),
            1 => Arc::from("x"),
            2 => Arc::from("é"),
            _ => Arc::from("[n][id]"),
        };
        candidate.replace_range(start..end, &replacement);
        revision = revision.checked_next().unwrap();
        let update = reparse_markdown(
            &snapshot,
            revision,
            source(&candidate),
            &[TextChange {
                old_range: range(start, end),
                replacement,
            }],
        )
        .unwrap();
        assert_full_oracle(&update.snapshot, &candidate);
        recovered_with_full_fallback |= matches!(
            update.outcome,
            MarkdownReparseOutcome::Full {
                reason: FullReparseReason::UnsafeSynchronization
            }
        );
        snapshot = update.snapshot;
    }
    assert!(recovered_with_full_fallback);
}

#[test]
fn multibyte_edit_into_reference_definition_updates_backlinks() {
    // Shrunk from randomized_full_and_incremental_snapshots_agree with
    // edits = [(109, 122, 243), (168, 170, 194)]. The second edit replaces the
    // two ASCII bytes " /" in "[id]: /one" with the two-byte "é", producing
    // "[id]:éone". The incremental path must notice the definition changed and
    // reresolve its backlinks, not keep the stale "/one" destination.
    let mut candidate = BASE.to_owned();
    let mut snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut revision = DocumentRevision::INITIAL;
    for (start, end, replacement) in [(109, 122, "[n][id]"), (35, 37, "é")] {
        let replacement: Arc<str> = Arc::from(replacement);
        candidate.replace_range(start..end, &replacement);
        revision = revision.checked_next().unwrap();
        let update = reparse_markdown(
            &snapshot,
            revision,
            source(&candidate),
            &[TextChange {
                old_range: range(start, end),
                replacement,
            }],
        )
        .unwrap();
        assert_full_oracle(&update.snapshot, &candidate);
        snapshot = update.snapshot;
    }
}

#[test]
fn window_reparse_keeps_reference_link_resolution() {
    // Shrunk from randomized_full_and_incremental_snapshots_agree with
    // edits = [(251, 50, 45), (29, 234, 169), (75, 226, 91), (56, 169, 84)].
    // The final one-byte deletion at the document start must not reparse the
    // later paragraph without reference definitions in scope; the resolved
    // "[n][id]" link has to survive as a Link node.
    let mut candidate = BASE.to_owned();
    let mut snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut revision = DocumentRevision::INITIAL;
    for (start, end, replacement) in [
        (50, 112, "x"),
        (0, 29, "x"),
        (25, 26, "[n][id]"),
        (0, 1, ""),
    ] {
        let replacement: Arc<str> = Arc::from(replacement);
        candidate.replace_range(start..end, &replacement);
        revision = revision.checked_next().unwrap();
        let update = reparse_markdown(
            &snapshot,
            revision,
            source(&candidate),
            &[TextChange {
                old_range: range(start, end),
                replacement,
            }],
        )
        .unwrap();
        assert_full_oracle(&update.snapshot, &candidate);
        snapshot = update.snapshot;
    }
}

#[test]
fn edit_in_sibling_line_keeps_reference_link_in_window() {
    // Shrunk from randomized_full_and_incremental_snapshots_agree with
    // edits = [(17, 155, 3), (8, 152, 37)]. The first edit breaks the
    // frontmatter close fence and leaves a resolved "[n][id]" link inside the
    // resulting paragraph. The second edit touches only that paragraph's first
    // line, so the shell window covers the link but not its definition; the
    // reparse must fall back to a full parse instead of dropping the Link.
    let mut candidate = BASE.to_owned();
    let mut snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let mut revision = DocumentRevision::INITIAL;
    for (start, end, replacement) in [(16, 17, "[n][id]"), (7, 8, "x")] {
        let replacement: Arc<str> = Arc::from(replacement);
        candidate.replace_range(start..end, &replacement);
        revision = revision.checked_next().unwrap();
        let update = reparse_markdown(
            &snapshot,
            revision,
            source(&candidate),
            &[TextChange {
                old_range: range(start, end),
                replacement,
            }],
        )
        .unwrap();
        assert_full_oracle(&update.snapshot, &candidate);
        snapshot = update.snapshot;
    }
}

const NESTED_FRONTMATTER_BASE: &str = "---\ntitle: test\nmeta:\n  owner: platform\n  tags:\n    - a\n    - b\nnotes: |\n  line one\n  line two\n---\n\n# Model\n\n[id]: /one\n\n- item\n\nuse [x][id]\n";

#[test]
fn frontmatter_indent_edit_forces_full_reparse() {
    // Inserting leading whitespace inside a nested frontmatter mapping
    // restructures the tree without moving the region or its fences, which
    // the range-and-fence check alone waves through.
    let candidate = NESTED_FRONTMATTER_BASE.to_owned();
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(&candidate),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let start = candidate.find("  owner: platform").unwrap();
    let replacement: Arc<str> = Arc::from("  ");
    let mut edited = candidate.clone();
    edited.insert_str(start, &replacement);
    let update = reparse_markdown(
        &snapshot,
        DocumentRevision::INITIAL.checked_next().unwrap(),
        source(&edited),
        &[TextChange {
            old_range: range(start, start),
            replacement,
        }],
    )
    .unwrap();
    assert_full_oracle(&update.snapshot, &edited);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::FrontmatterBoundaryChanged
        }
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn frontmatter_interior_edits_full_and_incremental_agree(
        first in any::<u8>(), second in any::<u8>(), fragment_index in 0usize..9
    ) {
        // Index 8 is the empty string: a deletion of the selected range.
        let fragments = [" ", "  ", "x", ":", "-", "\n", "#c", "\t", ""];
        let candidate = NESTED_FRONTMATTER_BASE.to_owned();
        let snapshot = parse_markdown(DocumentRevision::INITIAL, source(&candidate), MarkdownDialect::WAML_DEFAULT).unwrap();
        let frontmatter_end = candidate.find("\n---\n").map_or(candidate.len(), |at| at + "\n---\n".len());
        let points: Vec<usize> = boundaries(&candidate[..frontmatter_end]);
        prop_assume!(!points.is_empty());
        let left = usize::from(first) % points.len();
        let right = usize::from(second) % points.len();
        let (start, end) = if left <= right { (points[left], points[right]) } else { (points[right], points[left]) };
        let fragment = fragments[fragment_index % fragments.len()];
        let replacement: Arc<str> = Arc::from(fragment);
        let mut edited = candidate.clone();
        edited.replace_range(start..end, fragment);
        let update = reparse_markdown(
            &snapshot,
            DocumentRevision::INITIAL.checked_next().unwrap(),
            source(&edited),
            &[TextChange { old_range: range(start, end), replacement }],
        ).unwrap();
        assert_full_oracle(&update.snapshot, &edited);
    }
}

// A heading section carrying only a reference definition, followed by a
// second heading section whose only content is a line mixing an inline link
// (`[a](x)`) with a reference use (`[b][id]`) that follows it. The two
// sections land in separate shell windows, so an edit inside the second
// section can only resolve `[b][id]` correctly if the reference-label
// scanner still notices the use after the inline link (issue #21).
const INLINE_LINK_BEFORE_REFERENCE_USE_BASE: &str =
    "# A\n\n[id]: /one\n\n# B\n\nsee [a](x) then [b][id]\n";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn inline_link_before_reference_use_full_and_incremental_agree(letter in "[a-zA-Z]") {
        // A same-length, single-character substitution of the word "then"'s
        // first letter. Fixing the edit's position and length isolates the
        // reference-label scanning class this test targets (issue #21) from
        // the unrelated destination-range-tracking divergence a
        // length-changing edit near the inline link's `(x)` destination can
        // trigger (a separate, pre-existing incremental-splicing concern).
        let candidate = INLINE_LINK_BEFORE_REFERENCE_USE_BASE.to_owned();
        let snapshot = parse_markdown(DocumentRevision::INITIAL, source(&candidate), MarkdownDialect::WAML_DEFAULT).unwrap();
        let at = candidate.find("then").unwrap();
        let replacement: Arc<str> = Arc::from(letter.as_str());
        let mut edited = candidate.clone();
        edited.replace_range(at..at + 1, &letter);
        let update = reparse_markdown(
            &snapshot,
            DocumentRevision::INITIAL.checked_next().unwrap(),
            source(&edited),
            &[TextChange { old_range: range(at, at + 1), replacement }],
        ).unwrap();
        assert_full_oracle(&update.snapshot, &edited);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn randomized_full_and_incremental_snapshots_agree(edits in prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>()), 1..=8)) {
        let mut candidate = BASE.to_owned();
        let mut snapshot = parse_markdown(DocumentRevision::INITIAL, source(&candidate), MarkdownDialect::WAML_DEFAULT).unwrap();
        let mut revision = DocumentRevision::INITIAL;
        for (first, second, replacement_kind) in edits {
            let points = boundaries(&candidate);
            let left = usize::from(first) % points.len();
            let right = usize::from(second) % points.len();
            let (start, end) = if left <= right { (points[left], points[right]) } else { (points[right], points[left]) };
            let replacement: Arc<str> = match replacement_kind % 4 {
                0 => Arc::from(""),
                1 => Arc::from("x"),
                2 => Arc::from("é"),
                _ => Arc::from("[n][id]"),
            };
            candidate.replace_range(start..end, &replacement);
            revision = revision.checked_next().unwrap();
            let update = reparse_markdown(
                &snapshot,
                revision,
                source(&candidate),
                &[TextChange { old_range: range(start, end), replacement }],
            ).unwrap();
            assert_full_oracle(&update.snapshot, &candidate);
            match update.outcome {
                MarkdownReparseOutcome::Full { reason } => {
                    prop_assert!(matches!(reason,
                        FullReparseReason::NoPreviousSnapshot
                        | FullReparseReason::OverlappingChanges
                        | FullReparseReason::InvalidUtf8Boundary
                        | FullReparseReason::FrontmatterBoundaryChanged
                        | FullReparseReason::MarkdownContainerBoundaryChanged
                        | FullReparseReason::HeadingBoundaryChanged
                        | FullReparseReason::IslandBoundaryChanged
                        | FullReparseReason::UnsafeSynchronization
                    ));
                    prop_assert_eq!(update.affected_ranges.as_ref(), &[range(0, candidate.len())]);
                }
                MarkdownReparseOutcome::Incremental { reparsed_range, .. } => {
                    if let Some(reparsed_range) = reparsed_range {
                        prop_assert!(reparsed_range.end().to_usize() <= candidate.len());
                    }
                }
            }
            snapshot = update.snapshot;
        }
    }
}
