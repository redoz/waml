//! Load an OKF directory into its source-authoritative bundle.

use std::path::Path;
use waml::source::{SourceBundle, SourceError};

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Source(SourceError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(error) => error.fmt(f),
            LoadError::Source(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(error: std::io::Error) -> Self {
        LoadError::Io(error)
    }
}

impl From<SourceError> for LoadError {
    fn from(error: SourceError) -> Self {
        LoadError::Source(error)
    }
}

/// Walk `dir` recursively, returning `(rel_path, contents)` for every `*.md`
/// file, sorted by path. Paths use forward slashes so keys match `build_model`.
pub fn read_bundle(dir: &Path) -> Result<SourceBundle, LoadError> {
    let mut out = Vec::new();
    collect(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(SourceBundle::try_from_pairs(out)?)
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

/// Load an OKF directory into a resolved `Model`. Test-only convenience: the
/// app path now uses `load_bundle_and_model` (it retains the bundle for
/// drag-to-place write-back); tests that only need a `Model` still use this.
#[cfg(test)]
pub fn load_model(dir: &Path) -> Result<waml::uml::Projection, LoadError> {
    let bundle = read_bundle(dir)?;
    Ok(waml::parse::build_model_from_source(&bundle))
}

/// Return the raw markdown of the bundle file whose OKF id equals `key`. A
/// classifier's node key is exactly [`waml::okf::id_of`] of its source path
/// (the forward-slash-normalized bundle-relative path minus the trailing
/// `.md`), so the match is on the whole path -- a nested `shop/order.md` is
/// keyed `shop/order`, not the bare `order`, and duplicate basenames in
/// different directories stay distinct. `None` when no file matches. Bundle
/// paths are unique, so at most one file can match.
pub fn source_for<'a>(bundle: &'a SourceBundle, key: &str) -> Option<&'a str> {
    bundle
        .document_by_concept_id(key)
        .map(|document| document.text())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini")
    }

    fn named_fixture_dir(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn read_bundle_returns_sorted_md_pairs() {
        let bundle = read_bundle(&fixture_dir()).unwrap();
        let paths: Vec<&str> = bundle
            .documents()
            .iter()
            .map(|document| document.path().as_str())
            .collect();
        assert_eq!(
            paths,
            [
                "customer.md",
                "index.md",
                "order.md",
                "orders-diagram.md",
                "payment-gateway.md"
            ]
        );
        // Contents are the raw file text.
        let order = bundle.document_by_concept_id("order").unwrap();
        assert!(order.text().contains("title: Order"));
    }

    #[test]
    fn load_model_builds_two_nodes_one_diagram() {
        let model = load_model(&fixture_dir()).unwrap();
        // Order, Customer, and the U9 PaymentGateway interface (kind-styling fixture).
        assert_eq!(model.nodes.len(), 3);
        assert_eq!(model.diagrams.len(), 1);
        assert_eq!(model.edges.len(), 1);
    }

    #[test]
    fn mixed_fixture_loads_uml_and_generic_okf_concepts_together() {
        let source = read_bundle(&named_fixture_dir("mixed-okf")).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert!(prepared.okf().bundle.concept("runbook").is_some());
        assert!(prepared
            .uml()
            .projection
            .nodes
            .iter()
            .any(|node| node.key == "order"));
        assert!(prepared
            .uml()
            .projection
            .diagrams
            .iter()
            .any(|diagram| diagram.key == "orders-diagram"));
    }

    #[test]
    fn okf_only_fixture_loads_with_an_empty_uml_projection() {
        let source = read_bundle(&named_fixture_dir("okf-only")).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        assert!(prepared.okf().bundle.concept("notes").is_some());
        assert!(prepared.uml().projection.nodes.is_empty());
        assert!(prepared.uml().projection.diagrams.is_empty());
    }

    /// The `sixkind` fixture is the visual-regression bench for terminal
    /// adornments: one `Car` node wired to six targets, one edge per standard
    /// UML relationship kind, so every `end_marker` glyph is exercised in a
    /// single diagram. Guard that all six kinds resolve.
    #[test]
    fn sixkind_fixture_resolves_all_relationship_kinds() {
        use waml::model::RelationshipKind as RK;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sixkind");
        let model = load_model(&dir).unwrap();
        assert_eq!(model.edges.len(), 6);
        let kinds: Vec<RK> = model.edges.iter().map(|e| e.kind).collect();
        for k in [
            RK::Specializes,
            RK::Implements,
            RK::Depends,
            RK::Associates,
            RK::Aggregates,
            RK::Composes,
        ] {
            assert!(kinds.contains(&k), "sixkind fixture missing {k:?}");
        }
    }

    #[test]
    fn source_for_matches_top_level_slug() {
        let bundle = SourceBundle::try_from_pairs([
            ("order.md".to_string(), "# Order\nbody".to_string()),
            ("customer.md".to_string(), "# Customer".to_string()),
        ])
        .unwrap();
        assert_eq!(source_for(&bundle, "order"), Some("# Order\nbody"));
    }

    #[test]
    fn source_for_matches_nested_key_by_full_id() {
        // The key is the full OKF id (`shop/order`), not the bare basename --
        // a bare `order` must NOT match a nested `shop/order.md`.
        let bundle =
            SourceBundle::try_from_pairs([("shop/order.md".to_string(), "# Order".to_string())])
                .unwrap();
        assert_eq!(source_for(&bundle, "shop/order"), Some("# Order"));
        assert_eq!(source_for(&bundle, "order"), None);
    }

    #[test]
    fn source_for_disambiguates_duplicate_basenames_by_dir() {
        // Same basename in two packages: the full-id match returns the file in
        // the requested directory, never the first basename hit.
        let bundle = SourceBundle::try_from_pairs([
            ("shop/order.md".to_string(), "# Shop order".to_string()),
            (
                "warehouse/order.md".to_string(),
                "# Warehouse order".to_string(),
            ),
        ])
        .unwrap();
        assert_eq!(source_for(&bundle, "shop/order"), Some("# Shop order"));
        assert_eq!(
            source_for(&bundle, "warehouse/order"),
            Some("# Warehouse order")
        );
    }

    #[test]
    fn source_for_returns_none_when_absent() {
        let bundle =
            SourceBundle::try_from_pairs([("order.md".to_string(), "# Order".to_string())])
                .unwrap();
        assert_eq!(source_for(&bundle, "invoice"), None);
    }
}
