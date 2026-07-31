use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxElement, SyntaxNode,
    SyntaxToken, TextRange,
};

fn leaf_tokens(
    node: &SyntaxNode<waml_syntax::OkfMarkdownLanguage>,
) -> Vec<SyntaxToken<waml_syntax::OkfMarkdownLanguage>> {
    let mut tokens = Vec::new();
    for child in node.children() {
        match child {
            SyntaxElement::Node(node) => tokens.extend(leaf_tokens(&node)),
            SyntaxElement::Token(token) => tokens.push(token),
        }
    }
    tokens
}

fn assert_range(source: &str, range: TextRange) {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    assert!(start <= end && end <= source.len(), "range {range:?} is in source");
    assert!(source.is_char_boundary(start));
    assert!(source.is_char_boundary(end));
}

fn assert_recovery_matrix_case(name: &str, source: &str, expects_recovery: bool) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap_or_else(|error| panic!("{name} parses: {error:?}"));
    let tree = snapshot.tree();
    assert_eq!(tree.write_to_string(), source, "{name} retains exact source");

    let tokens = leaf_tokens(&tree.root());
    for token in &tokens {
        assert_range(source, token.range());
    }
    for byte in 0..source.len() {
        let owners = tokens
            .iter()
            .filter(|token| {
                let range = token.range();
                range.start().to_usize() <= byte && byte < range.end().to_usize()
            })
            .count();
        assert_eq!(owners, 1, "{name}: byte {byte} has {owners} token owners");
    }

    for diagnostic in snapshot.diagnostics().iter() {
        assert_range(source, diagnostic.range);
        assert!(
            snapshot.queries().has_recovery(diagnostic.range),
            "{name}: recovery is query-visible"
        );
    }
    assert_eq!(
        !snapshot.diagnostics().is_empty(),
        expects_recovery,
        "{name}: malformed construct has an explicit recovery diagnostic"
    );
}

#[test]
fn malformed_markdown_retains_all_source_and_exposes_recovery() {
    let cases = [
        ("bom-crlf-tabs-unicode", "\u{feff}# café\r\n\ttext e\u{301}\r\n", false),
        ("mixed-line-endings", "# one\r\nparagraph\nnext\r", false),
        ("unclosed-fence", "```rust\nfn main() {}\n", true),
        ("unclosed-link", "[label](destination\n", false),
        ("unclosed-emphasis", "before *emphasis\n", false),
        ("malformed-table", "| a | b |\n| --- | nope |\n| one |\n", false),
        ("raw-html", "<script>\n# not a heading\n", true),
        ("unclosed-frontmatter", "---\ntitle: broken\n# heading\n", true),
        ("waml-heading-in-fence", "```waml\n# not markdown\n", true),
        ("waml-heading-in-html", "<div>\n# not markdown\n</div>\n", false),
    ];

    for (name, source, expects_recovery) in cases {
        assert_recovery_matrix_case(name, source, expects_recovery);
    }
}
