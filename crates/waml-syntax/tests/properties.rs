use std::sync::Arc;

use proptest::prelude::*;
use waml_syntax::{
    parse_okf_markdown, reparse_okf_markdown, write_green_to, GreenText, MarkdownDialect,
    OkfMarkdownLanguage, ReparseOutcome, SourceText, SyntaxElement, SyntaxTree, TextChange,
    TextRange, TextSize,
};

fn shell_source() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        ("(?s).{0,96}").prop_map(|body| format!("---\ntype: uml.Class\n---\n{body}")),
        (1usize..7, "(?s).{0,80}")
            .prop_map(|(level, body)| format!("{} Heading\n{body}", "#".repeat(level))),
        "(?s).{0,80}".prop_map(|body| format!("> quote\n- item\n```\n{body}\n```\n<!-- html -->")),
        "(?s).{0,80}".prop_map(|body| format!("- type: uml.Class\n  name: Example\n{body}")),
    ]
}

fn edit_sequence() -> impl Strategy<Value = Vec<(usize, usize, String)>> {
    prop::collection::vec((any::<usize>(), any::<usize>(), any::<String>()), 0..8)
}

fn source(value: &str) -> SourceText {
    SourceText::from_shared(Arc::new(value.to_owned())).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(end).unwrap(),
    )
    .unwrap()
}

fn element_range(element: &SyntaxElement<OkfMarkdownLanguage>) -> TextRange {
    match element {
        SyntaxElement::Node(node) => node.range(),
        SyntaxElement::Token(token) => token.range(),
    }
}

fn assert_text_boundaries(text: &GreenText, source: &str) {
    if let GreenText::SourceSlice { range, .. } = text {
        assert!(range.start().to_usize() <= range.end().to_usize());
        assert!(range.end().to_usize() <= source.len());
        assert!(source.is_char_boundary(range.start().to_usize()));
        assert!(source.is_char_boundary(range.end().to_usize()));
    }
}

fn assert_red_tree(tree: &SyntaxTree<OkfMarkdownLanguage>, source: &str) {
    let root = tree.root();
    assert_eq!(root.range(), range(0, source.len()));
    assert!(source.is_char_boundary(root.range().start().to_usize()));
    assert!(source.is_char_boundary(root.range().end().to_usize()));
    for diagnostic in tree.diagnostics() {
        let span = diagnostic.range;
        assert!(span.start().to_usize() <= source.len());
        assert!(span.end().to_usize() <= source.len());
        assert!(source.is_char_boundary(span.start().to_usize()));
        assert!(source.is_char_boundary(span.end().to_usize()));
    }

    let mut stack = vec![root.into()];
    let mut elements = Vec::new();
    while let Some(element) = stack.pop() {
        let span = element_range(&element);
        assert!(span.start().to_usize() <= span.end().to_usize());
        assert!(span.end().to_usize() <= source.len());
        assert!(source.is_char_boundary(span.start().to_usize()));
        assert!(source.is_char_boundary(span.end().to_usize()));
        match &element {
            SyntaxElement::Node(node) => {
                let children: Vec<_> = node.children().collect();
                let mut cursor = span.start();
                for child in &children {
                    assert_eq!(element_range(child).start(), cursor);
                    cursor = element_range(child).end();
                }
                assert_eq!(cursor, span.end());
                stack.extend(children.into_iter().rev());
            }
            SyntaxElement::Token(token) => {
                let mut cursor = String::new();
                for trivia in token.leading_trivia() {
                    assert_text_boundaries(&trivia.text, source);
                    cursor.push_str(&trivia.text.write_to_string());
                }
                assert_text_boundaries(token.text(), source);
                cursor.push_str(&token.text().write_to_string());
                for trivia in token.trailing_trivia() {
                    assert_text_boundaries(&trivia.text, source);
                    cursor.push_str(&trivia.text.write_to_string());
                }
                assert_eq!(cursor.len(), span.len().to_usize());
            }
        }
        elements.push(element);
    }
    let mut written = String::new();
    for element in &elements {
        if let SyntaxElement::Token(token) = element {
            for trivia in token.leading_trivia() {
                written.push_str(&trivia.text.write_to_string());
            }
            written.push_str(&token.text().write_to_string());
            for trivia in token.trailing_trivia() {
                written.push_str(&trivia.text.write_to_string());
            }
        }
        let locator = element.locator();
        let resolved = tree.resolve(&locator).unwrap();
        assert_eq!(resolved.kind(), element.kind());
        assert_eq!(element_range(&resolved), element_range(element));
        let mut parent = element.clone();
        let mut terminated = false;
        for _ in 0..=elements.len() {
            let next = match &parent {
                SyntaxElement::Node(node) => node.parent().map(Into::into),
                SyntaxElement::Token(token) => token.parent().map(Into::into),
            };
            match next {
                Some(next) => parent = next,
                None => {
                    terminated = true;
                    break;
                }
            }
        }
        assert!(
            terminated,
            "parent chain did not terminate within element count"
        );
    }
    assert_eq!(written, source);
    let mut recorder = String::new();
    write_green_to(tree.root_green(), &mut recorder).unwrap();
    assert_eq!(recorder, source);
    assert_eq!(tree.write_to_string(), source);
}

fn shell_fingerprint(tree: &SyntaxTree<OkfMarkdownLanguage>) -> Vec<String> {
    let mut stack = vec![tree.root().into()];
    let mut out = Vec::new();
    while let Some(element) = stack.pop() {
        match element {
            SyntaxElement::Node(node) => {
                out.push(format!("node:{:?}:{:?}", node.kind(), node.range()));
                let children: Vec<_> = node.children().collect();
                stack.extend(children.into_iter().rev());
            }
            SyntaxElement::Token(token) => out.push(format!(
                "token:{:?}:{:?}:{:?}:{}:{}:{}",
                token.kind(),
                token.range(),
                token.flags(),
                token
                    .leading_trivia()
                    .iter()
                    .map(|t| t.text.write_to_string())
                    .collect::<String>(),
                token.text().write_to_string(),
                token
                    .trailing_trivia()
                    .iter()
                    .map(|t| t.text.write_to_string())
                    .collect::<String>(),
            )),
        }
    }
    out
}

fn diagnostic_fingerprint(tree: &SyntaxTree<OkfMarkdownLanguage>) -> Vec<String> {
    tree.diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{:?}:{}",
                diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
            )
        })
        .collect()
}

fn assert_shell_case(source_text: String) {
    let parsed =
        parse_okf_markdown(source(&source_text), MarkdownDialect::CommonMarkCurrent).unwrap();
    assert_red_tree(&parsed.tree, &source_text);
}

fn boundaries(value: &str) -> Vec<usize> {
    value
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(value.len()))
        .collect()
}

fn assert_incremental_sequence(mut current: String, edits: Vec<(usize, usize, String)>) {
    let mut previous = parse_okf_markdown(source(&current), MarkdownDialect::CommonMarkCurrent)
        .unwrap()
        .tree;
    for (raw_start, raw_end, replacement) in edits {
        let points = boundaries(&current);
        let start = points[raw_start % points.len()];
        let end = points[raw_end % points.len()];
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let mut candidate = current.clone();
        candidate.replace_range(start..end, &replacement);
        let change = TextChange {
            old_range: range(start, end),
            replacement: Arc::from(replacement),
        };
        let outcome = reparse_okf_markdown(&previous, source(&candidate), &[change]).unwrap();
        let tree = match outcome {
            ReparseOutcome::Incremental { tree, .. } | ReparseOutcome::Full { tree, .. } => tree,
        };
        let full = parse_okf_markdown(source(&candidate), MarkdownDialect::CommonMarkCurrent)
            .unwrap()
            .tree;
        assert_eq!(tree.write_to_string(), full.write_to_string());
        assert_eq!(diagnostic_fingerprint(&tree), diagnostic_fingerprint(&full));
        assert_eq!(shell_fingerprint(&tree), shell_fingerprint(&full));
        assert_red_tree(&tree, &candidate);
        previous = tree;
        current = candidate;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn arbitrary_utf8_is_lossless_and_ranges_navigate(source in shell_source()) { assert_shell_case(source); }
    #[test]
    fn valid_edit_sequences_match_full_parse(source in shell_source(), edits in edit_sequence()) {
        assert_incremental_sequence(source, edits);
    }
}
