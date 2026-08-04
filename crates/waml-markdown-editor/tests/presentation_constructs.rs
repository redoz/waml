use std::sync::Arc;

use waml_markdown_editor::presentation::{
    compile_presentation, render_plan_golden, BlockDecorationKind, EmbeddedBlockKind,
    HighlighterRegistry, PresentationBlockKind, PresentationItem, PresentationPlan,
    PresentationStyles, TextRole,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSnapshot, SourceText,
};

fn compile_fixture(name: &str) -> (Arc<MarkdownSyntaxSnapshot>, Arc<PresentationPlan>) {
    let text = std::fs::read_to_string(format!("tests/fixtures/{name}"))
        .unwrap_or_else(|error| panic!("fixture {name} is readable: {error}"));
    let source = SourceText::new(text).expect("the fixture is valid source");
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the fixture parses");
    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("the fixture compiles");
    (snapshot, plan)
}

#[test]
fn real_waml_frontmatter_compiles_into_a_complete_source_partition() {
    let source = "---\ntype: uml.Class\n---\n# Class\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).expect("the WAML source is valid"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the WAML source parses");

    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("frontmatter participates in presentation coverage");

    assert_eq!(plan.validate_source_partition(), Ok(()));
}

/// Every text run of the source slice that matches `needle`.
fn roles_for(
    plan: &PresentationPlan,
    snapshot: &MarkdownSyntaxSnapshot,
    needle: &str,
) -> Vec<TextRole> {
    plan.items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun { range, role, .. } => {
                (snapshot.text().slice(*range).unwrap_or_default() == needle).then_some(*role)
            }
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
fn every_construct_compiles_to_its_exact_treatment() {
    let (snapshot, plan) = compile_fixture("presentation-all.md");
    assert_eq!(plan.validate_source_partition(), Ok(()));

    // Headings: dim marker at the heading's own size, level-tagged content.
    assert_eq!(
        roles_for(&plan, &snapshot, "#"),
        vec![TextRole::HeadingMarker(1)]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "######"),
        vec![TextRole::HeadingMarker(6)]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "Heading 6"),
        vec![TextRole::Heading(6)]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "Heading 2"),
        vec![TextRole::Heading(2)]
    );

    // Inline emphasis, strong, and their nesting.
    assert_eq!(roles_for(&plan, &snapshot, "em"), vec![TextRole::Emphasis]);
    assert_eq!(
        roles_for(&plan, &snapshot, "strong"),
        vec![TextRole::Strong, TextRole::StrongEmphasis]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "emphasis"),
        vec![TextRole::StrongEmphasis]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "**"),
        vec![TextRole::SyntaxMarker; 6]
    );

    // Strikethrough, link, inline code, raw HTML.
    assert_eq!(
        roles_for(&plan, &snapshot, "strike"),
        vec![TextRole::Strikethrough]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "label"),
        vec![TextRole::LinkLabel]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "./other.md#part"),
        vec![TextRole::LinkDestination]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "inline"),
        vec![TextRole::InlineCode]
    );
    assert_eq!(roles_for(&plan, &snapshot, "kbd"), vec![TextRole::RawHtml]);

    // List, task, and quote markers keep their visible marker roles.
    assert_eq!(
        roles_for(&plan, &snapshot, "1."),
        vec![TextRole::ListMarker]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "[x]"),
        vec![TextRole::TaskMarker]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "> "),
        vec![TextRole::QuoteMarker]
    );

    // Fenced code splits fence, info, and content.
    assert_eq!(
        roles_for(&plan, &snapshot, "```"),
        vec![TextRole::CodeFence; 2]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "waml"),
        vec![TextRole::CodeInfo]
    );
    assert_eq!(
        roles_for(&plan, &snapshot, "type: uml.class\n"),
        vec![TextRole::CodeContent]
    );

    // Table pipes are delimiters; the thematic break stays a marker.
    assert!(roles_for(&plan, &snapshot, "|")
        .iter()
        .all(|role| *role == TextRole::TableDelimiter));
    assert_eq!(
        roles_for(&plan, &snapshot, "---"),
        vec![TextRole::SyntaxMarker]
    );
}

#[test]
fn metadata_drives_decorations_links_and_embeds() {
    let (_, plan) = compile_fixture("presentation-all.md");
    let kinds = decorations(&plan);
    assert!(kinds.contains(&BlockDecorationKind::QuoteRule));
    assert!(kinds.contains(&BlockDecorationKind::InlineCodeFill));
    assert!(kinds.contains(&BlockDecorationKind::FencedCodeSurface));
    assert!(kinds.contains(&BlockDecorationKind::ThematicRule));
    assert!(kinds.contains(&BlockDecorationKind::TaskCheckbox { checked: true }));
    assert!(kinds.contains(&BlockDecorationKind::TaskCheckbox { checked: false }));
    assert!(kinds.contains(&BlockDecorationKind::TableHeaderFill));
    assert!(kinds.contains(&BlockDecorationKind::TableGrid { columns: 3 }));

    // One image embed, and the image is not offered as a navigable link.
    let embeds = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::EmbeddedBlock { kind, .. } => Some(kind.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        embeds,
        vec![EmbeddedBlockKind::Image {
            destination: Arc::from("checker.svg"),
            alt: Arc::from("checker"),
            title: Some(Arc::from("fixture")),
        }]
    );
    assert_eq!(plan.links.len(), 1);
    assert_eq!(plan.links[0].destination.as_ref(), "./other.md#part");
}

#[test]
fn malformed_source_stays_literal_text_without_guessed_constructs() {
    let (snapshot, plan) = compile_fixture("malformed.md");
    assert_eq!(plan.validate_source_partition(), Ok(()));

    // Raw HTML is presented, never executed and never turned into an item.
    let script_roles = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun { range, role, .. } => snapshot
                .text()
                .slice(*range)
                .unwrap_or_default()
                .contains("<script>")
                .then_some(*role),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(script_roles, vec![TextRole::RawHtml]);
    assert!(plan
        .items
        .iter()
        .all(|item| !matches!(item, PresentationItem::EmbeddedBlock { .. })));

    // No guessed link or image survives the incomplete destinations, and the
    // unmatched emphasis opener stays ordinary body text.
    assert!(plan.links.is_empty());
    assert_eq!(
        roles_for(&plan, &snapshot, "**strong"),
        vec![TextRole::Body]
    );
}

#[test]
fn plan_goldens_are_byte_stable() {
    for name in ["presentation-all", "malformed"] {
        let (snapshot, plan) = compile_fixture(&format!("{name}.md"));
        let rendered = render_plan_golden(&plan, snapshot.text());
        let golden = std::fs::read_to_string(format!("tests/goldens/{name}.plan"))
            .unwrap_or_else(|error| panic!("golden {name} is readable: {error}"))
            .replace("\r\n", "\n");
        assert_eq!(rendered, golden, "golden mismatch for {name}");
    }
}

#[test]
fn frontmatter_compiles_to_its_own_code_block() {
    let source = "---\ntype: uml.Class\n---\n\nBody paragraph.\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).expect("the WAML source is valid"),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("the WAML source parses");
    let plan = compile_presentation(
        &snapshot,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("the document compiles");

    let frontmatter_len = "---\ntype: uml.Class\n---\n".len();
    assert!(
        plan.blocks.iter().any(|block| {
            block.kind == PresentationBlockKind::Code
                && block.source_range.start().to_usize() == 0
                && block.source_range.end().to_usize() == frontmatter_len
        }),
        "frontmatter must be its own Code block, not folded into the root paragraph: {:?}",
        plan.blocks
    );
    assert!(
        !plan
            .blocks
            .iter()
            .any(|block| block.kind == PresentationBlockKind::Paragraph
                && block.source_range.start().to_usize() == 0
                && block.source_range.end().to_usize() >= frontmatter_len),
        "frontmatter must not be folded into the document-root paragraph"
    );
}

#[test]
fn frontmatter_role_is_not_a_syntax_marker() {
    assert!(!TextRole::Frontmatter.is_syntax_marker());
}
