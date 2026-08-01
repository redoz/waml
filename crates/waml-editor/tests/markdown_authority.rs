use std::{fs, path::Path};

fn production_rust_sources(root: &Path) -> String {
    let mut paths = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
        .map(|entry| entry.expect("production source directory entry"))
        .collect::<Vec<_>>();
    paths.sort_by_key(|entry| entry.path());

    let mut source = String::new();
    for entry in paths {
        let path = entry.path();
        if path.is_dir() {
            source.push_str(&production_rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            source.push_str(&format!("\n// FILE: {}\n", path.display()));
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            source.push_str(contents.split("#[cfg(test)]").next().unwrap_or(&contents));
        }
    }
    source
}

#[test]
fn production_editor_has_one_markdown_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source = production_rust_sources(&root);
    for forbidden in [
        "MarkdownRef",
        "MarkdownAction",
        "as_markdown()",
        "makepad_widgets::Markdown",
        "pulldown_cmark::Parser",
        "regex::Regex",
    ] {
        assert!(
            !source.contains(forbidden),
            "production editor still contains forbidden authority: {forbidden}"
        );
    }
    assert!(!root.join("markdown_surface.rs").exists());
}
