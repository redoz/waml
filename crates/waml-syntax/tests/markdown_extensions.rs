use std::sync::Arc;

use waml_syntax::{
    parse_okf_markdown, MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SourceText,
    SyntaxElement, SyntaxNode, TextRange, TextSize, WamlSectionKind,
};

fn parse(source: &str) -> waml_syntax::ShellParse {
    parse_okf_markdown(
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap()
}

fn sections(node: &SyntaxNode<OkfMarkdownLanguage>) -> Vec<SyntaxNode<OkfMarkdownLanguage>> {
    let mut out = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(node) = child {
            if node.kind() == OkfMarkdownSyntaxKind::WamlSection {
                out.push(node.clone());
            }
            out.extend(sections(&node));
        }
    }
    out
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

#[test]
fn waml_extensions_keep_initial_frontmatter_and_mark_only_top_level_sections() {
    let source = "\u{feff}---\ntype: uml.Class\ntitle: Example\n---\n# Class\n## Attributes\n- field\n### Details\ntext\n## Values\nvalue\n\n---\n\n> ## Slots\n\n- ## Relationships\n\n```waml\n## Relationships\n```\n\n<div>\n## Members\n</div>\n\n## Notes\nnot a WAML section\n## Layout\nlayout\n## Nodes\nnodes\n## Lifelines\nlifelines\n## Messages\nmessages\n";
    let parsed = parse(source);

    assert_eq!(parsed.tree.write_to_string(), source);
    assert_eq!(
        sections(&parsed.tree.root())
            .iter()
            .map(|node| node.range())
            .collect::<Vec<_>>(),
        [
            source.find("## Attributes").unwrap()..source.find("## Values").unwrap(),
            source.find("## Values").unwrap()..source.find("## Notes").unwrap(),
            source.find("## Layout").unwrap()..source.find("## Nodes").unwrap(),
            source.find("## Nodes").unwrap()..source.find("## Lifelines").unwrap(),
            source.find("## Lifelines").unwrap()..source.find("## Messages").unwrap(),
            source.find("## Messages").unwrap()..source.len(),
        ]
        .into_iter()
        .map(|source_range| range(source_range.start, source_range.end))
        .collect::<Vec<_>>()
    );
    assert_eq!(
        parsed
            .structure
            .islands
            .iter()
            .map(|island| island.kind)
            .collect::<Vec<_>>(),
        [
            WamlSectionKind::Attributes,
            WamlSectionKind::Values,
            WamlSectionKind::Layout,
            WamlSectionKind::Nodes,
            WamlSectionKind::Lifelines,
            WamlSectionKind::Messages,
        ]
    );
    for island in parsed.structure.islands.iter() {
        assert!(sections(&parsed.tree.root()).iter().any(|section| {
            section
                .syntax_annotations()
                .iter()
                .find(|annotation| annotation.kind() == "waml.markdown.identity")
                .and_then(|annotation| annotation.data())
                .is_some_and(|value| value == island.owner.get().to_string())
        }));
    }
}

#[test]
fn waml_section_kinds_cover_every_recognized_name_and_end_at_lower_heading() {
    let source = "# Document\n## Attributes\na\n### nested\n## Values\nv\n## Slots\ns\n## Relationships\nr\n## Members\nm\n## Layout\nl\n## Nodes\nn\n## Lifelines\nlife\n## Messages\nmessage\n# Next\n";
    let parsed = parse(source);
    let islands = parsed.structure.islands.as_ref();

    assert_eq!(islands.len(), 9);
    assert_eq!(islands[0].kind, WamlSectionKind::Attributes);
    assert_eq!(islands[1].kind, WamlSectionKind::Values);
    assert_eq!(islands[2].kind, WamlSectionKind::Slots);
    assert_eq!(islands[3].kind, WamlSectionKind::Relationships);
    assert_eq!(islands[4].kind, WamlSectionKind::Members);
    assert_eq!(islands[5].kind, WamlSectionKind::Layout);
    assert_eq!(islands[6].kind, WamlSectionKind::Nodes);
    assert_eq!(islands[7].kind, WamlSectionKind::Lifelines);
    assert_eq!(islands[8].kind, WamlSectionKind::Messages);
    assert_eq!(
        islands[0].content_range,
        range(
            source.find("## Attributes").unwrap() + "## Attributes\n".len(),
            source.find("## Values").unwrap(),
        )
    );
    assert_eq!(
        islands[8].content_range,
        range(
            source.find("## Messages").unwrap() + "## Messages\n".len(),
            source.find("# Next").unwrap(),
        )
    );
}

#[test]
fn frontmatter_keeps_a_later_thematic_rule_in_commonmark() {
    let source = "\u{feff}---\ntype: uml.Class\n---\n# Class\n\n---\n\ntext\n";
    let parsed = parse(source);
    let root_nodes: Vec<_> = parsed
        .tree
        .root()
        .children()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node.kind()),
            SyntaxElement::Token(_) => None,
        })
        .collect();

    assert_eq!(parsed.tree.write_to_string(), source);
    assert_eq!(root_nodes[0], OkfMarkdownSyntaxKind::Frontmatter);
    assert!(root_nodes.contains(&OkfMarkdownSyntaxKind::ThematicBreak));
}

#[test]
fn unclosed_plausible_frontmatter_recovers_as_a_root_node() {
    let source = "\u{feff}---\ntype: uml.Class\ntitle: Example\n";
    let parsed = parse(source);

    assert_eq!(parsed.tree.write_to_string(), source);
    assert!(matches!(
        parsed.tree.root().children().next(),
        Some(SyntaxElement::Node(node)) if node.kind() == OkfMarkdownSyntaxKind::Frontmatter
    ));
    assert!(parsed.tree.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == waml_syntax::OkfSyntaxDiagnosticCode::MissingFrontmatterFence
    }));
}
