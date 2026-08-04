use waml_syntax::{
    parse_markdown, DocumentRevision, HtmlTagFilter, MarkdownDialect, MarkdownLinkKind,
    MarkdownListKind, MarkdownSemanticRole, MarkdownSourceRole, MarkdownSyntaxSpan,
    OkfMarkdownLanguage, OkfSyntaxDiagnosticCode, SourceText, SyntaxElement, SyntaxNode,
    TableAlignment, TaskListState, TextRange, TextSize, WamlSectionKind,
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

#[test]
fn tight_list_items_publish_typed_link_queries() {
    let source = "- [inline](./inline.md)\n- [reference][target]\n\n[target]: ./target.md\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();

    let links = snapshot.queries().links().collect::<Vec<_>>();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].content_range, range_of(source, "inline"));
    assert_eq!(links[0].destination.as_ref(), "./inline.md");
    assert_eq!(links[1].content_range, range_of(source, "reference"));
    assert_eq!(links[1].destination.as_ref(), "./target.md");
}

#[test]
fn frontmatter_spans_cover_the_source_with_semantic_owners() {
    let source = "---\ntype: uml.Class\n---\n# Class\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let frontmatter_end = source.find("# Class").unwrap();
    let spans = snapshot
        .queries()
        .spans(whole(source))
        .filter(|span| span.range.end().to_usize() <= frontmatter_end)
        .collect::<Vec<_>>();

    assert_eq!(spans.first().unwrap().range.start(), TextSize::new(0));
    assert_eq!(
        spans.last().unwrap().range.end(),
        TextSize::try_from(frontmatter_end).unwrap()
    );
    assert!(spans
        .windows(2)
        .all(|pair| pair[0].range.end() == pair[1].range.start()));

    // Frontmatter spans carry finer semantic roles than the parent
    // `Frontmatter` node kind: the fence is `FrontmatterFence`, a key is
    // `FrontmatterKey`, and its value is `FrontmatterScalar` — that
    // granularity is what lets the presentation layer color and (for the
    // reading view) hide keys, values, comments, and punctuation
    // independently. See `TextRole::FrontmatterToken` in
    // `waml-markdown-editor`.
    let owner_for = |needle: &str, expected: MarkdownSemanticRole| {
        let range = range_of(source, needle);
        spans
            .iter()
            .find(|span| span.range.start() <= range.start() && range.end() <= span.range.end())
            .map(|span| {
                assert_eq!(span.semantic_role, expected);
                span.owner
            })
            .unwrap()
    };
    let fence_owner = owner_for("---", MarkdownSemanticRole::FrontmatterFence);
    let entry_owner = owner_for("type", MarkdownSemanticRole::FrontmatterKey);
    assert_eq!(
        entry_owner,
        owner_for("uml.Class", MarkdownSemanticRole::FrontmatterScalar)
    );
    assert_ne!(fence_owner, entry_owner);
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
    assert!(headings.iter().any(|heading| heading.level == 1
        && heading.range == range_of(source, "# Heading\n")
        && heading.content_range == range_of(source, "Heading")
        && queries.heading(heading.owner) == Some(heading)));
    let task_list = spans
        .iter()
        .find_map(|span| queries.list(span.owner))
        .expect("task list metadata");
    assert_eq!(task_list.kind, MarkdownListKind::Bullet);
    assert_eq!(task_list.task, Some(TaskListState::Checked));
    assert_eq!(task_list.range, range_of(source, "- [x] task\n\n"));
    assert_eq!(queries.list(task_list.owner), Some(task_list));
    let cells: Vec<_> = spans
        .iter()
        .filter_map(|span| queries.table_cell(span.owner))
        .collect();
    assert!(cells.len() >= 4);
    let left_cell = cells
        .iter()
        .find(|cell| cell.alignment == TableAlignment::Left)
        .expect("left-aligned table cell");
    assert_eq!(left_cell.range, range_of(source, " left "));
    assert_eq!(queries.table_cell(left_cell.owner), Some(*left_cell));
    let right_cell = cells
        .iter()
        .find(|cell| cell.alignment == TableAlignment::Right)
        .expect("right-aligned table cell");
    assert_eq!(right_cell.range, range_of(source, " right "));
    assert_eq!(queries.table_cell(right_cell.owner), Some(*right_cell));

    let links: Vec<_> = queries.links().collect();
    assert!(links
        .iter()
        .any(|link| link.destination.as_ref() == "https://example.test"
            && link.source_range == range_of(source, "[link](https://example.test \"title\")")
            && link.content_range == range_of(source, "link")
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
    assert_eq!(
        image.source_range,
        range_of(source, "![image](image.png \"caption\")")
    );
    assert_eq!(image.source.as_ref(), "image.png");
    assert_eq!(image.alt_range, range_of(source, "image"));
    assert_eq!(
        image.source_definition_range,
        Some(range_of(source, "image.png"))
    );
    assert_eq!(image.title.as_deref(), Some("caption"));
    assert_eq!(image.kind, MarkdownLinkKind::Inline);
    assert_eq!(queries.image(image.owner), Some(image));

    let html = spans
        .iter()
        .find_map(|span| queries.raw_html(span.owner))
        .expect("filtered HTML metadata");
    assert_eq!(html.filter, HtmlTagFilter::Disallowed);
    assert_eq!(html.range, range_of(source, "<script>alert(1)</script>\n"));
    assert_eq!(queries.raw_html(html.owner), Some(html));
    let code = spans
        .iter()
        .find_map(|span| queries.fenced_code(span.owner))
        .expect("fenced code metadata");
    assert_eq!(code.info.as_ref(), "rust");
    assert_eq!(
        code.source_range,
        range_of(source, "```rust\nfn main() {}\n```")
    );
    assert_eq!(code.fence_range, range_of(source, "```"));
    assert_eq!(code.language.as_deref(), Some("rust"));
    assert_eq!(code.info_range, Some(range_of(source, "rust")));
    assert_eq!(code.content_range, range_of(source, "fn main() {}\n"));
    assert_eq!(queries.fenced_code(code.owner), Some(code));

    let filtered = queries
        .diagnostics(range_of(source, "script"))
        .find(|diagnostic| diagnostic.code == OkfSyntaxDiagnosticCode::FilteredHtmlTag)
        .expect("filtered HTML diagnostic");
    assert_eq!(filtered.range, range_of(source, "script"));
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

#[test]
fn island_lookup_uses_a_non_empty_owner_and_exact_ranges() {
    let source = "# Type\n\n## Attributes\nname: value\n\n# Next\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let islands = &snapshot.structure().islands;

    assert_eq!(islands.len(), 1);
    let island = &islands[0];
    assert_ne!(island.owner.get(), 0);
    assert_eq!(island.kind, WamlSectionKind::Attributes);
    assert_eq!(island.heading_range, range_of(source, "## Attributes\n"));
    assert_eq!(
        island.content_range,
        TextRange::new(
            range_of(source, "## Attributes\n").end(),
            range_of(source, "# Next\n").start(),
        )
        .unwrap()
    );
    assert_eq!(snapshot.queries().island(island.owner), Some(island));
}

#[test]
fn recovery_queries_return_the_exact_unclosed_fence_diagnostic() {
    let source = "```rust\nbody\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let eof = TextSize::try_from(source.len()).unwrap();
    let expected = TextRange::new(eof, eof).unwrap();
    let diagnostics: Vec<_> = snapshot.queries().diagnostics(whole(source)).collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, OkfSyntaxDiagnosticCode::UnclosedFence);
    assert_eq!(diagnostics[0].range, expected);
    assert!(snapshot.queries().has_recovery(whole(source)));
    assert!(!snapshot.queries().has_recovery(range_of(source, "rust")));
}

#[test]
fn link_reference_definition_colon_is_not_frontmatter_punctuation() {
    // `ColonToken` is emitted both by frontmatter entries and by link
    // reference definitions. Only the frontmatter one may carry a
    // frontmatter semantic role: the presentation layer hides
    // `FrontmatterPunctuation` in the reading view, which would delete the
    // colon from `[id]: dest`.
    let source = "[id]: https://example.test\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let colon = range_of(source, ":");
    let span = snapshot
        .queries()
        .spans(whole(source))
        .find(|span| span.range.start() <= colon.start() && colon.end() <= span.range.end())
        .expect("the definition colon is covered by a span");

    assert_ne!(
        span.semantic_role,
        MarkdownSemanticRole::FrontmatterPunctuation
    );
}
