use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, MarkdownSyntaxSpan, SourceText, TextRange,
    TextSize,
};

fn whole(source: &str) -> TextRange {
    TextRange::new(TextSize::new(0), TextSize::try_from(source.len()).unwrap()).unwrap()
}

#[test]
fn queries_publish_token_covering_spans_and_metadata_by_semantic_owner() {
    let source = concat!(
        "# Heading\n\n",
        "- [x] task\n\n",
        "| left | right |\n| :--- | ---: |\n| one | two |\n\n",
        "[link](https://example.test \"title\") ![image](image.png \"caption\")\n\n",
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
    for byte in 0..source.len() {
        if !source.as_bytes()[byte].is_ascii_whitespace() {
            let offset = TextSize::try_from(byte).unwrap();
            assert!(spans.iter().any(|span| span.range.contains(offset)));
        }
    }

    for heading in spans.iter().filter_map(|span| queries.heading(span.owner)) {
        assert_eq!(queries.heading(heading.owner), Some(heading));
    }
    for list in spans.iter().filter_map(|span| queries.list(span.owner)) {
        assert_eq!(queries.list(list.owner), Some(list));
    }
    for cell in spans
        .iter()
        .filter_map(|span| queries.table_cell(span.owner))
    {
        assert_eq!(queries.table_cell(cell.owner), Some(cell));
    }
    for link in queries.links() {
        assert_eq!(queries.link(link.identity), Some(link));
    }
    for image in queries.images() {
        assert_eq!(queries.image(image.owner), Some(image));
    }
    for html in spans.iter().filter_map(|span| queries.raw_html(span.owner)) {
        assert_eq!(queries.raw_html(html.owner), Some(html));
    }
    for code in spans
        .iter()
        .filter_map(|span| queries.fenced_code(span.owner))
    {
        assert_eq!(queries.fenced_code(code.owner), Some(code));
    }
    for island in snapshot.structure().islands.iter() {
        assert_eq!(queries.island(island.owner), Some(island));
    }
    assert!(queries.diagnostics(whole(source)).count() >= snapshot.diagnostics().len());
    assert!(queries.has_recovery(whole(source)));
}
