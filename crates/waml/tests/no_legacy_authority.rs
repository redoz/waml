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
