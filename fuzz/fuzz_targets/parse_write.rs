#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SyntaxElement};

fn visit(node: waml_syntax::SyntaxNode<waml_syntax::OkfMarkdownLanguage>) {
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            visit(child);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some(value) = support::valid_utf8(data) else {
        return;
    };
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        support::source(value),
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("bounded UTF-8 markdown parses");
    assert_eq!(snapshot.tree().write_to_string(), value);
    support::assert_tree_ranges(snapshot.tree(), value);
    visit(snapshot.tree().root());
    let queries = snapshot.queries();
    let whole = support::range(0, value.len());
    let spans: Vec<_> = queries.spans(whole).collect();
    for span in spans {
        support::assert_range(value, span.range);
        if let Some(heading) = queries.heading(span.owner) {
            support::assert_range(value, heading.range);
            support::assert_range(value, heading.content_range);
        }
        if let Some(list) = queries.list(span.owner) {
            support::assert_range(value, list.range);
        }
        if let Some(cell) = queries.table_cell(span.owner) {
            support::assert_range(value, cell.range);
        }
        if let Some(link) = queries.link(span.owner) {
            support::assert_range(value, link.source_range);
            support::assert_range(value, link.content_range);
            if let Some(range) = link.destination_range {
                support::assert_range(value, range);
            }
        }
        if let Some(image) = queries.image(span.owner) {
            support::assert_range(value, image.source_range);
            support::assert_range(value, image.alt_range);
            if let Some(range) = image.source_definition_range {
                support::assert_range(value, range);
            }
        }
        if let Some(html) = queries.raw_html(span.owner) {
            support::assert_range(value, html.range);
            for range in html.filtered_ranges.iter().copied() {
                support::assert_range(value, range);
            }
        }
        if let Some(fenced) = queries.fenced_code(span.owner) {
            support::assert_range(value, fenced.source_range);
            support::assert_range(value, fenced.fence_range);
            support::assert_range(value, fenced.content_range);
            if let Some(range) = fenced.info_range {
                support::assert_range(value, range);
            }
        }
        if let Some(island) = queries.island(span.owner) {
            support::assert_range(value, island.heading_range);
            support::assert_range(value, island.content_range);
        }
    }
    for link in queries.links() {
        support::assert_range(value, link.source_range);
        support::assert_range(value, link.content_range);
        assert!(queries.link(link.owner).is_some());
    }
    for image in queries.images() {
        support::assert_range(value, image.source_range);
        support::assert_range(value, image.alt_range);
        assert!(queries.image(image.owner).is_some());
    }
    for entity in queries.entities() {
        support::assert_range(value, entity.source_range);
    }
    for diagnostic in queries.diagnostics(whole) {
        support::assert_range(value, diagnostic.range);
        assert!(queries.has_recovery(diagnostic.range));
    }
    let mut rest = value;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        for identity in queries.reference_backlinks(&rest[..close]).iter() {
            assert!(queries.links().any(|link| link.identity == *identity));
        }
        rest = &rest[close + 1..];
    }
});
