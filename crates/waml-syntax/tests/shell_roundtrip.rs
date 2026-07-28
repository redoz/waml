use std::sync::Arc;

use waml_syntax::{parse_okf_markdown, MarkdownDialect, SourceText};

#[test]
fn preserves_bom_crlf_unicode_frontmatter_and_top_level_headings() {
    let source = "\u{feff}---\r\ntype: arbitrary.\u{1d11e}\r\n---\r\n# Title  \r\n## Section\r\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::CommonMarkCurrent).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert_eq!(shell.structure.headings.len(), 2);
    assert_eq!(shell.structure.headings[0].level, 1);
    assert_eq!(shell.structure.headings[1].level, 2);
}

#[test]
fn malformed_and_unclosed_frontmatter_recovers_without_losing_bytes() {
    let source = "---\r\ntype arbitrary\r\nname: \u{1d11e}\r\n# Recovered\r\n## Child\r\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::CommonMarkCurrent).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert_eq!(shell.tree.root().range().len().to_usize(), source.len());
    assert!(shell
        .tree
        .diagnostics()
        .iter()
        .any(|d| format!("{:?}", d.code).contains("MissingFrontmatterFence")));
    assert!(shell
        .tree
        .diagnostics()
        .iter()
        .any(|d| format!("{:?}", d.code).contains("FrontmatterNotClean")));
    assert_eq!(shell.structure.headings.len(), 2);
    assert!(shell
        .structure
        .headings
        .iter()
        .all(|h| h.range.end().to_usize() <= source.len()));
}

#[test]
fn thematic_rule_without_plausible_frontmatter_stays_markdown() {
    let source = "---\nnot a key value\n\n# Real title\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::CommonMarkCurrent).unwrap();
    assert_eq!(shell.tree.write_to_string(), source);
    assert!(shell.tree.diagnostics().is_empty());
}

#[test]
fn keeps_headings_in_protected_containers_raw() {
    let source = "> # quote\n\n- ## list\n\n```md\n# fence\n```\n\n<!-- # comment -->\n\n# top\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::CommonMarkCurrent).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert_eq!(shell.structure.headings.len(), 1);
    assert_eq!(shell.structure.headings[0].level, 1);
}
