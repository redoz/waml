# Tree Panel Header — Scope, Search, Type-Filter & Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a header band to the makepad `ProjectTree` panel — a title-dropdown scope picker, a search field with a rotating type-filter chip, collapse + pin chrome, and hover-driven translucency shared in behaviour with the inspector — backed by a new pure `nav.rs` scope/search/filter seam.

**Architecture:** A new pure module `crates/waml-editor/src/nav.rs` sits on top of `tree::build_tree` and projects a `Model` + `NavState { scope, query, filter }` into a `NavView` (Browse / Results / Elsewhere / Empty) the widget renders. The **app** owns `NavState`, rebuilds `NavView` on every scope/query/filter change, and pushes it via `set_view` (replacing today's `set_tree`). The **widget** stays a thin renderer that overlays an immediate-mode header band over the existing Atlas-framed `FileTree`; it emits `ScopeRequest` / `Query` / `RotateFilter` actions to the app and handles collapse + hover translucency panel-locally. The title dropdown reuses the landed `PopupRoot` seam.

**Tech Stack:** Rust, makepad (redoz fork, git rev `525aae6a`). Widget draws are immediate-mode `DrawColor`/`DrawText` over a `View` deref (the same hybrid `inspector_panel.rs` uses). Pure logic is colocated-`#[cfg(test)]` unit-tested like `tree.rs`. SDF glyphs are hand-authored in `icons.rs`.

## Global Constraints

- **Clean-room.** The nav logic is designed from scratch. Do NOT read or port `packages/core/src/nav/*` or `packages/web/src/components/NavigatorBody.svelte`. No OWOX-authored code (see `no-port-owox-code`).
- **Scope is set ONLY via the title dropdown.** No breadcrumb, no row double-click / right-click / command-wheel scope-in, no `×` close button. Out of scope entirely: context menu, drag-reorder, CRUD (rename/delete/create), a shared `PanelChrome` unit.
- **Chrome is per-panel per-widget.** The inspector grows the same hover-translucency behaviour, but implemented independently in `inspector_panel.rs` — no shared unit.
- **Icon catalog is add-only** and must respect the load-bearing order invariant: `enum == field == DSL == get == ALL == label`, plus the `Icon::ALL` count bump and the `icon_all_has_N_entries` test. Never reorder or drop existing glyphs (see `keep-unused-catalog-icons`).
- **Hand-rolled search field**, not the fork `TextInput` — matching the house convention in `inspector_panel.rs` / `doc_tabs.rs` (rects captured in `draw_walk`, `cx.set_key_focus`, `Hit::KeyDown`/`Hit::TextInput`). The pinned makepad `View` exposes **no** `opacity` field, so translucency is a bg-colored scrim quad, not a widget opacity.
  - **Open decision (redoz@, 2026-07-22):** floated adding a real `opacity` field to the makepad fork `View` instead. The scrim is the default here because it is zero-fork and lands fast, but it has a real flaw — it fades toward the window bg token, which is wrong where a panel overlaps the *canvas* (a different colour), and it can't fade child-widget text per-alpha. A fork `View.opacity` (multiply the draw-list output alpha) is visually correct and would replace the Task 7/8 scrim with a single `view.set_opacity(cx, opacity)` call, at the cost of a fork edit + build-cache bump (see `arc-spike-is-fxc-skip-opt` / fork-rev pinning notes). Pick one before/at Task 7; if the fork route wins, Tasks 7–8 drop `draw_scrim` entirely.
- **`nav.rs` is pure** — no `makepad_widgets`, no `Cx`. It reuses `tree::{build_tree, ProjectTree, TreeNode, TreeKind}`.
- **redoz@ works in a worktree**, never editing the main checkout directly. This plan's code lands in an isolated worktree (created via `superpowers:using-git-worktrees` at execution time); integrate to `main` often and push `origin/main`.
- **Spec:** `docs/superpowers/specs/2026-07-22-tree-panel-header-scope-design.md`.

---

## File Structure

- `crates/waml-editor/src/nav.rs` — CREATE. The pure scope/search/filter seam: `NavState`, `NavView`, `PackageRow`, `view`, `packages`, `kinds_in_model`, `kind_label`, `chip_label`. All unit tests colocated.
- `crates/waml-editor/src/icons.rs` — MODIFY. Add one glyph, `Search` (a magnifier), appended at the end of every parallel list (shader, field, DSL binding, `get` arm, `ALL`, `label`); bump `ALL` count 89 → 90.
- `crates/waml-editor/src/tree_panel.rs` — MODIFY. Rename the renderer's input from `set_tree(ProjectTreeData)` to `set_view(NavView)`; add the immediate-mode header band (title trigger, search field + magnifier, type chip, collapse, pin), panel-local collapse + hover-scrim translucency, empty-state text, and the new action enum (`ScopeRequest`/`Query`/`RotateFilter`).
- `crates/waml-editor/src/inspector_panel.rs` — MODIFY. Add the same hover-scrim translucency (opacity 0.55 idle, 1.0 hovered-or-pinned) alongside the existing pin.
- `crates/waml-editor/src/app.rs` — MODIFY. Own `NavState`; replace the two `build_tree` + `set_tree` call sites with `nav::view` + `set_view`; handle `ScopeRequest` (open the title dropdown via `PopupRoot` from `nav::packages`), the dropdown's `closed` result (`ScopeTo`), `Query`, and `RotateFilter`; rebuild + re-push `NavView` on each.
- `crates/waml-editor/src/lib.rs` (or the module-declaring root — verify with `grep -n "mod tree;" crates/waml-editor/src/*.rs`) — MODIFY. Add `mod nav;`.

The nav pass is pure and runs before any widget touch; the widget consumes only the projected `ProjectTree` inside a `NavView` plus a small state tag for empty-state rendering.

---

### Task 1: Add the `Search` magnifier glyph to the icon catalog

Adds one hand-authored SDF glyph, `Icon::Search`, for the search field's leading magnifier. Pure add-only edit across the six parallel lists in `icons.rs`, guarded by the catalog's own count/order tests.

**Files:**
- Modify: `crates/waml-editor/src/icons.rs` (shader `IconSearch`; DSL binding; field `search`; `get` arm; `ALL` entry + count 89→90; `label` arm; update `icon_all_has_89_entries` test)

**Interfaces:**
- Produces: `Icon::Search` (new last variant), `IconSet.search: DrawColor`, `IconSet::get(Icon::Search)`, `Icon::ALL` len 90, `Icon::Search.label() == "search"`.

- [ ] **Step 1: Update the count test to expect 90 (make it fail first)**

In `crates/waml-editor/src/icons.rs`, the `tests` module, change the existing assertion:

```rust
    #[test]
    fn icon_all_has_90_entries() {
        assert_eq!(Icon::ALL.len(), 90);
    }
```

(Rename the fn from `icon_all_has_89_entries` to `icon_all_has_90_entries` and change `89` → `90`.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p waml-editor --lib icons::tests::icon_all_has_90_entries`
Expected: FAIL — `Icon::ALL` still has 89 entries (`assert_eq!(89, 90)`), and/or a compile error once you also touch `ALL` in step 4. If it fails to compile because the old name is referenced nowhere, that's fine; the assertion failure is the target.

- [ ] **Step 3: Add the `IconSearch` shader**

In the `script_mod! { ... }` block in `icons.rs`, immediately after the `mod.draw.IconDoorOpen` shader definition (the last icon shader, just before `mod.widgets.IconSetBase = ...`), add:

```rust
    // Search: magnifier — lens circle (upper-left) + diagonal handle (lower-right).
    // Hand-authored (no Lucide port); tuned live in the `icon_harness` bin.
    mod.draw.IconSearch = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Lens: full circle centered (0.4375,0.4375) r=0.2917, two half-arcs.
            sdf.move_to(s * 0.7292, s * 0.4375)
            sdf.arc_to(s * 0.4375, s * 0.4375, s * 0.2917, 0.0000, 3.1416)
            sdf.arc_to(s * 0.4375, s * 0.4375, s * 0.2917, 3.1416, 6.2832)
            sdf.close_path()
            sdf.stroke(self.color, w)
            // Handle: from the lens's lower-right edge (45deg) to the corner.
            sdf.move_to(s * 0.6437, s * 0.6437)
            sdf.line_to(s * 0.8750, s * 0.8750)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }
```

- [ ] **Step 4: Add the field, DSL binding, `get` arm, `ALL` entry, and `label` arm — all appended last**

Four appends, each after the current final `door_open` / `DoorOpen` entry in its list (keep the invariant: `Search` is last everywhere).

DSL binding — after `door_open: mod.draw.IconDoorOpen{ color: atlas.accent }` (last line before the closing `}` of `mod.widgets.IconSet`):

```rust
        search: mod.draw.IconSearch{ color: atlas.accent }
```

Struct field — after `pub door_open: DrawColor,` (last field of `struct IconSet`):

```rust
    #[live]
    pub search: DrawColor,
```

`get` match arm — after `Icon::DoorOpen => &mut self.door_open,`:

```rust
            Icon::Search => &mut self.search,
```

`enum Icon` variant — after `DoorOpen,`:

```rust
    Search,
```

`Icon::ALL` — bump the array length and append the entry: change `pub const ALL: [Icon; 89]` → `pub const ALL: [Icon; 90]`, and after `Icon::DoorOpen,` add:

```rust
        Icon::Search,
```

`label` match arm — after `Icon::DoorOpen => "door-open",`:

```rust
            Icon::Search => "search",
```

- [ ] **Step 5: Run the catalog tests to verify they pass**

Run: `cargo test -p waml-editor --lib icons::`
Expected: PASS — `icon_all_has_90_entries`, `icon_all_is_in_field_order_at_the_edges` (unaffected — it checks indices 0/1/85–88, all unchanged), `icon_labels_are_unique_and_nonempty` (now 90 unique), `label_reflects_lucide_slugs_not_field_names`.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/icons.rs
git commit -m "feat(icons): add Search magnifier glyph"
```

---

### Task 2: `nav.rs` — types, module wiring, `kind_label`/`chip_label`, and `kinds_in_model`

Creates the pure module with its public types and the two label helpers plus the type-filter chip's kind list. Establishes the module so later tasks extend one file.

**Files:**
- Create: `crates/waml-editor/src/nav.rs`
- Modify: the module root that declares `mod tree;` (add `mod nav;`)

**Interfaces:**
- Produces:
  - `pub struct NavState { pub scope: String, pub query: String, pub filter: Option<TreeKind> }` — derives `Debug, Clone, Default, PartialEq`.
  - `pub enum NavView { Browse(ProjectTree), Results(ProjectTree), Elsewhere(ProjectTree), Empty }` — derives `Debug, Clone, PartialEq`.
  - `pub struct PackageRow { pub key: String, pub title: String, pub depth: usize }` — derives `Debug, Clone, PartialEq`.
  - `pub fn kind_label(kind: TreeKind) -> &'static str` — de-prefixed display name ("Package", "Class", …).
  - `pub fn chip_label(filter: Option<TreeKind>) -> &'static str` — `"All"` for `None`, else `kind_label`.
  - `pub fn kinds_in_model(model: &Model) -> Vec<TreeKind>` — distinct kinds present, in canonical (enum-declaration) order.
- Consumes: `crate::tree::{build_tree, ProjectTree, TreeNode, TreeKind}`, `waml::model::Model`.

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-editor/src/nav.rs` with the module skeleton and this test module. The `mini()` fixture loader mirrors `tree.rs`:

```rust
//! The nav seam: project a `Model` + `NavState` into a `NavView` the tree panel
//! renders. Pure — no makepad, no `Cx` — and unit-tested like `tree.rs`. Sits on
//! top of `tree::build_tree`; clean-room (not a port of the web navigator).

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
                node("", ElementType::Uml(UmlMetaclass::Package), "Root", vec!["sub", "iface"]),
                node("sub", ElementType::Uml(UmlMetaclass::Package), "Sub Pkg", vec!["cls"]),
            ],
            nodes: vec![
                node("cls", ElementType::Uml(UmlMetaclass::Class), "Customer", vec![]),
                node("iface", ElementType::Uml(UmlMetaclass::Interface), "Payments", vec![]),
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
        assert_eq!(kinds, vec![TreeKind::Package, TreeKind::Class, TreeKind::Interface]);
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
```

- [ ] **Step 2: Declare the module and run to verify failure**

Find the module root: `grep -n "mod tree;" crates/waml-editor/src/*.rs`. In that file, add `mod nav;` next to `mod tree;`.

Run: `cargo test -p waml-editor --lib nav::tests`
Expected: FAIL to compile until `mod nav;` is added, then PASS the three tests (the implementation above is complete). If it compiles and passes on the first run, that is acceptable — the deliverable is green tests; there is no separate "impl" step for this pure task because the code and tests were authored together.

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p waml-editor --lib nav::tests`
Expected: PASS — 3 tests.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/nav.rs crates/waml-editor/src/<module-root>.rs
git commit -m "feat(nav): NavState/NavView types + kind labels + kinds_in_model"
```

---

### Task 3: `nav.rs` — `packages()` (title-dropdown rows)

Adds the package-only nested row list for the title dropdown, prepended with the synthetic whole-model root row.

**Files:**
- Modify: `crates/waml-editor/src/nav.rs`

**Interfaces:**
- Produces: `pub fn packages(model: &Model) -> Vec<PackageRow>` — row 0 is the synthetic root (`key: ""`, `title: model.path` or `"Untitled"`, `depth: 0`); real sub-packages follow, depth-indented from 1.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `nav.rs`:

```rust
    #[test]
    fn packages_lead_with_synthetic_root_then_nest_real_packages() {
        let rows = packages(&built());
        // Row 0: synthetic whole-model root, key "", titled from model.path.
        assert_eq!(rows[0], PackageRow { key: String::new(), title: "Root".to_string(), depth: 0 });
        // The one real sub-package, indented to depth 1. (Only packages appear;
        // `cls`/`iface` classifiers are excluded.)
        assert_eq!(
            rows.iter().map(|r| (r.key.as_str(), r.depth)).collect::<Vec<_>>(),
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib nav::tests::packages`
Expected: FAIL — `packages` is not defined.

- [ ] **Step 3: Implement `packages`**

Add to `nav.rs` (after `kinds_in_model`):

```rust
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
    let mut out = vec![PackageRow { key: String::new(), title: root_title, depth: 0 }];
    fn walk(nodes: &[TreeNode], depth: usize, out: &mut Vec<PackageRow>) {
        for n in nodes {
            if n.kind == TreeKind::Package {
                out.push(PackageRow { key: n.key.clone(), title: n.title.clone(), depth });
                walk(&n.children, depth + 1, out);
            }
        }
    }
    if let Some(root) = full.roots.first() {
        walk(&root.children, 1, &mut out);
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml-editor --lib nav::tests::packages`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/nav.rs
git commit -m "feat(nav): packages() for the title scope dropdown"
```

---

### Task 4: `nav.rs` — `view()` browse state (scope + type filter)

Adds `view()` covering the no-query path: root the display at the scope's subtree, apply the type filter (keep matching kinds + ancestor packages), return `Browse`.

**Files:**
- Modify: `crates/waml-editor/src/nav.rs`

**Interfaces:**
- Produces: `pub fn view(model: &Model, state: &NavState) -> NavView`. With `state.query` empty, always returns `NavView::Browse(_)`.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `nav.rs`:

```rust
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
        let state = NavState { scope: "sub".to_string(), ..Default::default() };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        // "sub"'s members at depth 0; "sub" itself is not shown.
        assert_eq!(flat(t), vec![("cls".to_string(), TreeKind::Class)]);
    }

    #[test]
    fn type_filter_keeps_matching_kinds_and_ancestor_packages_prunes_rest() {
        let state = NavState { filter: Some(TreeKind::Class), ..Default::default() };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        // Only the Class survives, but its ancestor package "sub" is retained for
        // structure; the sibling Interface "iface" is pruned.
        assert_eq!(
            flat(t),
            vec![("sub".to_string(), TreeKind::Package), ("cls".to_string(), TreeKind::Class)]
        );
    }

    #[test]
    fn type_filter_on_package_keeps_package_rows() {
        let state = NavState { filter: Some(TreeKind::Package), ..Default::default() };
        let v = view(&built(), &state);
        let t = browse_roots(&v);
        assert_eq!(flat(t), vec![("sub".to_string(), TreeKind::Package)]);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib nav::tests`
Expected: FAIL — `view` is not defined.

- [ ] **Step 3: Implement `view` (browse path) + private helpers**

Add to `nav.rs`:

```rust
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
                Some(TreeNode { children: kids, ..n.clone() })
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml-editor --lib nav::tests`
Expected: PASS — the four new browse tests plus all earlier nav tests.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/nav.rs
git commit -m "feat(nav): view() browse — scope rooting + type filter"
```

---

### Task 5: `nav.rs` — `view()` search states (Results / Elsewhere / Empty)

Completes `view()`: with a non-empty query, prune non-matching leaves (keep matching branches) within scope → `Results`; if none in scope but some in the whole model → `Elsewhere`; if none anywhere → `Empty`.

**Files:**
- Modify: `crates/waml-editor/src/nav.rs`

**Interfaces:**
- Produces: `view` now returns `Results`/`Elsewhere`/`Empty` for non-empty queries (browse behaviour from Task 4 unchanged for empty queries).

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module in `nav.rs`:

```rust
    #[test]
    fn query_prunes_non_matching_leaves_and_keeps_matching_branches() {
        let state = NavState { query: "custom".to_string(), ..Default::default() };
        let v = view(&built(), &state);
        let t = match &v {
            NavView::Results(t) => t,
            other => panic!("expected Results, got {other:?}"),
        };
        // "Customer" matches; its ancestor "sub" is kept; "Payments" is pruned.
        assert_eq!(
            flat(t),
            vec![("sub".to_string(), TreeKind::Package), ("cls".to_string(), TreeKind::Class)]
        );
    }

    #[test]
    fn query_is_case_insensitive() {
        let state = NavState { query: "PAYMENTS".to_string(), ..Default::default() };
        match view(&built(), &state) {
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
            scope: "sub".to_string(),
            query: "payments".to_string(),
            ..Default::default()
        };
        let v = view(&built(), &state);
        let t = match &v {
            NavView::Elsewhere(t) => t,
            other => panic!("expected Elsewhere, got {other:?}"),
        };
        assert!(flat(t).iter().any(|(k, _)| k == "iface"));
    }

    #[test]
    fn no_match_anywhere_is_empty() {
        let state = NavState { query: "zzzznope".to_string(), ..Default::default() };
        assert_eq!(view(&built(), &state), NavView::Empty);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib nav::tests`
Expected: FAIL — the query still returns `Browse` (from Task 4's placeholder), so the `Results`/`Elsewhere`/`Empty` matches panic.

- [ ] **Step 3: Finish `view` and add `query_prune`**

Add the helper and replace the query-path tail of `view`:

```rust
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
                Some(TreeNode { children: kids, ..n.clone() })
            } else {
                None
            }
        })
        .collect()
}
```

Then replace the placeholder tail of `view` (everything after the `if state.query.trim().is_empty()` block) with:

```rust
    let in_scope = query_prune(&filtered, &state.query);
    if !in_scope.is_empty() {
        return NavView::Results(ProjectTree { roots: in_scope });
    }
    // Nothing in scope: search the whole model (same depth-0 base as scope "").
    let whole = scoped_roots(&full, "");
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
```

Note: when `scope == ""`, `whole_filtered == filtered`, so `elsewhere == in_scope == []` and the result is `Empty` — the correct fall-through, no special-case needed.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p waml-editor --lib nav::`
Expected: PASS — all nav tests (browse, packages, kinds, and the four search-state tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/nav.rs
git commit -m "feat(nav): view() search states — Results/Elsewhere/Empty"
```

---

### Task 6: `tree_panel.rs` — render `NavView` via `set_view` (replace `set_tree`)

Swaps the widget's input from a bare `ProjectTree` to a `NavView`, storing the projected roots plus a state tag for later empty-state rendering. Rewires the app's two feed sites. No header band yet — the tree body renders exactly as today for the `Browse`/`Results`/`Elsewhere` cases and blank for `Empty`.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (add `use crate::nav::NavView;`; `set_view`; internal state tag; keep `draw_nodes` over the inner roots)
- Modify: `crates/waml-editor/src/app.rs` (both `build_tree(...) + set_tree(...)` sites → `nav::view(...) + set_view(...)`)

**Interfaces:**
- Consumes: `crate::nav::NavView`.
- Produces: `ProjectTree::set_view(&mut self, cx: &mut Cx, view: NavView)` replacing `set_tree`. Internal `NavStateTag { Browse, Results, Elsewhere, Empty }` (private) drives Task 8's empty-state text.

- [ ] **Step 1: Add `set_view` and the state tag; keep the roots-rendering path**

In `tree_panel.rs`, add `use crate::nav::NavView;` and a private tag enum near the top:

```rust
/// Which projection the panel is showing, for the header note + empty state
/// (Task 8). The rendered rows live in `self.tree`; this only records intent.
#[derive(Clone, Copy, PartialEq, Default)]
enum NavStateTag {
    #[default]
    Browse,
    Results,
    Elsewhere,
    Empty,
}
```

Add a `#[rust] nav_tag: NavStateTag,` field to `struct ProjectTree` (next to `tree`). Replace `pub fn set_tree` with:

```rust
    pub fn set_view(&mut self, cx: &mut Cx, view: NavView) {
        let (tree, tag) = match view {
            NavView::Browse(t) => (t, NavStateTag::Browse),
            NavView::Results(t) => (t, NavStateTag::Results),
            NavView::Elsewhere(t) => (t, NavStateTag::Elsewhere),
            NavView::Empty => (ProjectTreeData::default(), NavStateTag::Empty),
        };
        let (id_to_key, id_to_kind) = build_id_maps(&tree);
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        // Open every top-level package by default so the panel isn't collapsed.
        // (Under scope the roots are the scope's members, not one wrapper.)
        for root in &tree.roots {
            if matches!(root.kind, TreeKind::Package) {
                file_tree.set_folder_is_open(cx, LiveId::from_str(&root.key), true, Animate::No);
            }
        }
        self.id_to_key = id_to_key;
        self.id_to_kind = id_to_kind;
        self.tree = tree;
        self.nav_tag = tag;
        self.view.redraw(cx);
    }
```

Keep `draw_nodes`, `set_selected_key`, `selected_diagram`, `focused_classifier` unchanged — they already operate on `self.tree.roots`.

- [ ] **Step 2: Rewire the app's two feed sites**

In `app.rs`, both existing sites read (near lines 556 and 697):

```rust
let tree = crate::tree::build_tree(&self.model, &self.open_name);
// ... borrow project_tree as panel ...
panel.set_tree(cx, tree);
```

Replace each with (the app owns `NavState` from Task 9; for this task use a default state so behaviour matches today — whole-model browse):

```rust
let view = crate::nav::view(&self.model, &self.nav_state);
// ... borrow project_tree as panel ...
panel.set_view(cx, view);
```

Add the field to `struct App`: `#[rust] nav_state: crate::nav::NavState,` (near `open_name`). With `NavState::default()` (scope `""`, query `""`, filter `None`), `nav::view` returns `Browse` of the whole model at depth 0 — the root wrapper row disappears, but every package/leaf is present, so the tree reads the same minus the redundant "Untitled" top folder.

- [ ] **Step 3: Build + run the existing tree tests**

Run: `cargo build -p waml-editor` then `cargo test -p waml-editor --lib tree_panel::`
Expected: PASS/compile — `id_maps_round_trip_key_and_kind` and `tree_kind_maps_to_catalog_icon` are unaffected (they call `build_id_maps` / `icon_for` directly, not `set_view`).

- [ ] **Step 4: Manual smoke check**

Run: `cargo run -p waml-editor` (open the mini bundle). Expected: the tree panel renders the model's packages/classes/diagrams as before, now without the single "Untitled" wrapper folder at the very top (its children are the top rows). Clicking a diagram/classifier row still works.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app.rs
git commit -m "refactor(tree-panel): render NavView via set_view (replaces set_tree)"
```

---

### Task 7: Inspector hover-scrim translucency

Adds the shared hover-translucency behaviour to the inspector first (it is the smaller, self-contained surface, and proves the scrim approach before the tree panel reuses it): opacity 1.0 when the pointer is over the panel or it is pinned, else 0.55, realized as a bg-colored scrim quad drawn last.

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs` (hover tracking; `draw_scrim`; DSL token; wire the existing `pinned`)

**Interfaces:**
- Produces: no new public API — panel-local. The existing `pinned` field now also locks opacity to 1.0.

- [ ] **Step 1: Add the scrim draw field + DSL token**

In `inspector_panel.rs`, add a `#[live] draw_scrim: DrawColor` field to `struct Inspector` and, in the `script_mod!` `Inspector` DSL, a token pointing it at the window background so the fade reads as true translucency toward what sits behind the panel:

```rust
        // Hover-translucency scrim: a window-bg quad painted over the whole panel
        // at alpha (1 - opacity), so an unhovered/unpinned panel dims toward the
        // backdrop. `atlas.bg` is the app window's background token.
        draw_scrim +: { color: atlas.bg }
```

(Verify the exact background token name: `grep -n "bg" crates/waml-editor/src/theme_atlas.rs` — use the window/app background swatch, not `field_bg`.)

Add a `#[rust] hovered: bool,` field to `struct Inspector`.

- [ ] **Step 2: Track hover in `handle_event`**

In `Inspector::handle_event`, alongside the existing `hits_with_capture_overload` match, add hover in/out handling that redraws on change:

```rust
            Hit::FingerHoverIn(_) => {
                if !self.hovered {
                    self.hovered = true;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.hovered {
                    self.hovered = false;
                    self.view.redraw(cx);
                }
            }
```

- [ ] **Step 3: Draw the scrim last in `draw_walk`**

At the very end of `Inspector::draw_walk`, just before the final `DrawStep::done()`, paint the scrim over the panel rect (`rect` = `self.view.area().rect(cx)`, captured earlier as `self.view_rect`):

```rust
        // Hover translucency (last, over everything): opaque panel when hovered
        // or pinned, else dim to 0.55 via a (1 - opacity) backdrop scrim.
        let opacity = if self.hovered || self.pinned { 1.0 } else { 0.55 };
        if opacity < 1.0 {
            self.draw_scrim.color.w = (1.0 - opacity) as f32;
            self.draw_scrim.draw_abs(cx, self.view_rect);
        }
```

(`self.view_rect` is already set near the top of `draw_walk`; if the collapsed early-returns skip past this, add the scrim before each `return DrawStep::done()` too, or compute `opacity`/scrim once in a small closure. Simplest: hoist the scrim into a helper `fn draw_scrim(&mut self, cx)` and call it before every `DrawStep::done()` return in `draw_walk`.)

- [ ] **Step 4: Build + manual check**

Run: `cargo run -p waml-editor`. Expected: with the pointer off the inspector it dims to ~0.55; moving onto it restores full opacity; toggling its pin locks it opaque even when the pointer leaves.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): hover-scrim translucency, pin locks opaque"
```

---

### Task 8: `tree_panel.rs` — header band chrome (title trigger, collapse, pin, hover scrim)

Adds the immediate-mode header band above the `FileTree`: the scope-title trigger (label + `⌄`), the right-cluster collapse + pin glyphs, panel-local collapse (hides the body), and the same hover-scrim translucency. Emits `ScopeRequest` (title click → app opens the dropdown). Collapse + pin + hover are panel-local (no app round-trip). Search row + type chip land in Task 9.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (header spacer child; `ProjectTreeAction` gains `ScopeRequest`; immediate draws; hit rects; hover/collapse/pin state; scope-title setter)

**Interfaces:**
- Produces:
  - `ProjectTreeAction::ScopeRequest { anchor: Rect }` — the title trigger's open-request; the app relays it to `PopupRoot`.
  - `ProjectTree::set_scope_title(&mut self, cx: &mut Cx, title: String)` — the current scope label shown in the header.
  - `ProjectTree::scope_request(&self, actions: &Actions) -> Option<Rect>` — reader for `App`, mirroring `selected_diagram`.

- [ ] **Step 1: Reserve the header strip + restructure the DSL**

In the `script_mod!` `ProjectTree` DSL, make the widget `flow: Down` and add a header spacer as the first child, above `file_tree`:

```rust
        flow: Down
        // Header band: an empty spacer reserving the top strip; the title trigger,
        // collapse/pin glyphs, and (Task 9) the search row + type chip are all
        // hand-drawn immediate-mode in `draw_walk`, same hybrid as the inspector.
        header := View {
            width: Fill
            height: 64.0
        }
```

Add header ink/scrim draw fields + tokens like the inspector: `draw_title: DrawText`, `draw_dim: DrawText`, `draw_scrim: DrawColor` on `struct ProjectTree`, with DSL tokens (`draw_title` → `atlas.text` at ~16px; `draw_dim` → `atlas.text_dim` at ~12px; `draw_scrim` → `atlas.bg`). Reuse the inspector's font-family block verbatim.

- [ ] **Step 2: Add state + the new action variant**

Add to `struct ProjectTree`: `#[rust] scope_title: String,`, `#[rust] collapsed: bool,`, `#[rust] pinned: bool,`, `#[rust] hovered: bool,`, `#[rust] header_rect: Rect,`, `#[rust] title_rect: Rect,`, `#[rust] collapse_rect: Rect,`, `#[rust] pin_rect: Rect,`.

Extend the action enum:

```rust
#[derive(Clone, Debug, Default)]
pub enum ProjectTreeAction {
    #[default]
    None,
    SelectDiagram(String),
    FocusClassifier(String),
    ScopeRequest { anchor: Rect },
    Query(String),      // used in Task 9
    RotateFilter,       // used in Task 9
}
```

- [ ] **Step 3: Draw the header band in `draw_walk`**

Reuse the header-geometry constants (`HEADER_H = 64`, `TITLE_ROW_H = 34`, `PAD = 10`, `ICON = 16`, `ICON_GAP = 10`). After the `view.draw_walk` / `draw_nodes` loop, capture `let rect = self.view.area().rect(cx);` and draw:
- The scope title (`self.scope_title`, or `"Untitled"` if empty) via `draw_title` at the header's left, followed by a `⌄` — record `self.title_rect` spanning the title text width for the click target.
- The right cluster (right → left): **pin** (`Icon::Pin`/`Icon::PinOff`, tinted from `draw_dim.color`, using the shared `self.icons.get(...)` tint-copy idiom from the inspector), then **collapse** (`Icon::ListCollapse` when expanded, `Icon::ListExpand` when collapsed — reusing the inspector's caret glyphs, no new chevron glyph). Record `self.pin_rect` / `self.collapse_rect`.

Follow `inspector_panel.rs:415-472` for the exact tint-copy + `draw_abs` idiom.

- [ ] **Step 4: Collapse the body when collapsed**

When `self.collapsed`, hide the `FileTree` body: before the `view.draw_walk` loop, set the file_tree child invisible and shrink the walk to the header. Simplest reliable approach mirroring the inspector's collapse: toggle the child's visibility:

```rust
        let ft_widget = self.view.file_tree(cx, ids!(file_tree));
        ft_widget.set_visible(cx, !self.collapsed);
```

and, when collapsed, set `walk.height = Size::Fit { min: None, max: None }` so the frame hugs the header (as `inspector_panel.rs:377-384` does).

- [ ] **Step 5: Handle header clicks + hover + scrim**

In `handle_event`, after the existing `file_tree` handling, add the aligned-parent-safe hit path (tree panel is left-aligned so `hit_off ≈ 0`, but keep the pattern per `makepad-aligned-parent-hit-rect-offset`):

```rust
        let hit_off = self.view.area().rect(cx).pos - self.header_rect.pos; // header_rect set in draw_walk
        match event.hits(cx, self.view.area()) {
            Hit::FingerHoverIn(_)  => { if !self.hovered { self.hovered = true;  self.view.redraw(cx); } }
            Hit::FingerHoverOut(_) => { if  self.hovered { self.hovered = false; self.view.redraw(cx); } }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let p = fe.abs - hit_off;
                if self.pin_rect.contains(p) {
                    self.pinned = !self.pinned; self.view.redraw(cx); return;
                }
                if self.collapse_rect.contains(p) {
                    self.collapsed = !self.collapsed; self.view.redraw(cx); return;
                }
                if self.title_rect.contains(p) {
                    let anchor = Rect { pos: self.title_rect.pos + hit_off, size: self.title_rect.size };
                    cx.widget_action(self.widget_uid(), ProjectTreeAction::ScopeRequest { anchor });
                    return;
                }
            }
            _ => {}
        }
```

Paint the scrim last in `draw_walk` exactly as Task 7 (opacity `1.0` when `self.hovered || self.pinned`, else `0.55`; `self.draw_scrim.color.w = 1.0 - opacity`; `draw_abs` over `rect`).

Add the readers:

```rust
    pub fn set_scope_title(&mut self, cx: &mut Cx, title: String) {
        if self.scope_title != title { self.scope_title = title; self.view.redraw(cx); }
    }
    pub fn scope_request(&self, actions: &Actions) -> Option<Rect> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::ScopeRequest { anchor } = item.cast() { Some(anchor) } else { None }
    }
```

- [ ] **Step 6: Build + manual check**

Run: `cargo run -p waml-editor`. Expected: the tree panel now shows a header with the scope title + `⌄`, a collapse chevron, and a pin; clicking collapse hides the tree body (header only); the pin toggles its glyph; an unhovered/unpinned panel dims to ~0.55. (The title click emits `ScopeRequest` but nothing opens yet — wired in Task 10.)

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs
git commit -m "feat(tree-panel): header band — title trigger, collapse, pin, hover scrim"
```

---

### Task 9: `tree_panel.rs` — search field, magnifier, rotating type chip, empty states

Adds the search row under the title: a hand-rolled text field with a leading `Search` magnifier, and a rotating type-filter chip. Emits `Query(String)` and `RotateFilter`. Renders the `Elsewhere` note and the `Empty` centered message from `self.nav_tag`.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (search field draw + edit state; magnifier glyph; type chip draw + hit; empty-state text)

**Interfaces:**
- Produces: emits `ProjectTreeAction::Query(String)` on edit and `ProjectTreeAction::RotateFilter` on chip click. New setters `set_chip_label(&mut self, cx, &str)` and `set_query_text(&mut self, cx, &str)` (the app pushes the authoritative query/chip back so the field reflects `NavState`). Readers `query_changed(&self, actions) -> Option<String>` and `rotate_filter_clicked(&self, actions) -> bool`.

- [ ] **Step 1: Draw the search row (field + magnifier + chip)**

Below the title row in `draw_walk` (only when `!self.collapsed`), draw a search field spanning `Fill` and a `Fit` type chip at the right:
- Field background via a `draw_field_bg: DrawColor` (token `atlas.field_bg`), a leading `Icon::Search` glyph drawn inside at the left (`self.icons.draw(cx, Icon::Search, rect, tint)`), then the query text (`self.query_text` + a `│` caret when editing) or the placeholder `"Search model"` in `draw_dim`. Record `self.search_rect`.
- The chip: `draw_field_bg` pill + `self.chip_label` text + a trailing `⌄`. Record `self.chip_rect`.

Add fields: `#[rust] query_text: String,`, `#[rust] editing_search: bool,`, `#[rust] chip_label: String,`, `#[rust] search_rect: Rect,`, `#[rust] chip_rect: Rect,` and `#[live] draw_field_bg: DrawColor,` (DSL token `atlas.field_bg`).

- [ ] **Step 2: Hand-rolled search editing + chip click**

Extend the `FingerUp` branch from Task 8: if `self.search_rect.contains(p)` → `self.editing_search = true; cx.set_key_focus(self.view.area()); redraw`. If `self.chip_rect.contains(p)` → emit `ProjectTreeAction::RotateFilter` and `return`. Add the keyboard handlers (guarded by `self.editing_search`), mirroring `inspector_panel.rs:351-367`:

```rust
            Hit::KeyDown(ke) if self.editing_search => match ke.key_code {
                KeyCode::Backspace => { self.query_text.pop(); self.emit_query(cx); }
                KeyCode::Escape    => { self.editing_search = false; self.view.redraw(cx); }
                _ => {}
            },
            Hit::TextInput(ti) if self.editing_search => {
                for ch in ti.input.chars() { if !ch.is_control() { self.query_text.push(ch); } }
                self.emit_query(cx);
            }
```

where `emit_query` redraws and fires `cx.widget_action(uid, ProjectTreeAction::Query(self.query_text.clone()))`.

Add setters/readers:

```rust
    pub fn set_chip_label(&mut self, cx: &mut Cx, label: &str) {
        if self.chip_label != label { self.chip_label = label.to_string(); self.view.redraw(cx); }
    }
    pub fn set_query_text(&mut self, cx: &mut Cx, text: &str) {
        if self.query_text != text { self.query_text = text.to_string(); self.view.redraw(cx); }
    }
    pub fn query_changed(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::Query(q) = item.cast() { Some(q) } else { None }
    }
    pub fn rotate_filter_clicked(&self, actions: &Actions) -> bool {
        actions.find_widget_action(self.widget_uid())
            .map(|i| matches!(i.cast(), ProjectTreeAction::RotateFilter))
            .unwrap_or(false)
    }
```

- [ ] **Step 3: Empty-state text from `nav_tag`**

In `draw_walk`, after the `draw_nodes` overlay, key off `self.nav_tag`:
- `NavStateTag::Elsewhere` → draw a dim `format!("No matches in {}", scope_or_untitled)` line, then an `"Elsewhere in model"` header, above the (already-drawn) whole-model rows.
- `NavStateTag::Empty` → draw a centered `"No matches found"` in `draw_dim` over the body area (no rows drawn).
- `Browse`/`Results` → no note.

- [ ] **Step 4: Build + manual check**

Run: `cargo run -p waml-editor`. Expected: typing in the field filters the tree live (once Task 10 wires `Query` → `NavState`); the chip shows `All` and cycles on click; a query with no in-scope match shows the "Elsewhere" note + whole-model matches; a nonsense query shows "No matches found". (Live filtering depends on Task 10; at this task the field/chip render and emit actions, verified via a temporary `log!` in `handle_event` if needed.)

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs
git commit -m "feat(tree-panel): search field + magnifier + type chip + empty states"
```

---

### Task 10: `app.rs` — own `NavState`, wire scope dropdown / query / filter

Closes the loop: the app owns `NavState` + the model's `kinds_in_model` cycle, rebuilds `NavView` on every scope/query/filter change and re-pushes it, opens the title dropdown via `PopupRoot` from `nav::packages`, and applies the picked scope.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (`nav_state` already added in Task 6; add `nav_kinds: Vec<TreeKind>`; a `refresh_nav` helper; `ScopeRequest`/`Query`/`RotateFilter` handling; the `nav_scope` popup tag + its `closed` result)

**Interfaces:**
- Consumes: `ProjectTree::{scope_request, query_changed, rotate_filter_clicked, set_view, set_scope_title, set_chip_label}`, `nav::{view, packages, kinds_in_model, chip_label}`.

- [ ] **Step 1: Add `nav_kinds` + a `refresh_nav` helper**

Add `#[rust] nav_kinds: Vec<crate::tree::TreeKind>,` to `struct App`. On model load (in `open_dir`, where `build_tree` was), set `self.nav_kinds = crate::nav::kinds_in_model(&self.model);` and reset `self.nav_state = Default::default();`. Add:

```rust
    /// Rebuild the nav projection from the current `nav_state` and push it to the
    /// tree panel, along with the header's scope-title + chip labels.
    fn refresh_nav(&mut self, cx: &mut Cx) {
        let view = crate::nav::view(&self.model, &self.nav_state);
        let title = crate::nav::packages(&self.model)
            .into_iter()
            .find(|r| r.key == self.nav_state.scope)
            .map(|r| r.title)
            .unwrap_or_else(|| "Untitled".to_string());
        let chip = crate::nav::chip_label(self.nav_state.filter).to_string();
        if let Some(mut panel) = self
            .ui.widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_view(cx, view);
            panel.set_scope_title(cx, title);
            panel.set_chip_label(cx, &chip);
        }
    }
```

Replace the two `nav::view + set_view` call sites from Task 6 with a `self.refresh_nav(cx);` call (after the model is loaded and the panel exists).

- [ ] **Step 2: Handle `ScopeRequest` → open the title dropdown**

In `handle_actions`, alongside the tree readers (near the `focused_classifier` / `selected_diagram` block), add:

```rust
        let scope_anchor = self
            .ui.widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.scope_request(actions));
        if let Some(anchor_rect) = scope_anchor {
            let items: Vec<crate::popup::base::PopupItem> = crate::nav::packages(&self.model)
                .into_iter()
                .map(|r| crate::popup::base::PopupItem {
                    id: LiveId::from_str(&format!("scope:{}", r.key)),
                    label: format!("{}{}", "  ".repeat(r.depth), r.title),
                    icon: crate::icons::Icon::Folder,
                    danger: false,
                    enabled: true,
                })
                .collect();
            let anchor = dvec2(anchor_rect.pos.x, anchor_rect.pos.y + anchor_rect.size.y + crate::popup::menu::MENU_GAP);
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                pr.show_at(cx, PopupSpec::Menu {
                    tag: live_id!(nav_scope), anchor, bounds, items, open: MenuOpen::Popup,
                });
            }
            return;
        }
```

The scope key is recovered from the item id: keep a `self.nav_scope_ids: Vec<(LiveId, String)>` map (rebuilt here, like the inspector's `picker_ids`) so the `closed` handler maps `LiveId` → scope key. (Depth indent via leading spaces keeps the popup a flat `MenuPopup`; real per-row indent is a later polish.)

- [ ] **Step 3: Apply the picked scope in the `closed` block**

In the popup-outcomes block (near the `element_picker`/`burger` `closed` reads), add:

```rust
            let nav_scope_closed = pr.closed(actions, live_id!(nav_scope));
```

and after `drop(pr);`:

```rust
            if let Some(PopupResult::Invoked(id)) = nav_scope_closed {
                if let Some((_, key)) = self.nav_scope_ids.iter().find(|(i, _)| *i == id) {
                    self.nav_state.scope = key.clone();
                    self.refresh_nav(cx);
                }
            }
```

- [ ] **Step 4: Handle `Query` + `RotateFilter`**

Add two more tree readers in `handle_actions`:

```rust
        let query = self
            .ui.widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.query_changed(actions));
        if let Some(q) = query {
            self.nav_state.query = q;
            self.refresh_nav(cx);
            return;
        }

        let rotate = self
            .ui.widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .map(|panel| panel.rotate_filter_clicked(actions))
            .unwrap_or(false);
        if rotate {
            // Cycle None -> kinds_in_model[0] -> ... -> last -> None.
            let cycle: Vec<Option<crate::tree::TreeKind>> = std::iter::once(None)
                .chain(self.nav_kinds.iter().copied().map(Some))
                .collect();
            let cur = cycle.iter().position(|f| *f == self.nav_state.filter).unwrap_or(0);
            self.nav_state.filter = cycle[(cur + 1) % cycle.len()];
            self.refresh_nav(cx);
            return;
        }
```

(Per the user's note, an empty `nav_kinds` cycle is fine — the chip just stays `All`; no special-casing.)

- [ ] **Step 5: Build + full manual check**

Run: `cargo run -p waml-editor`. Verify against the spec's manual checklist:
- title dropdown lists packages (root = whole model) and scopes on pick;
- the chip shows `All` and cycles the model's actual kinds;
- search shows Results / Elsewhere-note / "No matches found";
- collapse hides the body; pin locks opacity; an unhovered tree AND inspector both dim to ~0.55.

- [ ] **Step 6: Run the whole editor test suite**

Run: `cargo test -p waml-editor`
Expected: PASS — nav + tree + tree_panel + icons + inspector suites all green.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "feat(app): own NavState — scope dropdown, live query, filter cycle"
```

---

## Self-Review Notes

- **Spec coverage:** `nav.rs` types + `view`/`packages`/`kinds_in_model` (Tasks 2–5); title dropdown (Tasks 8 trigger + 10 popup); search field + magnifier + rotating chip (Tasks 1 glyph + 9); three search states (Task 5 logic + Task 9 render); collapse + pin chrome (Task 8); hover translucency on BOTH panels (Task 7 inspector + Task 8 tree); `Search` glyph verified absent and added, chevrons reused from `ListCollapse`/`ListExpand` (no redundant glyph). Out-of-scope items (breadcrumb, row scope-in, `×`, context menu, CRUD, shared chrome unit) are absent by construction.
- **Deviations from the spec's wording, justified:** search field is hand-rolled (house convention in `inspector_panel.rs`/`doc_tabs.rs`), not the fork `TextInput`; hover translucency is a bg-colored scrim quad because the pinned makepad `View` has no `opacity` field. Both are noted in Global Constraints.
- **Type consistency:** `set_view`/`NavView`, `ScopeRequest { anchor: Rect }` ↔ `scope_request -> Option<Rect>`, `Query(String)` ↔ `query_changed`, `RotateFilter` ↔ `rotate_filter_clicked`, `nav_state`/`nav_kinds`/`nav_scope_ids` on `App`, and the `nav_scope` popup tag are used consistently across Tasks 6–10.
