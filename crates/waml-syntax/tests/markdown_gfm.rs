use std::sync::Arc;

use waml_syntax::{
    parse_markdown, parse_okf_markdown, DocumentRevision, MarkdownDialect, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind as Kind, SourceText, SyntaxElement, SyntaxNode,
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
            SyntaxElement::Token(token) => {
                result.push((token.kind(), token.text().write_to_string()))
            }
        }
    }
    result
}

fn annotation_data<'a>(node: &'a SyntaxNode<OkfMarkdownLanguage>, kind: &str) -> Option<&'a str> {
    node.syntax_annotations()
        .iter()
        .find(|annotation| annotation.kind() == kind)
        .and_then(|annotation| annotation.data())
}

fn identity(node: &SyntaxNode<OkfMarkdownLanguage>) -> u64 {
    annotation_data(node, "waml.markdown.identity")
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn gfm_tables_segment_markers_and_preserve_escaped_pipes_and_code_spans() {
    let source =
        "| left | middle | right |\n| :--- | :---: | ---: |\n| `code` | two \\| cell | three |\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    let nodes = descendants(&parsed.tree.root());
    assert!(nodes.iter().any(|node| node.kind() == Kind::Table));
    assert!(nodes.iter().any(|node| node.kind() == Kind::TableHead));
    assert!(nodes.iter().any(|node| node.kind() == Kind::TableBody));
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.kind() == Kind::TableCell)
            .count(),
        6
    );
    let spelling = tokens(&parsed.tree.root());
    assert!(spelling
        .iter()
        .any(|token| token == &(Kind::TablePipeToken, "|".into())));
    assert!(
        spelling
            .iter()
            .any(|token| token == &(Kind::TableAlignmentColonToken, ":".into())),
        "{spelling:?}"
    );
    assert!(spelling
        .iter()
        .any(|token| token == &(Kind::BackslashToken, "\\".into())));
    assert!(nodes.iter().any(|node| node.kind() == Kind::CodeSpan));
    let cells: Vec<_> = nodes
        .iter()
        .filter(|node| node.kind() == Kind::TableCell)
        .collect();
    assert_eq!(
        cells
            .iter()
            .take(3)
            .map(|cell| annotation_data(cell, "waml.markdown.gfm.table_alignment").unwrap())
            .collect::<Vec<_>>(),
        ["left", "center", "right"]
    );
    for cell in cells {
        let metadata = cell
            .syntax_annotations()
            .iter()
            .find(|annotation| annotation.kind() == "waml.markdown.gfm.table_alignment")
            .unwrap();
        assert_eq!(metadata.id().get(), identity(cell));
    }
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
    let items: Vec<_> = descendants(&parsed.tree.root())
        .into_iter()
        .filter(|node| node.kind() == Kind::ListItem)
        .collect();
    assert_eq!(
        items
            .iter()
            .map(|item| annotation_data(item, "waml.markdown.gfm.task_state"))
            .collect::<Vec<_>>(),
        [Some("unchecked"), Some("checked"), Some("checked"), None]
    );
    for item in &items[..3] {
        let metadata = item
            .syntax_annotations()
            .iter()
            .find(|annotation| annotation.kind() == "waml.markdown.gfm.task_state")
            .unwrap();
        assert_eq!(metadata.id().get(), identity(item));
    }
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
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.kind() == Kind::Strikethrough)
            .count(),
        1
    );
    assert_eq!(
        tokens(&parsed.tree.root())
            .into_iter()
            .filter(|(kind, _)| *kind == Kind::StrikethroughDelimiterToken)
            .map(|(_, text)| text)
            .collect::<Vec<_>>(),
        ["~~", "~~"]
    );
}

#[test]
fn gfm_strikethrough_requires_flanking_exact_double_runs() {
    let source = "~~ok~~ ~~ no~~ ~~no ~~ ~one~ ~~~three~~~ \\~~escaped~~ `~~code~~`\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    let strikes: Vec<_> = descendants(&parsed.tree.root())
        .into_iter()
        .filter(|node| node.kind() == Kind::Strikethrough)
        .collect();
    assert_eq!(strikes.len(), 1);
    assert_eq!(
        &source[strikes[0].range().start().to_usize()..strikes[0].range().end().to_usize()],
        "~~ok~~"
    );
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
    assert_eq!(
        autolinks,
        [
            "www.example.test",
            "http://one.test",
            "https://two.test",
            "a@b.test"
        ]
    );
}

#[test]
fn extended_autolinks_publish_destinations_and_trim_gfm_tails() {
    let source = "www.example.test http://one.test/a(b)) https://two.test&copy; person@example.test HTTPS://three.test:8443/path o'hara+tag@example.test\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let links: Vec<_> = snapshot.queries().links().collect();
    assert_eq!(
        links
            .iter()
            .map(|link| link.destination.as_ref())
            .collect::<Vec<_>>(),
        [
            "http://www.example.test",
            "http://one.test/a(b)",
            "https://two.test",
            "mailto:person@example.test",
            "HTTPS://three.test:8443/path",
            "mailto:o'hara+tag@example.test"
        ]
    );
    assert_eq!(
        links
            .iter()
            .map(|link| &source
                [link.source_range.start().to_usize()..link.source_range.end().to_usize()])
            .collect::<Vec<_>>(),
        [
            "www.example.test",
            "http://one.test/a(b)",
            "https://two.test",
            "person@example.test",
            "HTTPS://three.test:8443/path",
            "o'hara+tag@example.test"
        ]
    );
}

#[test]
fn extended_autolinks_reject_embedded_and_malformed_candidates() {
    let source = "xwww.example.test xhttp://example.test www._bad.test www.bad_.test a@b a..b@example.test a@-bad.test a@bad-.test\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    assert_eq!(snapshot.queries().links().count(), 0);
}

#[test]
fn gfm_tag_filter_is_case_insensitive_and_leaves_html_lossless() {
    let source = "<SCRIPT>x</SCRIPT> <div>ok</div>\n";
    let parsed = parse(source, MarkdownDialect::WAML_DEFAULT);
    assert_eq!(parsed.tree.write_to_string(), source);
    let filtered: Vec<_> = parsed
        .tree
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == waml_syntax::OkfSyntaxDiagnosticCode::FilteredHtmlTag
        })
        .collect();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].range.start().to_usize(), 1);
    assert_eq!(filtered[0].range.end().to_usize(), 7);
    let html: Vec<_> = descendants(&parsed.tree.root())
        .into_iter()
        .filter(|node| matches!(node.kind(), Kind::RawHtml | Kind::HtmlBlock))
        .collect();
    assert!(html
        .iter()
        .all(|node| annotation_data(node, "waml.markdown.gfm.html_tag_filter").is_some()));
    for node in html {
        let metadata = node
            .syntax_annotations()
            .iter()
            .find(|annotation| annotation.kind() == "waml.markdown.gfm.html_tag_filter")
            .unwrap();
        assert_eq!(metadata.id().get(), identity(&node));
    }
}

#[test]
fn inline_raw_html_records_allowed_and_disallowed_filter_metadata() {
    let parsed = parse(
        "text <ScRiPt> and <span> and <!-- comment -->\n",
        MarkdownDialect::WAML_DEFAULT,
    );
    let html: Vec<_> = descendants(&parsed.tree.root())
        .into_iter()
        .filter(|node| node.kind() == Kind::RawHtml)
        .collect();
    assert_eq!(
        html.iter()
            .map(|node| annotation_data(node, "waml.markdown.gfm.html_tag_filter").unwrap())
            .collect::<Vec<_>>(),
        ["disallowed", "allowed", "allowed"]
    );
}

#[test]
fn commonmark_dialect_keeps_all_five_extensions_as_ordinary_structure() {
    let source = "| a | b |\n| --- | --- |\n| c | d |\n\n- [x] task\n\n~~gone~~ www.example.test <script>x</script>\n";
    let parsed = parse(source, MarkdownDialect::COMMONMARK_0_31_2);
    assert_eq!(parsed.tree.write_to_string(), source);
    let nodes = descendants(&parsed.tree.root());
    for kind in [
        Kind::Table,
        Kind::TableHead,
        Kind::TableBody,
        Kind::TableCell,
        Kind::Strikethrough,
    ] {
        assert!(
            !nodes.iter().any(|node| node.kind() == kind),
            "unexpected {kind:?}"
        );
    }
    assert!(!tokens(&parsed.tree.root())
        .iter()
        .any(|(kind, _)| *kind == Kind::TaskListMarkerToken));
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.kind() == Kind::Autolink)
            .count(),
        0
    );
    assert!(
        !parsed
            .tree
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code
                == waml_syntax::OkfSyntaxDiagnosticCode::FilteredHtmlTag)
    );
}
