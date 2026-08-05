//! The tree panel's geometry and state core. No `Cx`, no makepad draw types --
//! everything here is pure so the row math is unit-testable, the same split
//! `popup/menu.rs` uses for the menu list.
//!
//! Built up incrementally across several tasks; `tree_panel.rs` only starts
//! consuming it in Task 6, so allow dead code until then.
#![allow(dead_code)]

use std::collections::HashSet;

use crate::icons::Icon;
use crate::tree::{key_string, TreeKind, TreeNode};

/// Row band height in lpx. Matches the `node_height: 27.0` the fork `FileTree`
/// was configured with, so rows land where they always did.
pub const ROW_HEIGHT: f64 = 27.0;

/// One row the panel will draw this frame, already flattened out of the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleRow {
    pub key: String,
    pub depth: usize,
    pub title: String,
    pub kind: TreeKind,
    pub icon: Icon,
    pub is_directory: bool,
    pub openable: bool,
    pub view_degraded: bool,
    pub concept_id: Option<String>,
    pub address: Option<String>,
    /// Fold amount of this row's own subtree (directories only; 1.0 for files).
    pub fold: f32,
    /// Product of every ancestor's fold amount -- the factor this row's height
    /// and marks shrink by mid-collapse.
    pub scale: f64,
}

#[derive(Default)]
pub struct TreeLayout {
    roots: Vec<TreeNode>,
    open: HashSet<String>,
    rows: Vec<VisibleRow>,
}

impl TreeLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn roots(&self) -> &[TreeNode] {
        &self.roots
    }

    pub fn set_roots(&mut self, roots: Vec<TreeNode>) {
        self.roots = roots;
        self.reflow();
    }

    pub fn rows(&self) -> &[VisibleRow] {
        &self.rows
    }

    pub fn open_keys(&self) -> &HashSet<String> {
        &self.open
    }

    pub fn set_open_keys(&mut self, keys: HashSet<String>) {
        self.open = keys;
        self.reflow();
    }

    pub fn is_folder_open(&self, key: &str) -> bool {
        self.open.contains(key)
    }

    pub fn set_folder_open(&mut self, key: &str, open: bool) {
        let changed = if open {
            self.open.insert(key.to_string())
        } else {
            self.open.remove(key)
        };
        if changed {
            self.reflow();
        }
    }

    /// Rebuild the visible-row list from the roots and the open set.
    fn reflow(&mut self) {
        let mut rows = Vec::new();
        let roots = std::mem::take(&mut self.roots);
        flatten(&roots, 0, 1.0, &self.open, &mut rows);
        self.roots = roots;
        self.rows = rows;
    }
}

fn flatten(
    nodes: &[TreeNode],
    depth: usize,
    scale: f64,
    open: &HashSet<String>,
    out: &mut Vec<VisibleRow>,
) {
    for node in nodes {
        let key = key_string(&node.key);
        let is_open = node.is_directory && open.contains(&key);
        // Task 2 replaces this hard 0/1 with the animated fold amount.
        let fold = if is_open { 1.0f32 } else { 0.0f32 };
        out.push(VisibleRow {
            key,
            depth,
            title: node.title.clone(),
            kind: node.kind,
            icon: node.presentation.icon,
            is_directory: node.is_directory,
            openable: node.openable,
            view_degraded: node.view_degraded,
            concept_id: node.concept_id.clone(),
            address: node.address.clone(),
            fold,
            scale,
        });
        if is_open {
            flatten(&node.children, depth + 1, scale, open, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::TreeNode;

    /// Build a directory node with `children`, keyed by a single-segment RowId.
    fn dir(key: &str, children: Vec<TreeNode>) -> TreeNode {
        let mut node = crate::tree::test_support::node(key);
        node.is_directory = true;
        node.address = Some(format!("/{key}"));
        node.children = children;
        node
    }

    fn file(key: &str) -> TreeNode {
        let mut node = crate::tree::test_support::node(key);
        node.openable = true;
        node.concept_id = Some(key.to_string());
        node
    }

    #[test]
    fn closed_folders_hide_their_children() {
        let mut layout = TreeLayout::new();
        layout.set_roots(vec![dir("pkg", vec![file("a"), file("b")])]);

        // Default: nothing forced open, so only the folder row is visible.
        let keys: Vec<&str> = layout.rows().iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec![crate::tree::key_string(&layout.roots()[0].key)]);
        let first_key = keys[0].to_owned();

        layout.set_folder_open(&first_key, true);
        let keys: Vec<String> = layout.rows().iter().map(|r| r.key.clone()).collect();
        assert_eq!(keys.len(), 3, "folder + two children");
        assert_eq!(layout.rows()[1].depth, 1);
        assert_eq!(layout.rows()[2].depth, 1);
    }
}
