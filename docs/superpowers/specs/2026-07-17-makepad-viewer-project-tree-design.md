# Makepad Viewer — Project Tree Panel

## Goal

Add a left-hand tree panel to the makepad viewer that renders the bundle's
**package/bundle hierarchy** and lets the user click a diagram to load it into
the existing `GraphCanvas`. Reuse makepad's shipped `FileTree` widget rather
than hand-rolling row rendering, scrolling, folding, and selection.

## Context

The viewer today (`crates/waml-editor`) is a single-window app: `GraphCanvas`
fills the window body and one diagram is loaded at startup
(`App::handle_startup` → `load_model` → `select_diagram` → `build_scene` →
`canvas.set_scene`). The `Model` is dropped after startup.

Relevant `waml::model` shapes:

- `Model.packages: Vec<Node>` — discovered `uml.Package` nodes. A root package
  always exists with `key == ""` (synthesized from `index.md`); nested packages
  carry `key` = their directory path.
- `Node.members: Vec<String>` — owned member keys (sub-packages, classifiers,
  diagrams, behaviors) in progressive-disclosure order. Meaningful on packages.
- Member keys resolve against five collections: `Model.nodes` (classifiers),
  `Model.diagrams`, `Model.packages`, `Model.flows`, `Model.interactions`.
- `Model.path` — the bundle/root name (root `index.md` H1); `""` when absent.

`crates/waml/src/index_md.rs` (`reindex_bundle`) is the precedent for walking
the package forest and building a unified `key → (title, kind)` resolver.

## Makepad `FileTree` (vendored at `C:/dev/vendor/makepad/widgets/src/file_tree.rs`)

`FileTree` is an **immediate-mode** widget. It provides, for free:

- vertical scrolling (built-in `ScrollBars`),
- fold/expand state (`open_nodes`),
- single-selection highlight + keyboard focus (`selected_node_id`),
- hover animation, even/odd row striping, a generic folder icon.

Its own `draw_walk` only draws blank fillers; a **container drives it** each
frame. The idiomatic pattern (from studio's `DesktopFileTree` /
`FlatFileTree`):

```rust
while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
    if let Some(mut file_tree) = step.as_file_tree().borrow_mut() {
        self.tree.draw(cx, &mut file_tree); // emits begin_folder/file
    }
}
```

`FileTree::begin_folder(cx, node_id, name)` / `end_folder()` bracket a folder's
children; `FileTree::file(cx, node_id, name)` emits a leaf. Node ids are
`LiveId::from_str(key)` — stable because member keys are unique bundle slugs.
On click it emits `FileTreeAction::FileClicked(LiveId)` /
`FolderClicked(LiveId)`; the container maps the `LiveId` back to a key.

## Architecture

Two new modules in `crates/waml-editor/src`, mirroring the existing
`scene.rs` (pure data seam) / `canvas.rs` (makepad widget) split.

### `tree.rs` — pure data seam (makepad-free)

Flattens a `Model` into a `ProjectTree`, unit-testable exactly like `scene.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeKind { Package, Class, Diagram, Behavior, Note, Unknown }

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub key: String,
    pub title: String,
    pub kind: TreeKind,
    pub children: Vec<TreeNode>, // packages only; leaves are empty
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectTree {
    pub roots: Vec<TreeNode>,
}

pub fn build_tree(model: &Model) -> ProjectTree;
```

Build:

1. Assemble a unified `key → (title, TreeKind)` map from all five collections.
   Kind derives from the resolved node's `ElementType`: `Uml(Package)` →
   `Package`, `Diagram` → `Diagram`, `Behavior(_)` → `Behavior`,
   `Uml(Note)` → `Note`, everything else classifier-ish → `Class`,
   `Unknown` → `Unknown`. Diagrams (own collection) → `Diagram`; flows /
   interactions → `Behavior`.
2. Root: the package with `key == ""`. Emit it as a single top `TreeNode`
   (kind `Package`) titled from `model.path` (fallback `"bundle"` when empty).
   Its children come from walking `members` in order; each member key resolves
   through the map. A member that is itself a package recurses (its own
   `members`); non-package members become leaves.
3. **Fallback:** if `model.packages` is empty, emit a flat list of all
   `model.diagrams` as depth-0 `Diagram` leaves under a single synthetic root,
   so the panel is never blank.

**Decision: keep `tree.rs` strictly makepad-free** — it produces only the
`ProjectTree` data. The `LiveId` bridge (`key → LiveId` via
`LiveId::from_str(key)`, and the reverse `LiveId → key`) lives in
`tree_panel.rs`, built by walking the `ProjectTree` once on `set_tree`. The
round-trip is still test-covered against the widget's id map (see Testing).

### `tree_panel.rs` — the `ProjectTree` widget (makepad)

A thin widget wrapping `FileTree`, mirroring studio's `DesktopFileTree` but
without the filter page or git-status dots.

- DSL: a `View` (or `#[deref] view: View`) containing
  `file_tree := FileTree{ node_height: … }`, themed to the canvas palette
  (`#x14161d` bg family).
- State: `#[rust] tree: ProjectTree`, plus a `#[rust] id_to_key: HashMap<LiveId,
  String>` and `#[rust] id_to_kind: HashMap<LiveId, TreeKind>` rebuilt on
  `set_tree`, and `#[rust] active_key: Option<String>` (the loaded diagram).
- `set_tree(cx, ProjectTree)`: store data, rebuild the id maps, open the root
  folder(s) by default (`set_folder_is_open`), redraw.
- `set_active_diagram(cx, key)`: record which diagram is loaded (used to keep
  the tree's selection in sync when the canvas changes; the `FileTree`'s own
  `selected_node_id` already highlights user clicks).
- `draw_walk`: step-loop; on `step.as_file_tree()`, recurse `self.tree`
  emitting `begin_folder`(Package)/`end_folder` and `file`(leaf) with
  `LiveId::from_str(key)`. **Icons:** encode kind as a glyph prefix in the row
  name string — `▤ ` diagram, `◻ ` class, `⤳ ` behavior, `✎ ` note, packages use
  the built-in folder icon. No shader work.
- `handle_event`: forward to the inner view; on
  `FileTreeAction::FileClicked(id)`, look up `id_to_kind[id]`; if `Diagram`,
  emit `ProjectTreeAction::SelectDiagram(id_to_key[id].clone())` via
  `cx.widget_action`. Folder clicks are handled by `FileTree` internally
  (fold/unfold); non-diagram file clicks just select.
- `ProjectTreeRef::selected_diagram(&self, actions) -> Option<String>` — the
  convenience reader the app calls, matching the `file_clicked` pattern.

### `app.rs` — layout + diagram-switch loop

- Window body becomes a `Splitter{}` (makepad reuse — resizable divider):
  left slot = `project_tree := ProjectTree{}`, right slot =
  `canvas := GraphCanvas{ width: Fill, height: Fill }`. Initial split sized so
  the tree gets ~280px.
- `App` gains `#[rust] model: waml::model::Model`.
- `handle_startup`: as today, but keep the model. After loading:
  `build_tree(&model)` → `project_tree.set_tree(cx, tree)`; select the initial
  diagram, `build_scene`, `canvas.set_scene`, `project_tree.set_active_diagram`.
- Action handling (in `App`'s `MatchEvent`): on
  `project_tree.selected_diagram(actions)` → find the diagram by key in
  `self.model.diagrams`, `build_scene(&self.model, diagram)`,
  `canvas.set_scene(cx, scene)`, `project_tree.set_active_diagram(cx, key)`.
  Log diagnostics as startup does.

`App` must register the new widget's `script_mod` alongside the canvas's in
`App::script_mod`.

## Data flow

```
startup: dir ──load_model──> Model ──build_tree──> ProjectTree ──set_tree──> panel
                              │
                              └select_diagram──> Diagram ──build_scene──> Scene ──set_scene──> canvas

click diagram in panel: FileTree ──FileClicked(id)──> ProjectTree
    ──SelectDiagram(key)──> App ──lookup+build_scene──> canvas.set_scene
                                 └──set_active_diagram──> panel
```

The `Model` lives on `App` for the session; the tree and scene are both
derived, rebuildable projections.

## Error handling / edge cases

- Empty `model.packages` → flat diagram fallback (never a blank panel).
- A `member` key that resolves to nothing in the map → skipped (same as
  `reindex_bundle`'s `filter_map`), not a crash.
- `model.path` empty → root folder titled `"bundle"`.
- `SelectDiagram(key)` for a key not in `model.diagrams` (shouldn't happen,
  since only `Diagram`-kind rows emit it) → no-op, logged.
- Clicking the already-active diagram → rebuild + set_scene is idempotent
  (canvas re-fits); acceptable, no special-casing.

## Testing

- `tree.rs` pure unit tests, in the style of `scene.rs`:
  - mini fixture → expected `ProjectTree` roots/children (kinds, order,
    nesting).
  - a nested-package fixture (or hand-built `Model`) → recursion + child order.
  - empty-`packages` `Model` → flat diagram fallback.
  - unknown/dangling member key → skipped.
- Widget id-map round-trip: `build_tree` → walk → `LiveId::from_str(key)` →
  recover key (covered against the widget's `id_to_key`).
- Extend the existing headless render check to the two-pane (`Splitter`)
  layout so the panel + canvas both draw without panicking.

## Non-goals (fast-follows)

- Clicking a classifier/behavior to focus/highlight it on the canvas (this MVP
  only wires diagram → canvas).
- Editing, drag-reorder, or context menus in the tree.
- Persisting fold/scroll state across runs.
- Multi-select or search/filter in the tree.
