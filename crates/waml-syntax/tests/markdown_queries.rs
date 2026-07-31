use waml_syntax::{
    parse_markdown, DocumentRevision, HtmlTagFilter, MarkdownDialect, MarkdownLinkKind,
    MarkdownListKind, MarkdownSemanticRole, MarkdownSourceRole, MarkdownSyntaxSpan,
    OkfMarkdownLanguage, SourceText, SyntaxElement, SyntaxNode, TableAlignment, TaskListState,
    TextRange, TextSize,
};

fn whole(source: &str) -> TextRange {
    TextRange::new(TextSize::new(0), TextSize::try_from(source.len()).unwrap()).unwrap()
}

fn range_of(source: &str, needle: &str) -> TextRange {
    let start = source.find(needle).unwrap();
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(start + needle.len()).unwrap(),
    )
    .unwrap()
}

fn token_ranges(node: &SyntaxNode<OkfMarkdownLanguage>, out: &mut Vec<TextRange>) {
    for child in node.children() {
        match child {
            SyntaxElement::Node(child) => token_ranges(&child, out),
            SyntaxElement::Token(token) if token.range().start() < token.range().end() => {
                out.push(token.range())
            }
            SyntaxElement::Token(_) => {}
        }
    }
}

#[test]
fn queries_publish_token_covering_spans_and_metadata_by_semantic_owner() {
    let source = concat!(
        "# Heading\n\n",
        "- [x] task\n\n",
        "| left | right |\n| :--- | ---: |\n| one | two |\n\n",
        "[link](https://example.test \"title\") ![image](image.png \"caption\")\n\n",
        "<https://auto.test> www.extended.test\n\n",
        "```rust\nfn main() {}\n```\n\n",
        "<script>alert(1)</script>\n\n",
        "[malformed](\n\n",
        "## Attributes\nname: value\n",
    );
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let queries = snapshot.queries();

    let spans: Vec<&MarkdownSyntaxSpan> = queries.spans(whole(source)).collect();
    assert!(!spans.is_empty());
    assert!(spans
        .windows(2)
        .all(|pair| pair[0].range.end() <= pair[1].range.start()));
    let mut expected_ranges = Vec::new();
    token_ranges(&snapshot.tree().root(), &mut expected_ranges);
    assert_eq!(
        spans.iter().map(|span| span.range).collect::<Vec<_>>(),
        expected_ranges
    );
    for byte in 0..source.len() {
        if !source.as_bytes()[byte].is_ascii_whitespace() {
            let offset = TextSize::try_from(byte).unwrap();
            assert!(spans.iter().any(|span| span.range.contains(offset)));
        }
    }

    let headings: Vec<_> = spans
        .iter()
        .filter_map(|span| queries.heading(span.owner))
        .collect();
    assert!(headings
        .iter()
        .any(|heading| heading.level == 1 && heading.content_range == range_of(source, "Heading")));
    let task_list = spans
        .iter()
        .find_map(|span| queries.list(span.owner))
        .expect("task list metadata");
    assert_eq!(task_list.kind, MarkdownListKind::Bullet);
    assert_eq!(task_list.task, Some(TaskListState::Checked));
    let cells: Vec<_> = spans
        .iter()
        .filter_map(|span| queries.table_cell(span.owner))
        .collect();
    assert!(cells.len() >= 4);
    assert!(cells
        .iter()
        .any(|cell| cell.alignment == TableAlignment::Left));
    assert!(cells
        .iter()
        .any(|cell| cell.alignment == TableAlignment::Right));

    let links: Vec<_> = queries.links().collect();
    assert!(links
        .iter()
        .any(|link| link.destination.as_ref() == "https://example.test"
            && link.destination_range == Some(range_of(source, "https://example.test"))
            && link.title.as_deref() == Some("title")
            && link.kind == MarkdownLinkKind::Inline));
    assert!(links
        .iter()
        .any(|link| link.kind == MarkdownLinkKind::Autolink));
    assert!(links
        .iter()
        .any(|link| link.kind == MarkdownLinkKind::ExtendedAutolink));
    for link in queries.links() {
        assert_eq!(queries.link(link.owner), Some(link));
    }
    let image = queries.images().next().expect("image metadata");
    assert_eq!(image.source.as_ref(), "image.png");
    assert_eq!(image.alt_range, range_of(source, "image"));
    assert_eq!(
        image.source_definition_range,
        Some(range_of(source, "image.png"))
    );
    assert_eq!(queries.image(image.owner), Some(image));

    let html = spans
        .iter()
        .find_map(|span| queries.raw_html(span.owner))
        .expect("filtered HTML metadata");
    assert_eq!(html.filter, HtmlTagFilter::Disallowed);
    let code = spans
        .iter()
        .find_map(|span| queries.fenced_code(span.owner))
        .expect("fenced code metadata");
    assert_eq!(code.info.as_ref(), "rust");
    assert_eq!(code.language.as_deref(), Some("rust"));
    assert_eq!(code.info_range, Some(range_of(source, "rust")));
    assert_eq!(code.content_range, range_of(source, "fn main() {}\n"));

    for island in snapshot.structure().islands.iter() {
        assert_eq!(queries.island(island.owner), Some(island));
    }
    assert!(queries.diagnostics(whole(source)).count() >= snapshot.diagnostics().len());
    assert!(queries.has_recovery(whole(source)));
    assert!(spans.iter().any(|span| {
        span.range == range_of(source, "#")
            && span.source_role == MarkdownSourceRole::SyntaxMarker
            && span.semantic_role == MarkdownSemanticRole::Heading
    }));
    assert!(spans.iter().any(|span| {
        span.range == range_of(source, "Heading")
            && span.source_role == MarkdownSourceRole::Content
            && span.semantic_role == MarkdownSemanticRole::Text
    }));
}
