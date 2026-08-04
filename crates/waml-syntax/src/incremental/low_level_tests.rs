use std::sync::Arc;

use super::properties::{diagnostic_fingerprint, text_fingerprint};
use super::{reparse_okf_markdown, reparse_okf_markdown_with_structure};
use crate::{
    annotate_occurrence, markdown::parser::parse as parse_okf_markdown, rebase_unchanged_green,
    transfer_mapped_annotations, ChangeMap, FullReparseReason, GreenElement, GreenText,
    MarkdownDialect, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode, ReparseOutcome, RewriteError,
    SourceText, SyntaxAnnotation, SyntaxElement, SyntaxTree, TextChange, TextRange, TextSize,
};
use std::num::NonZeroU64;

#[test]
fn incremental_structure_is_derived_from_the_rebuilt_waml_tree() {
    let previous_source = "# Document\n## Attributes\none\n";
    let next_source = "# Document\n## Attributes\ntwo\n";
    let previous =
        parse_okf_markdown(text(previous_source), MarkdownDialect::WAML_DEFAULT).unwrap();
    let start = previous_source.find("one").unwrap();
    let (outcome, structure) = reparse_okf_markdown_with_structure(
        &previous.tree,
        text(next_source),
        &[TextChange {
            old_range: range(start, start + "one".len()),
            replacement: Arc::from("two"),
        }],
        None,
        None,
    )
    .unwrap();
    let tree = match outcome {
        ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
    };
    let section = tree
        .root()
        .children()
        .find_map(|child| match child {
            SyntaxElement::Node(node) if node.kind() == OkfMarkdownSyntaxKind::WamlSection => {
                Some(node)
            }
            _ => None,
        })
        .expect("rebuilt tree keeps the WAML section wrapper");
    let owner = section
        .syntax_annotations()
        .iter()
        .find(|annotation| annotation.kind() == "waml.markdown.identity")
        .and_then(|annotation| annotation.data())
        .unwrap();

    assert_eq!(structure.islands.len(), 1);
    assert_eq!(structure.islands[0].owner.get().to_string(), owner);
    assert_eq!(structure.islands[0].heading_range, range(11, 25));
    assert_eq!(
        structure.islands[0].content_range,
        range(25, next_source.len())
    );
}

fn incremental_outcome(
    previous: &str,
    next: &str,
    changes: &[TextChange],
) -> ReparseOutcome<crate::OkfMarkdownLanguage> {
    let previous = parse_okf_markdown(text(previous), MarkdownDialect::WAML_DEFAULT).unwrap();
    reparse_okf_markdown(&previous.tree, text(next), changes).unwrap()
}

fn assert_incremental(previous: &str, next: &str, changes: &[TextChange], range: TextRange) {
    match incremental_outcome(previous, next, changes) {
        ReparseOutcome::Incremental { reparsed_range, .. } => assert_eq!(reparsed_range, range),
        ReparseOutcome::Full { reason, .. } => {
            panic!("expected incremental result, got {reason:?}")
        }
    }
}

#[test]
fn zero_width_insert_at_raw_start_has_one_owner() {
    assert_incremental(
        "body\n",
        "xbody\n",
        &[TextChange {
            old_range: range(0, 0),
            replacement: Arc::from("x"),
        }],
        range(0, 6),
    );
}

#[test]
fn zero_width_insert_at_child_boundary_selects_unique_raw_owner() {
    assert_incremental(
        "# H\nbody\n",
        "# H\nxbody\n",
        &[TextChange {
            old_range: range(4, 4),
            replacement: Arc::from("x"),
        }],
        range(4, 10),
    );
}

#[test]
fn zero_width_insert_at_eof_reparses_tail() {
    assert_incremental(
        "body\n",
        "body\nx",
        &[TextChange {
            old_range: range(5, 5),
            replacement: Arc::from("x"),
        }],
        range(0, 6),
    );
}

#[test]
fn source_backed_eof_trivia_moves_through_tail_window() {
    let previous = "body   ";
    let next = "body   x";
    let outcome = incremental_outcome(
        previous,
        next,
        &[TextChange {
            old_range: range(7, 7),
            replacement: Arc::from("x"),
        }],
    );
    let ReparseOutcome::Incremental {
        tree,
        reparsed_range,
        ..
    } = outcome
    else {
        panic!("tail insertion must be incremental")
    };
    assert_eq!(reparsed_range, range(0, 8));
    assert!(!first_token(
        &parse_okf_markdown(text(previous), MarkdownDialect::WAML_DEFAULT)
            .unwrap()
            .tree,
        OkfMarkdownSyntaxKind::EndOfFileToken
    )
    .same_green(&first_token(&tree, OkfMarkdownSyntaxKind::EndOfFileToken)));
}

#[test]
fn frontmatter_creation_at_zero_is_named() {
    assert!(matches!(
        incremental_outcome(
            "body\n",
            "---\ntype: x\n---\nbody\n",
            &[TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("---\ntype: x\n---\n")
            }]
        ),
        ReparseOutcome::Full {
            reason: FullReparseReason::FrontmatterBoundaryChanged,
            ..
        }
    ));
}

#[test]
fn edit_inside_block_scalar_literal_stays_incremental() {
    // Shaped after fm_fence_inside_block_scalar.md: the literal block's own
    // "---" line must not be mistaken for the frontmatter close fence by
    // either side of `same_frontmatter_fences`.
    let previous = "---\ndescription: |\n  ---\nkey: value\n---\n\nBody.\n";
    let start = previous.find("  ---").unwrap() + 2;
    let next = "---\ndescription: |\n  xxx\nkey: value\n---\n\nBody.\n";
    let outcome = exact_oracle(
        previous,
        next,
        &[TextChange {
            old_range: range(start, start + 3),
            replacement: Arc::from("xxx"),
        }],
    );
    assert!(
        matches!(outcome, ReparseOutcome::Incremental { .. }),
        "an edit fully inside the literal block's content must not force a full reparse"
    );
}

#[test]
fn unchanged_input_reuses_root_green() {
    let source = text("body\n");
    let previous = parse_okf_markdown(source.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
    let ReparseOutcome::Incremental {
        tree,
        shared_source_independent_green,
        reparsed_range,
    } = reparse_okf_markdown(&previous.tree, source, &[]).unwrap()
    else {
        panic!("unchanged input must be incremental")
    };
    assert_eq!(shared_source_independent_green, 1);
    assert_eq!(reparsed_range, range(0, 5));
    assert!(previous.tree.root().same_green(&tree.root()));
}

#[test]
fn unchanged_bytes_on_fresh_source_rebase_source_backed_greens() {
    let old_source = text("---\nbad\n---\nbody\n");
    let parsed = parse_okf_markdown(old_source, MarkdownDialect::WAML_DEFAULT).unwrap();
    let raw = first_node(&parsed.tree, OkfMarkdownSyntaxKind::Paragraph);
    let annotated = annotate_occurrence(
        &parsed.tree,
        &raw.locator(),
        SyntaxAnnotation::new(NonZeroU64::new(9).unwrap(), "retained", None),
    )
    .unwrap();
    let previous = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::WAML_DEFAULT,
    );
    assert!(!previous.diagnostics().is_empty());
    let new_source = text("---\nbad\n---\nbody\n");
    let (outcome, _) =
        reparse_okf_markdown_with_structure(&previous, new_source.clone(), &[], None, None)
            .unwrap();
    let ReparseOutcome::Incremental {
        tree,
        shared_source_independent_green,
        reparsed_range,
    } = outcome
    else {
        panic!("empty change map must stay incremental")
    };

    assert_eq!(shared_source_independent_green, 1);
    assert_eq!(reparsed_range, range(0, 17));
    assert!(all_source_slices_use(
        &GreenElement::Node(tree.root_green().clone()),
        &new_source
    ));
    assert!(!previous.root().same_green(&tree.root()));
    assert!(!first_node(&previous, OkfMarkdownSyntaxKind::Paragraph)
        .same_green(&first_node(&tree, OkfMarkdownSyntaxKind::Paragraph)));
    assert!(
        first_token(&previous, OkfMarkdownSyntaxKind::EndOfFileToken)
            .same_green(&first_token(&tree, OkfMarkdownSyntaxKind::EndOfFileToken))
    );
    assert_eq!(
        structural_fingerprint(&tree),
        structural_fingerprint(&previous)
    );
    assert_eq!(
        diagnostic_fingerprint(&tree),
        diagnostic_fingerprint(&previous)
    );
}

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
    let previous = parse_okf_markdown(text(previous), MarkdownDialect::WAML_DEFAULT).unwrap();
    let full = parse_okf_markdown(text(next), MarkdownDialect::WAML_DEFAULT).unwrap();
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

fn annotations(annotations: &[SyntaxAnnotation]) -> Vec<(u64, &str, Option<&str>)> {
    annotations
        .iter()
        .map(|annotation| {
            (
                annotation.id().get(),
                annotation.kind(),
                if annotation.kind() == "waml.markdown.identity" {
                    None
                } else {
                    annotation.data()
                },
            )
        })
        .collect()
}

fn structural_fingerprint(tree: &SyntaxTree<crate::OkfMarkdownLanguage>) -> Vec<String> {
    fn visit(
        element: &GreenElement<crate::OkfMarkdownLanguage>,
        at: TextSize,
        out: &mut Vec<String>,
    ) -> TextSize {
        match element {
            GreenElement::Node(node) => {
                let end = at.checked_add(node.width()).unwrap();
                out.push(format!(
                    "node:{:?}:{at:?}..{end:?}:{:?}",
                    node.kind(),
                    annotations(node.annotations())
                ));
                node.children()
                    .iter()
                    .fold(at, |offset, child| visit(child, offset, out))
            }
            GreenElement::Token(token) => {
                let end = at.checked_add(token.width()).unwrap();
                let leading: Vec<_> = token
                    .leading_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect();
                let trailing: Vec<_> = token
                    .trailing_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect();
                out.push(format!("token:{:?}:{at:?}..{end:?}:{:?}:{leading:?}:{trailing:?}:missing={}:bad={}:codes={:?}:{:?}", token.kind(), text_fingerprint(token.text()), token.flags().is_missing(), token.flags().is_bad(), token.annotations(), annotations(token.syntax_annotations())));
                end
            }
        }
    }
    let mut out = Vec::new();
    visit(
        &GreenElement::Node(tree.root_green().clone()),
        size(0),
        &mut out,
    );
    out
}

#[test]
fn text_fingerprint_detects_allocation_kind_and_slice_partition() {
    let source = text("xx");
    let first = GreenText::SourceSlice {
        source: source.clone(),
        range: range(0, 1),
    };
    let second = GreenText::SourceSlice {
        source,
        range: range(1, 2),
    };
    let owned = GreenText::Owned(Arc::from("x"));
    assert_ne!(text_fingerprint(&first), text_fingerprint(&second));
    assert_ne!(text_fingerprint(&first), text_fingerprint(&owned));
}

#[test]
fn invalid_utf8_boundary_is_a_named_full_outcome_with_structure() {
    let previous = parse_okf_markdown(text("# Café\n"), MarkdownDialect::WAML_DEFAULT).unwrap();
    let (outcome, structure) = reparse_okf_markdown_with_structure(
        &previous.tree,
        text("# Cafx\n"),
        &[TextChange {
            old_range: range(5, 6),
            replacement: Arc::from("x"),
        }],
        None,
        None,
    )
    .unwrap();
    assert!(matches!(
        outcome,
        ReparseOutcome::Full {
            reason: FullReparseReason::InvalidUtf8Boundary,
            ..
        }
    ));
    assert_eq!(structure.headings.len(), 1);
}

#[test]
fn safe_frontmatter_boundary_insertions_are_incremental() {
    for (old, new, at, replacement) in [
        ("---\na: b\n---\n", "---\nx: y\na: b\n---\n", 4, "x: y\n"),
        ("---\na: b\n---\n", "---\na: b\n---\nbody\n", 13, "body\n"),
    ] {
        assert!(matches!(
            exact_oracle(
                old,
                new,
                &[TextChange {
                    old_range: range(at, at),
                    replacement: Arc::from(replacement)
                }]
            ),
            ReparseOutcome::Incremental { .. }
        ));
    }
}

#[test]
fn repeated_unclosed_frontmatter_edits_do_not_accumulate_eof_diagnostics() {
    let mut previous_source = "---\ntype: x\n".to_owned();
    let mut previous = parse_okf_markdown(text(&previous_source), MarkdownDialect::WAML_DEFAULT)
        .unwrap()
        .tree;
    for replacement in ["y", "z"] {
        let next = format!("---\ntype: {replacement}\n");
        let change = TextChange {
            old_range: range(10, 11),
            replacement: Arc::from(replacement),
        };
        let outcome = reparse_okf_markdown(&previous, text(&next), &[change]).unwrap();
        let tree = match outcome {
            ReparseOutcome::Full {
                tree,
                reason: FullReparseReason::UnsafeSynchronization,
            } => tree,
            ReparseOutcome::Full { reason, .. } => {
                panic!("boundary diagnostic must use unsafe synchronization, got {reason:?}")
            }
            ReparseOutcome::Incremental { .. } => {
                panic!("boundary diagnostic must conservatively fall back")
            }
        };
        let full = parse_okf_markdown(text(&next), MarkdownDialect::WAML_DEFAULT).unwrap();
        assert_eq!(
            diagnostic_fingerprint(&tree),
            diagnostic_fingerprint(&full.tree)
        );
        assert_eq!(
            tree.diagnostics()
                .iter()
                .filter(|diagnostic| {
                    diagnostic.code == OkfSyntaxDiagnosticCode::MissingFrontmatterFence
                })
                .count(),
            1
        );
        previous_source = next;
        previous = tree;
    }
    assert_eq!(previous.write_to_string(), previous_source);
}

#[test]
fn unclosed_frontmatter_heading_edit_at_boundary_falls_back() {
    let previous = parse_okf_markdown(
        text("---\ntype: x\n# Before\n"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let ReparseOutcome::Full { tree, reason } = reparse_okf_markdown(
        &previous.tree,
        text("---\ntype: x\n# After!\n"),
        &[TextChange {
            old_range: range(14, 20),
            replacement: Arc::from("After!"),
        }],
    )
    .unwrap() else {
        panic!("unclosed frontmatter boundary diagnostic must conservatively fall back")
    };
    assert_eq!(reason, FullReparseReason::UnsafeSynchronization);
    let full = parse_okf_markdown(
        text("---\ntype: x\n# After!\n"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(
        diagnostic_fingerprint(&tree),
        diagnostic_fingerprint(&full.tree)
    );
    assert_eq!(
        tree.diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == OkfSyntaxDiagnosticCode::MissingFrontmatterFence
            })
            .count(),
        1
    );
}

#[test]
fn annotation_transfer_reuses_unchanged_source_independent_greens() {
    let old = text("# One\nbody\n");
    let parsed = parse_okf_markdown(old.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
    let annotated_region = first_node(&parsed.tree, OkfMarkdownSyntaxKind::Paragraph);
    let annotated = annotate_occurrence(
        &parsed.tree,
        &annotated_region.locator(),
        SyntaxAnnotation::new(NonZeroU64::new(11).unwrap(), "retained", None),
    )
    .unwrap();
    let previous = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::WAML_DEFAULT,
    );
    let ReparseOutcome::Incremental {
        tree,
        shared_source_independent_green,
        ..
    } = reparse_okf_markdown(
        &previous,
        text("# Two\nbody\n"),
        &[TextChange {
            old_range: range(2, 5),
            replacement: Arc::from("Two"),
        }],
    )
    .unwrap()
    else {
        panic!("heading edit must be incremental")
    };
    let previous_eof = first_token(&previous, OkfMarkdownSyntaxKind::EndOfFileToken);
    let final_eof = first_token(&tree, OkfMarkdownSyntaxKind::EndOfFileToken);
    assert!(previous_eof.same_green(&final_eof));
    assert_eq!(shared_source_independent_green, 1);
    assert!(!first_token(&previous, OkfMarkdownSyntaxKind::TextToken)
        .same_green(&first_token(&tree, OkfMarkdownSyntaxKind::TextToken)));
    assert!(!first_node(&previous, OkfMarkdownSyntaxKind::Paragraph)
        .same_green(&first_node(&tree, OkfMarkdownSyntaxKind::Paragraph)));
    assert_eq!(
        first_node(&tree, OkfMarkdownSyntaxKind::Paragraph)
            .syntax_annotations()
            .iter()
            .filter(|annotation| annotation.id().get() == 11)
            .count(),
        1
    );
}

#[test]
fn same_length_heading_text_edit_is_incremental_and_reuses_eof() {
    let old_source = text("# Before\nraw text\n");
    let previous = parse_okf_markdown(old_source, MarkdownDialect::WAML_DEFAULT).unwrap();
    let next_source = text("# After!\nraw text\n");
    let change = TextChange {
        old_range: range(2, 8),
        replacement: Arc::from("After!"),
    };
    let full = parse_okf_markdown(next_source.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
    let (outcome, structure) = reparse_okf_markdown_with_structure(
        &previous.tree,
        next_source.clone(),
        &[change],
        None,
        None,
    )
    .unwrap();
    let ReparseOutcome::Incremental { tree, .. } = outcome else {
        panic!("same-length heading text edit must be incremental")
    };

    assert_eq!(structure.headings.len(), 1);
    assert_eq!(
        structure.headings[0].level,
        previous.structure.headings[0].level
    );
    assert_eq!(
        structure.headings[0].range,
        previous.structure.headings[0].range
    );
    assert_eq!(
        structure.headings[0].text_range,
        previous.structure.headings[0].text_range
    );
    assert_eq!(
        structural_fingerprint(&tree),
        structural_fingerprint(&full.tree)
    );
    assert_eq!(
        diagnostic_fingerprint(&tree),
        diagnostic_fingerprint(&full.tree)
    );
    assert!(all_source_slices_use(
        &GreenElement::Node(tree.root_green().clone()),
        &next_source
    ));
    assert!(
        first_token(&previous.tree, OkfMarkdownSyntaxKind::EndOfFileToken)
            .same_green(&first_token(&tree, OkfMarkdownSyntaxKind::EndOfFileToken))
    );
    assert!(
        !first_node(&previous.tree, OkfMarkdownSyntaxKind::AtxHeading)
            .same_green(&first_node(&tree, OkfMarkdownSyntaxKind::AtxHeading))
    );
    assert!(!previous.tree.root().same_green(&tree.root()));
}

fn exact_oracle(
    previous: &str,
    next: &str,
    changes: &[TextChange],
) -> ReparseOutcome<crate::OkfMarkdownLanguage> {
    let old = parse_okf_markdown(text(previous), MarkdownDialect::WAML_DEFAULT).unwrap();
    let clean_source = text(next);
    let full = parse_okf_markdown(clean_source.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
    let outcome = reparse_okf_markdown(&old.tree, clean_source.clone(), changes).unwrap();
    let tree = match &outcome {
        ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
    };
    assert_eq!(tree.write_to_string(), full.tree.write_to_string());
    assert_eq!(
        structural_fingerprint(tree),
        structural_fingerprint(&full.tree)
    );
    assert_eq!(
        diagnostic_fingerprint(tree),
        diagnostic_fingerprint(&full.tree)
    );
    assert!(all_source_slices_use(
        &GreenElement::Node(tree.root_green().clone()),
        &clean_source
    ));
    outcome
}

#[test]
fn ambiguous_zero_width_boundary_falls_back() {
    assert!(matches!(
        exact_oracle(
            "# A\n# B\n",
            "# A\n\n# B\n",
            &[TextChange {
                old_range: range(4, 4),
                replacement: Arc::from("\n")
            }]
        ),
        ReparseOutcome::Full {
            reason: FullReparseReason::UnsafeSynchronization,
            ..
        }
    ));
}

#[test]
fn covering_container_boundaries_are_compared_globally() {
    assert!(matches!(
        exact_oracle(
            "> a\n> b\n",
            "> a\n> bee\n",
            &[TextChange {
                old_range: range(6, 7),
                replacement: Arc::from("bee")
            }]
        ),
        ReparseOutcome::Incremental { .. }
    ));
}

#[test]
fn added_or_removed_container_is_named() {
    assert!(matches!(
        exact_oracle(
            "body\n",
            "> body\n",
            &[TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("> ")
            }]
        ),
        ReparseOutcome::Full {
            reason: FullReparseReason::MarkdownContainerBoundaryChanged,
            ..
        }
    ));
}

#[test]
fn deleting_invalidated_container_range_falls_back_without_panicking() {
    let previous = "- first\n- second\n";
    let next = "- firstcond\n";
    let outcome = exact_oracle(
        previous,
        next,
        &[TextChange {
            old_range: range(7, 12),
            replacement: Arc::from(""),
        }],
    );

    assert!(matches!(
        outcome,
        ReparseOutcome::Full {
            reason: FullReparseReason::MarkdownContainerBoundaryChanged,
            ..
        }
    ));
}

#[test]
fn property_sequence_is_incremental_when_selected_window_is_consumed() {
    let initial = "- type: uml.Class\n  name: Example\n=";
    let replacement =
        "\u{1d456}0\u{ab09}  \u{ae}a  \u{1cf00}\u{a1}a \u{1f860}\u{ad0}0 \u{fb40}\u{c0e}A 0";
    let first = format!("{replacement}xample\n=");
    let second = &first[4..];

    let first_tree = match exact_oracle(
        initial,
        &first,
        &[TextChange {
            old_range: range(0, 27),
            replacement: Arc::from(replacement),
        }],
    ) {
        ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
    };
    let full = parse_okf_markdown(text(second), MarkdownDialect::WAML_DEFAULT).unwrap();
    let outcome = reparse_okf_markdown(
        &first_tree,
        text(second),
        &[TextChange {
            old_range: range(0, 4),
            replacement: Arc::from(""),
        }],
    )
    .unwrap();
    let ReparseOutcome::Incremental { tree, .. } = outcome else {
        panic!("Task 3 block parsing must consume the selected window")
    };
    assert_eq!(tree.write_to_string(), full.tree.write_to_string());
    assert_eq!(
        structural_fingerprint(&tree),
        structural_fingerprint(&full.tree)
    );
    assert_eq!(
        diagnostic_fingerprint(&tree),
        diagnostic_fingerprint(&full.tree)
    );
}

#[test]
fn safe_edit_matrix_matches_full_oracle() {
    for (case, old, new, changes) in [
        (
            "replace body middle",
            "body\n",
            "bXdy\n",
            vec![TextChange {
                old_range: range(1, 3),
                replacement: Arc::from("Xd"),
            }],
        ),
        (
            "insert punctuation",
            "body\n",
            "body!\n",
            vec![TextChange {
                old_range: range(4, 4),
                replacement: Arc::from("!"),
            }],
        ),
        (
            "frontmatter type",
            "---\ntype: uml.Class\n---\n",
            "---\ntype: uml.Interface\n---\n",
            vec![TextChange {
                old_range: range(10, 19),
                replacement: Arc::from("uml.Interface"),
            }],
        ),
        (
            "bracket value",
            "a: [b, c]\n",
            "a: [b, d]\n",
            vec![TextChange {
                old_range: range(7, 8),
                replacement: Arc::from("d"),
            }],
        ),
        (
            "two lines",
            "one\ntwo\n",
            "ONE\nTWO\n",
            vec![
                TextChange {
                    old_range: range(0, 3),
                    replacement: Arc::from("ONE"),
                },
                TextChange {
                    old_range: range(4, 7),
                    replacement: Arc::from("TWO"),
                },
            ],
        ),
    ] {
        let outcome = exact_oracle(old, new, &changes);
        let actual = match &outcome {
            ReparseOutcome::Incremental { .. } => "Incremental".to_owned(),
            ReparseOutcome::Full { reason, .. } => format!("Full({reason:?})"),
        };
        assert!(
            matches!(outcome, ReparseOutcome::Incremental { .. }),
            "{case}: {actual}"
        );
    }
}

#[test]
fn resolved_reference_label_edit_falls_back() {
    let outcome = exact_oracle(
        "[n][id]\n\n[id]: /one\n",
        "[n][ix]\n\n[id]: /one\n",
        &[TextChange {
            old_range: range(4, 6),
            replacement: Arc::from("ix"),
        }],
    );

    assert!(matches!(
        outcome,
        ReparseOutcome::Full {
            reason: FullReparseReason::UnsafeSynchronization,
            ..
        }
    ));
}

#[test]
fn boundary_fallback_matrix_is_named() {
    let non_utf8 = ChangeMap::checked(
        &text("# Café\n"),
        &[TextChange {
            old_range: range(5, 6),
            replacement: Arc::from("x"),
        }],
    );
    assert_eq!(
        non_utf8.unwrap_err(),
        FullReparseReason::InvalidUtf8Boundary
    );
    for (old, new, changes, reason) in [
        (
            "---\na: b\n---\n",
            "---\na: b\n",
            vec![TextChange {
                old_range: range(9, 13),
                replacement: Arc::from(""),
            }],
            FullReparseReason::FrontmatterBoundaryChanged,
        ),
        (
            "# H\n",
            "## H\n",
            vec![TextChange {
                old_range: range(1, 1),
                replacement: Arc::from("#"),
            }],
            FullReparseReason::HeadingBoundaryChanged,
        ),
        (
            "body\n",
            "    body\n",
            vec![TextChange {
                old_range: range(0, 0),
                replacement: Arc::from("    "),
            }],
            FullReparseReason::MarkdownContainerBoundaryChanged,
        ),
        (
            "# One\n# Two\n",
            "# Uno\n# Dos\n",
            vec![
                TextChange {
                    old_range: range(2, 5),
                    replacement: Arc::from("Uno"),
                },
                TextChange {
                    old_range: range(8, 11),
                    replacement: Arc::from("Dos"),
                },
            ],
            FullReparseReason::UnsafeSynchronization,
        ),
    ] {
        let outcome = exact_oracle(old, new, &changes);
        let actual = match outcome {
            ReparseOutcome::Full { reason, .. } => format!("{reason:?}"),
            ReparseOutcome::Incremental { .. } => "Incremental".into(),
        };
        assert_eq!(
            actual,
            format!("{reason:?}"),
            "expected {reason:?}, got {actual}"
        );
    }
}

fn first_node(
    tree: &SyntaxTree<crate::OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
) -> crate::SyntaxNode<crate::OkfMarkdownLanguage> {
    fn find(
        node: crate::SyntaxNode<crate::OkfMarkdownLanguage>,
        kind: OkfMarkdownSyntaxKind,
    ) -> Option<crate::SyntaxNode<crate::OkfMarkdownLanguage>> {
        if node.kind() == kind {
            return Some(node);
        }
        node.children()
            .find_map(|child| child.into_node().and_then(|node| find(node, kind)))
    }
    find(tree.root(), kind).unwrap()
}

fn first_token(
    tree: &SyntaxTree<crate::OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
) -> crate::SyntaxToken<crate::OkfMarkdownLanguage> {
    fn find(
        node: crate::SyntaxNode<crate::OkfMarkdownLanguage>,
        kind: OkfMarkdownSyntaxKind,
    ) -> Option<crate::SyntaxToken<crate::OkfMarkdownLanguage>> {
        node.children().find_map(|child| match child {
            SyntaxElement::Token(token) if token.kind() == kind => Some(token),
            SyntaxElement::Node(node) => find(node, kind),
            _ => None,
        })
    }
    find(tree.root(), kind).unwrap()
}

fn all_source_slices_use(
    element: &GreenElement<crate::OkfMarkdownLanguage>,
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
    let previous = parse_okf_markdown(old.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
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
            MarkdownDialect::WAML_DEFAULT,
        ),
        GreenElement::Token(_) => panic!("root is a node"),
    };
    assert!(
        !first_node(&previous.tree, OkfMarkdownSyntaxKind::AtxHeading)
            .same_green(&first_node(&candidate, OkfMarkdownSyntaxKind::AtxHeading))
    );
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
    let parsed = parse_okf_markdown(old.clone(), MarkdownDialect::WAML_DEFAULT).unwrap();
    let heading = first_node(&parsed.tree, OkfMarkdownSyntaxKind::AtxHeading);
    let heading_text = first_token(&parsed.tree, OkfMarkdownSyntaxKind::TextToken);
    let node_locator = heading.locator();
    let token_locator = heading_text.locator();
    let node_annotation = SyntaxAnnotation::new(NonZeroU64::new(1).unwrap(), "node", None);
    let token_annotation = SyntaxAnnotation::new(NonZeroU64::new(2).unwrap(), "token", None);
    let annotated =
        annotate_occurrence(&parsed.tree, &node_locator, node_annotation.clone()).unwrap();
    let annotated_tree = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::WAML_DEFAULT,
    );
    let annotated_token = first_token(&annotated_tree, OkfMarkdownSyntaxKind::TextToken);
    let annotated = annotate_occurrence(
        &annotated_tree,
        &annotated_token.locator(),
        token_annotation.clone(),
    )
    .unwrap();
    let previous = SyntaxTree::new(
        annotated,
        Arc::from(parsed.tree.diagnostics()),
        MarkdownDialect::WAML_DEFAULT,
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
    let candidate = parse_okf_markdown(new, MarkdownDialect::WAML_DEFAULT).unwrap();
    let candidate_diagnostics: Arc<[_]> = Arc::from(candidate.tree.diagnostics());
    let candidate_heading = first_node(&candidate.tree, OkfMarkdownSyntaxKind::AtxHeading);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate.tree,
            &candidate_heading.locator(),
            node_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::WAML_DEFAULT,
    );
    let candidate_heading = first_node(&candidate, OkfMarkdownSyntaxKind::AtxHeading);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_heading.locator(),
            node_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::WAML_DEFAULT,
    );
    let candidate_token = first_token(&candidate, OkfMarkdownSyntaxKind::TextToken);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_token.locator(),
            token_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::WAML_DEFAULT,
    );
    let candidate_token = first_token(&candidate, OkfMarkdownSyntaxKind::TextToken);
    let candidate = SyntaxTree::new(
        annotate_occurrence(
            &candidate,
            &candidate_token.locator(),
            token_annotation.clone(),
        )
        .unwrap(),
        candidate_diagnostics.clone(),
        MarkdownDialect::WAML_DEFAULT,
    );
    let transferred = SyntaxTree::new(
        transfer_mapped_annotations(&previous, &candidate, &map),
        candidate_diagnostics,
        MarkdownDialect::WAML_DEFAULT,
    );
    let mapped_heading = first_node(&transferred, OkfMarkdownSyntaxKind::AtxHeading);
    let mapped_token = first_token(&transferred, OkfMarkdownSyntaxKind::TextToken);
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
            &parse_okf_markdown(source, MarkdownDialect::WAML_DEFAULT)
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
    let previous = parse_okf_markdown(text("# One\n"), MarkdownDialect::WAML_DEFAULT).unwrap();
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
        crate::ParseError::StructuralInvariant { reason } => {
            assert_eq!(
                &*reason,
                "incremental changes do not reconstruct candidate source"
            );
        }
        other => panic!("unexpected mismatch error: {other:?}"),
    }
}
