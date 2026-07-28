use std::fs;
use std::path::{Path, PathBuf};

fn rust_sources(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("read source directory") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

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
        (
            "src/uml.rs",
            ["pub fn project(bundle: &crate::okf::Bundle)"].as_slice(),
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
    let syntax_authority = manifest.join("src/uml/syntax");
    let mut sources = Vec::new();
    rust_sources(&manifest.join("src"), &mut sources);
    for path in sources {
        if path.starts_with(&syntax_authority) {
            continue;
        }
        let relative = path
            .strip_prefix(&manifest)
            .expect("manifest-relative path");
        if !relative.starts_with(Path::new("src/uml"))
            && relative != Path::new("src/layout.rs")
            && relative != Path::new("src/frontmatter.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read production Rust source");
        for line in source.lines() {
            let trimmed = line.trim_start();
            let grammar_entry = [
                "parse_layout",
                "render_layout",
                "parse_attribute",
                "render_attribute",
                "parse_relationship",
                "render_relationship",
                "parse_member",
                "render_member",
                "parse_flow",
                "render_flow",
                "parse_lifeline",
                "render_lifeline",
                "parse_message",
                "render_message",
            ]
            .iter()
            .find(|stem| {
                (trimmed.starts_with("fn ")
                    || trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub(crate) fn "))
                    && trimmed.contains(*stem)
            });
            if let Some(stem) = grammar_entry {
                violations.push(format!(
                    "{}: grammar-shaped `{stem}` entry point outside uml::syntax: {trimmed}",
                    relative.display()
                ));
            }
        }
        if relative == Path::new("src/uml/analysis.rs") {
            for forbidden in ["typed_atoms()", "struct LayoutCursor", "LayoutAtomSyntax"] {
                if source.contains(forbidden) {
                    violations.push(format!(
                        "{}: layout lowering reconstructs grammar through `{forbidden}`",
                        relative.display()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "shadow parser/renderer authority remains:\n{}",
        violations.join("\n")
    );
}
