use std::sync::Arc;

use waml_syntax::{
    parse_okf_markdown, MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind as Kind,
    SourceText, SyntaxElement, SyntaxNode,
};

fn parse(source: &str, dialect: MarkdownDialect) -> waml_syntax::ShellParse {
    parse_okf_markdown(
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        dialect,
    )
    .unwrap()
}

fn descendants(node: &SyntaxNode<OkfMarkdownLanguage>) -> Vec<SyntaxNode<OkfMarkdownLanguage>> {
    let mut result = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            result.push(child.clone());
            result.extend(descendants(&child));
        }
    }
    result
}

fn tokens(node: &SyntaxNode<OkfMarkdownLanguage>) -> Vec<(Kind, String)> {
    let mut result = Vec::new();
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => result.extend(tokens(&child)),
            SyntaxElement::Token(token) => result.push((token.kind(), token.text().write_to_string())),
        }
    }
    result
}

#[test]
fn gfm_tables_segment_markers_and_preserve_escaped_pipes_and_code_spans() {
    let source = "| left | middle | right |\n| :--- | :---: | ---: |\n| `code` | two \\| cell | three |\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    let nodes = descendants(&parsed.tree.root());
    assert!(nodes.iter().any(|node| node.kind() == Kind::Table));
    assert!(nodes.iter().any(|node| node.kind() == Kind::TableHead));
    assert!(nodes.iter().any(|node| node.kind() == Kind::TableBody));
    assert_eq!(nodes.iter().filter(|node| node.kind() == Kind::TableCell).count(), 6);
    let spelling = tokens(&parsed.tree.root());
    assert!(spelling.iter().any(|token| token == &(Kind::TablePipeToken, "|".into())));
    assert!(spelling.iter().any(|token| token == &(Kind::TableAlignmentColonToken, ":".into())), "{spelling:?}");
    assert!(spelling.iter().any(|token| token == &(Kind::BackslashToken, "\\".into())));
    assert!(nodes.iter().any(|node| node.kind() == Kind::CodeSpan));
}

#[test]
fn unfinished_table_remains_commonmark_paragraph() {
    let parsed = parse("left | right\nnot a table\n", MarkdownDialect::WAML_DEFAULT);
    let nodes = descendants(&parsed.tree.root());
    assert!(!nodes.iter().any(|node| node.kind() == Kind::Table));
    assert!(nodes.iter().any(|node| node.kind() == Kind::Paragraph));
}

#[test]
fn gfm_tasks_mark_only_initial_lower_or_upper_x() {
    let source = "- [ ] todo\n- [x] done\n- [X] done\n- text [x] later\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    assert_eq!(
        tokens(&parsed.tree.root())
            .into_iter()
            .filter(|(kind, _)| *kind == Kind::TaskListMarkerToken)
            .map(|(_, text)| text)
            .collect::<Vec<_>>(),
        ["[ ]", "[x]", "[X]"]
    );
}

#[test]
fn unfinished_task_marker_stays_text() {
    let parsed = parse("- [y] no\n", MarkdownDialect::WAML_DEFAULT);
    assert!(!tokens(&parsed.tree.root())
        .iter()
        .any(|(kind, _)| *kind == Kind::TaskListMarkerToken));
}

#[test]
fn gfm_strikethrough_uses_double_tilde_delimiters() {
    let parsed = parse("~~gone~~ and ~still here~\n", MarkdownDialect::WAML_DEFAULT);
    let nodes = descendants(&parsed.tree.root());
    assert_eq!(nodes.iter().filter(|node| node.kind() == Kind::Strikethrough).count(), 1);
    assert_eq!(tokens(&parsed.tree.root())
        .into_iter()
        .filter(|(kind, _)| *kind == Kind::StrikethroughDelimiterToken)
        .map(|(_, text)| text)
        .collect::<Vec<_>>(), ["~~", "~~"]);
}

#[test]
fn gfm_extended_autolinks_trim_punctuation_and_skip_code_raw_html_and_links() {
    let source = "www.example.test, http://one.test! https://two.test. a@b.test: `www.code.test` <i data-url=www.html.test> [www.link.test](https://target.test)\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    let autolinks: Vec<_> = descendants(&parsed.tree.root())
        .into_iter()
        .filter(|node| node.kind() == Kind::Autolink)
        .map(|node| {
            let range = node.range();
            source[range.start().to_usize()..range.end().to_usize()].to_owned()
        })
        .collect();
    assert_eq!(autolinks, ["www.example.test", "http://one.test", "https://two.test", "a@b.test"]);
}

#[test]
fn gfm_tag_filter_is_case_insensitive_and_leaves_html_lossless() {
    let source = "<SCRIPT>x</SCRIPT> <div>ok</div>\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    let filtered: Vec<_> = parsed.tree.diagnostics().iter()
        .filter(|diagnostic| diagnostic.code == waml_syntax::OkfSyntaxDiagnosticCode::FilteredHtmlTag)
        .collect();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].range.start().to_usize(), 1);
    assert_eq!(filtered[0].range.end().to_usize(), 7);
}

#[test]
fn commonmark_dialect_keeps_all_five_extensions_as_ordinary_structure() {
    let source = "| a | b |\n| --- | --- |\n| c | d |\n\n- [x] task\n\n~~gone~~ www.example.test <script>x</script>\n";
    let parsed = parse(source, MarkdownDialect::COMMONMARK_0_31_2);
    assert_eq!(parsed.tree.write_to_string(), source);
    let nodes = descendants(&parsed.tree.root());
    for kind in [Kind::Table, Kind::TableHead, Kind::TableBody, Kind::TableCell, Kind::Strikethrough] {
        assert!(!nodes.iter().any(|node| node.kind() == kind), "unexpected {kind:?}");
    }
    assert!(!tokens(&parsed.tree.root()).iter().any(|(kind, _)| *kind == Kind::TaskListMarkerToken));
    assert_eq!(nodes.iter().filter(|node| node.kind() == Kind::Autolink).count(), 0);
    assert!(!parsed.tree.diagnostics().iter().any(|diagnostic| diagnostic.code == waml_syntax::OkfSyntaxDiagnosticCode::FilteredHtmlTag));
}
