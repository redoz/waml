use std::fs;
use std::path::PathBuf;

#[test]
fn legacy_parser_and_serializer_authority_is_absent() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();

    for module in ["grammar", "parse", "syntax", "serialize"] {
        let path = manifest.join("src").join(format!("{module}.rs"));
        if path.exists() {
            violations.push(format!("legacy authority still exists: {}", path.display()));
        }
    }

    let lib = fs::read_to_string(manifest.join("src/lib.rs")).expect("read src/lib.rs");
    for export in [
        "pub mod grammar;",
        "pub mod parse;",
        "pub mod syntax;",
        "pub mod serialize;",
    ] {
        if lib.contains(export) {
            violations.push(format!("legacy authority still exported: {export}"));
        }
    }

    assert!(
        violations.is_empty(),
        "legacy parser/serializer authority must be retired:\n{}",
        violations.join("\n")
    );
}

#[test]
fn handwritten_shadow_parsers_and_renderers_are_absent() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let prohibited = [
        (
            "src/frontmatter.rs",
            [
                "pub struct ParsedFrontmatter",
                "pub fn parse_frontmatter_spanned",
                "pub fn parse_frontmatter(",
            ]
            .as_slice(),
        ),
        (
            "src/layout.rs",
            [
                "pub fn render_layout_line",
                "pub fn parse_layout_line",
                "pub fn parse_layout_body",
            ]
            .as_slice(),
        ),
        (
            "src/uml/lower.rs",
            [
                "fn parse_attribute(",
                "fn render_attribute(",
                "fn parse_relationship(",
                "fn render_relationship(",
            ]
            .as_slice(),
        ),
    ];
    let mut violations = Vec::new();
    for (relative, symbols) in prohibited {
        let source = fs::read_to_string(manifest.join(relative)).expect("read authority source");
        for symbol in symbols {
            if source.contains(symbol) {
                violations.push(format!("{relative}: prohibited `{symbol}`"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "shadow parser/renderer authority remains:\n{}",
        violations.join("\n")
    );
}
