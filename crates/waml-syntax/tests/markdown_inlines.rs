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
    let source = "text \\* &amp; &#65; &#x41; `a``b` *em* **strong** *outer **inner*** [inline](/one \"title\") ![image][id] <https://example.test> <i>x</i>\\\nsoft  \nhard\\\nhard\n\n[id]: /two \"reference title\"\n";
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
    let source = "[a][id] ![b][] [id]\n\n[id]: /one \"title\"\n[b]: /image\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    assert_eq!(found.iter().filter(|kind| **kind == Kind::Link).count(), 2);
    assert!(found.contains(&Kind::Image));
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let destinations: Vec<_> = snapshot
        .queries()
        .links()
        .map(|link| link.destination.as_ref())
        .collect();
    assert_eq!(destinations, ["/one", "/image", "/one"]);
}

#[test]
fn snapshot_queries_use_the_first_normalized_reference_definition() {
    let source = "[a][id] and ![b][]\n\n[id]: /one \"title\"\n[b]: /image\n[ID]: /two\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::from_shared(Arc::new(source.into())).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let links: Vec<_> = snapshot.queries().links().collect();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].destination.as_ref(), "/one");
    assert_eq!(
        links[0].destination_range.unwrap().start().to_usize(),
        source.find("/one").unwrap()
    );
    assert_eq!(links[0].kind, MarkdownLinkKind::Reference);
    assert_eq!(links[0].title.as_deref(), Some("title"));
    assert_ne!(links[0].identity, links[0].owner);
    assert_eq!(snapshot.queries().reference_backlinks(" ID ").len(), 1);
    assert_eq!(snapshot.queries().reference_backlinks("b").len(), 1);
    assert_eq!(
        snapshot.tree().write_to_string(),
        snapshot.text().shared().as_str()
    );
}

#[test]
fn reference_definition_normalization_keeps_the_first_definition() {
    let source = "[x][  Ä\tLABEL ]\n\n[ä label]: /first\n[Ä  LABEL]: /second\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    let links: Vec<_> = snapshot.queries().links().collect();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].destination.as_ref(), "/first");
    assert_eq!(
        links[0].destination_range.unwrap().start().to_usize(),
        source.find("/first").unwrap()
    );
    assert_eq!(snapshot.queries().reference_backlinks("ä label").len(), 1);
}

#[test]
fn emphasis_uses_flanking_rule_of_three_and_nested_mixed_runs() {
    let source = "foo_bar_baz ***outer **inner** end***\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    assert_eq!(
        found
            .iter()
            .filter(|kind| **kind == Kind::StrongEmphasis)
            .count(),
        2
    );
    assert_eq!(
        found.iter().filter(|kind| **kind == Kind::Emphasis).count(),
        1
    );
}

#[test]
fn code_spans_require_equal_bounded_runs_and_hide_inline_syntax() {
    let source = "``a`b`` ``[hidden](/bad)`` `no``\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    assert_eq!(snapshot.tree().write_to_string(), source);
    let mut found = Vec::new();
    kinds(&snapshot.tree().root(), &mut found);
    assert_eq!(
        found.iter().filter(|kind| **kind == Kind::CodeSpan).count(),
        2
    );
    assert_eq!(snapshot.queries().links().count(), 0);
}

#[test]
fn entities_and_escapes_are_validated_without_losing_spelling() {
    let source = "&amp; &#65; &#x41; &bogus; \\* \\a\n";
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        SourceText::new(source).unwrap(),
        MarkdownDialect::CommonMarkCurrent,
    )
    .unwrap();
    assert_eq!(snapshot.tree().write_to_string(), source);
    let mut found = Vec::new();
    kinds(&snapshot.tree().root(), &mut found);
    assert_eq!(
        found.iter().filter(|kind| **kind == Kind::Entity).count(),
        3
    );
    assert_eq!(
        found.iter().filter(|kind| **kind == Kind::Escape).count(),
        1
    );
    let entities: Vec<_> = snapshot.queries().entities().collect();
    assert_eq!(
        entities
            .iter()
            .map(|entity| entity.value.as_ref())
            .collect::<Vec<_>>(),
        ["&", "A", "A"]
    );
    assert_eq!(
        entities[0].source_range.start().to_usize(),
        source.find("&amp;").unwrap()
    );
    assert_ne!(entities[0].identity.get(), 0);
}

#[test]
fn hard_breaks_keep_delimiters_separate_from_newlines() {
    let source = "space  \nslash\\\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let hard_breaks: Vec<_> = descendant_nodes(&parsed.tree.root())
        .into_iter()
        .filter(|node| node.kind() == Kind::HardLineBreak)
        .collect();
    assert_eq!(hard_breaks.len(), 2);
    assert_eq!(
        direct_token_kinds(&hard_breaks[0]),
        [Kind::WhitespaceToken, Kind::NewlineToken]
    );
    assert_eq!(
        direct_token_kinds(&hard_breaks[1]),
        [Kind::BackslashToken, Kind::NewlineToken]
    );
}

#[test]
fn heading_content_uses_the_inline_phase_without_changing_markers() {
    let source = "# *heading* and `code`\n";
    let parsed = parse(source);
    assert_eq!(parsed.tree.write_to_string(), source);
    let mut found = Vec::new();
    kinds(&parsed.tree.root(), &mut found);
    assert!(found.contains(&Kind::AtxHeading));
    assert!(found.contains(&Kind::Emphasis));
    assert!(found.contains(&Kind::CodeSpan));
}

fn descendant_nodes(
    node: &SyntaxNode<OkfMarkdownLanguage>,
) -> Vec<SyntaxNode<OkfMarkdownLanguage>> {
    let mut out = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            out.push(child.clone());
            out.extend(descendant_nodes(&child));
        }
    }
    out
}

fn direct_token_kinds(node: &SyntaxNode<OkfMarkdownLanguage>) -> Vec<Kind> {
    node.children()
        .filter_map(|child| match child {
            SyntaxElement::Token(token) => Some(token.kind()),
            SyntaxElement::Node(_) => None,
        })
        .collect()
}
