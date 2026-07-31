use std::{fs, path::PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn provenance_names_every_adapted_module_and_exact_upstream_revision() {
    let text = fs::read_to_string(crate_root().join("PROVENANCE.md")).unwrap();
    assert!(text.contains("https://github.com/redoz/makepad.git"));
    assert!(text.contains("c38f529984eda61e258ca69fb50c6712d85c74c1"));
    assert!(text.contains("MIT License"));
    for module in ["selection.rs", "history.rs", "input.rs", "widget.rs"] {
        assert!(text.contains(module), "missing provenance for {module}");
    }
}

#[test]
fn crate_does_not_depend_on_upstream_editor_or_markdown_widgets() {
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml")).unwrap();
    let source = [
        "src/lib.rs",
        "src/document.rs",
        "src/selection.rs",
        "src/edit.rs",
        "src/history.rs",
        "src/unicode.rs",
        "src/ime.rs",
        "src/session.rs",
        "src/input.rs",
        "src/widget.rs",
        "src/layout/mod.rs",
        "src/layout/geometry.rs",
        "src/layout/engine.rs",
        "src/layout/makepad.rs",
    ]
    .into_iter()
    .map(|path| fs::read_to_string(crate_root().join(path)).unwrap())
    .collect::<String>();
    assert!(!manifest.contains("makepad-code-editor"));
    assert!(!source.contains("makepad_code_editor"));
    assert!(!source.contains("CodeEditor"));
    assert!(!source.contains("widgets::Markdown"));
    assert!(!source.contains("MarkdownAction"));
    assert!(!source.contains("as_markdown()"));
}
