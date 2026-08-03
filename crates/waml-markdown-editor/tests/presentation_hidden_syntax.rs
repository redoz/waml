//! Read-only presentation hides markdown syntax without changing the source.
//!
//! Hiding is a presentation concern only: every source byte still belongs to
//! exactly one text run, so selection, copy, and caret mapping are unaffected.
//! Markers that carry meaning a reader needs (a list bullet) are replaced by a
//! block decoration anchored to the marker's own source range, never by
//! glyphs that belong to no source.

use std::sync::Arc;

use waml_markdown_editor::presentation::{
    compile_presentation, BlockDecorationKind, HighlighterRegistry, PresentationItem,
    PresentationPlan, PresentationStyles, TextRole,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSnapshot, SourceText,
};

fn compile(
    source: &str,
    styles: &PresentationStyles,
) -> (Arc<MarkdownSyntaxSnapshot>, Arc<PresentationPlan>) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source.to_owned()).expect("the source is valid"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the source parses");
    let plan = compile_presentation(&snapshot, styles, &HighlighterRegistry::default())
        .expect("the source compiles");
    (snapshot, plan)
}

/// `(slice, role, hidden)` for every text run, in source order.
fn runs(
    plan: &PresentationPlan,
    snapshot: &MarkdownSyntaxSnapshot,
) -> Vec<(String, TextRole, bool)> {
    plan.items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun {
                range,
                role,
                hidden,
                ..
            } => Some((
                snapshot.text().slice(*range).unwrap_or_default().to_owned(),
                *role,
                *hidden,
            )),
            _ => None,
        })
        .collect()
}

fn decorations(plan: &PresentationPlan) -> Vec<BlockDecorationKind> {
    plan.items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::BlockDecoration { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect()
}

#[test]
fn the_default_style_hides_nothing() {
    let (snapshot, plan) = compile("# Title\n", &PresentationStyles::balanced());

    assert!(
        runs(&plan, &snapshot).iter().all(|(_, _, hidden)| !hidden),
        "the editing style keeps every marker visible"
    );
}

#[test]
fn hidden_syntax_hides_the_heading_marker_but_not_its_text() {
    let (snapshot, plan) = compile("# Title\n", &PresentationStyles::hiding_syntax());

    let hidden = runs(&plan, &snapshot)
        .into_iter()
        .filter(|(_, _, hidden)| *hidden)
        .map(|(slice, role, _)| (slice, role))
        .collect::<Vec<_>>();

    assert_eq!(hidden, vec![("#".to_owned(), TextRole::HeadingMarker(1))]);
}

#[test]
fn hidden_syntax_hides_emphasis_delimiters_but_not_the_emphasized_text() {
    let (snapshot, plan) = compile("a **b** c\n", &PresentationStyles::hiding_syntax());

    let hidden = runs(&plan, &snapshot)
        .into_iter()
        .filter(|(_, _, hidden)| *hidden)
        .map(|(slice, _, _)| slice)
        .collect::<Vec<_>>();

    assert_eq!(hidden, vec!["**".to_owned(), "**".to_owned()]);
}

#[test]
fn hidden_syntax_keeps_the_complete_source_partition() {
    let source = "# Title\n\n- alpha\n- [ ] task\n\n> quote\n\n```rust\nlet x = 1;\n```\n";
    let (_, plan) = compile(source, &PresentationStyles::hiding_syntax());

    assert_eq!(
        plan.validate_source_partition(),
        Ok(()),
        "hiding never removes a run, so every source byte stays covered exactly once"
    );
}

#[test]
fn hidden_syntax_replaces_an_unordered_marker_with_a_bullet_decoration() {
    let (snapshot, plan) = compile("- alpha\n", &PresentationStyles::hiding_syntax());

    assert!(
        decorations(&plan).contains(&BlockDecorationKind::ListBullet { level: 0 }),
        "an unordered item draws a bullet in place of its hidden marker"
    );
    let hidden = runs(&plan, &snapshot)
        .into_iter()
        .filter(|(_, role, hidden)| *hidden && *role == TextRole::ListMarker)
        .count();
    assert_eq!(hidden, 1, "the literal `- ` marker is hidden");
}

#[test]
fn hidden_syntax_keeps_ordered_list_numbers_visible() {
    let (snapshot, plan) = compile("1. alpha\n", &PresentationStyles::hiding_syntax());

    let marker_hidden = runs(&plan, &snapshot)
        .into_iter()
        .any(|(_, role, hidden)| role == TextRole::ListMarker && hidden);

    assert!(
        !marker_hidden,
        "an ordered number is rendered content, not syntax, so it stays visible"
    );
    assert!(
        !decorations(&plan)
            .iter()
            .any(|kind| matches!(kind, BlockDecorationKind::ListBullet { .. })),
        "an ordered item keeps its number instead of gaining a bullet"
    );
}

#[test]
fn hidden_syntax_hides_a_link_destination_but_keeps_its_label() {
    let (snapshot, plan) = compile("[label](target)\n", &PresentationStyles::hiding_syntax());

    let visible = runs(&plan, &snapshot)
        .into_iter()
        .filter(|(_, _, hidden)| !hidden)
        .map(|(slice, _, _)| slice)
        .collect::<Vec<_>>()
        .join("");

    assert_eq!(
        visible.trim_end(),
        "label",
        "a reader sees the label; the destination and its punctuation are hidden"
    );
}

#[test]
fn hidden_syntax_hides_frontmatter_metadata_but_keeps_the_prose_below_it() {
    let source = "---
type: uml.Class
---
# Class
";
    let (snapshot, plan) = compile(source, &PresentationStyles::hiding_syntax());

    let visible = runs(&plan, &snapshot)
        .into_iter()
        .filter(|(_, _, hidden)| !hidden)
        .map(|(slice, _, _)| slice)
        .collect::<Vec<_>>()
        .join("");

    assert!(
        !visible.contains("type: uml.Class"),
        "frontmatter is document metadata, not prose a reader should see"
    );
    assert!(
        visible.contains("Class"),
        "the prose below the frontmatter stays visible"
    );
    assert_eq!(
        plan.validate_source_partition(),
        Ok(()),
        "hidden frontmatter still covers its source bytes"
    );
}
