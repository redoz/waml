//! The nav seam: project a `Model` + `NavState` into a `NavView` the tree panel
//! renders. Pure — no makepad, no `Cx` — and unit-tested like `tree.rs`. Sits on
//! top of `tree::build_tree`; clean-room (not a port of the web navigator).

// The public surface here is exercised only by its own unit tests until the
// tree panel / app wiring lands (later tasks of the same plan); until then a
// bin crate's dead-code lint would otherwise flag every item. Same convention
// as `icons.rs`'s catalog surface.
#![allow(dead_code)]

use crate::tree::{build_tree, ProjectTree, TreeKind, TreeNode};
use waml::model::Model;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NavState {
    /// Package key; `""` = whole-model scope.
    pub scope: String,
    /// Search text; `""` = browse (never a search state).
    pub query: String,
    /// `None` = All.
    pub filter: Option<TreeKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavView {
    /// Scoped subtree, type-filtered, no query.
    Browse(ProjectTree),
    /// Query matches inside scope (matches + their ancestor packages).
    Results(ProjectTree),
    /// No scope match; whole-model matches, shown under a note.
    Elsewhere(ProjectTree),
    /// Nothing matches anywhere.
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackageRow {
    pub key: String,
    pub title: String,
    pub depth: usize,
}

/// De-prefixed display name for a kind (drives the type-filter chip label and
/// any kind-labelled UI). `Unknown` reads as "Other".
pub fn kind_label(kind: TreeKind) -> &'static str {
    match kind {
        TreeKind::Package => "Package",
        TreeKind::Class => "Class",
        TreeKind::Interface => "Interface",
        TreeKind::Enum => "Enum",
        TreeKind::DataType => "DataType",
        TreeKind::Diagram => "Diagram",
        TreeKind::Behavior => "Behavior",
        TreeKind::Sequence => "Sequence",
        TreeKind::Note => "Note",
        TreeKind::Unknown => "Other",
    }
}

/// The type-filter chip's current label: `All` for no filter, else the kind.
pub fn chip_label(filter: Option<TreeKind>) -> &'static str {
    match filter {
        None => "All",
        Some(k) => kind_label(k),
    }
}

/// Canonical kind order (matches `TreeKind`'s declaration), used to give
/// `kinds_in_model` a stable, model-independent ordering.
const KIND_ORDER: [TreeKind; 10] = [
    TreeKind::Package,
    TreeKind::Class,
    TreeKind::Interface,
    TreeKind::Enum,
    TreeKind::DataType,
    TreeKind::Diagram,
    TreeKind::Behavior,
    TreeKind::Sequence,
    TreeKind::Note,
    TreeKind::Unknown,
];

/// The distinct `TreeKind`s present anywhere in the model, in canonical order.
/// Drives the type-filter chip's cycle; compute once on Model load, not per
/// keystroke.
pub fn kinds_in_model(model: &Model) -> Vec<TreeKind> {
    let full = build_tree(model, "Untitled");
    let mut present: Vec<TreeKind> = Vec::new();
    fn walk(nodes: &[TreeNode], present: &mut Vec<TreeKind>) {
        for n in nodes {
            if !present.contains(&n.kind) {
                present.push(n.kind);
            }
            walk(&n.children, present);
        }
    }
    walk(&full.roots, &mut present);
    KIND_ORDER
        .iter()
        .copied()
        .filter(|k| present.contains(k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;
    use waml::model::{ElementType, Model, Node, UmlMetaclass};
    use waml::okf::Concept;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    fn concept(title: &str) -> Concept {
        Concept {
            id: String::new(),
            ty: String::new(),
            title: Some(title.to_string()),
            description: None,
            resource: None,
            tags: vec![],
            timestamp: None,
            body: String::new(),
            links: vec![],
            citations: vec![],
            role: Default::default(),
            extra: Default::default(),
        }
    }

    fn node(key: &str, ty: ElementType, title: &str, members: Vec<&str>) -> Node {
        Node {
            concept: concept(title),
            key: key.to_string(),
            ty,
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
            note_body: None,
            annotates: vec![],
            members: members.iter().map(|s| s.to_string()).collect(),
            slots: vec![],
        }
    }

    /// A small hand-built model: root package -> [sub package -> [Cls class],
    /// Iface interface]. Reused across nav tests.
    fn built() -> Model {
        Model {
            path: "Root".to_string(),
            packages: vec![
                node(
                    "",
                    ElementType::Uml(UmlMetaclass::Package),
                    "Root",
                    vec!["sub", "iface"],
                ),
                node(
                    "sub",
                    ElementType::Uml(UmlMetaclass::Package),
                    "Sub Pkg",
                    vec!["cls"],
                ),
            ],
            nodes: vec![
                node(
                    "cls",
                    ElementType::Uml(UmlMetaclass::Class),
                    "Customer",
                    vec![],
                ),
                node(
                    "iface",
                    ElementType::Uml(UmlMetaclass::Interface),
                    "Payments",
                    vec![],
                ),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn chip_label_is_all_when_unfiltered_else_the_kind() {
        assert_eq!(chip_label(None), "All");
        assert_eq!(chip_label(Some(TreeKind::Class)), "Class");
        assert_eq!(chip_label(Some(TreeKind::Package)), "Package");
    }

    #[test]
    fn kinds_in_model_is_distinct_and_canonically_ordered() {
        let kinds = kinds_in_model(&built());
        // Present: Package (root+sub), Class (cls), Interface (iface). Canonical
        // order puts Package before Class before Interface; no dupes.
        assert_eq!(
            kinds,
            vec![TreeKind::Package, TreeKind::Class, TreeKind::Interface]
        );
    }

    #[test]
    fn kinds_in_model_covers_the_mini_fixture_without_unknown_leak() {
        let kinds = kinds_in_model(&mini());
        assert!(kinds.contains(&TreeKind::Package));
        assert!(kinds.contains(&TreeKind::Diagram));
        assert!(!kinds.contains(&TreeKind::Unknown));
        // Canonical order: every entry's index in KIND_ORDER strictly increases.
        let idx = |k: &TreeKind| KIND_ORDER.iter().position(|x| x == k).unwrap();
        assert!(kinds.windows(2).all(|w| idx(&w[0]) < idx(&w[1])));
    }
}
