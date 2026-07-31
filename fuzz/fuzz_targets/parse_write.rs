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
    let Some(value) = support::valid_utf8(data) else { return; };
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        support::source(value),
        MarkdownDialect::WAML_DEFAULT,
    ).expect("bounded UTF-8 markdown parses");
    assert_eq!(snapshot.tree().write_to_string(), value);
    support::assert_tree_ranges(snapshot.tree(), value);
    visit(snapshot.tree().root());
    for _ in snapshot.queries().links() {}
    for _ in snapshot.queries().images() {}
    for _ in snapshot.queries().entities() {}
    for _ in snapshot.queries().spans(support::range(0, value.len())) {}
});
