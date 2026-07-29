use std::sync::Arc;

use std::num::NonZeroU64;
use waml_syntax::{
    annotate_occurrence, parse_okf_markdown, rebase_unchanged_green, reparse_okf_markdown,
    transfer_mapped_annotations, ChangeMap, FullReparseReason, GreenElement, GreenText,
    MarkdownDialect, OkfMarkdownSyntaxKind, ReparseOutcome, RewriteError, SourceText,
    SyntaxAnnotation, SyntaxElement, SyntaxTree, TextChange, TextRange, TextSize,
};

fn text(value: &str) -> SourceText {
    SourceText::from_shared(Arc::new(value.to_owned())).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(end).unwrap(),
    )
    .unwrap()
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
    assert_eq!(
        incremental.diagnostics().len(),
        full.tree.diagnostics().len()
    );
}

fn first_node(
    tree: &SyntaxTree<waml_syntax::OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
) -> waml_syntax::SyntaxNode<waml_syntax::OkfMarkdownLanguage> {
    fn find(
        node: waml_syntax::SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
        kind: OkfMarkdownSyntaxKind,
    ) -> Option<waml_syntax::SyntaxNode<waml_syntax::OkfMarkdownLanguage>> {
        if node.kind() == kind {
            return Some(node);
        }
        node.children()
            .find_map(|child| child.into_node().and_then(|node| find(node, kind)))
    }
    find(tree.root(), kind).unwrap()
}

fn first_token(
    tree: &SyntaxTree<waml_syntax::OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
) -> waml_syntax::SyntaxToken<waml_syntax::OkfMarkdownLanguage> {
    fn find(
        node: waml_syntax::SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
        kind: OkfMarkdownSyntaxKind,
    ) -> Option<waml_syntax::SyntaxToken<waml_syntax::OkfMarkdownLanguage>> {
        node.children().find_map(|child| match child {
            SyntaxElement::Token(token) if token.kind() == kind => Some(token),
            SyntaxElement::Node(node) => find(node, kind),
            _ => None,
        })
    }
    find(tree.root(), kind).unwrap()
}

fn all_source_slices_use(
    element: &GreenElement<waml_syntax::OkfMarkdownLanguage>,
    source: &SourceText,
) -> bool {
    match element {
        GreenElement::Node(node) => node
            .children()
            .iter()
            .all(|child| all_source_slices_use(child, source)),
        GreenElement::Token(token) => std::iter::once(token.text())
            .chain(token.leading_trivia().iter().map(|trivia| &trivia.text))
            .chain(token.trailing_trivia().iter().map(|trivia| &trivia.text))
            .all(|text| match text {
                GreenText::SourceSlice { source: actual, .. } => {
                    Arc::ptr_eq(actual.shared(), source.shared())
                }
                GreenText::Static(_) | GreenText::Owned(_) => true,
            }),
    }
}

#[test]
fn green_rebase_rebuilds_source_backed_and_reuses_static_greens() {
    let old = text("# One\nbody\n");
    let previous = parse_okf_markdown(old.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
    let new = text("# One\nbody\n");
    let map = ChangeMap::checked(&old, &[]).unwrap();

    let rebased = rebase_unchanged_green(
        &GreenElement::Node(previous.tree.root_green().clone()),
        &new,
        &map,
    )
    .unwrap()
    .unwrap();
    assert!(all_source_slices_use(&rebased.element, &new));
    let candidate = match rebased.element {
        GreenElement::Node(root) => SyntaxTree::new(
            root,
            Arc::from(previous.tree.diagnostics()),
            MarkdownDialect::CommonMarkCurrent,
        ),
        GreenElement::Token(_) => panic!("root is a node"),
    };
    assert!(!first_node(&previous.tree, OkfMarkdownSyntaxKind::Heading)
        .same_green(&first_node(&candidate, OkfMarkdownSyntaxKind::Heading)));
    assert!(!previous.tree.root().same_green(&candidate.root()));
    assert!(
        first_token(&previous.tree, OkfMarkdownSyntaxKind::EndOfFileToken).same_green(
            &first_token(&candidate, OkfMarkdownSyntaxKind::EndOfFileToken)
        )
    );
    assert!(rebased.shared_source_independent_green >= 1);
}

#[test]
fn mapped_annotations_preserve_node_and_token_occurrences() {
    let old = text("# One\nbody\n");
    let parsed = parse_okf_markdown(old.clone(), MarkdownDialect::CommonMarkCurrent).unwrap();
    let heading = first_node(&parsed.tree, OkfMarkdownSyntaxKind::Heading);
    let heading_text = first_token(&parsed.tree, OkfMarkdownSyntaxKind::HeadingText);
    let node_locator = heading.locator();
    let token_locator = heading_text.locator();
    let node_annotation = SyntaxAnnotation::new(NonZeroU64::new(1).unwrap(), "node", None);
    let token_annotation = SyntaxAnnotation::new(NonZeroU64::new(2).unwrap(), "token", None);
    let annotated =
        annotate_occurrence(&parsed.tree, &node_locator, node_annotation.clone()).unwrap();
    let annotated_tree = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::CommonMarkCurrent,
    );
    let annotated_token = first_token(&annotated_tree, OkfMarkdownSyntaxKind::HeadingText);
    let annotated = annotate_occurrence(
        &annotated_tree,
        &annotated_token.locator(),
        token_annotation.clone(),
    )
    .unwrap();
    let previous = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::CommonMarkCurrent,
    );
    let new = text("# One\nbody!\n");
    let map = ChangeMap::checked(
        &old,
        &[TextChange {
            old_range: range(10, 10),
            replacement: Arc::from("!"),
        }],
    )
    .unwrap();
    let candidate = parse_okf_markdown(new, MarkdownDialect::CommonMarkCurrent).unwrap();
    let candidate_diagnostics: Arc<[_]> = Arc::from(candidate.tree.diagnostics());
    let candidate_heading = first_node(&candidate.tree, OkfMarkdownSyntaxKind::Heading);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate.tree,
            &candidate_heading.locator(),
            node_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::CommonMarkCurrent,
    );
    let candidate_heading = first_node(&candidate, OkfMarkdownSyntaxKind::Heading);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_heading.locator(),
            node_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::CommonMarkCurrent,
    );
    let candidate_token = first_token(&candidate, OkfMarkdownSyntaxKind::HeadingText);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_token.locator(),
            token_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::CommonMarkCurrent,
    );
    let candidate_token = first_token(&candidate, OkfMarkdownSyntaxKind::HeadingText);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_token.locator(),
            token_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::CommonMarkCurrent,
    );
    let transferred = SyntaxTree::new(
        transfer_mapped_annotations(&previous, &candidate, &map),
        candidate_diagnostics,
        MarkdownDialect::CommonMarkCurrent,
    );
    let mapped_heading = first_node(&transferred, OkfMarkdownSyntaxKind::Heading);
    let mapped_token = first_token(&transferred, OkfMarkdownSyntaxKind::HeadingText);
    assert_eq!(
        mapped_heading
            .syntax_annotations()
            .iter()
            .filter(|annotation| annotation.id() == node_annotation.id())
            .count(),
        1
    );
    assert_eq!(
        mapped_token
            .syntax_annotations()
            .iter()
            .filter(|annotation| annotation.id() == token_annotation.id())
            .count(),
        1
    );
    assert_eq!(
        mapped_token.range(),
        map.translate_unchanged(heading_text.range()).unwrap()
    );
    assert!(matches!(
        transferred.resolve(&node_locator),
        Err(RewriteError::WrongTree { .. })
    ));
    assert!(matches!(
        transferred.resolve(&token_locator),
        Err(RewriteError::WrongTree { .. })
    ));
}

#[test]
fn reparse_matches_full_oracle_for_safe_edits_and_fallback_boundaries() {
    for (previous, next, changes) in [
        (
            "# One\nbody\n",
            "# Two\nbody\n",
            vec![TextChange {
                old_range: range(2, 5),
                replacement: Arc::from("Two"),
            }],
        ),
        (
            "# One\nbody\n",
            "# One\nbody!\n",
            vec![TextChange {
                old_range: range(10, 10),
                replacement: Arc::from("!"),
            }],
        ),
        (
            "# Café\nbody\n",
            "# Café\nbody!\n",
            vec![TextChange {
                old_range: range(12, 12),
                replacement: Arc::from("!"),
            }],
        ),
        (
            "---\ntype: uml.Class\n---\n# One\n",
            "---\ntype: uml.Interface\n---\n# One\n",
            vec![TextChange {
                old_range: range(10, 19),
                replacement: Arc::from("uml.Interface"),
            }],
        ),
        (
            "# One\nbody\n",
            "## One\nbody\n",
            vec![TextChange {
                old_range: range(1, 1),
                replacement: Arc::from("#"),
            }],
        ),
        (
            "# One\n  body\n",
            "# One\n    body\n",
            vec![TextChange {
                old_range: range(6, 8),
                replacement: Arc::from("    "),
            }],
        ),
        (
            "# One\na: [b, c]\n",
            "# One\na: [b, d]\n",
            vec![TextChange {
                old_range: range(13, 14),
                replacement: Arc::from("d"),
            }],
        ),
        (
            "# One\na\nb\n",
            "# Uno\na\nbee\n",
            vec![
                TextChange {
                    old_range: range(2, 5),
                    replacement: Arc::from("Uno"),
                },
                TextChange {
                    old_range: range(8, 9),
                    replacement: Arc::from("bee"),
                },
            ],
        ),
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
            &[TextChange {
                old_range: range(5, 6),
                replacement: Arc::from("x")
            }],
        )
        .unwrap_err(),
        FullReparseReason::InvalidUtf8Boundary,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[
                TextChange {
                    old_range: range(2, 3),
                    replacement: Arc::from("x")
                },
                TextChange {
                    old_range: range(1, 2),
                    replacement: Arc::from("y")
                },
            ],
        )
        .unwrap_err(),
        FullReparseReason::OverlappingChanges,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[
                TextChange {
                    old_range: range(2, 4),
                    replacement: Arc::from("x")
                },
                TextChange {
                    old_range: range(3, 5),
                    replacement: Arc::from("y")
                },
            ],
        )
        .unwrap_err(),
        FullReparseReason::OverlappingChanges,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[
                TextChange {
                    old_range: range(2, 2),
                    replacement: Arc::from("x")
                },
                TextChange {
                    old_range: range(2, 2),
                    replacement: Arc::from("y")
                },
            ],
        )
        .unwrap_err(),
        FullReparseReason::OverlappingChanges,
    );
    assert_eq!(
        ChangeMap::checked(
            &source,
            &[TextChange {
                old_range: range(0, 9),
                replacement: Arc::from("x")
            }],
        )
        .unwrap_err(),
        FullReparseReason::UnsafeSynchronization,
    );
    assert!(matches!(
        reparse_okf_markdown(
            &parse_okf_markdown(source, MarkdownDialect::CommonMarkCurrent)
                .unwrap()
                .tree,
            text("# Café\n"),
            &[]
        )
        .unwrap(),
        ReparseOutcome::Full {
            reason: FullReparseReason::NoPreviousSnapshot,
            ..
        } | ReparseOutcome::Incremental { .. }
    ));
}

#[test]
fn change_map_translates_only_unchanged_occurrences_and_surviving_boundaries() {
    let source = text("zero one two");
    let map = ChangeMap::checked(
        &source,
        &[TextChange {
            old_range: range(5, 8),
            replacement: Arc::from("ONE!"),
        }],
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
        &[TextChange {
            old_range: range(4, 4),
            replacement: Arc::from("x"),
        }],
    )
    .unwrap();

    assert_eq!(map.translate_end_boundary(size(4)), Some(size(4)));
    assert_eq!(map.translate_start_boundary(size(4)), Some(size(5)));
    assert_eq!(map.translate_unchanged(range(0, 4)), Some(range(0, 4)));
    assert_eq!(map.translate_unchanged(range(4, 9)), Some(range(5, 10)));
}

#[test]
fn change_map_accumulates_deletion_and_insertion_deltas() {
    let source = text("abcdefghi");
    let map = ChangeMap::checked(
        &source,
        &[
            TextChange {
                old_range: range(1, 4),
                replacement: Arc::from(""),
            },
            TextChange {
                old_range: range(6, 6),
                replacement: Arc::from("XYZ"),
            },
        ],
    )
    .unwrap();

    assert_eq!(map.old_len(), size(9));
    assert_eq!(map.new_len(), size(9));
    assert_eq!(map.translate_unchanged(range(0, 1)), Some(range(0, 1)));
    assert_eq!(map.translate_unchanged(range(4, 6)), Some(range(1, 3)));
    assert_eq!(map.translate_end_boundary(size(6)), Some(size(3)));
    assert_eq!(map.translate_start_boundary(size(6)), Some(size(6)));
    assert_eq!(map.translate_unchanged(range(6, 9)), Some(range(6, 9)));
}

#[test]
fn change_map_candidate_source_mismatch_is_a_hard_error() {
    let previous = parse_okf_markdown(text("# One\n"), MarkdownDialect::CommonMarkCurrent).unwrap();
    let error = match reparse_okf_markdown(
        &previous.tree,
        text("# Two\n"),
        &[TextChange {
            old_range: range(2, 5),
            replacement: Arc::from("Uno"),
        }],
    ) {
        Ok(_) => panic!("mismatched candidate source was accepted"),
        Err(error) => error,
    };

    match error {
        waml_syntax::ParseError::StructuralInvariant { reason } => {
            assert_eq!(
                &*reason,
                "incremental changes do not reconstruct candidate source"
            );
        }
        other => panic!("unexpected mismatch error: {other:?}"),
    }
}
