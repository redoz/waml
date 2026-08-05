//! The tree panel's geometry and state core. No `Cx`, no makepad draw types --
//! everything here is pure so the row math is unit-testable, the same split
//! `popup/menu.rs` uses for the menu list.
//!
//! Built up incrementally across several tasks; `tree_panel.rs` only starts
//! consuming it in Task 6, so allow dead code until then.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use makepad_widgets::*;

use crate::icons::Icon;
use crate::tree::{key_string, TreeKind, TreeNode};

/// Row band height in lpx. Matches the `node_height: 27.0` the fork `FileTree`
/// was configured with, so rows land where they always did.
pub const ROW_HEIGHT: f64 = 27.0;

/// Left margin of the fold chevron box within its row.
pub const CHEVRON_LEFT_MARGIN: f64 = 4.0;
/// Side length of the (square) fold chevron box.
pub const CHEVRON_SIZE: f64 = 10.0;
/// Per-depth-level indent applied to the chevron (and, by the fork's layout,
/// the icon after it).
pub const ICON_DEPTH_INDENT: f64 = 15.0;

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

/// What a pointer position resolves to. `Chevron` only ever names a directory
/// row; every other position on a row -- and every position on a file row --
/// is `Row`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeHit {
    Chevron(String),
    Row(String),
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
    origin: DVec2,
    size: DVec2,
    scroll: f64,
    selected: Option<String>,
    hover: Option<String>,
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

    /// The panel's drawn rect for this frame, in absolute coordinates. Set at
    /// draw time; hit-testing reads exactly what was drawn rather than
    /// recomputing it, which is what removes the cached-rect side table.
    pub fn set_viewport(&mut self, origin: DVec2, size: DVec2) {
        self.origin = origin;
        self.size = size;
        // A shorter viewport can strand the scroll past the new maximum.
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Total height of every visible row, honouring mid-collapse scale.
    pub fn content_height(&self) -> f64 {
        self.rows.iter().map(|row| ROW_HEIGHT * row.scale).sum()
    }

    pub fn max_scroll(&self) -> f64 {
        (self.content_height() - self.size.y).max(0.0)
    }

    pub fn scroll(&self) -> f64 {
        self.scroll
    }

    pub fn set_scroll(&mut self, scroll: f64) {
        self.scroll = scroll.clamp(0.0, self.max_scroll());
    }

    /// Absolute rect of row `index`, already shifted by the scroll offset. Rows
    /// are stacked by their SCALED heights so a collapsing subtree closes the
    /// gap as it shrinks.
    pub fn row_rect(&self, index: usize) -> Rect {
        let mut y = self.origin.y - self.scroll;
        for row in &self.rows[..index.min(self.rows.len())] {
            y += ROW_HEIGHT * row.scale;
        }
        let scale = self.rows.get(index).map_or(1.0, |row| row.scale);
        Rect {
            pos: dvec2(self.origin.x, y),
            size: dvec2(self.size.x, ROW_HEIGHT * scale),
        }
    }

    /// Absolute rect of row `index`'s fold chevron. Meaningful for directory
    /// rows; computed for any row so drawing and hit-testing share one formula.
    pub fn chevron_rect(&self, index: usize) -> Rect {
        let row = self.row_rect(index);
        let depth = self.rows.get(index).map_or(0, |r| r.depth);
        let scale = self.rows.get(index).map_or(1.0, |r| r.scale);
        let size = CHEVRON_SIZE * scale;
        Rect {
            pos: dvec2(
                row.pos.x + CHEVRON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT,
                row.pos.y + (ROW_HEIGHT * scale - size) / 2.0,
            ),
            size: dvec2(size, size),
        }
    }

    /// Index of the row under `pos`, or `None` off the rows. A row outside the
    /// clipped viewport is not hittable even though its band still maps -- the
    /// same rule `popup/menu.rs` applies to a scrolled menu.
    pub fn row_at(&self, pos: DVec2) -> Option<usize> {
        if pos.x < self.origin.x || pos.x > self.origin.x + self.size.x {
            return None;
        }
        if pos.y < self.origin.y || pos.y >= self.origin.y + self.size.y {
            return None;
        }
        (0..self.rows.len()).find(|index| self.row_rect(*index).contains(pos))
    }

    pub fn hit(&self, pos: DVec2) -> Option<TreeHit> {
        let index = self.row_at(pos)?;
        let row = &self.rows[index];
        if row.is_directory && self.chevron_rect(index).contains(pos) {
            return Some(TreeHit::Chevron(row.key.clone()));
        }
        Some(TreeHit::Row(row.key.clone()))
    }

    /// Scroll the row named `key` into the viewport. Returns `false` if no
    /// visible row carries that key (a collapsed ancestor, or a stale key).
    pub fn scroll_key_into_view(&mut self, key: &str) -> bool {
        let Some(index) = self.rows.iter().position(|row| row.key == key) else {
            return false;
        };
        let rect = self.row_rect(index);
        let top = rect.pos.y - self.origin.y + self.scroll;
        let bottom = top + rect.size.y;
        if top < self.scroll {
            self.set_scroll(top);
        } else if bottom > self.scroll + self.size.y {
            self.set_scroll(bottom - self.size.y);
        }
        true
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Returns `true` when the selection actually moved, so the caller can skip
    /// a redraw otherwise.
    pub fn set_selected(&mut self, key: Option<String>) -> bool {
        if self.selected == key {
            return false;
        }
        self.selected = key;
        true
    }

    pub fn hover(&self) -> Option<&str> {
        self.hover.as_deref()
    }

    /// Resolve the hovered row from a pointer position (`None` = pointer left
    /// the panel). Driven from `MouseMove` containment rather than
    /// `Hit::FingerHover`, so an arbiter handing the hit to another widget
    /// cannot strand the tint on a row (see `bc53c22`). Returns `true` when the
    /// hovered row changed.
    pub fn set_hover_at(&mut self, pos: Option<DVec2>) -> bool {
        let next = pos
            .and_then(|pos| self.row_at(pos))
            .map(|index| self.rows[index].key.clone());
        if self.hover == next {
            return false;
        }
        self.hover = next;
        true
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

    fn laid_out() -> TreeLayout {
        let mut layout = TreeLayout::new();
        layout.set_roots(vec![dir("pkg", vec![file("a"), file("b")])]);
        let key = layout.rows()[0].key.clone();
        layout.set_folder_open(&key, true, false);
        layout.set_viewport(dvec2(0.0, 0.0), dvec2(280.0, ROW_HEIGHT * 2.0));
        layout
    }

    #[test]
    fn rows_stack_by_row_height_and_shift_with_scroll() {
        let mut layout = laid_out();
        assert_eq!(layout.row_rect(0).pos.y, 0.0);
        assert_eq!(layout.row_rect(1).pos.y, ROW_HEIGHT);

        layout.set_scroll(ROW_HEIGHT);
        assert_eq!(layout.row_rect(0).pos.y, -ROW_HEIGHT);
        assert_eq!(layout.row_rect(1).pos.y, 0.0);
    }

    #[test]
    fn scroll_clamps_to_the_content() {
        let mut layout = laid_out();
        // 3 rows in a 2-row viewport: one row of overflow.
        assert_eq!(layout.content_height(), ROW_HEIGHT * 3.0);
        assert_eq!(layout.max_scroll(), ROW_HEIGHT);

        layout.set_scroll(9999.0);
        assert_eq!(layout.scroll(), ROW_HEIGHT);
        layout.set_scroll(-50.0);
        assert_eq!(layout.scroll(), 0.0);
    }

    #[test]
    fn hit_splits_chevron_from_row_body_and_rejects_outside_the_viewport() {
        let layout = laid_out();
        let folder_key = layout.rows()[0].key.clone();

        // Over the chevron box of the folder row.
        let chevron = layout.chevron_rect(0);
        let hit = layout.hit(chevron.pos + dvec2(2.0, 2.0));
        assert_eq!(hit, Some(TreeHit::Chevron(folder_key.clone())));

        // Further right on the same row: the body.
        let hit = layout.hit(dvec2(200.0, ROW_HEIGHT * 0.5));
        assert_eq!(hit, Some(TreeHit::Row(folder_key)));

        // A file row has no chevron: a hit in the chevron band is still body.
        let file_key = layout.rows()[1].key.clone();
        let hit = layout.hit(dvec2(chevron.pos.x + 2.0, ROW_HEIGHT * 1.5));
        assert_eq!(hit, Some(TreeHit::Row(file_key)));

        // Row 2 is scrolled out of the 2-row viewport: not hittable.
        assert_eq!(layout.hit(dvec2(100.0, ROW_HEIGHT * 2.5)), None);
        // Above the first row.
        assert_eq!(layout.hit(dvec2(100.0, -5.0)), None);
        // Left of the panel.
        assert_eq!(layout.hit(dvec2(-5.0, 5.0)), None);
    }

    #[test]
    fn scroll_key_into_view_moves_an_offscreen_row_into_the_viewport() {
        let mut layout = laid_out();
        let last = layout.rows()[2].key.clone();
        assert!(layout.scroll_key_into_view(&last));
        assert_eq!(layout.scroll(), ROW_HEIGHT, "scrolled just enough");
        assert!(layout.hit(dvec2(100.0, ROW_HEIGHT * 1.5)).is_some());
        assert!(!layout.scroll_key_into_view("no-such-key"));
    }

    #[test]
    fn hover_follows_the_pointer_and_clears_off_the_rows() {
        let mut layout = laid_out();
        let first = layout.rows()[0].key.clone();

        assert!(layout.set_hover_at(Some(dvec2(100.0, 5.0))));
        assert_eq!(layout.hover(), Some(first.as_str()));
        // Same row again: no change, so no redraw.
        assert!(!layout.set_hover_at(Some(dvec2(120.0, 6.0))));

        // Off the panel entirely.
        assert!(layout.set_hover_at(None));
        assert_eq!(layout.hover(), None);
    }

    #[test]
    fn scrolling_a_row_out_from_under_the_pointer_clears_hover() {
        let mut layout = laid_out();
        layout.set_hover_at(Some(dvec2(100.0, ROW_HEIGHT * 1.5)));
        let hovered = layout.hover().expect("row hovered").to_string();

        layout.set_scroll(ROW_HEIGHT);
        // Re-resolve at the same pointer position after the scroll.
        layout.set_hover_at(Some(dvec2(100.0, ROW_HEIGHT * 1.5)));
        assert_ne!(layout.hover(), Some(hovered.as_str()));
    }

    #[test]
    fn selection_is_set_and_cleared_by_key() {
        let mut layout = laid_out();
        let key = layout.rows()[1].key.clone();
        assert!(layout.set_selected(Some(key.clone())));
        assert_eq!(layout.selected(), Some(key.as_str()));
        assert!(!layout.set_selected(Some(key)), "unchanged: no redraw");
        assert!(layout.set_selected(None));
        assert_eq!(layout.selected(), None);
    }
}
