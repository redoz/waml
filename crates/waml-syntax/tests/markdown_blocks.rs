use std::sync::Arc;

use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind,
    OkfSyntaxDiagnosticCode, ShellParse, SourceText, SyntaxElement, SyntaxNode,
};

fn parse(source: &str) -> waml_syntax::ShellParse {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    ShellParse {
        tree: snapshot.tree().clone(),
        structure: snapshot.structure().clone(),
    }
}

fn node_kinds(tree: &waml_syntax::SyntaxTree<OkfMarkdownLanguage>) -> Vec<OkfMarkdownSyntaxKind> {
    fn visit(node: &SyntaxNode<OkfMarkdownLanguage>, out: &mut Vec<OkfMarkdownSyntaxKind>) {
        out.push(node.kind());
        for child in node.children() {
            if let SyntaxElement::Node(child) = child {
                visit(&child, out);
            }
        }
    }

    let mut kinds = Vec::new();
    visit(&tree.root(), &mut kinds);
    kinds
}

#[test]
fn block_phase_preserves_commonmark_shapes_and_markers() {
    let source = "\u{feff}> quote\n\n1. ordered\n\n- bullet\n\n# atx #\n\nsetext\n======\n\n---\n\nparagraph\n\n    indented\n\n```rust\ncode\n```\n\n<!-- html -->\n\n[label]: <destination> \"title\"\n";
    let shell = parse(source);
    let kinds = node_kinds(&shell.tree);

    assert_eq!(shell.tree.write_to_string(), source);
    for kind in [
        OkfMarkdownSyntaxKind::BlockQuote,
        OkfMarkdownSyntaxKind::List,
        OkfMarkdownSyntaxKind::ListItem,
        OkfMarkdownSyntaxKind::AtxHeading,
        OkfMarkdownSyntaxKind::SetextHeading,
        OkfMarkdownSyntaxKind::ThematicBreak,
        OkfMarkdownSyntaxKind::Paragraph,
        OkfMarkdownSyntaxKind::IndentedCodeBlock,
        OkfMarkdownSyntaxKind::FencedCodeBlock,
        OkfMarkdownSyntaxKind::HtmlBlock,
        OkfMarkdownSyntaxKind::LinkReferenceDefinition,
    ] {
        assert!(kinds.contains(&kind), "missing {kind:?}");
    }

    let spellings = token_spellings(&shell.tree.root());
    for marker in [
        "> ", "1.", "-", "#", "======", "---", "    ", "```", "rust", "[", "]", ":", "<", ">", "\"",
    ] {
        assert!(
            spellings.iter().any(|text| text == marker),
            "missing marker {marker:?}: {spellings:?}"
        );
    }
    let top_level: Vec<_> = shell
        .tree
        .root()
        .children()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node.kind()),
            SyntaxElement::Token(_) => None,
        })
        .collect();
    assert_eq!(
        top_level,
        [
            OkfMarkdownSyntaxKind::BlockQuote,
            OkfMarkdownSyntaxKind::List,
            OkfMarkdownSyntaxKind::List,
            OkfMarkdownSyntaxKind::AtxHeading,
            OkfMarkdownSyntaxKind::SetextHeading,
            OkfMarkdownSyntaxKind::ThematicBreak,
            OkfMarkdownSyntaxKind::Paragraph,
            OkfMarkdownSyntaxKind::IndentedCodeBlock,
            OkfMarkdownSyntaxKind::FencedCodeBlock,
            OkfMarkdownSyntaxKind::HtmlBlock,
            OkfMarkdownSyntaxKind::LinkReferenceDefinition,
        ]
    );
    let heading = shell
        .tree
        .root()
        .children()
        .find_map(|child| match child {
            SyntaxElement::Node(node) if node.kind() == OkfMarkdownSyntaxKind::AtxHeading => {
                Some(node)
            }
            _ => None,
        })
        .unwrap();
    let heading_tokens = direct_tokens(&heading);
    assert_eq!(
        heading_tokens
            .iter()
            .map(|token| (token.kind(), token.text().write_to_string()))
            .collect::<Vec<_>>(),
        [
            (OkfMarkdownSyntaxKind::HeadingMarkerToken, "#".into()),
            (OkfMarkdownSyntaxKind::WhitespaceToken, " ".into()),
            (OkfMarkdownSyntaxKind::TextToken, "atx ".into()),
            (OkfMarkdownSyntaxKind::HeadingMarkerToken, "#".into()),
            (OkfMarkdownSyntaxKind::NewlineToken, "\n".into()),
        ]
    );
    assert_eq!(
        heading.range().start().to_usize(),
        source.find("# atx").unwrap()
    );
    assert_eq!(
        heading.range().end().to_usize(),
        source.find("# atx").unwrap() + "# atx #\n".len()
    );
}

#[test]
fn event_boundaries_reject_invalid_fence_closers() {
    let source = "```rust\ncode\n``` trailing\n# still code\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    let kinds = node_kinds(&shell.tree);
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == OkfMarkdownSyntaxKind::FencedCodeBlock)
            .count(),
        1
    );
    assert!(!kinds.contains(&OkfMarkdownSyntaxKind::AtxHeading));
    assert!(shell
        .tree
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == OkfSyntaxDiagnosticCode::UnclosedFence));
}

#[test]
fn event_boundaries_keep_html_blocks_whole() {
    let source = "<section>\n# hidden\n</section>\n\n# visible\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    let top_level: Vec<_> = shell
        .tree
        .root()
        .children()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some((node.kind(), node.range())),
            SyntaxElement::Token(_) => None,
        })
        .collect();
    assert_eq!(top_level[0].0, OkfMarkdownSyntaxKind::HtmlBlock);
    assert_eq!(
        &source[top_level[0].1.start().to_usize()..top_level[0].1.end().to_usize()],
        "<section>\n# hidden\n</section>\n"
    );
    assert_eq!(top_level[1].0, OkfMarkdownSyntaxKind::AtxHeading);
}

#[test]
fn quoted_list_item_stays_within_event_confirmed_containers() {
    let source = "> - quoted\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    let nodes = descendant_nodes(&shell.tree.root());
    assert_eq!(
        nodes
            .iter()
            .map(|node| (
                node.kind(),
                node.range().start().to_usize(),
                node.range().end().to_usize()
            ))
            .collect::<Vec<_>>(),
        [
            (OkfMarkdownSyntaxKind::BlockQuote, 0, 11),
            (OkfMarkdownSyntaxKind::List, 2, 11),
            (OkfMarkdownSyntaxKind::ListItem, 2, 11),
        ]
    );
}

#[test]
fn nested_list_item_stays_within_its_event_confirmed_list() {
    let source = "- outer\n  - inner\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    let nodes = descendant_nodes(&shell.tree.root());
    assert_eq!(
        nodes
            .iter()
            .map(|node| (
                node.kind(),
                node.range().start().to_usize(),
                node.range().end().to_usize()
            ))
            .collect::<Vec<_>>(),
        [
            (OkfMarkdownSyntaxKind::List, 0, 18),
            (OkfMarkdownSyntaxKind::ListItem, 0, 18),
            (OkfMarkdownSyntaxKind::List, 10, 18),
            (OkfMarkdownSyntaxKind::ListItem, 10, 18),
        ]
    );
    let markers: Vec<_> = leaf_tokens(&shell.tree.root())
        .into_iter()
        .filter(|token| token.kind() == OkfMarkdownSyntaxKind::ListMarkerToken)
        .map(|token| {
            (
                token.text().write_to_string(),
                token.range().start().to_usize(),
                token.range().end().to_usize(),
            )
        })
        .collect();
    assert_eq!(markers, [("-".into(), 0, 1), ("-".into(), 10, 11)]);
}

#[test]
fn dialect_does_not_enable_unrequested_extensions() {
    let tree = parse("[^x]\n\n[^x]: note\n\nterm\n: definition\n\n$math$")
        .tree
        .clone();
    let kinds = node_kinds(&tree);
    assert!(!kinds
        .iter()
        .any(|kind| format!("{kind:?}").contains("Footnote")));
    assert!(!kinds
        .iter()
        .any(|kind| format!("{kind:?}").contains("DefinitionList")));
    assert!(!kinds
        .iter()
        .any(|kind| format!("{kind:?}").contains("Math")));
    let top_level: Vec<_> = tree
        .root()
        .children()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node.kind()),
            SyntaxElement::Token(_) => None,
        })
        .collect();
    assert_eq!(
        top_level,
        [
            OkfMarkdownSyntaxKind::Paragraph,
            OkfMarkdownSyntaxKind::LinkReferenceDefinition,
            OkfMarkdownSyntaxKind::Paragraph,
            OkfMarkdownSyntaxKind::Paragraph,
        ]
    );
    assert_eq!(
        tree.write_to_string(),
        "[^x]\n\n[^x]: note\n\nterm\n: definition\n\n$math$"
    );
}

fn token_spellings(node: &SyntaxNode<OkfMarkdownLanguage>) -> Vec<String> {
    let mut spellings = Vec::new();
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => spellings.extend(token_spellings(&child)),
            SyntaxElement::Token(token) => spellings.push(token.text().write_to_string()),
        }
    }
    spellings
}

fn direct_tokens(
    node: &SyntaxNode<OkfMarkdownLanguage>,
) -> Vec<waml_syntax::SyntaxToken<OkfMarkdownLanguage>> {
    node.children()
        .filter_map(|child| match child {
            SyntaxElement::Token(token) => Some(token),
            SyntaxElement::Node(_) => None,
        })
        .collect()
}

fn leaf_tokens(
    node: &SyntaxNode<OkfMarkdownLanguage>,
) -> Vec<waml_syntax::SyntaxToken<OkfMarkdownLanguage>> {
    let mut tokens = Vec::new();
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => tokens.extend(leaf_tokens(&child)),
            SyntaxElement::Token(token) => tokens.push(token),
        }
    }
    tokens
}

fn descendant_nodes(
    node: &SyntaxNode<OkfMarkdownLanguage>,
) -> Vec<SyntaxNode<OkfMarkdownLanguage>> {
    fn visit(
        node: &SyntaxNode<OkfMarkdownLanguage>,
        out: &mut Vec<SyntaxNode<OkfMarkdownLanguage>>,
    ) {
        for child in node.children() {
            if let SyntaxElement::Node(child) = child {
                out.push(child.clone());
                visit(&child, out);
            }
        }
    }
    let mut nodes = Vec::new();
    visit(node, &mut nodes);
    nodes
}
