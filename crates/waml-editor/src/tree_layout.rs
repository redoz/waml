//! The tree panel's geometry and state core. No `Cx`, no makepad draw types --
//! everything here is pure so the row math is unit-testable, the same split
//! `popup/menu.rs` uses for the menu list.
//!
//! Built up incrementally across several tasks; `tree_panel.rs` only starts
//! consuming it in Task 6, so allow dead code until then.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use crate::icons::Icon;
use crate::tree::{key_string, TreeKind, TreeNode};

/// Row band height in lpx. Matches the `node_height: 27.0` the fork `FileTree`
/// was configured with, so rows land where they always did.
pub const ROW_HEIGHT: f64 = 27.0;

/// Fold transition duration in seconds. Matches the fork `FileTree`'s
/// `Play.Forward {duration: 0.2}` so the motion is indistinguishable.
pub const FOLD_SECS: f64 = 0.2;

/// Below this a folder is treated as fully closed and its children are not
/// flattened at all -- the same threshold the fork culled at.
const FOLD_CULL: f32 = 0.001;

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
    /// Animated fold amount per directory key, 0.0 closed .. 1.0 open. A key
    /// absent from the map is closed. This is the authority: unlike the fork's
    /// per-node state, a culled folder is never forgotten, so a collapsing
    /// subtree cannot report itself closed while its rows are still drawn.
    fold: HashMap<String, f32>,
    /// Per-key animation target, present only while a fold is in flight.
    fold_target: HashMap<String, f32>,
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

    pub fn open_keys(&self) -> HashSet<String> {
        self.fold
            .iter()
            .filter(|(_, &amount)| amount > 0.5)
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub fn set_open_keys(&mut self, keys: HashSet<String>) {
        self.fold_target.clear();
        self.fold = keys.into_iter().map(|key| (key, 1.0)).collect();
        self.reflow();
    }

    pub fn is_folder_open(&self, key: &str) -> bool {
        // "Open" means open-or-opening: the target if animating, else the
        // resting amount. Callers ask this to decide chevron direction and what
        // a click should do, and both must agree with where the fold is headed.
        let target = self
            .fold_target
            .get(key)
            .copied()
            .or_else(|| self.fold.get(key).copied())
            .unwrap_or(0.0);
        target > 0.5
    }

    pub fn set_folder_open(&mut self, key: &str, open: bool, animate: bool) {
        let target = if open { 1.0 } else { 0.0 };
        if animate {
            self.fold_target.insert(key.to_string(), target);
        } else {
            self.fold_target.remove(key);
            self.fold.insert(key.to_string(), target);
        }
        self.reflow();
    }

    /// Advance every in-flight fold by `dt` seconds. Returns `true` while any
    /// fold is still moving, i.e. the caller must schedule another frame.
    pub fn advance(&mut self, dt: f64) -> bool {
        if self.fold_target.is_empty() {
            return false;
        }
        let step = (dt / FOLD_SECS).clamp(0.0, 1.0) as f32;
        let mut settled = Vec::new();
        for (key, target) in &self.fold_target {
            let current = self.fold.get(key).copied().unwrap_or(0.0);
            // ExpDecay-shaped approach: move a fixed fraction of the remaining
            // distance per step, which is what makes the fork's fold ease out.
            let next = current + (target - current) * ease_fraction(step);
            let next = if (target - next).abs() <= FOLD_CULL {
                settled.push(key.clone());
                *target
            } else {
                next
            };
            self.fold.insert(key.clone(), next);
        }
        for key in settled {
            self.fold_target.remove(&key);
        }
        self.reflow();
        !self.fold_target.is_empty()
    }

    /// Rebuild the visible-row list from the roots and the fold map.
    fn reflow(&mut self) {
        let mut rows = Vec::new();
        let roots = std::mem::take(&mut self.roots);
        flatten(&roots, 0, 1.0, &self.fold, &mut rows);
        self.roots = roots;
        self.rows = rows;
    }
}

/// Fraction of the remaining distance to cover for a step of `step` (in units
/// of the full duration). Exponential so the tail eases out like the fork's
/// `Ease.ExpDecay {d1: 0.80, d2: 0.97}` rather than arriving linearly.
fn ease_fraction(step: f32) -> f32 {
    1.0 - (0.02f32).powf(step)
}

fn flatten(
    nodes: &[TreeNode],
    depth: usize,
    scale: f64,
    fold: &HashMap<String, f32>,
    out: &mut Vec<VisibleRow>,
) {
    for node in nodes {
        let key = key_string(&node.key);
        let amount = if node.is_directory {
            fold.get(&key).copied().unwrap_or(0.0)
        } else {
            1.0
        };
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
            fold: amount,
            scale,
        });
        if node.is_directory && amount > FOLD_CULL {
            flatten(&node.children, depth + 1, scale * amount as f64, fold, out);
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

        layout.set_folder_open(&first_key, true, false);
        let keys: Vec<String> = layout.rows().iter().map(|r| r.key.clone()).collect();
        assert_eq!(keys.len(), 3, "folder + two children");
        assert_eq!(layout.rows()[1].depth, 1);
        assert_eq!(layout.rows()[2].depth, 1);
    }

    #[test]
    fn fold_amount_animates_then_settles() {
        let mut layout = TreeLayout::new();
        layout.set_roots(vec![dir("pkg", vec![file("a")])]);
        let key = layout.rows()[0].key.clone();

        layout.set_folder_open(&key, true, true);
        // Mid-flight: partially open, child visible, scaled below 1.
        assert!(layout.advance(0.05), "still animating");
        let fold = layout.rows()[0].fold;
        assert!(fold > 0.0 && fold < 1.0, "fold mid-flight, got {fold}");
        assert_eq!(layout.rows().len(), 2, "child visible while opening");
        assert!(layout.rows()[1].scale < 1.0);

        // Run past the duration: settles exactly at 1.0 and stops asking for frames.
        for _ in 0..40 {
            layout.advance(0.02);
        }
        assert_eq!(layout.rows()[0].fold, 1.0);
        assert_eq!(layout.rows()[1].scale, 1.0);
        assert!(!layout.advance(0.02), "settled: no further frames");
    }

    #[test]
    fn closing_culls_children_at_the_threshold() {
        let mut layout = TreeLayout::new();
        layout.set_roots(vec![dir("pkg", vec![file("a")])]);
        let key = layout.rows()[0].key.clone();
        layout.set_folder_open(&key, true, false);
        assert_eq!(layout.rows().len(), 2);

        layout.set_folder_open(&key, false, true);
        for _ in 0..60 {
            layout.advance(0.02);
        }
        assert_eq!(layout.rows()[0].fold, 0.0);
        assert_eq!(layout.rows().len(), 1, "culled below 0.001");
        assert!(!layout.advance(0.02));
    }

    #[test]
    fn scale_is_the_product_of_ancestor_folds() {
        let mut layout = TreeLayout::new();
        layout.set_roots(vec![dir("outer", vec![dir("inner", vec![file("a")])])]);
        let outer = layout.rows()[0].key.clone();
        layout.set_folder_open(&outer, true, false);
        let inner = layout.rows()[1].key.clone();
        layout.set_folder_open(&inner, true, false);
        assert_eq!(layout.rows().len(), 3);

        // Half-close the OUTER folder; the grandchild scales by the outer fold.
        layout.set_folder_open(&outer, false, true);
        layout.advance(0.05);
        let outer_fold = layout.rows()[0].fold as f64;
        assert!(outer_fold > 0.0 && outer_fold < 1.0);
        assert!((layout.rows()[2].scale - outer_fold).abs() < 1e-6);
    }
}
