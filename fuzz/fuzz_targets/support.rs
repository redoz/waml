#![allow(dead_code)]

use std::{fmt::Debug, sync::Arc};

use waml_syntax::{
    parse_markdown, DocumentRevision, GreenText, MarkdownDialect, MarkdownStructureMap,
    OkfMarkdownLanguage, SourceText, SyntaxElement, SyntaxLanguage, SyntaxTree, TextRange,
    TextSize,
};

const MAX_INPUT_BYTES: usize = 256 * 1024;

pub fn valid_utf8(data: &[u8]) -> Option<&str> {
    (data.len() <= MAX_INPUT_BYTES)
        .then(|| std::str::from_utf8(data).ok())
        .flatten()
}

pub fn source(value: &str) -> SourceText {
    SourceText::from_shared(Arc::new(value.to_owned())).expect("bounded UTF-8 source")
}

pub fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).expect("bounded start"),
        TextSize::try_from_usize(end).expect("bounded end"),
    )
    .expect("ordered range")
}

fn element_range<L: SyntaxLanguage>(element: &SyntaxElement<L>) -> TextRange {
    match element {
        SyntaxElement::Node(node) => node.range(),
        SyntaxElement::Token(token) => token.range(),
    }
}

pub fn assert_range(value: &str, range: TextRange) {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    assert!(start <= end);
    assert!(end <= value.len());
    assert!(value.is_char_boundary(start));
    assert!(value.is_char_boundary(end));
}

pub fn assert_tree_ranges<L: SyntaxLanguage>(tree: &SyntaxTree<L>, value: &str) {
    assert_eq!(tree.root().range(), range(0, value.len()));
    for diagnostic in tree.diagnostics() {
        assert_range(value, diagnostic.range);
    }

    let mut stack = vec![SyntaxElement::Node(tree.root())];
    while let Some(element) = stack.pop() {
        let current_range = element_range(&element);
        assert_range(value, current_range);
        let locator = element.locator();
        let resolved = tree.resolve(&locator).expect("same-tree locator resolves");
        assert_eq!(resolved.kind(), element.kind());
        assert_eq!(element_range(&resolved), current_range);

        if let SyntaxElement::Node(node) = element {
            let children: Vec<_> = node.children().collect();
            let mut cursor = node.range().start();
            for child in &children {
                let child_range = element_range(child);
                assert_eq!(child_range.start(), cursor);
                cursor = child_range.end();
            }
            assert_eq!(cursor, node.range().end());
            stack.extend(children.into_iter().rev());
        }
    }
}

fn assert_structure_ranges(value: &str, structure: &MarkdownStructureMap) {
    for heading in structure
        .headings
        .iter()
        .chain(structure.nested_headings.iter())
    {
        assert!((1..=6).contains(&heading.level));
        assert_range(value, heading.range);
        assert_range(value, heading.text_range);
        assert!(heading.range.start() <= heading.text_range.start());
        assert!(heading.text_range.end() <= heading.range.end());
    }
    for range in structure
        .protected_ranges
        .iter()
        .chain(structure.list_item_lines.iter())
        .chain(structure.tab_indented_item_lines.iter())
        .chain(structure.opaque_ranges.iter())
    {
        assert_range(value, *range);
    }
}

pub fn assert_shell_invariants(value: &str) {
    let parsed = parse_markdown(
        DocumentRevision::INITIAL,
        source(value),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("bounded UTF-8 shell parses");
    assert_eq!(parsed.tree().write_to_string(), value);
    assert_tree_ranges(parsed.tree(), value);
    assert_structure_ranges(value, parsed.structure());
}

pub fn syntax_fingerprint(tree: &SyntaxTree<OkfMarkdownLanguage>) -> Vec<String> {
    let mut output = Vec::new();
    let mut stack = vec![SyntaxElement::Node(tree.root())];
    while let Some(element) = stack.pop() {
        match element {
            SyntaxElement::Node(node) => {
                output.push(format!("node:{:?}:{:?}", node.kind(), node.range()));
                let children: Vec<_> = node.children().collect();
                stack.extend(children.into_iter().rev());
            }
            SyntaxElement::Token(token) => {
                let text = |value: &GreenText| value.write_to_string();
                output.push(format!(
                    "token:{:?}:{:?}:{:?}:{:?}:{}:{:?}",
                    token.kind(),
                    token.range(),
                    token.flags(),
                    token
                        .leading_trivia()
                        .iter()
                        .map(|trivia| (trivia.kind, text(&trivia.text)))
                        .collect::<Vec<_>>(),
                    text(token.text()),
                    token
                        .trailing_trivia()
                        .iter()
                        .map(|trivia| (trivia.kind, text(&trivia.text)))
                        .collect::<Vec<_>>()
                ));
            }
        }
    }
    output
}

pub fn diagnostic_fingerprint<L>(tree: &SyntaxTree<L>) -> Vec<String>
where
    L: SyntaxLanguage,
    L::DiagnosticCode: Debug,
{
    let mut diagnostics: Vec<_> = tree
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{:?}:{}",
                diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
            )
        })
        .collect();
    diagnostics.sort_unstable();
    diagnostics
}

pub fn derived_valid_edit(data: &[u8], value: &str) -> (usize, usize, Arc<str>) {
    let boundaries: Vec<_> = value
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(value.len()))
        .collect();
    let first = usize::from(data.first().copied().unwrap_or(0)) % boundaries.len();
    let second = usize::from(data.get(1).copied().unwrap_or(0)) % boundaries.len();
    let (first, second) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let replacement = valid_utf8(data.get(2..).unwrap_or_default())
        .unwrap_or_default()
        .chars()
        .take(16)
        .collect::<String>();
    (
        boundaries[first],
        boundaries[second],
        Arc::from(replacement),
    )
}
