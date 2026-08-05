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
}

impl Default for NavState {
    fn default() -> Self {
        Self { scope: "/".into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavView {
    /// The scoped subtree. The panel's only non-empty projection: the header's
    /// search field and type-filter chip are gone, so there is nothing left to
    /// narrow the tree by and no `Results`/`Elsewhere` state to fall into.
    Browse(ProjectTree),
    /// The scope address resolves to no node in the model.
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackageRow {
    pub key: String,
    pub title: String,
    pub depth: usize,
}

/// Nested directory-only rows for the title dropdown, depth-indented.
pub fn packages(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    mode: crate::folder_projection::ViewMode,
    limits: waml::view::chain::ChainLimits,
) -> Vec<PackageRow> {
    let full = build_tree(okf, uml, "Untitled", mode, limits);
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

/// The scope node itself, children stripped. It *is* the single root row of
/// every view (the header no longer carries the scope title, so this row is
/// the model's on-screen identity and the only thing to select when a whole
/// bundle/directory is the subject). Unknown scope -> `None`.
fn scope_node(full: &ProjectTree, scope: &str) -> Option<TreeNode> {
    find_node(&full.roots, scope).map(|n| TreeNode {
        children: Vec::new(),
        ..n.clone()
    })
}

/// The scope node's members at depth 0.
fn scoped_roots(full: &ProjectTree, scope: &str) -> Vec<TreeNode> {
    find_node(&full.roots, scope)
        .map(|n| n.children.clone())
        .unwrap_or_default()
}

/// Hang `children` under `wrapper` as the single root of a view.
fn under(wrapper: &TreeNode, children: Vec<TreeNode>) -> ProjectTree {
    ProjectTree {
        roots: vec![TreeNode {
            children,
            ..wrapper.clone()
        }],
    }
}

pub fn view(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    state: &NavState,
    mode: crate::folder_projection::ViewMode,
    limits: waml::view::chain::ChainLimits,
) -> NavView {
    let full = build_tree(okf, uml, "Untitled", mode, limits);
    // A non-`Empty` view is the scope row with its members beneath it: that row
    // is the model's on-screen identity and the thing to select when a whole
    // bundle/directory is the subject.
    let Some(scope) = scope_node(&full, &state.scope) else {
        return NavView::Empty;
    };
    NavView::Browse(under(&scope, scoped_roots(&full, &state.scope)))
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
    fn packages_lead_with_synthetic_root_then_nest_real_packages() {
        let (bundle, projection) = built();
        let rows = packages(
            &bundle,
            &projection,
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
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
        let rows = packages(
            &bundle,
            &projection,
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
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
    fn empty_scope_puts_the_whole_model_under_the_bundle_root_row() {
        let (bundle, projection) = built();
        let v = view(
            &bundle,
            &projection,
            &NavState::default(),
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
        let t = browse_roots(&v);
        // The scope package IS the single root row; its members hang beneath it.
        let roots: Vec<&str> = t.roots.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(roots, vec!["/"]);
        let keys: Vec<&str> = t.roots[0].children.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["/sub", "iface"]);
    }

    #[test]
    fn scope_roots_at_the_packages_subtree() {
        let state = NavState {
            scope: "/sub".to_string(),
        };
        let (bundle, projection) = built();
        let v = view(
            &bundle,
            &projection,
            &state,
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
        let t = browse_roots(&v);
        // "sub" is the root row; its members hang beneath it.
        assert_eq!(
            flat(t),
            vec![
                ("/sub".to_string(), TreeKind::Directory),
                ("sub/cls".to_string(), TreeKind::Class)
            ]
        );
    }

    #[test]
    fn unknown_scope_is_empty() {
        let state = NavState {
            scope: "/nope".to_string(),
        };
        let (bundle, projection) = built();
        assert_eq!(
            view(
                &bundle,
                &projection,
                &state,
                crate::folder_projection::ViewMode::Projected,
                waml::view::chain::ChainLimits::default(),
            ),
            NavView::Empty
        );
    }
}
