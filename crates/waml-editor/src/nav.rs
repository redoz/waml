//! The nav seam: project a `Model` + `NavState` into a `NavView` the tree panel
//! renders. Pure — no makepad, no `Cx` — and unit-tested like `tree.rs`. Sits on
//! top of `tree::build_tree`; clean-room (not a port of the web navigator).

// The public surface here is exercised only by its own unit tests until the
// tree panel / app wiring lands (later tasks of the same plan); until then a
// bin crate's dead-code lint would otherwise flag every item. Same convention
// as `icons.rs`'s catalog surface.
#![allow(dead_code)]

use crate::tree::{build_tree, ProjectTree, TreeKind, TreeNode};

#[derive(Debug, Clone, PartialEq)]
pub struct NavState {
    /// Directory address; `"/"` = whole-bundle scope.
    pub scope: String,
    /// Search text; `""` = browse (never a search state).
    pub query: String,
    /// `None` = All.
    pub filter: Option<TreeKind>,
}

impl Default for NavState {
    fn default() -> Self {
        Self {
            scope: "/".into(),
            query: String::new(),
            filter: None,
        }
    }
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
        TreeKind::Directory => "Directory",
        TreeKind::OkfDocument => "OKF",
        TreeKind::Class => "Class",
        TreeKind::Interface => "Interface",
        TreeKind::Enum => "Enum",
        TreeKind::DataType => "DataType",
        TreeKind::Diagram => "Diagram",
        TreeKind::Behavior => "Behavior",
        TreeKind::Sequence => "Sequence",
        TreeKind::Note => "Note",
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
    TreeKind::Directory,
    TreeKind::OkfDocument,
    TreeKind::Class,
    TreeKind::Interface,
    TreeKind::Enum,
    TreeKind::DataType,
    TreeKind::Diagram,
    TreeKind::Behavior,
    TreeKind::Sequence,
    TreeKind::Note,
];

/// The distinct `TreeKind`s present anywhere in the model, in canonical order.
/// Drives the type-filter chip's cycle; compute once on Model load, not per
/// keystroke.
pub fn kinds_in_model(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
) -> Vec<TreeKind> {
    let full = build_tree(okf, uml, "Untitled");
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

/// Nested directory-only rows for the title dropdown, depth-indented.
pub fn packages(okf: &waml::analysis::OkfAnalysis, uml: &waml::uml::Analysis) -> Vec<PackageRow> {
    let full = build_tree(okf, uml, "Untitled");
    let root_title = okf
        .bundle
        .index("/")
        .and_then(|index| index.title.clone())
        .unwrap_or_else(|| "Untitled".into());
    let mut out = vec![PackageRow {
        key: "/".into(),
        title: root_title,
        depth: 0,
    }];
    fn walk(nodes: &[TreeNode], depth: usize, out: &mut Vec<PackageRow>) {
        for n in nodes {
            if n.kind == TreeKind::Directory {
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
/// root has key `"/"`.
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

/// Case-insensitive substring on `title`.
fn title_matches(title: &str, q: &str) -> bool {
    title.to_lowercase().contains(&q.to_lowercase())
}

/// Prune non-matching leaves; keep a node if its own title matches OR any
/// descendant is kept (packages thus survive on a matching member).
fn query_prune(nodes: &[TreeNode], q: &str) -> Vec<TreeNode> {
    nodes
        .iter()
        .filter_map(|n| {
            let kids = query_prune(&n.children, q);
            if title_matches(&n.title, q) || !kids.is_empty() {
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

pub fn view(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    state: &NavState,
) -> NavView {
    let full = build_tree(okf, uml, "Untitled");
    let scoped = scoped_roots(&full, &state.scope);
    let filtered = match state.filter {
        Some(k) => filter_kind(&scoped, k),
        None => scoped,
    };
    if state.query.trim().is_empty() {
        return NavView::Browse(ProjectTree { roots: filtered });
    }
    let in_scope = query_prune(&filtered, &state.query);
    if !in_scope.is_empty() {
        return NavView::Results(ProjectTree { roots: in_scope });
    }
    // Nothing in scope: search the whole bundle.
    let whole = scoped_roots(&full, "/");
    let whole_filtered = match state.filter {
        Some(k) => filter_kind(&whole, k),
        None => whole,
    };
    let elsewhere = query_prune(&whole_filtered, &state.query);
    if elsewhere.is_empty() {
        NavView::Empty
    } else {
        NavView::Elsewhere(ProjectTree { roots: elsewhere })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn built() -> (waml::analysis::OkfAnalysis, waml::uml::Analysis) {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Sub Pkg](sub/)\n* [Payments](iface.md)\n",
            ),
            ("sub/index.md", "# Sub Pkg\n\n* [Customer](cls.md)\n"),
            (
                "sub/cls.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
            (
                "iface.md",
                "---\ntype: uml.Interface\ntitle: Payments\n---\n# Payments\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, okf, uml, _) = prepared.into_parts();
        (okf, uml)
    }

    #[test]
    fn chip_label_is_all_when_unfiltered_else_the_kind() {
        assert_eq!(chip_label(None), "All");
        assert_eq!(chip_label(Some(TreeKind::Class)), "Class");
        assert_eq!(chip_label(Some(TreeKind::Directory)), "Directory");
    }

    #[test]
    fn kinds_in_model_is_distinct_and_canonically_ordered() {
        let (bundle, projection) = built();
        let kinds = kinds_in_model(&bundle, &projection);
        // Present: Package (root+sub), Class (cls), Interface (iface). Canonical
        // order puts Package before Class before Interface; no dupes.
        assert_eq!(
            kinds,
            vec![TreeKind::Directory, TreeKind::Class, TreeKind::Interface]
        );
    }

    #[test]
    fn kinds_are_canonically_ordered() {
        let (bundle, projection) = built();
        let kinds = kinds_in_model(&bundle, &projection);
        let idx = |k: &TreeKind| KIND_ORDER.iter().position(|x| x == k).unwrap();
        assert!(kinds.windows(2).all(|w| idx(&w[0]) < idx(&w[1])));
    }

    #[test]
    fn packages_lead_with_synthetic_root_then_nest_real_packages() {
        let (bundle, projection) = built();
        let rows = packages(&bundle, &projection);
        // Row 0: synthetic whole-model root, key "", titled from model.path.
        assert_eq!(
            rows[0],
            PackageRow {
                key: "/".to_string(),
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
            vec![("/", 0usize), ("/sub", 1usize)]
        );
    }

    #[test]
    fn packages_exclude_documents() {
        let (bundle, projection) = built();
        let rows = packages(&bundle, &projection);
        assert!(!rows.iter().any(|row| row.key == "sub/cls"));
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
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &NavState::default());
        let t = browse_roots(&v);
        // Whole-model members are at depth 0 — the "Root" package itself is NOT a
        // row (it is the dropdown's scope, not tree content).
        let keys: Vec<&str> = t.roots.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["/sub", "iface"]);
    }

    #[test]
    fn scope_roots_at_the_packages_subtree() {
        let state = NavState {
            scope: "/sub".to_string(),
            ..Default::default()
        };
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &state);
        let t = browse_roots(&v);
        // "sub"'s members at depth 0; "sub" itself is not shown.
        assert_eq!(flat(t), vec![("sub/cls".to_string(), TreeKind::Class)]);
    }

    #[test]
    fn type_filter_keeps_matching_kinds_and_ancestor_packages_prunes_rest() {
        let state = NavState {
            filter: Some(TreeKind::Class),
            ..Default::default()
        };
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &state);
        let t = browse_roots(&v);
        // Only the Class survives, but its ancestor package "sub" is retained for
        // structure; the sibling Interface "iface" is pruned.
        assert_eq!(
            flat(t),
            vec![
                ("/sub".to_string(), TreeKind::Directory),
                ("sub/cls".to_string(), TreeKind::Class)
            ]
        );
    }

    #[test]
    fn type_filter_on_package_keeps_package_rows() {
        let state = NavState {
            filter: Some(TreeKind::Directory),
            ..Default::default()
        };
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &state);
        let t = browse_roots(&v);
        assert_eq!(flat(t), vec![("/sub".to_string(), TreeKind::Directory)]);
    }

    #[test]
    fn query_prunes_non_matching_leaves_and_keeps_matching_branches() {
        let state = NavState {
            query: "custom".to_string(),
            ..Default::default()
        };
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &state);
        let t = match &v {
            NavView::Results(t) => t,
            other => panic!("expected Results, got {other:?}"),
        };
        // "Customer" matches; its ancestor "sub" is kept; "Payments" is pruned.
        assert_eq!(
            flat(t),
            vec![
                ("/sub".to_string(), TreeKind::Directory),
                ("sub/cls".to_string(), TreeKind::Class)
            ]
        );
    }

    #[test]
    fn query_is_case_insensitive() {
        let state = NavState {
            query: "PAYMENTS".to_string(),
            ..Default::default()
        };
        let (bundle, projection) = built();
        match view(&bundle, &projection, &state) {
            NavView::Results(t) => {
                assert!(flat(&t).iter().any(|(k, _)| k == "iface"));
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn no_scope_match_but_whole_model_match_is_elsewhere() {
        // Scope into "sub" (holds only "Customer"), search for the interface that
        // lives outside the scope.
        let state = NavState {
            scope: "/sub".to_string(),
            query: "payments".to_string(),
            ..Default::default()
        };
        let (bundle, projection) = built();
        let v = view(&bundle, &projection, &state);
        let t = match &v {
            NavView::Elsewhere(t) => t,
            other => panic!("expected Elsewhere, got {other:?}"),
        };
        assert!(flat(t).iter().any(|(k, _)| k == "iface"));
    }

    #[test]
    fn no_match_anywhere_is_empty() {
        let state = NavState {
            query: "zzzznope".to_string(),
            ..Default::default()
        };
        let (bundle, projection) = built();
        assert_eq!(view(&bundle, &projection, &state), NavView::Empty);
    }
}
