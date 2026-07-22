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

/// Nested package-only rows for the title dropdown, depth-indented. Row 0 is the
/// synthetic root (whole-model scope, key `""`); real sub-packages follow. The
/// `build_tree` root (key `""`) IS a package, so it is skipped here and replaced
/// by the synthetic row, then its children are recursed for real packages.
pub fn packages(model: &Model) -> Vec<PackageRow> {
    let full = build_tree(model, "Untitled");
    let root_title = if model.path.is_empty() {
        "Untitled".to_string()
    } else {
        model.path.clone()
    };
    let mut out = vec![PackageRow {
        key: String::new(),
        title: root_title,
        depth: 0,
    }];
    fn walk(nodes: &[TreeNode], depth: usize, out: &mut Vec<PackageRow>) {
        for n in nodes {
            if n.kind == TreeKind::Package {
                out.push(PackageRow {
                    key: n.key.clone(),
                    title: n.title.clone(),
                    depth,
                });
                walk(&n.children, depth + 1, out);
            }
        }
    }
    if let Some(root) = full.roots.first() {
        walk(&root.children, 1, &mut out);
    }
    out
}

/// Find the node with `key` anywhere in `nodes` (depth-first). The `build_tree`
/// root has key `""`, so `find_node(roots, "")` returns the synthetic root.
fn find_node<'a>(nodes: &'a [TreeNode], key: &str) -> Option<&'a TreeNode> {
    for n in nodes {
        if n.key == key {
            return Some(n);
        }
        if let Some(found) = find_node(&n.children, key) {
            return Some(found);
        }
    }
    None
}

/// The rows shown for `scope`: the scope node's children (its members at depth
/// 0). The scope package itself is never a row. Unknown scope -> empty.
fn scoped_roots(full: &ProjectTree, scope: &str) -> Vec<TreeNode> {
    find_node(&full.roots, scope)
        .map(|n| n.children.clone())
        .unwrap_or_default()
}

/// Keep rows whose kind == `kind`; retain ancestor packages of any kept row for
/// structure; prune everything else. (Only packages carry children, so a pruned
/// non-package never strands descendants.)
fn filter_kind(nodes: &[TreeNode], kind: TreeKind) -> Vec<TreeNode> {
    nodes
        .iter()
        .filter_map(|n| {
            let kids = filter_kind(&n.children, kind);
            if n.kind == kind || !kids.is_empty() {
                Some(TreeNode {
                    children: kids,
                    ..n.clone()
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn view(model: &Model, state: &NavState) -> NavView {
    let full = build_tree(model, "Untitled");
    let scoped = scoped_roots(&full, &state.scope);
    let filtered = match state.filter {
        Some(k) => filter_kind(&scoped, k),
        None => scoped,
    };
    if state.query.trim().is_empty() {
        return NavView::Browse(ProjectTree { roots: filtered });
    }
    // Query path lands in Task 5; a temporary Browse keeps the crate compiling.
    NavView::Browse(ProjectTree { roots: filtered })
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

    #[test]
    fn packages_lead_with_synthetic_root_then_nest_real_packages() {
        let rows = packages(&built());
        // Row 0: synthetic whole-model root, key "", titled from model.path.
        assert_eq!(
            rows[0],
            PackageRow {
                key: String::new(),
                title: "Root".to_string(),
                depth: 0
            }
        );
        // The one real sub-package, indented to depth 1. (Only packages appear;
        // `cls`/`iface` classifiers are excluded.)
        assert_eq!(
            rows.iter()
                .map(|r| (r.key.as_str(), r.depth))
                .collect::<Vec<_>>(),
            vec![("", 0usize), ("sub", 1usize)]
        );
    }

    #[test]
    fn packages_synthetic_root_falls_back_to_untitled_when_path_empty() {
        let mut m = built();
        m.path = String::new();
        let rows = packages(&m);
        assert_eq!(rows[0].title, "Untitled");
        assert_eq!(rows[0].key, "");
    }

    fn browse_roots(v: &NavView) -> &ProjectTree {
        match v {
            NavView::Browse(t) => t,
            other => panic!("expected Browse, got {other:?}"),
        }
    }

    // Depth-first (key, kind) pairs for order-independent assertions.
    fn flat(t: &ProjectTree) -> Vec<(String, TreeKind)> {
        fn walk(nodes: &[TreeNode], out: &mut Vec<(String, TreeKind)>) {
            for n in nodes {
                out.push((n.key.clone(), n.kind));
                walk(&n.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&t.roots, &mut out);
        out
    }

    #[test]
    fn empty_scope_roots_at_whole_model_without_the_synthetic_root_row() {
        let v = view(&built(), &NavState::default());
        let t = browse_roots(&v);
        // Whole-model members are at depth 0 — the "Root" package itself is NOT a
        // row (it is the dropdown's scope, not tree content).
        let keys: Vec<&str> = t.roots.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["sub", "iface"]);
    }

    #[test]
    fn scope_roots_at_the_packages_subtree() {
        let state = NavState {
            scope: "sub".to_string(),
            ..Default::default()
        };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        // "sub"'s members at depth 0; "sub" itself is not shown.
        assert_eq!(flat(t), vec![("cls".to_string(), TreeKind::Class)]);
    }

    #[test]
    fn type_filter_keeps_matching_kinds_and_ancestor_packages_prunes_rest() {
        let state = NavState {
            filter: Some(TreeKind::Class),
            ..Default::default()
        };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        // Only the Class survives, but its ancestor package "sub" is retained for
        // structure; the sibling Interface "iface" is pruned.
        assert_eq!(
            flat(t),
            vec![
                ("sub".to_string(), TreeKind::Package),
                ("cls".to_string(), TreeKind::Class)
            ]
        );
    }

    #[test]
    fn type_filter_on_package_keeps_package_rows() {
        let state = NavState {
            filter: Some(TreeKind::Package),
            ..Default::default()
        };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        assert_eq!(flat(t), vec![("sub".to_string(), TreeKind::Package)]);
    }
}
