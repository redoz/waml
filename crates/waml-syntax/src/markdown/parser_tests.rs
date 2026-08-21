use std::sync::Arc;

use super::parser::parse as parse_okf_markdown;
use crate::{
    GreenElement, MarkdownDialect, OkfMarkdownLanguage, OkfMarkdownSyntaxKind,
    OkfSyntaxDiagnosticCode, ShellParse, SourceText, SyntaxElement, SyntaxNode, TextSize,
};

struct Fixture {
    name: &'static str,
    source: &'static str,
    golden: &'static str,
    escaped: bool,
}

macro_rules! fixture {
    ($name:literal) => {
        Fixture {
            name: $name,
            source: include_str!(concat!("../../tests/fixtures/shell/", $name, ".md")),
            golden: include_str!(concat!("../../tests/fixtures/shell/", $name, ".golden")),
            escaped: false,
        }
    };
}

macro_rules! escaped_fixture {
    ($name:literal) => {
        Fixture {
            name: $name,
            source: include_str!(concat!("../../tests/fixtures/shell/", $name, ".escaped")),
            golden: include_str!(concat!("../../tests/fixtures/shell/", $name, ".golden")),
            escaped: true,
        }
    };
}

const FIXTURES: &[Fixture] = &[
    fixture!("clean_lf"),
    fixture!("missing_type"),
    fixture!("unknown_type"),
    fixture!("malformed_clean"),
    fixture!("unclosed_h2"),
    fixture!("unclosed_false_positive"),
    fixture!("later_thematic_rule"),
    fixture!("protected_containers"),
    fixture!("lower_headings"),
    fixture!("html_comment"),
    escaped_fixture!("heading_eof_spaces"),
    escaped_fixture!("closed_frontmatter_eof_spaces"),
    escaped_fixture!("unclosed_frontmatter_eof_spaces"),
    fixture!("fm_nested_map"),
    fixture!("fm_block_seq_scalars"),
    fixture!("fm_block_seq_maps"),
    fixture!("fm_comments"),
    fixture!("fm_quotes"),
    fixture!("fm_indent_errors"),
    fixture!("fm_block_scalars"),
    fixture!("fm_fence_inside_block_scalar"),
];

#[test]
fn dash_at_a_mapping_indent_is_rejected_rather_than_silently_dropped() {
    // A `- item` line at an indent whose open block is a MAPPING has no
    // reading: `map_entries_from_mapping` skips sequence items, so accepting
    // it would drop the content with no diagnostic. It must stay malformed,
    // as it was before block sequences existed.
    for source in [
        "---\n- item\ntype: uml.Class\n---\n",
        "---\nk:\n  a: 1\n  - item\n---\n",
    ] {
        let shell = parse(source);
        let codes: Vec<_> = shell
            .tree
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(
            codes.contains(&OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry),
            "dash at a mapping indent must be malformed: {source:?} -> {codes:?}"
        );
        assert!(
            codes.contains(&OkfSyntaxDiagnosticCode::FrontmatterNotClean),
            "dash at a mapping indent must be unclean: {source:?} -> {codes:?}"
        );
    }
}

#[test]
fn block_sequence_at_its_keys_own_indent_parses_clean_and_lossless() {
    for source in [
        "---\ntags:\n- a\n- b\ntitle: T\n---\n",
        "---\nmeta:\n  tags:\n  - a\n  owner: ana\n---\n",
        "---\nauthors:\n- name: Ana\n  team: platform\n---\n",
    ] {
        let shell = parse(source);
        assert_eq!(shell.tree.write_to_string(), source);
        let codes: Vec<_> = shell
            .tree
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.is_empty(), "{source:?} -> {codes:?}");
    }
}

#[test]
fn preserves_bom_crlf_unicode_frontmatter_and_top_level_headings() {
    let source = "\u{feff}---\r\ntype: arbitrary.\u{1d11e}\r\n---\r\n# Title  \r\n## Section\r\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

    assert_shell_invariants("bom_crlf_unicode", source, &shell);
    assert_eq!(shell.structure.headings.len(), 2);
    assert_eq!(shell.structure.headings[0].level, 1);
    assert_eq!(shell.structure.headings[1].level, 2);
    assert!(leaf_tokens(&shell.tree.root())
        .iter()
        .filter(|token| token.kind() == OkfMarkdownSyntaxKind::NewlineToken)
        .all(|token| token.text().write_to_string() == "\r\n"));
}

#[test]
fn malformed_and_unclosed_frontmatter_recovers_without_losing_bytes() {
    let source = "---\r\ntype: arbitrary\r\nname: \u{1d11e}\r\n# Recovered\r\n## Child\r\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

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
fn block_scalar_header_with_trailing_junk_stays_a_plain_value() {
    // `|` followed by whitespace and a non-comment character is not a valid
    // block scalar header; the value must stay a plain scalar and the tree
    // must keep every byte.
    let source = "---\ntype: |\t\u{a1}\n---\n# Unclaimed\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();
    assert_shell_invariants("block_scalar_trailing_junk", source, &shell);
    assert!(!leaf_tokens(&shell.tree.root())
        .iter()
        .any(|token| token.kind() == OkfMarkdownSyntaxKind::FrontmatterBlockScalarHeaderToken));
}

#[test]
fn thematic_rule_without_plausible_frontmatter_stays_markdown() {
    let source = "---\nnot a key value\n\n# Real title\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();
    assert_eq!(shell.tree.write_to_string(), source);
    assert!(shell.tree.diagnostics().is_empty());
}

#[test]
fn keeps_headings_in_protected_containers_raw() {
    let source = "> # quote\n\n- ## list\n\n```md\n# fence\n```\n\n<!-- # comment -->\n\n# top\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert_eq!(shell.structure.headings.len(), 1);
    assert_eq!(shell.structure.headings[0].level, 1);
}

#[test]
fn unclosed_frontmatter_uses_plausible_h2_fallback() {
    let source = "---\ntype: arbitrary\n## Section\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    assert_eq!(
        shell.tree.root().child_at(0).unwrap().kind(),
        OkfMarkdownSyntaxKind::Frontmatter
    );
    assert_eq!(shell.structure.headings.len(), 1);
    assert_eq!(shell.structure.headings[0].level, 2);
}

#[test]
fn unclosed_frontmatter_h2_fallback_rejects_implausible_candidates() {
    let source = "---\ntype arbitrary\n## Section\n";
    let shell = parse(source);
    assert_eq!(shell.tree.write_to_string(), source);
    assert!(shell.tree.diagnostics().is_empty());
    assert_ne!(
        shell.tree.root().child_at(0).unwrap().kind(),
        OkfMarkdownSyntaxKind::Frontmatter
    );
}

#[test]
fn shell_tokens_apply_normative_trivia_ownership() {
    let source = "---  \n type :  arbitrary  \n---\n#  Title  \n   ";
    let shell = parse(source);
    let tokens = leaf_tokens(&shell.tree.root());
    let kinds: Vec<_> = tokens.iter().map(|token| token.kind()).collect();
    assert!(kinds.contains(&OkfMarkdownSyntaxKind::NewlineToken));
    let eof = tokens.last().unwrap();
    assert_eq!(eof.kind(), OkfMarkdownSyntaxKind::EndOfFileToken);
    assert!(eof.flags().is_missing());
    assert_eq!(
        eof.leading_trivia()
            .iter()
            .map(|t| t.text.write_to_string())
            .collect::<String>(),
        "   "
    );
    assert!(tokens
        .iter()
        .all(|token| token.trailing_trivia().is_empty()));
    assert!(tokens
        .iter()
        .filter(|token| token.kind() == OkfMarkdownSyntaxKind::NewlineToken)
        .any(|token| token
            .leading_trivia()
            .iter()
            .map(|t| t.text.write_to_string())
            .collect::<String>()
            == "  "));
}

#[test]
fn structure_distinguishes_list_item_lines_from_nested_fenced_bullets() {
    let source = "## Attributes\n- real: Good [1]\n\n  ```text\n  - fenced: Bad [1]\n  ```\n";
    let shell = parse(source);
    let lines: Vec<_> = shell
        .structure
        .list_item_lines
        .iter()
        .map(|range| &source[range.start().to_usize()..range.end().to_usize()])
        .collect();
    assert_eq!(lines, ["- real: Good [1]\n"]);
    assert!(shell
        .structure
        .protected_ranges
        .iter()
        .any(
            |range| range.start().to_usize() <= source.find("- fenced").unwrap()
                && range.end().to_usize() > source.find("- fenced").unwrap()
        ));
    assert!(shell
        .structure
        .opaque_ranges
        .iter()
        .any(
            |range| range.start().to_usize() <= source.find("- fenced").unwrap()
                && range.end().to_usize() > source.find("- fenced").unwrap()
        ));
}

#[test]
fn structure_exposes_tab_indented_item_lines_separately_from_commonmark_items() {
    let source = "## Attributes\n\t- tabbed: Good [1]\n";
    let shell = parse(source);
    assert!(shell.structure.list_item_lines.is_empty());
    let lines: Vec<_> = shell
        .structure
        .tab_indented_item_lines
        .iter()
        .map(|range| &source[range.start().to_usize()..range.end().to_usize()])
        .collect();
    assert_eq!(lines, ["\t- tabbed: Good [1]\n"]);
}

#[test]
fn shell_fixtures_are_exact_bounded_progressing_and_golden() {
    let mut actuals = Vec::new();
    for fixture in FIXTURES {
        let source = fixture_source(fixture);
        let shell = parse(&source);
        assert_shell_invariants(fixture.name, &source, &shell);
        assert_fixture_shape(fixture.name, &source, &shell);
        actuals.push((fixture, golden(&shell)));
    }
    if let Ok(filter) = std::env::var("WAML_DUMP_SHELL_GOLDENS") {
        for (fixture, actual) in &actuals {
            if !fixture.name.contains(&filter) {
                continue;
            }
            println!(
                "@@GOLDEN:{}@@\n{}@@END:{}@@",
                fixture.name, actual, fixture.name
            );
        }
    }
    for (fixture, actual) in actuals {
        assert_eq!(actual, fixture.golden, "golden mismatch: {}", fixture.name);
    }
}

fn assert_fixture_shape(name: &str, source: &str, shell: &ShellParse) {
    let codes: Vec<_> = shell
        .tree
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    match name {
        "clean_lf" | "missing_type" | "unknown_type" => {
            assert!(
                codes.is_empty(),
                "clean arbitrary/missing/unknown type: {name}"
            );
        }
        "malformed_clean" => assert_eq!(
            codes,
            vec![
                OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry,
                OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            ]
        ),
        "unclosed_h2" => assert_eq!(
            codes,
            vec![
                OkfSyntaxDiagnosticCode::MissingFrontmatterFence,
                OkfSyntaxDiagnosticCode::FrontmatterNotClean,
            ]
        ),
        "unclosed_false_positive" => assert!(codes.is_empty()),
        "later_thematic_rule" => assert_eq!(shell.structure.headings.len(), 2),
        "protected_containers" => {
            assert_eq!(shell.structure.headings.len(), 1);
            for marker in [
                "# quote",
                "## unordered item",
                "# ordered item",
                "# fenced code",
                "## indented code",
                "# html block",
                "# table",
                "## cell",
                "# footnote",
                "## definition",
            ] {
                let offset = TextSize::try_from_usize(source.find(marker).unwrap()).unwrap();
                assert!(
                    shell
                        .structure
                        .protected_ranges
                        .iter()
                        .any(|range| range.contains(offset)),
                    "{marker} is protected"
                );
            }
        }
        "lower_headings" => {
            assert_eq!(shell.structure.headings.len(), 1);
            assert_eq!(shell.structure.nested_headings.len(), 4);
            assert_eq!(
                shell
                    .structure
                    .nested_headings
                    .iter()
                    .map(|heading| heading.level)
                    .collect::<Vec<_>>(),
                [3, 4, 5, 6]
            );
        }
        "html_comment" => assert_eq!(shell.structure.headings.len(), 1),
        "heading_eof_spaces"
        | "closed_frontmatter_eof_spaces"
        | "unclosed_frontmatter_eof_spaces" => {
            let eof = leaf_tokens(&shell.tree.root()).pop().unwrap();
            assert_eq!(eof.kind(), OkfMarkdownSyntaxKind::EndOfFileToken);
            assert!(eof.flags().is_missing());
            assert_eq!(
                eof.leading_trivia()
                    .iter()
                    .map(|trivia| trivia.text.write_to_string())
                    .collect::<String>(),
                "   "
            );
        }
        "fm_nested_map" | "fm_block_seq_scalars" | "fm_block_seq_maps" | "fm_comments" => {
            assert!(codes.is_empty(), "clean nested frontmatter: {name}");
        }
        "fm_quotes" => {
            assert!(
                codes.contains(&OkfSyntaxDiagnosticCode::UnterminatedQuotedScalar),
                "unterminated quote: {name}"
            );
            assert!(
                codes.contains(&OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry),
                "mapping-lookalike bare value: {name}"
            );
        }
        "fm_indent_errors" => {
            assert!(
                codes.contains(&OkfSyntaxDiagnosticCode::TabInFrontmatterIndent),
                "tab indent: {name}"
            );
            assert!(
                codes.contains(&OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent),
                "invalid indent: {name}"
            );
        }
        "fm_block_scalars" | "fm_fence_inside_block_scalar" => {
            assert!(codes.is_empty(), "clean block-scalar frontmatter: {name}");
        }
        _ => unreachable!("fixture table is exhaustive"),
    }
}

fn fixture_source(fixture: &Fixture) -> String {
    if !fixture.escaped {
        return fixture.source.to_owned();
    }
    let encoded = fixture.source.strip_suffix('\n').unwrap_or(fixture.source);
    let mut decoded = String::new();
    let mut chars = encoded.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        match chars.next().expect("fixture escape") {
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'x' => {
                let digits = [chars.next().unwrap(), chars.next().unwrap()];
                let byte = u8::from_str_radix(&digits.iter().collect::<String>(), 16).unwrap();
                decoded.push(char::from(byte));
            }
            escaped => panic!("unsupported fixture escape: {escaped}"),
        }
    }
    decoded
}

fn parse(source: &str) -> crate::ShellParse {
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap()
}

fn leaf_tokens(
    node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
) -> Vec<crate::SyntaxToken<crate::OkfMarkdownLanguage>> {
    fn visit(
        node: &crate::SyntaxNode<crate::OkfMarkdownLanguage>,
        out: &mut Vec<crate::SyntaxToken<crate::OkfMarkdownLanguage>>,
    ) {
        for child in node.children() {
            match child {
                SyntaxElement::Node(node) => visit(&node, out),
                SyntaxElement::Token(token) => out.push(token),
            }
        }
    }
    let mut out = Vec::new();
    visit(node, &mut out);
    out
}

fn assert_shell_invariants(name: &str, source: &str, shell: &ShellParse) {
    assert_eq!(shell.tree.write_to_string(), source, "roundtrip: {name}");
    assert_eq!(
        shell.tree.root().range().end().to_usize(),
        source.len(),
        "root range: {name}"
    );
    let leaves = leaf_tokens(&shell.tree.root());
    assert!(!leaves.is_empty(), "parser progress: {name}");
    let leaf_text = leaves
        .iter()
        .flat_map(|token| {
            token
                .leading_trivia()
                .iter()
                .map(|trivia| trivia.text.write_to_string())
                .chain(std::iter::once(token.text().write_to_string()))
                .chain(
                    token
                        .trailing_trivia()
                        .iter()
                        .map(|trivia| trivia.text.write_to_string()),
                )
        })
        .collect::<String>();
    assert_eq!(leaf_text, source, "leaf bytes exactly once: {name}");
    assert!(
        leaves
            .iter()
            .all(|token| token.range().end().to_usize() <= source.len()),
        "leaf bounds: {name}"
    );
    assert_red_ranges(&shell.tree.root(), source.len(), name);
    assert_node_widths(shell.tree.root_green(), name);
    assert!(
        shell
            .structure
            .headings
            .iter()
            .all(|h| h.range.end().to_usize() <= source.len()
                && h.text_range.end().to_usize() <= source.len()),
        "heading bounds: {name}"
    );
    assert!(
        shell
            .structure
            .nested_headings
            .iter()
            .all(|h| h.range.end().to_usize() <= source.len()
                && h.text_range.end().to_usize() <= source.len()),
        "nested heading bounds: {name}"
    );
    assert!(
        shell
            .structure
            .protected_ranges
            .windows(2)
            .all(|pair| pair[0].end() <= pair[1].start()),
        "protected ranges sorted/non-overlapping: {name}"
    );
    assert!(
        shell
            .structure
            .list_item_lines
            .iter()
            .all(|range| range.start() <= range.end() && range.end().to_usize() <= source.len()),
        "list item line bounds: {name}"
    );
    assert!(
        shell
            .structure
            .list_item_lines
            .windows(2)
            .all(|pair| pair[0].end() <= pair[1].start()),
        "list item lines sorted/non-overlapping: {name}"
    );
    assert!(
        shell
            .structure
            .opaque_ranges
            .windows(2)
            .all(|pair| pair[0].end() <= pair[1].start()),
        "opaque ranges sorted/non-overlapping: {name}"
    );
    assert!(
        shell
            .structure
            .tab_indented_item_lines
            .windows(2)
            .all(|pair| pair[0].end() <= pair[1].start()),
        "tab-indented item lines sorted/non-overlapping: {name}"
    );
    assert!(
        shell
            .tree
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.range.end().to_usize() <= source.len()),
        "diagnostic bounds: {name}"
    );
}

fn assert_red_ranges(node: &SyntaxNode<OkfMarkdownLanguage>, source_len: usize, name: &str) {
    assert!(
        node.range().end().to_usize() <= source_len,
        "node bounds: {name}"
    );
    for child in node.children() {
        if let SyntaxElement::Node(node) = child {
            assert_red_ranges(&node, source_len, name);
        }
    }
}

fn assert_node_widths(node: &crate::GreenNode<OkfMarkdownLanguage>, name: &str) {
    let sum: usize = node
        .children()
        .iter()
        .map(|child| match child {
            GreenElement::Node(node) => {
                assert_node_widths(node, name);
                node.width().to_usize()
            }
            GreenElement::Token(token) => token.width().to_usize(),
        })
        .sum();
    assert_eq!(sum, node.width().to_usize(), "child widths: {name}");
}

fn golden(shell: &ShellParse) -> String {
    fn escape(text: &str) -> String {
        text.chars().flat_map(char::escape_default).collect()
    }
    fn walk(node: &SyntaxNode<OkfMarkdownLanguage>, path: &str, out: &mut String) {
        out.push_str(&format!(
            "N {path} {:?} {}..{}\n",
            node.kind(),
            node.range().start().to_usize(),
            node.range().end().to_usize()
        ));
        for (index, child) in node.children().enumerate() {
            let child_path = format!("{path}/{index}");
            match child {
                SyntaxElement::Node(node) => walk(&node, &child_path, out),
                SyntaxElement::Token(token) => {
                    let leading = token
                        .leading_trivia()
                        .iter()
                        .map(|t| t.text.write_to_string())
                        .collect::<String>();
                    let trailing = token
                        .trailing_trivia()
                        .iter()
                        .map(|t| t.text.write_to_string())
                        .collect::<String>();
                    out.push_str(&format!("T {child_path} {:?} {}..{} missing={} leading=\"{}\" text=\"{}\" trailing=\"{}\"\n", token.kind(), token.range().start().to_usize(), token.range().end().to_usize(), token.flags().is_missing(), escape(&leading), escape(&token.text().write_to_string()), escape(&trailing)));
                }
            }
        }
    }
    let mut output = String::new();
    walk(&shell.tree.root(), "root", &mut output);
    for diagnostic in shell.tree.diagnostics() {
        output.push_str(&format!(
            "D {:?} {}..{}\n",
            diagnostic.code,
            diagnostic.range.start().to_usize(),
            diagnostic.range.end().to_usize()
        ));
    }
    output
}

/// Frontmatter nesting is capped like markdown containers: without the cap a
/// document of progressively indented keys builds a tree deep enough that
/// walking or dropping it overflows the stack (an unrecoverable crash from
/// untrusted document content, cheaper still on wasm's 1MB stack).
#[test]
fn deeply_indented_frontmatter_is_capped_instead_of_overflowing_the_stack() {
    let mut source = String::from("---\n");
    for depth in 0..2000 {
        source.push_str(&"  ".repeat(depth));
        source.push_str("k:\n");
    }
    source.push_str("---\n");

    let text = SourceText::from_shared(Arc::new(source.as_str().into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert!(shell
        .tree
        .diagnostics()
        .iter()
        .any(|d| d.code == OkfSyntaxDiagnosticCode::InvalidFrontmatterIndent));
}

/// A quoted key and a bare key that DECODE to the same text are the same key:
/// the model's reader collapses them last-wins, so the parser must say so.
#[test]
fn quoted_and_bare_keys_that_decode_alike_report_a_duplicate() {
    let source = "---\n'a': 1\na: 2\n---\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    assert!(shell
        .tree
        .diagnostics()
        .iter()
        .any(|d| d.code == OkfSyntaxDiagnosticCode::DuplicateFrontmatterKey));
}

/// An unknown escape whose escaped character is multi-byte must still be
/// reported on character boundaries; a fixed two-byte span cuts a UTF-8
/// sequence in half and hands consumers a range they cannot slice with.
#[test]
fn an_invalid_escape_of_a_multibyte_character_reports_whole_characters() {
    let source = "---\ntype: \"a\\\u{10036}\"\n---\n";
    let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
    let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

    assert_eq!(shell.tree.write_to_string(), source);
    let escape = shell
        .tree
        .diagnostics()
        .iter()
        .find(|d| d.code == OkfSyntaxDiagnosticCode::InvalidEscapeSequence)
        .expect("unknown escape is reported");
    assert!(source.is_char_boundary(escape.range.start().to_usize()));
    assert!(source.is_char_boundary(escape.range.end().to_usize()));
    assert_eq!(
        &source[escape.range.start().to_usize()..escape.range.end().to_usize()],
        "\\\u{10036}"
    );
}

/// The slice scanner must never disagree with the frontmatter classifier: it
/// answers `Some` only for a canonical fence, and `None` — "parse the whole
/// document" — for every shape the two rules resolve differently.
#[test]
fn leading_frontmatter_slice_bails_out_where_the_classifier_could_disagree() {
    use crate::markdown::{has_leading_frontmatter_fence, leading_frontmatter_slice};

    assert_eq!(
        leading_frontmatter_slice("---\ntype: uml.Class\n---\n# Order\n"),
        Some("---\ntype: uml.Class\n---\n")
    );
    assert_eq!(
        leading_frontmatter_slice("---\ntype: uml.Class\n...\n# Order\n"),
        Some("---\ntype: uml.Class\n...\n")
    );
    assert_eq!(
        leading_frontmatter_slice("\u{feff}---\na: 1\n---\n"),
        Some("\u{feff}---\na: 1\n---\n")
    );

    for ambiguous in [
        "---   \na: 1\n---\n",              // trailing space on the open fence
        "---\na: 1\n---  \n",               // trailing space on the close fence
        "---\na: 1\n  ---\n",               // indented close fence
        "---\nnote: |\n  ---\na: 1\n---\n", // fence hidden in a block scalar
        "---\na: 1\n",                      // unclosed, classifier recovers it
        "# Order\n",                        // no frontmatter at all
    ] {
        assert_eq!(
            leading_frontmatter_slice(ambiguous),
            None,
            "expected a bail-out for {ambiguous:?}"
        );
    }

    assert!(has_leading_frontmatter_fence("---   \na: 1\n"));
    assert!(has_leading_frontmatter_fence("\u{feff}---\na: 1\n---\n"));
    assert!(!has_leading_frontmatter_fence("# Order\n"));
    assert!(!has_leading_frontmatter_fence("  ---\na: 1\n---\n"));
}

/// Collects every token of `kind` in the tree, in document order.
fn tokens_of_kind(
    node: &SyntaxNode<OkfMarkdownLanguage>,
    kind: OkfMarkdownSyntaxKind,
) -> Vec<String> {
    let mut out = Vec::new();
    for element in node.children() {
        match element {
            SyntaxElement::Node(child) => out.extend(tokens_of_kind(&child, kind)),
            SyntaxElement::Token(token) if token.kind() == kind => {
                out.push(token.text().write_to_string());
            }
            SyntaxElement::Token(_) => {}
        }
    }
    out
}

/// A `{...}` flow mapping is a VALUE, never a plain key: YAML forbids `{` at
/// the head of a plain scalar, so the key scan must stop there the way it
/// already does for `[`. Without the guard, `- { id: a, title: b }` split at
/// its first `: ` and read as a mapping keyed `{ id` — the frontmatter model
/// then lost the entry, and the formatter wrote the mangled split back into
/// the document.
#[test]
fn a_flow_mapping_is_a_value_not_a_plain_key() {
    for source in [
        "---\nsources:\n  - { id: a, title: b }\n---\n",
        "---\nmeta: { id: a, title: b }\n---\n",
    ] {
        let text = SourceText::from_shared(Arc::new(source.into())).unwrap();
        let shell = parse_okf_markdown(text, MarkdownDialect::WAML_DEFAULT).unwrap();

        assert_eq!(shell.tree.write_to_string(), source);
        assert!(
            shell.tree.diagnostics().is_empty(),
            "expected no diagnostics for {source:?}, got {:?}",
            shell.tree.diagnostics()
        );
        let keys = tokens_of_kind(&shell.tree.root(), OkfMarkdownSyntaxKind::FrontmatterKey);
        assert!(
            keys.iter().all(|key| !key.contains('{')),
            "a flow mapping opened a key in {source:?}: {keys:?}"
        );
    }
}
