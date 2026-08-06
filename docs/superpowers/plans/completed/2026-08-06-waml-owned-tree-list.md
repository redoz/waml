# waml-owned tree row list — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace makepad's `FileTree` in `tree_panel.rs` with a waml-owned immediate-mode tree row list, so selection, fold state, scroll and hit-testing live in one struct that also computes the rects used to draw.

**Architecture:** A pure `Cx`-free geometry core (`tree_layout.rs`) owns all row state and hands out rects; stateless drawing functions (`tree_row_draw.rs`) paint one row into a rect; the widget (`tree_panel.rs`) shrinks to a draw loop plus event routing, keeping `ScrollBars` for scroll input. Modelled on `crates/waml-editor/src/popup/menu.rs`, which already does exactly this for the menu list.

**Tech Stack:** Rust, makepad (fork `redoz/makepad`, branch `waml`), `makepad-widgets` (`View`, `ScrollBars`, `DrawText`, `DrawColor`), waml's own `IconSet` / `Icon` / `DrawChevron`.

**Spec:** `docs/superpowers/specs/2026-08-05-waml-owned-tree-list-design.md`

## Global Constraints

- **Strict parity plus hover.** A user must not be able to tell the widget changed, except for the newly added hover tint. No keyboard navigation, no virtualization, no restyling.
- **Zero upstream lines.** Do NOT copy any code out of `makepad/widgets/src/file_tree.rs` into waml. waml stays MPL-2.0 with no vendored upstream source. Reading it to match behaviour is fine; pasting from it is not.
- **`ROW_HEIGHT` stays `27.0`**, `ICON_SIZE`, `ICON_LEFT_MARGIN` (20), `ICON_DEPTH_INDENT`, `CHEVRON_LEFT_MARGIN` (4), `CHEVRON_SIZE` (10) keep their current values — the label sits 4px past the glyph at `padding.left: 38.0`.
- **Fold animation: 0.2s, `ExpDecay`** (`d1: 0.80, d2: 0.97` closing; `0.82 / 0.95` opening), cull at `opened <= 0.001`. If that exact ease is unavailable, use the closest ease at 0.2s and **say so in the commit message** — do not silently substitute a different feel.
- **The panel's public API does not change**: `set_view`, `set_view_with_fold_reset`, `set_selected_document`, `set_selected_key`, `set_scope_title`, `set_view_mode`, `set_presentation_visible`, `dock_state`, `slot_width`, `drawn_rect`, `toggle_dock`, `open_dock`, `close_dock`, `reveal_target`, `toggle_directory`, `navigation`, `view_mode_toggled`, `context_menu`. `app/shell.rs` and `app/navigation.rs` must compile untouched (one comment deletion aside, Task 8).
- **Gate for every task:** `cargo test --workspace` must pass before committing. The vscode extension gate (`cd editors/vscode && pnpm test && pnpm lint && pnpm build`) is required only for tasks that touch `editors/`; no task here does.
- **Rows are keyed by `RowId`** (via `crate::tree::key_string`) throughout — never by OKF address. See the note in Task 1 about the address/key divergence this removes.
- **No visual verification inside a task.** Nothing in the gate can see pixels. The visual checks are listed at the end of this plan as owed sign-off items for the user; no task may block on them.

---

### Task 1: The row-flattening core

Creates `TreeLayout` with the flatten walk and the open-set. No animation, no hit-testing yet — those are Tasks 2 and 3.

**Note on a bug this removes:** today the panel stores `open_directories` as OKF **addresses** and calls `set_folder_is_open(cx, LiveId::from_str(address), ..)` (`tree_panel.rs:1235`, `:1319`), while rows are drawn with `LiveId::from_str(&key_string(&node.key))` (`tree_panel.rs:804`). Those two id spaces only coincide when a directory's `key_string` happens to equal its address. `TreeLayout` keys folds by `RowId` string only, so the divergence cannot occur. `toggle_directory(address)` keeps its address-taking signature (the app calls it) and resolves address → key internally.

**Files:**
- Create: `crates/waml-editor/src/tree_layout.rs`
- Modify: `crates/waml-editor/src/lib.rs` (add `mod tree_layout;`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/tree_layout.rs`

**Interfaces:**
- Consumes: `crate::tree::{TreeNode, TreeKind, key_string}`, `crate::document::DocumentPresentation`.
- Produces:
  - `pub struct TreeLayout` with `pub fn new() -> Self`, `pub fn set_roots(&mut self, roots: Vec<TreeNode>)`, `pub fn rows(&self) -> &[VisibleRow]`, `pub fn set_folder_open(&mut self, key: &str, open: bool)`, `pub fn is_folder_open(&self, key: &str) -> bool`, `pub fn open_keys(&self) -> &HashSet<String>`, `pub fn set_open_keys(&mut self, keys: HashSet<String>)`.
  - `pub struct VisibleRow { pub key: String, pub depth: usize, pub title: String, pub kind: TreeKind, pub icon: crate::icons::Icon, pub is_directory: bool, pub openable: bool, pub view_degraded: bool, pub concept_id: Option<String>, pub address: Option<String>, pub scale: f64, pub fold: f32 }`
  - `pub const ROW_HEIGHT: f64 = 27.0;`

- [ ] **Step 1: Write the failing test**

Add to `crates/waml-editor/src/tree_layout.rs`:

```rust
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

        layout.set_folder_open(&keys[0].to_string(), true);
        let keys: Vec<String> = layout.rows().iter().map(|r| r.key.clone()).collect();
        assert_eq!(keys.len(), 3, "folder + two children");
        assert_eq!(layout.rows()[1].depth, 1);
        assert_eq!(layout.rows()[2].depth, 1);
    }
}
```

Add the test-support helper to `crates/waml-editor/src/tree.rs` (module-level, `#[cfg(test)]` only — but it must be reachable from `tree_layout`'s tests, so gate it on `cfg(test)` at the crate level):

```rust
#[cfg(test)]
pub mod test_support {
    use super::*;

    /// A minimal leaf `TreeNode` whose `key_string` is `key`. Callers mutate
    /// the fields they care about.
    pub fn node(key: &str) -> TreeNode {
        TreeNode {
            key: waml::view::row::RowId::from_segments([key]),
            address: None,
            title: key.to_string(),
            kind: TreeKind::Unknown,
            presentation: DocumentPresentation::default(),
            is_directory: false,
            openable: false,
            concept_id: None,
            caps: waml::view::row::RowCaps::default(),
            child_caps: waml::view::row::ChildCaps::default(),
            view_degraded: false,
            children: Vec::new(),
        }
    }
}
```

If `RowId::from_segments` does not exist with that name, find the real constructor with `rg 'impl RowId' crates/waml/src/view/row.rs` and use it — do not invent one. Likewise check `TreeKind::Unknown` and `DocumentPresentation::default()` exist; if `DocumentPresentation` has no `Default`, construct it literally from its fields.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: FAIL to compile — `TreeLayout` not found.

- [ ] **Step 3: Write the minimal implementation**

```rust
//! The tree panel's geometry and state core. No `Cx`, no makepad draw types --
//! everything here is pure so the row math is unit-testable, the same split
//! `popup/menu.rs` uses for the menu list.

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
```

Add `mod tree_layout;` to `crates/waml-editor/src/lib.rs`, next to `mod tree;`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS (nothing consumes `tree_layout` yet).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/tree_layout.rs crates/waml-editor/src/lib.rs crates/waml-editor/src/tree.rs
git commit -m "feat(tree): add a pure row-flattening core for the tree panel"
```

---

### Task 2: Animated fold amounts in the core

Replaces the hard 0/1 fold with an animated amount per directory, matching the fork's 0.2s `ExpDecay`.

**Files:**
- Modify: `crates/waml-editor/src/tree_layout.rs`
- Test: inline tests in the same file

**Interfaces:**
- Consumes: `TreeLayout` from Task 1.
- Produces: `pub fn advance(&mut self, dt: f64) -> bool` (returns `true` while another frame is needed), and `set_folder_open` gains a `animate: bool` third parameter: `pub fn set_folder_open(&mut self, key: &str, open: bool, animate: bool)`. `pub const FOLD_SECS: f64 = 0.2;`

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: FAIL to compile — `advance` not found, `set_folder_open` takes 2 args.

- [ ] **Step 3: Write the implementation**

Replace the open-set handling in `tree_layout.rs` with animated amounts:

```rust
use std::collections::HashMap;

/// Fold transition duration in seconds. Matches the fork `FileTree`'s
/// `Play.Forward {duration: 0.2}` so the motion is indistinguishable.
pub const FOLD_SECS: f64 = 0.2;

/// Below this a folder is treated as fully closed and its children are not
/// flattened at all -- the same threshold the fork culled at.
const FOLD_CULL: f32 = 0.001;
```

Add to `TreeLayout`:

```rust
    /// Animated fold amount per directory key, 0.0 closed .. 1.0 open. A key
    /// absent from the map is closed. This is the authority: unlike the fork's
    /// per-node state, a culled folder is never forgotten, so a collapsing
    /// subtree cannot report itself closed while its rows are still drawn.
    fold: HashMap<String, f32>,
    /// Per-key animation target, present only while a fold is in flight.
    fold_target: HashMap<String, f32>,
```

```rust
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
```

```rust
/// Fraction of the remaining distance to cover for a step of `step` (in units
/// of the full duration). Exponential so the tail eases out like the fork's
/// `Ease.ExpDecay {d1: 0.80, d2: 0.97}` rather than arriving linearly.
fn ease_fraction(step: f32) -> f32 {
    1.0 - (0.02f32).powf(step)
}
```

Rewrite `reflow`/`flatten` to read the fold map:

```rust
    fn reflow(&mut self) {
        let mut rows = Vec::new();
        let roots = std::mem::take(&mut self.roots);
        flatten(&roots, 0, 1.0, &self.fold, &mut rows);
        self.roots = roots;
        self.rows = rows;
    }
```

```rust
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
```

Update Task 1's `closed_folders_hide_their_children` test to pass `false` for the new `animate` argument.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: PASS, all four tests.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/tree_layout.rs
git commit -m "feat(tree): animate fold amounts in the layout core"
```

---

### Task 3: Geometry, scroll and hit-testing

Row rects, scroll clamping, and the single-hop hit test that replaces `chevron_rects` + `chevron_hit`.

**Files:**
- Modify: `crates/waml-editor/src/tree_layout.rs`
- Test: inline tests in the same file

**Interfaces:**
- Consumes: `TreeLayout`, `VisibleRow`, `ROW_HEIGHT` from Tasks 1–2.
- Produces:
  - `pub fn set_viewport(&mut self, origin: DVec2, size: DVec2)`
  - `pub fn content_height(&self) -> f64`
  - `pub fn max_scroll(&self) -> f64`, `pub fn scroll(&self) -> f64`, `pub fn set_scroll(&mut self, scroll: f64)`
  - `pub fn row_rect(&self, index: usize) -> Rect`
  - `pub fn chevron_rect(&self, index: usize) -> Rect`
  - `pub fn row_at(&self, pos: DVec2) -> Option<usize>`
  - `pub enum TreeHit { Chevron(String), Row(String) }` and `pub fn hit(&self, pos: DVec2) -> Option<TreeHit>`
  - `pub fn scroll_key_into_view(&mut self, key: &str) -> bool`
  - `pub const CHEVRON_LEFT_MARGIN: f64 = 4.0; pub const CHEVRON_SIZE: f64 = 10.0; pub const ICON_DEPTH_INDENT: f64 = 10.0;` (moved from `tree_panel.rs` — take their real current values from there, do not retype from memory)

Note: `DVec2`/`Rect` come from `makepad_widgets::*`. These are plain math types, not draw state, so the core stays `Cx`-free and the tests stay pure.

- [ ] **Step 1: Write the failing test**

```rust
    use makepad_widgets::*;

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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: FAIL to compile — `set_viewport`, `row_rect`, `hit`, `TreeHit` not found.

- [ ] **Step 3: Write the implementation**

```rust
/// What a pointer position resolves to. `Chevron` only ever names a directory
/// row; every other position on a row -- and every position on a file row --
/// is `Row`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeHit {
    Chevron(String),
    Row(String),
}
```

Add to `TreeLayout`:

```rust
    origin: DVec2,
    size: DVec2,
    scroll: f64,
```

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/tree_layout.rs
git commit -m "feat(tree): add row geometry, scroll and hit-testing to the core"
```

---

### Task 4: Selection and hover state in the core

Moves selection into the core and adds the hover tracking the spec calls for.

**Files:**
- Modify: `crates/waml-editor/src/tree_layout.rs`
- Test: inline tests in the same file

**Interfaces:**
- Produces: `pub fn selected(&self) -> Option<&str>`, `pub fn set_selected(&mut self, key: Option<String>) -> bool`, `pub fn hover(&self) -> Option<&str>`, `pub fn set_hover_at(&mut self, pos: Option<DVec2>) -> bool` (returns `true` when the hovered row changed, i.e. a redraw is needed).

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: FAIL to compile — `set_hover_at`, `set_selected` not found.

- [ ] **Step 3: Write the implementation**

Add to `TreeLayout`:

```rust
    selected: Option<String>,
    hover: Option<String>,
```

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib tree_layout`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/tree_layout.rs
git commit -m "feat(tree): own selection and hover in the layout core"
```

---

### Task 5: Stateless row drawing

Moves the existing `draw_row_*` functions into their own module and adds the row label, which the fork used to draw.

**Files:**
- Create: `crates/waml-editor/src/tree_row_draw.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs` (remove the moved functions, re-export nothing — `tree_panel` calls the new module)
- Modify: `crates/waml-editor/src/lib.rs` (add `mod tree_row_draw;`)

**Interfaces:**
- Consumes: `VisibleRow`, `ROW_HEIGHT` from `tree_layout`; `IconSet`, `Icon`, `DrawChevron` from the existing code.
- Produces, all taking an absolute `rect` from `TreeLayout` rather than a `row_top`:
  - `pub fn row_fill(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, scale: f64)`
  - `pub fn row_icon(cx: &mut Cx2d, icons: &mut IconSet, icon: Icon, rect: Rect, depth: usize, color: Vec4, scale: f64)`
  - `pub fn row_chevron(cx: &mut Cx2d, draw: &mut DrawChevron, rect: Rect, open: f32, scale: f64)`
  - `pub fn row_diag_marker(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, scale: f64)`
  - `pub fn row_label(cx: &mut Cx2d, draw: &mut DrawText, rect: Rect, depth: usize, text: &str, scale: f64)`
  - `pub fn fade(color: Vec4, scale: f64) -> Vec4`

- [ ] **Step 1: Write the failing test**

Drawing needs a `Cx2d`, so the test covers the one pure helper plus the label's x-origin formula, which is the part that can silently drift from the old `padding.left: 38.0`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_scales_alpha_only() {
        let color = vec4(0.2, 0.4, 0.6, 0.8);
        let faded = fade(color, 0.5);
        assert_eq!((faded.x, faded.y, faded.z), (0.2, 0.4, 0.6));
        assert!((faded.w - 0.4).abs() < 1e-6);
    }

    #[test]
    fn label_starts_past_the_glyph_column() {
        // The fork sat labels at padding.left 38 plus indent_width 10 per depth.
        assert_eq!(label_x(0.0, 0, 1.0), 38.0);
        assert_eq!(label_x(0.0, 2, 1.0), 58.0);
        // Mid-collapse the whole column shrinks with the row.
        assert_eq!(label_x(0.0, 0, 0.5), 19.0);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib tree_row_draw`
Expected: FAIL to compile — module not found.

- [ ] **Step 3: Write the implementation**

Create `crates/waml-editor/src/tree_row_draw.rs`. Move the bodies of `draw_row_icon`, `draw_row_chevron`, `draw_row_diag_marker`, `draw_row_highlight` and `fade` from `tree_panel.rs` verbatim, changing only the positional parameter from `row_top: Vec2d` to `rect: Rect` (use `rect.pos` where the old code used `row_top`, and `rect.size.x` where it used the panel width). Keep every existing doc comment — they record why the pixel rounding and the fade exist.

Add the two new pieces:

```rust
/// Left edge of a row's label. The fork placed labels at `padding.left: 38.0`
/// with `indent_width: 10.0` per depth; reproduce both here so the glyph column
/// and the text stay aligned exactly as before.
pub const LABEL_LEFT: f64 = 38.0;
pub const LABEL_INDENT: f64 = 10.0;

fn label_x(row_x: f64, depth: usize, scale: f64) -> f64 {
    row_x + (LABEL_LEFT + depth as f64 * LABEL_INDENT) * scale
}

/// Draw a row's label. Previously the fork `FileTree`'s job; ours now.
///
/// Vertically centred in the row's SCALED band and faded with it, so a label
/// dissolves with its row mid-collapse instead of standing at full ink.
pub fn row_label(
    cx: &mut Cx2d,
    draw: &mut DrawText,
    rect: Rect,
    depth: usize,
    text: &str,
    scale: f64,
) {
    let color = draw.color;
    draw.color = fade(color, scale);
    let size = draw
        .layout(cx, 0.0, 0.0, None, false, Align::default(), text)
        .size_in_lpxs;
    let x = label_x(rect.pos.x, depth, scale).round();
    let y = (rect.pos.y + (rect.size.y - size.height as f64) / 2.0).round();
    draw.draw_abs(cx, dvec2(x, y), text);
    draw.color = color;
}
```

`row_fill` is `draw_row_highlight` renamed — it is used for both the selection tint and the new hover tint, so give it the neutral name.

Add `mod tree_row_draw;` to `crates/waml-editor/src/lib.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib tree_row_draw`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS. `tree_panel.rs` still uses `FileTree` at this point and must keep compiling — if a moved function is still referenced there, call it through `crate::tree_row_draw::`.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/tree_row_draw.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/lib.rs
git commit -m "refactor(tree): extract stateless row drawing, add the row label"
```

---

### Task 6: Draw the tree from the core

Replaces the `FileTree` child with an owned scrolled view and drives drawing entirely from `TreeLayout`. **Drawing only** — event routing is Task 7, so at the end of this task clicking does nothing. That split is deliberate: this task is where the pixels must match, and mixing input changes in would make a visual regression ambiguous.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (the `live_design!`/script block, `ProjectTree` fields, `draw_walk`)

**Interfaces:**
- Consumes: `TreeLayout` (Tasks 1–4), `tree_row_draw` (Task 5).
- Produces: `ProjectTree` gains `layout: TreeLayout` (`#[rust]`) and `draw_hover: DrawColor` (`#[live]`); loses the `file_tree` child.

- [ ] **Step 1: Replace the script block's tree child**

In the `live_design!` block, delete the `file_tree := FileTree { .. }` node (lines ~199–281, including `file_node`, `folder_node`, `filler` and the `node_height` / `auto_toggle_folders` settings) and replace it with an owned scrolled body inside the existing `tree_scroll` view:

```rust
        tree_scroll := View {
            width: Fill
            height: Fill
            flow: Down
            // We draw rows ourselves; this view exists to clip them and to own
            // the scrollbars. Same tinted bar the FileTree carried, so an
            // overflowing tree still visibly says "there's more".
            scroll_bars: ScrollBars {
                scroll_bar_y: ScrollBar {
                    draw_bg +: {
                        color: atlas.text_dim
                        color_hover: atlas.accent
                        color_drag: atlas.accent
                    }
                }
            }
        }
```

Add the row-text and hover pens alongside the existing `draw_selection`:

```rust
        // Row label ink. The fork FileTree drew labels with its own text style;
        // this reproduces it (fonts.text_menu, atlas.text) so rows read the same.
        draw_row_text +: {
            color: atlas.text
            text_style: fonts.text_menu
        }
        // Hover tint, painted BENEATH the selection fill so a hovered-and-
        // selected row still reads as selected.
        draw_hover: mod.draw.DrawColor{
            color: atlas.hover
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, 4.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
```

If `atlas.hover` does not exist, add it to the atlas next to `atlas.selection` at roughly half its alpha, in both light and dark — check `rg 'selection' crates/waml-editor/src/atlas*` for where the token lives.

**Do not delete `draw_title`.** Its field comment records that removing it silently blanks every row label; that cause is still not understood, and this task is not where to find out.

- [ ] **Step 2: Swap the widget's state over**

In `pub struct ProjectTree`, replace `tree: ProjectTreeData` with `layout: TreeLayout` (`#[rust]`), and add `#[live] draw_hover: DrawColor` plus `#[live] draw_row_text: DrawText`. Leave `id_to_key` / `id_to_concept` / `openable_ids` / `chevron_rects` / `pending_tap_count` / `pending_click_abs` in place for now — Task 7 deletes them, and deleting them here would break `handle_event` mid-task.

Keep `self.tree` readable where other methods need the node list by adding a passthrough:

```rust
    /// The current roots. `node_for_key` and `reveal_path` still walk the real
    /// `TreeNode` graph; only row layout moved into the core.
    fn roots(&self) -> &[TreeNode] {
        self.layout.roots()
    }
```

- [ ] **Step 3: Rewrite the draw loop**

Replace the `while let Some(step) = self.view.draw_walk(..)` block that borrows the `FileTree` (`tree_panel.rs:1075-1097`) with an owned pass. Delete `draw_nodes` entirely.

```rust
        // Draw the body, then paint our rows into the area it claimed.
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let body = self.view.view(cx, ids!(tree_scroll)).area().rect(cx);
        self.layout.set_viewport(body.pos, body.size);

        let mut reveal_was_drawn = false;
        for (index, row) in self.layout.rows().iter().enumerate() {
            let rect = self.layout.row_rect(index);
            // A row outside the clipped body draws nothing -- same cull the
            // fork applied, and it keeps the cost proportional to what's seen.
            if rect.pos.y + rect.size.y < body.pos.y || rect.pos.y > body.pos.y + body.size.y {
                if self.reveal_key.as_deref() == Some(row.key.as_str()) {
                    // Still flag it: a reveal target is usually off-screen,
                    // which is exactly why it needs scrolling to.
                    reveal_was_drawn = true;
                }
                continue;
            }

            if self.layout.hover() == Some(row.key.as_str()) {
                crate::tree_row_draw::row_fill(cx, &mut self.draw_hover, rect, row.scale);
            }
            if self.layout.selected() == Some(row.key.as_str()) {
                crate::tree_row_draw::row_fill(cx, &mut self.draw_selection, rect, row.scale);
            }
            if self.reveal_key.as_deref() == Some(row.key.as_str()) {
                reveal_was_drawn = true;
                self.draw_reveal.color = vec4(
                    self.reveal_color.x,
                    self.reveal_color.y,
                    self.reveal_color.z,
                    0.24 * self.reveal_strength,
                );
                crate::tree_row_draw::row_fill(cx, &mut self.draw_reveal, rect, row.scale);
            }

            let icon_color = if row.kind == TreeKind::Diagram {
                crate::accent::icon_tint(self.diagram_icon_color, self.icon_color)
            } else {
                self.icon_color
            };
            crate::tree_row_draw::row_icon(
                cx,
                &mut self.icons,
                row.icon,
                rect,
                row.depth,
                icon_color,
                row.scale,
            );
            if row.is_directory {
                crate::tree_row_draw::row_chevron(
                    cx,
                    &mut self.draw_chevron,
                    self.layout.chevron_rect(index),
                    row.fold,
                    row.scale,
                );
                if row.view_degraded {
                    crate::tree_row_draw::row_diag_marker(cx, &mut self.draw_diag, rect, row.scale);
                }
            }
            crate::tree_row_draw::row_label(
                cx,
                &mut self.draw_row_text,
                rect,
                row.depth,
                &row.title,
                row.scale,
            );
        }

        // Scroll-into-view is now a scroll offset, not a trigger sent at the
        // fork's area: the core owns the offset, so ask it directly.
        if let Some(key) = self.pending_scroll_key.take() {
            if self.layout.scroll_key_into_view(&key) {
                self.view.redraw(cx);
            }
        }
        let _ = reveal_was_drawn;
```

Delete the `cx.send_trigger(.., live_id!(scroll_focus_nav), ..)` block — that existed only to ask the fork widget to scroll.

- [ ] **Step 4: Build and run the gate**

Run: `cargo test --workspace`
Expected: PASS. Clicking is dead at this point (Task 7 restores it); tests that assert on click behaviour may fail — if so, mark them `#[ignore = "re-enabled in Task 7"]` with that exact reason and re-enable them in Task 7. Do not delete them.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs
git commit -m "feat(tree): draw the tree panel's rows from the layout core"
```

---

### Task 7: Route events through the core

Restores clicking, folding, hover and the fold animation frame loop, and deletes the `LiveId` bridge.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (`handle_event`, `set_view_with_fold_reset`, `reveal_target`, `toggle_directory`, struct fields)

**Interfaces:**
- Consumes: `TreeHit`, `TreeLayout` from Tasks 1–4.
- Produces: no new public API. `ProjectTreeAction` is unchanged.

- [ ] **Step 1: Re-enable and extend the widget tests**

Un-ignore anything Task 6 marked, and rewrite the synthetic-action scaffolding. Tests currently fabricate `FileTreeAction::FolderClicked(LiveId::from_str(&k(..)))` (`tree_panel.rs:1918`, `:1979`, `:1994`) and mount a real `FileTree` via `mounted_project_tree_test_context` (`tree_panel.rs:1465`). Replace those with direct core assertions:

```rust
    #[test]
    fn chevron_hit_folds_and_body_hit_navigates() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, sample_view());
        panel.layout.set_viewport(dvec2(0.0, 0.0), dvec2(280.0, 400.0));

        let folder = panel.layout.rows()[0].key.clone();
        assert!(panel.layout.rows()[0].is_directory);
        let open_before = panel.layout.is_folder_open(&folder);

        // Chevron: folds locally, emits nothing.
        let chevron = panel.layout.chevron_rect(0);
        assert_eq!(
            panel.layout.hit(chevron.pos + dvec2(2.0, 2.0)),
            Some(TreeHit::Chevron(folder.clone()))
        );

        // Body: same row, further right.
        let body = panel.layout.row_rect(0);
        assert_eq!(
            panel.layout.hit(dvec2(body.pos.x + 200.0, body.pos.y + 4.0)),
            Some(TreeHit::Row(folder.clone()))
        );
        assert_eq!(panel.layout.is_folder_open(&folder), open_before);
    }
```

Keep `mounted_project_tree_test_context` but drop the `FileTree` it constructs (`tree_panel.rs:1470-1475`) — it returns `(Cx, ProjectTree)` now. Update `file_tree_folder_is_open` (`tree_panel.rs:1483`) to call `panel.layout.is_folder_open(key)` and delete its `begin_folder`/`end_folder` probing.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib tree_panel`
Expected: FAIL to compile — the helper still returns three values / `TreeHit` unimported.

- [ ] **Step 3: Rewrite `handle_event`**

```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(frame) = self.reveal_next_frame.is_event(event) {
            self.update_reveal_pulse(cx, frame.time);
        }
        // Fold animation clock. One `NextFrame` while any fold is in flight;
        // the core reports when it has settled, so the loop stops on its own.
        if let Some(frame) = self.fold_next_frame.is_event(event) {
            let dt = if self.fold_last_time < 0.0 {
                1.0 / 60.0
            } else {
                (frame.time - self.fold_last_time).clamp(0.0, 0.1)
            };
            self.fold_last_time = frame.time;
            if self.layout.advance(dt) {
                self.fold_next_frame = cx.new_next_frame();
            } else {
                self.fold_last_time = -1.0;
            }
            self.view.redraw(cx);
        }

        let uid = self.widget_uid();
        self.view.handle_event(cx, event, scope);

        // Hover tracks MouseMove containment, NOT Hit::FingerHover: an arbiter
        // handing the hit to another widget must not strand the tint (bc53c22).
        if let Event::MouseMove(e) = event {
            let inside = self.view.area().rect(cx).contains(e.abs);
            if self.layout.set_hover_at(inside.then_some(e.abs)) {
                if self.layout.hover().is_some() {
                    crate::cursor::hover_in(cx, MouseCursor::Hand);
                } else {
                    crate::cursor::hover_out(cx);
                }
                self.view.redraw(cx);
            }
        }

        if let Hit::FingerDown(fe) = tree_panel_hit(event, cx, self.view.area()) {
            if fe.is_primary_hit() {
                match self.layout.hit(fe.abs) {
                    Some(TreeHit::Chevron(key)) => {
                        let open = self.layout.is_folder_open(&key);
                        self.layout.set_folder_open(&key, !open, true);
                        self.fold_next_frame = cx.new_next_frame();
                        self.view.redraw(cx);
                    }
                    Some(TreeHit::Row(key)) => {
                        if let Some(row) =
                            self.layout.rows().iter().find(|row| row.key == key).cloned()
                        {
                            if let Some(intent) = row_navigation(
                                row.address.as_deref(),
                                row.concept_id.as_deref(),
                                row.is_directory,
                                row.openable,
                                fe.tap_count,
                            ) {
                                cx.widget_action(uid, ProjectTreeAction::Navigate(intent));
                            }
                        }
                    }
                    None => {}
                }
            } else if let Some(TreeHit::Row(key)) = self.layout.hit(fe.abs) {
                // Secondary button: context menu, openable rows only.
                if self
                    .layout
                    .rows()
                    .iter()
                    .any(|row| row.key == key && row.openable)
                {
                    cx.widget_action(
                        uid,
                        ProjectTreeAction::ContextMenu {
                            key,
                            anchor: fe.abs,
                        },
                    );
                }
            }
        }

        if let Event::Actions(actions) = event {
            if self
                .view
                .icon_button(cx, ids!(view_mode_btn))
                .clicked(actions)
            {
                cx.widget_action(uid, ProjectTreeAction::ToggleViewMode);
            }
        }
    }
```

Check how the secondary button is actually distinguished in this codebase before writing that arm — `rg 'is_primary_hit|MouseButton|secondary' crates/waml-editor/src` — and match the existing idiom rather than inventing one. The old right-click path came from `file_tree.file_right_clicked(actions)`, which no longer exists.

Add the two new fields to `ProjectTree`: `#[rust] fold_next_frame: NextFrame` and `#[rust(-1.0)] fold_last_time: f64`.

- [ ] **Step 4: Delete the `LiveId` bridge**

Remove these fields and every use: `id_to_key`, `id_to_concept`, `openable_ids`, `directory_addresses`, `open_directories`, `chevron_rects`, `pending_tap_count`, `pending_click_abs`. Remove the functions `build_id_maps`, `chevron_hit`, `directory_addresses`, and `reconcile_open_directories`'s address plumbing.

Rewrite `set_view_with_fold_reset` to drive the core:

```rust
    pub fn set_view_with_fold_reset(&mut self, cx: &mut Cx, view: NavView, reset_folds: bool) {
        let (tree, tag) = match view {
            NavView::Browse(t) => (t, NavStateTag::Browse),
            NavView::Empty => (ProjectTreeData::default(), NavStateTag::Empty),
        };
        // Fold state is keyed by RowId, which is stable across a re-projection
        // (tree.rs:12-16), so an unchanged row keeps its fold through a mode
        // flip. `reset_folds` throws that away and re-seeds from the plan.
        let previous = if reset_folds {
            HashSet::new()
        } else {
            self.layout.open_keys().clone()
        };
        self.layout.set_roots(tree.roots.clone());
        let planned: HashSet<String> = folders_to_open(&tree).into_iter().collect();
        let live: HashSet<String> = self
            .layout
            .rows()
            .iter()
            .filter(|row| row.is_directory)
            .map(|row| row.key.clone())
            .collect();
        let mut open: HashSet<String> = previous.intersection(&live).cloned().collect();
        for key in planned.intersection(&live) {
            open.insert(key.clone());
        }
        self.layout.set_open_keys(open);
        self.nav_tag = tag;
        self.view.redraw(cx);
    }
```

`folders_to_open` currently returns addresses (`tree_panel.rs:573`) — change it to return `key_string(&node.key)` for each package folder, since the core keys on `RowId`. Same for `reveal_target`'s `ancestors`: `reveal_path` must yield ancestor **keys**, not addresses.

Rewrite `toggle_directory` to resolve address → key:

```rust
    /// Fold/unfold by OKF address. Address-taking because the app calls it that
    /// way; the core is keyed by `RowId`, so resolve through the visible rows.
    pub fn toggle_directory(&mut self, cx: &mut Cx, address: &str) -> bool {
        let Some(key) = self
            .layout
            .rows()
            .iter()
            .find(|row| row.is_directory && row.address.as_deref() == Some(address))
            .map(|row| row.key.clone())
        else {
            return false;
        };
        let open = self.layout.is_folder_open(&key);
        self.layout.set_folder_open(&key, !open, true);
        self.fold_next_frame = cx.new_next_frame();
        self.view.redraw(cx);
        true
    }
```

Point `set_selected_key` and `set_selected_document` at the core:

```rust
    pub fn set_selected_key(&mut self, cx: &mut Cx, key: Option<String>) {
        if self.layout.set_selected(key) {
            self.view.redraw(cx);
        }
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib tree_panel`
Expected: PASS.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS, including `opening_a_folder_highlights_its_row` in `app/tests/navigation.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs
git commit -m "feat(tree): route tree input through the layout core, drop the LiveId bridge"
```

---

### Task 8: Drop the `FileTree` dependency and the stale comment

The cleanup that makes the removal real.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (imports)
- Modify: `crates/waml-editor/src/app/navigation.rs:498-502` (delete the stale comment)
- Modify: `crates/waml-editor/src/app/tests/mod.rs`, `crates/waml-editor/src/app/tests/navigation.rs` (drop `FileTree` references)

- [ ] **Step 1: Verify nothing references `FileTree`**

Run: `rg 'FileTree' crates/ --glob '!*.md'`
Expected: only the imports you are about to delete. If anything else appears, fix it before continuing.

- [ ] **Step 2: Delete the stale comment**

In `crates/waml-editor/src/app/navigation.rs`, `transition_to_location` carries:

```rust
        // Re-submit the complete composed tree after the selection change.
        // Makepad's immediate-mode `FileTree` otherwise retains only the rows
        // visited before its clicked leaf on that redraw, making a trailing
        // Generic OKF row disappear until the next query/filter event.
        self.refresh_nav(cx, false);
```

Keep the `refresh_nav` call; replace the comment with:

```rust
        // Re-submit the composed tree after the selection change so the panel's
        // projection matches the newly active document.
        self.refresh_nav(cx, false);
```

- [ ] **Step 3: Remove the imports**

Delete `FileTree`, `FileTreeAction`, `FileTreeRef` and `Animate` from `tree_panel.rs`'s imports if now unused, and the same from the test modules. Let the compiler find them: `cargo build -p waml-editor 2>&1 | rg 'unused'`.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`
Expected: PASS with no warnings about unused imports.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src crates/waml-editor/src/app
git commit -m "refactor(tree): drop the FileTree dependency from the editor"
```

---

### Task 9: Revert the fork commits and bump the pin

**Do not start this task until the user has signed off the visual checks below.** Steps 1–8 must be verified against a running editor first; if the tree is wrong, the fix belongs in waml with the fork untouched.

**Files:**
- Modify (in `C:\dev\makepad`): `widgets/src/file_tree.rs`
- Modify: `Cargo.toml` (the `makepad` git `rev` pin)

- [ ] **Step 1: Confirm sign-off**

Confirm with the user that the visual checks passed. If they have not been run, stop and ask — do not proceed on assumption.

- [ ] **Step 2: Revert on the fork**

In `C:\dev\makepad`, on the `waml` integration branch:

```bash
git checkout waml
git pull --rebase
git revert --no-commit 92df3316 2ad35404 fbb881c5
git commit -m "revert(file_tree): drop the waml-only fold and draw hooks

The consumer moved into waml as an owned immediate-mode row list, so
last_node_drawn, folder_opened, current_scale and the app-owned folder
toggle have no callers. Returns file_tree.rs to stock."
git push
git rev-parse HEAD
```

If a revert conflicts, resolve toward stock upstream `file_tree.rs`.

- [ ] **Step 3: Bump the pin**

In waml's `Cargo.toml`, set the makepad `rev` to the SHA printed by `git rev-parse HEAD`. **A SHA, never a branch name** — pinning a branch tip makes builds non-reproducible.

- [ ] **Step 4: Rebuild and re-gate**

Run: `cargo test --workspace`
Expected: PASS against the reverted fork. A failure here means waml still depends on a reverted API — fix waml, not the fork.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): bump the makepad pin past the file_tree revert"
```

---

## Owed visual checks (user, not automated)

Nothing in the gate can see pixels. Run the editor with `pwsh run.ps1 -Title tree-owned-list` and check, **before Task 9**:

1. Rows, indent, glyphs and text baseline identical to the pre-change build (side-by-side, same fixture).
2. Fold/unfold motion matches the old 0.2s feel; no popping, no stuck partially-folded row.
3. Selection tracks the active tab — file rows **and** folder rows.
4. Hover tint reads correctly, including hovered-and-selected, and clears when a row scrolls out from under a stationary cursor.
5. Cursor resets when leaving the panel (no leak).
6. Scrollbar sits flush right, scrolls at the same feel, and rows scrolled out are not clickable.
7. Projected/raw toggle, reveal pulse, degraded-chain marker and right-click menu all still work.
8. Re-run 1–7 after Task 9's pin bump.
