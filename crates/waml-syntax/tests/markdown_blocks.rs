use std::sync::Arc;

use waml_syntax::{
    parse_okf_markdown, MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SourceText,
    SyntaxElement, SyntaxNode,
};

fn parse(source: &str) -> waml_syntax::ShellParse {
    parse_okf_markdown(
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap()
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
            "missing marker {marker:?}"
        );
    }
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
