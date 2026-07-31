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
