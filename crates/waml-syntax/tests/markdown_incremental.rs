use std::{collections::HashSet, sync::Arc};

use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, FullReparseReason, GreenElement,
    MarkdownDialect, MarkdownLinkKind, MarkdownReparseOutcome, SourceText, SyntaxElement,
    SyntaxIdentity, SyntaxNode, TextChange, TextRange, TextSize,
};

fn text(value: &str) -> SourceText {
    SourceText::new(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn intersects(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn reference_destinations(snapshot: &waml_syntax::MarkdownSyntaxSnapshot) -> Vec<String> {
    snapshot
        .queries()
        .links()
        .filter(|link| link.kind == MarkdownLinkKind::Reference)
        .map(|link| link.destination.to_string())
        .collect()
}

fn source_independent_addresses(
    element: &GreenElement<waml_syntax::OkfMarkdownLanguage>,
    addresses: &mut HashSet<usize>,
) {
    match element {
        GreenElement::Node(node) => {
            if node.is_source_independent() {
                addresses.insert(Arc::as_ptr(node) as usize);
            }
            for child in node.children() {
                source_independent_addresses(child, addresses);
            }
        }
        GreenElement::Token(token) if token.is_source_independent() => {
            addresses.insert(Arc::as_ptr(token) as usize);
        }
        GreenElement::Token(_) => {}
    }
}

fn has_shared_source_independent_green(
    previous: &waml_syntax::MarkdownSyntaxSnapshot,
    next: &waml_syntax::MarkdownSyntaxSnapshot,
) -> bool {
    let mut old = HashSet::new();
    let mut new = HashSet::new();
    source_independent_addresses(
        &GreenElement::Node(previous.tree().root_green().clone()),
        &mut old,
    );
    source_independent_addresses(
        &GreenElement::Node(next.tree().root_green().clone()),
        &mut new,
    );
    !old.is_disjoint(&new)
}

fn node_for_identity(
    node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
    identity: SyntaxIdentity,
) -> Option<SyntaxNode<waml_syntax::OkfMarkdownLanguage>> {
    if waml_syntax::syntax_identity(&node) == Some(identity) {
        return Some(node);
    }
    node.children().find_map(|child| match child {
        SyntaxElement::Node(child) => node_for_identity(child, identity),
        SyntaxElement::Token(_) => None,
    })
}

fn structural_fingerprint(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
) -> Vec<(String, TextRange)> {
    fn collect(
        node: SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
        out: &mut Vec<(String, TextRange)>,
    ) {
        out.push((format!("{:?}", node.kind()), node.range()));
        for child in node.children() {
            match child {
                SyntaxElement::Node(child) => collect(child, out),
                SyntaxElement::Token(token) => {
                    out.push((format!("{:?}", token.kind()), token.range()));
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(snapshot.tree().root(), &mut out);
    out
}

fn semantic_fingerprint(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let links = snapshot
        .queries()
        .links()
        .map(|link| {
            format!(
                "{:?}:{:?}:{:?}:{:?}",
                link.kind, link.source_range, link.destination, link.title
            )
        })
        .collect();
    let diagnostics = snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{:?}:{}",
                diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
            )
        })
        .collect();
    let structure = vec![format!(
        "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
        snapshot.structure().headings,
        snapshot.structure().nested_headings,
        snapshot.structure().protected_ranges,
        snapshot.structure().list_item_lines,
        snapshot.structure().tab_indented_item_lines,
        snapshot.structure().opaque_ranges,
    )];
    let islands = snapshot
        .structure()
        .islands
        .iter()
        .map(|island| {
            format!(
                "{:?}:{:?}:{:?}",
                island.kind, island.heading_range, island.content_range
            )
        })
        .collect();
    (links, diagnostics, structure, islands)
}

fn assert_matches_full_oracle(old: &str, new: &str, changes: &[TextChange]) {
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let update = reparse_markdown(&previous, DocumentRevision::new(1), text(new), changes).unwrap();
    let full = parse_markdown(
        DocumentRevision::new(1),
        text(new),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();

    assert_eq!(update.snapshot.text().shared().as_str(), new);
    assert_eq!(update.snapshot.tree().write_to_string(), new);
    assert_eq!(
        structural_fingerprint(&update.snapshot),
        structural_fingerprint(&full)
    );
    assert_eq!(
        semantic_fingerprint(&update.snapshot),
        semantic_fingerprint(&full)
    );
    for window in update.affected_ranges.windows(2) {
        assert!(window[0].end() < window[1].start());
    }
    assert!(update
        .affected_ranges
        .iter()
        .all(|affected| affected.start() < affected.end()));
    for island in update.snapshot.structure().islands.iter() {
        assert!(node_for_identity(update.snapshot.tree().root(), island.owner).is_some());
    }
}

#[test]
fn definition_change_updates_non_contiguous_reference_dependents() {
    // This fails if definition edits only reparse their local shell window and
    // leave reference annotations in non-contiguous paragraphs unchanged.
    let old = "[id]: /one\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let new = "[id]: /two\n\nfirst [a][id]\n\nsecond [b][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();

    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(6, 10),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();
    let oracle = parse_markdown(
        DocumentRevision::new(1),
        text(new),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();

    assert!(
        matches!(update.outcome, MarkdownReparseOutcome::Incremental { .. }),
        "unexpected outcome: {:?}",
        update.outcome,
    );
    assert_eq!(
        reference_destinations(&update.snapshot),
        vec!["/two", "/two"]
    );
    assert_eq!(
        reference_destinations(&update.snapshot),
        reference_destinations(&oracle)
    );
    assert!(
        matches!(
            &update.outcome,
            MarkdownReparseOutcome::Incremental {
                reparsed_range: None,
                ..
            }
        ),
        "unexpected outcome: {:?}",
        update.outcome
    );
    assert!(update.affected_ranges.len() >= 2);
}

#[test]
fn local_edit_publishes_the_caller_source_and_a_single_normalized_range() {
    // This fails if direct incremental reparse allocates another source or if
    // normalization reports an unrelated shell window.
    let old = "# one\n";
    let new = "# two\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let next_source = text(new);
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        next_source.clone(),
        &[TextChange {
            old_range: range(2, 5),
            replacement: Arc::from("two"),
        }],
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        update.snapshot.text().shared(),
        next_source.shared()
    ));
    assert_eq!(update.snapshot.tree().write_to_string(), new);
    assert_eq!(update.affected_ranges.len(), 1);
    assert!(matches!(
        update.outcome,
        MarkdownReparseOutcome::Incremental {
            shared_source_independent_green,
            reparsed_range: Some(_),
        } if shared_source_independent_green > 0
    ));
}

#[test]
fn multiline_change_detects_definition_on_an_interior_line() {
    // This fails if detection inspects only the first or last intersecting line.
    let old = "before\n\n[id]: /one\n\nafter\n\nuse [x][id]\n";
    let new = "BEFORE\n\n[id]: /two\n\nAFTER\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(0, old.rfind("\n\nuse").unwrap()),
            replacement: Arc::from("BEFORE\n\n[id]: /two\n\nAFTER"),
        }],
    )
    .unwrap();

    assert_eq!(reference_destinations(&update.snapshot), vec!["/two"]);
    assert!(update.affected_ranges.len() >= 2);
}

#[test]
fn changed_definition_fans_out_only_its_label_and_reuses_unaffected_greens() {
    // This fails if fan-out reparses every reference instead of only changed labels.
    let old = "# stable\n\n[id]: /one\n[other]: /keep\n\nuse [x][id]\n\nkeep [y][other]\n";
    let new = "# stable\n\n[id]: /two\n[other]: /keep\n\nuse [x][id]\n\nkeep [y][other]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();
    let unrelated = update
        .snapshot
        .queries()
        .links()
        .find(|link| link.destination.as_ref() == "/keep")
        .unwrap()
        .source_range;

    assert!(update
        .affected_ranges
        .iter()
        .all(|affected| !intersects(*affected, unrelated)));
    assert!(has_shared_source_independent_green(
        &previous,
        &update.snapshot
    ));
}

#[test]
fn renamed_definition_invalidates_old_backlinks() {
    // This fails if fan-out consults only new backlinks after a label disappears.
    let old = "[id]: /one\n\nuse [x][id]\n";
    let new = "[new]: /one\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let old_use = previous.queries().links().next().unwrap().source_range;
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(1, 3),
            replacement: Arc::from("new"),
        }],
    )
    .unwrap();

    assert!(reference_destinations(&update.snapshot).is_empty());
    assert!(update
        .affected_ranges
        .iter()
        .any(|affected| intersects(*affected, old_use)));
}

#[test]
fn definition_change_does_not_mask_named_bridge_fallback() {
    // This fails if reference handling replaces a bridge Full outcome.
    let old = "---\n---\n\n[id]: /one\n\nuse [x][id]\n";
    let new = "----\n---\n\n[id]: /two\n\nuse [x][id]\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[
            TextChange {
                old_range: range(3, 3),
                replacement: Arc::from("-"),
            },
            TextChange {
                old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
                replacement: Arc::from("/two"),
            },
        ],
    )
    .unwrap();

    assert_eq!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::FrontmatterBoundaryChanged,
        }
    );
    assert_eq!(update.affected_ranges.as_ref(), &[range(0, new.len())]);
}

#[test]
fn definition_edit_preserves_an_unchanged_island_identity() {
    // This fails if a definition update replaces the entire tree and regenerates
    // identities for an island outside the affected roots.
    let old = "[id]: /one\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let new = "[id]: /two\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(old),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    assert_eq!(previous.structure().islands.len(), 1);
    let owner = previous.structure().islands[0].owner;
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text(new),
        &[TextChange {
            old_range: range(old.find("/one").unwrap(), old.find("/one").unwrap() + 4),
            replacement: Arc::from("/two"),
        }],
    )
    .unwrap();

    assert!(
        matches!(update.outcome, MarkdownReparseOutcome::Incremental { .. }),
        "island edit outcome: {:?}",
        update.outcome,
    );
    assert_eq!(
        update.snapshot.structure().islands[0].owner,
        owner,
        "old owner node: {:?}; new island owner node: {:?}",
        node_for_identity(previous.tree().root(), owner).map(|node| (node.kind(), node.range())),
        node_for_identity(
            update.snapshot.tree().root(),
            update.snapshot.structure().islands[0].owner,
        )
        .map(|node| (node.kind(), node.range())),
    );
}

#[test]
fn deterministic_incremental_cases_match_the_full_parse_oracle() {
    let cases = [
        ("local paragraph", "alpha\n", "alpHa\n", 3, 4, "H"),
        ("list marker", "- one\n", "* one\n", 0, 1, "*"),
        ("heading boundary", "# one\n", "## one\n", 1, 1, "#"),
        (
            "fence boundary",
            "```\na\n```\n",
            "~~~~\na\n~~~~\n",
            0,
            10,
            "~~~~\na\n~~~~\n",
        ),
        (
            "table delimiter",
            "| a |\n| - |\n| b |\n",
            "| a |\n| :-: |\n| b |\n",
            8,
            9,
            ":-:",
        ),
        (
            "frontmatter fence",
            "---\ntype: uml.Class\n---\n# A\n",
            "----\ntype: uml.Class\n---\n# A\n",
            3,
            3,
            "-",
        ),
        (
            "WAML section boundary",
            "# Model\n## Attributes\none\n",
            "# Model\n## Values\none\n",
            11,
            21,
            "Values",
        ),
        ("Unicode replacement", "café\n", "cafø\n", 3, 5, "ø"),
        ("EOF insertion", "tail\n", "tail\nmore\n", 5, 5, "more\n"),
    ];
    for (name, old, new, start, end, replacement) in cases {
        let changes = [TextChange {
            old_range: range(start, end),
            replacement: Arc::from(replacement),
        }];
        let result = std::panic::catch_unwind(|| assert_matches_full_oracle(old, new, &changes));
        assert!(result.is_ok(), "oracle case failed: {name}");
    }
}

#[test]
fn boundary_edits_report_named_fallbacks() {
    for (old, new, change, reason) in [
        (
            "# one\n",
            "## one\n",
            TextChange {
                old_range: range(1, 1),
                replacement: Arc::from("#"),
            },
            FullReparseReason::HeadingBoundaryChanged,
        ),
        (
            "---\ntype: uml.Class\n---\n",
            "----\ntype: uml.Class\n---\n",
            TextChange {
                old_range: range(3, 3),
                replacement: Arc::from("-"),
            },
            FullReparseReason::FrontmatterBoundaryChanged,
        ),
        (
            "# Model\n## Attributes\none\n",
            "# Model\n## Values\none\n",
            TextChange {
                old_range: range(11, 21),
                replacement: Arc::from("Values"),
            },
            FullReparseReason::IslandBoundaryChanged,
        ),
    ] {
        let previous = parse_markdown(
            DocumentRevision::INITIAL,
            text(old),
            MarkdownDialect::CommonMarkCurrent,
        )
        .unwrap();
        let update =
            reparse_markdown(&previous, DocumentRevision::new(1), text(new), &[change]).unwrap();
        assert_eq!(update.outcome, MarkdownReparseOutcome::Full { reason });
        assert_eq!(update.affected_ranges.as_ref(), &[range(0, new.len())]);
    }
}

#[test]
fn overlapping_changes_use_the_named_full_fallback() {
    let source = "abcdef\n";
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text(source),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let update = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text("aXYf\n"),
        &[
            TextChange {
                old_range: range(1, 4),
                replacement: Arc::from("X"),
            },
            TextChange {
                old_range: range(3, 5),
                replacement: Arc::from("Y"),
            },
        ],
    )
    .unwrap();

    assert_eq!(
        update.outcome,
        MarkdownReparseOutcome::Full {
            reason: FullReparseReason::OverlappingChanges,
        }
    );
    assert_eq!(update.affected_ranges.as_ref(), &[range(0, 5)]);
}

#[test]
fn new_text_change_mismatch_is_a_hard_error() {
    let previous = parse_markdown(
        DocumentRevision::INITIAL,
        text("old\n"),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let error = reparse_markdown(
        &previous,
        DocumentRevision::new(1),
        text("new\n"),
        &[TextChange {
            old_range: range(0, 3),
            replacement: Arc::from("other"),
        }],
    )
    .err()
    .expect("mismatched changes must fail");

    assert!(matches!(
        error,
        waml_syntax::ParseError::StructuralInvariant { .. }
    ));
}

#[test]
fn published_query_and_structure_owners_resolve_in_the_published_tree() {
    let source = "[id]: /dest\n\nuse [x][id]\n\n# Model\n## Attributes\nname: String\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        text(source),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    for link in snapshot.queries().links() {
        assert!(node_for_identity(snapshot.tree().root(), link.owner).is_some());
        assert!(node_for_identity(snapshot.tree().root(), link.identity).is_some());
    }
    for island in snapshot.structure().islands.iter() {
        assert!(node_for_identity(snapshot.tree().root(), island.owner).is_some());
        assert!(snapshot.queries().island(island.owner).is_some());
    }
}
