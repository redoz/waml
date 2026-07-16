//! Load an OKF directory into a `waml::model::Model`.

use std::path::Path;

/// Walk `dir` recursively, returning `(rel_path, contents)` for every `*.md`
/// file, sorted by path. Paths use forward slashes so keys match `build_model`.
pub fn read_bundle(dir: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    collect(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// Load an OKF directory into a resolved `Model`.
pub fn load_model(dir: &Path) -> std::io::Result<waml::model::Model> {
    let bundle = read_bundle(dir)?;
    Ok(waml::parse::build_model(&bundle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini")
    }

    #[test]
    fn read_bundle_returns_sorted_md_pairs() {
        let bundle = read_bundle(&fixture_dir()).unwrap();
        let paths: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            ["customer.md", "index.md", "order.md", "orders-diagram.md"]
        );
        // Contents are the raw file text.
        let order = bundle.iter().find(|(p, _)| p == "order.md").unwrap();
        assert!(order.1.contains("title: Order"));
    }

    #[test]
    fn load_model_builds_two_nodes_one_diagram() {
        let model = load_model(&fixture_dir()).unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.diagrams.len(), 1);
        assert_eq!(model.edges.len(), 1);
    }
}
