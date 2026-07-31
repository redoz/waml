use std::sync::Arc;

use waml_syntax::{
    parse_markdown, parse_okf_markdown, DocumentRevision, MarkdownDialect, MarkdownLinkKind,
    OkfMarkdownLanguage, OkfMarkdownSyntaxKind as Kind, SourceText, SyntaxElement, SyntaxNode,
};

fn parse(source: &str) -> waml_syntax::ShellParse {
    parse_okf_markdown(
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap()
}

fn kinds(node: &SyntaxNode<OkfMarkdownLanguage>, out: &mut Vec<Kind>) {
    out.push(node.kind());
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            kinds(&child, out);
        }
    }
}

#[test]
fn inline_phase_builds_lossless_commonmark_nodes() {
    let source = "text \\* &amp; &#65; &#x41; `a``b` *em* **strong** *outer **inner*** [inline](/one \\\"title\\\") ![image][id] <https://example.test> <i>x</i>\\\nsoft  \\nhard\\\\\\nhard\n\n[id]: /two \\\"reference title\\\"\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);

    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    for kind in [
        Kind::Text,
        Kind::Escape,
        Kind::Entity,
        Kind::CodeSpan,
        Kind::Emphasis,
        Kind::StrongEmphasis,
        Kind::Link,
        Kind::Image,
        Kind::Autolink,
        Kind::RawHtml,
        Kind::SoftLineBreak,
        Kind::HardLineBreak,
    ] {
        assert!(found.contains(&kind), "missing {kind:?}: {found:?}");
    }
}

#[test]
fn inline_phase_resolves_full_collapsed_and_shortcut_references() {
    let source = "[a][id] ![b][] [id]\n\n[id]: /one \\\"title\\\"\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    assert_eq!(found.iter().filter(|kind| **kind == Kind::Link).count(), 3);
    assert!(found.contains(&Kind::Image));
}

#[test]
fn snapshot_queries_use_the_first_normalized_reference_definition() {
    let source = "[a][id] and ![b][]\n\n[id]: /one \\\"title\\\"\n[ID]: /two\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let links: Vec<_> = snapshot.queries().links().collect();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].destination.as_ref(), "/one");
    assert_eq!(
        links[0].destination_range.unwrap().start().to_usize(),
        source.find("/one").unwrap()
    );
    assert_eq!(links[0].kind, MarkdownLinkKind::Reference);
    assert_eq!(
        snapshot.tree().write_to_string(),
        snapshot.text().shared().as_str()
    );
}

#[test]
fn reference_definition_normalization_keeps_the_first_definition() {
    let source = "[x][  Ä\\tLABEL ]\n\n[ä label]: /first\n[Ä  LABEL]: /second\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    assert!(found.contains(&Kind::Link));
}
