use std::path::Path;

#[test]
fn low_level_shell_parser_is_not_a_public_authority() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!manifest.join("src/shell/parser.rs").exists());

    let public_surface = include_str!("../src/lib.rs");
    for forbidden in [
        "parse_okf_markdown",
        "reparse_okf_markdown,",
        "reparse_okf_markdown_with_structure",
    ] {
        assert!(
            !public_surface.contains(forbidden),
            "low-level Markdown parser remains public: {forbidden}"
        );
    }
}
