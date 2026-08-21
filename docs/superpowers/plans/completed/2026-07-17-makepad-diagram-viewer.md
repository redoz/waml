# Makepad Diagram Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a native, GPU-rendered, read-only viewer (`crates/waml-editor`) that loads an OKF directory, solves one diagram with the existing `waml` pipeline, and draws its groups, edges, and nodes on a pan/zoom canvas.

**Architecture:** Everything from the OKF directory down to a plain `Scene { nodes, groups, edges }` render model is engine-agnostic Rust reusing `waml::parse` + `waml::solve` verbatim (`load` → `sizing` → `scene`). A pure-math `camera` module handles pan/zoom and world↔screen. A makepad `GraphCanvas` widget draws the scene under the camera; an `app` shell wires the CLI arg and window. The `scene` module is the seam: above it is testable without a GPU, below it is makepad drawing.

**Tech Stack:** Rust 2021, `waml` core crate (with `serde` feature only — NOT `wasm`), makepad (`makepad-widgets`, vendored externally). Testing via `cargo test`; visual check via a real app run and a headless render.

## Global Constraints

- **No OWOX-derived code.** This viewer is a clean-room rebuild. Do NOT read from or port any `packages/**` TypeScript (e.g. `packages/core/src/canvas/layoutSize.ts`) — that code predates our ownership. Every sizing/geometry/render value in this crate is derived here from first principles (the constants below), not copied from the web app. The only code reused is the public `waml` Rust crate API.
- **Rust edition `2021`, `rust-version` `1.80`** (inherited from `[workspace.package]`).
- **`waml` dependency uses the `serde` feature only.** Never enable `wasm`/`tsify` for this native build.
- **makepad path dependency — CONFIRM BEFORE TASK 1.** The vendored makepad is at `C:\dev\vendor\makepad` (OUTSIDE this repo, not the in-repo `vendor/makepad` the design doc assumed). A relative path dep resolves differently from the main checkout (`C:\dev\waml`) than from this worktree (`C:\dev\waml\.worktrees\makepad-viewer`). Before writing `Cargo.toml`, confirm with the user whether to (a) use the main-checkout-relative path `../../../../vendor/makepad/widgets` (worktree) / `../../../vendor/makepad/widgets` (main), or (b) an absolute path, or (c) build from the main checkout. This plan's Task 1 uses the worktree-relative form and flags it inline.
- **Read-only.** No editing, no writing to disk, no inspector. Any step that adds mutation is out of scope.

---

### Task 1: Crate skeleton + directory loader (`load.rs`)

**Files:**
- Modify: `Cargo.toml` (workspace root, line 3 `members`)
- Create: `crates/waml-editor/Cargo.toml`
- Create: `crates/waml-editor/src/main.rs`
- Create: `crates/waml-editor/src/load.rs`
- Create: `crates/waml-editor/tests/fixtures/mini/index.md`
- Create: `crates/waml-editor/tests/fixtures/mini/order.md`
- Create: `crates/waml-editor/tests/fixtures/mini/customer.md`
- Create: `crates/waml-editor/tests/fixtures/mini/orders-diagram.md`

**Interfaces:**
- Produces: `pub fn read_bundle(dir: &std::path::Path) -> std::io::Result<Vec<(String, String)>>` — walks `*.md` under `dir`, returning `(relative_path_with_forward_slashes, file_contents)` pairs sorted by path.
- Produces: `pub fn load_model(dir: &std::path::Path) -> std::io::Result<waml::model::Model>` — `read_bundle` then `waml::parse::build_model`.

- [ ] **Step 1: Register the crate in the workspace**

Modify `Cargo.toml` line 3:

```toml
members = ["crates/waml", "crates/waml-cli", "crates/waml-ops-dto", "crates/waml-wasm", "crates/waml-editor"]
```

- [ ] **Step 2: Write the crate manifest**

Create `crates/waml-editor/Cargo.toml`. NOTE the makepad path — see Global Constraints; the value below is worktree-relative (five `..` to reach `C:\dev`). Confirm before proceeding.

```toml
[package]
name = "waml-editor"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Native, read-only GPU viewer for WAML/OKF diagrams."

[dependencies]
waml = { path = "../waml", features = ["serde"] }
makepad-widgets = { path = "../../../../vendor/makepad/widgets" }
```

- [ ] **Step 3: Create a placeholder binary entry so the crate compiles**

Create `crates/waml-editor/src/main.rs`:

```rust
mod load;

fn main() {
    // Real app shell wired in Task 8.
    println!("waml-editor");
}
```

- [ ] **Step 4: Create the four fixture files**

These form a minimal OKF bundle with two classifiers, one group, one edge, and a `## Layout` section so the diagram solves to a real arrangement.

Create `crates/waml-editor/tests/fixtures/mini/index.md`:

```markdown
# Mini
```

Create `crates/waml-editor/tests/fixtures/mini/order.md`:

```markdown
---
type: uml.Class
title: Order
---
# Order

## Attributes
- id: OrderId {1}
- total: Decimal {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
```

Create `crates/waml-editor/tests/fixtures/mini/customer.md`:

```markdown
---
type: uml.Class
title: Customer
---
# Customer

## Attributes
- id: CustomerId {1}
- name: String {1}
```

Create `crates/waml-editor/tests/fixtures/mini/orders-diagram.md`:

```markdown
---
type: Diagram
title: Orders
profile: uml-domain
---
# Orders

## Members
- [Order](./order.md)
- [Customer](./customer.md)

## Layout
- [Order](./order.md) left of [Customer](./customer.md)
```

- [ ] **Step 5: Write the failing test for `read_bundle`**

Create `crates/waml-editor/src/load.rs`:

```rust
//! Load an OKF directory into a `waml::model::Model`.

use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini")
    }

    #[test]
    fn read_bundle_returns_sorted_md_pairs() {
        let bundle = read_bundle(&fixture_dir()).unwrap();
        let paths: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            ["customer.md", "index.md", "order.md", "orders-diagram.md"]
        );
        // Contents are the raw file text.
        let order = bundle.iter().find(|(p, _)| p == "order.md").unwrap();
        assert!(order.1.contains("title: Order"));
    }

    #[test]
    fn load_model_builds_two_nodes_one_diagram() {
        let model = load_model(&fixture_dir()).unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.diagrams.len(), 1);
        assert_eq!(model.edges.len(), 1);
    }
}
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `cargo test -p waml-editor --lib`
Expected: FAIL to compile — `read_bundle`/`load_model` not defined.

- [ ] **Step 7: Implement the loader**

Add above the `#[cfg(test)]` block in `crates/waml-editor/src/load.rs`:

```rust
/// Walk `dir` recursively, returning `(rel_path, contents)` for every `*.md`
/// file, sorted by path. Paths use forward slashes so keys match `build_model`.
pub fn read_bundle(dir: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    collect(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// Load an OKF directory into a resolved `Model`.
pub fn load_model(dir: &Path) -> std::io::Result<waml::model::Model> {
    let bundle = read_bundle(dir)?;
    Ok(waml::parse::build_model(&bundle))
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib`
Expected: PASS (both tests). Also run `cargo test -p waml-editor` to confirm the crate builds cleanly.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/waml-editor
git commit -m "feat(editor): scaffold waml-editor crate + OKF directory loader"
```

---

### Task 2: Node sizing (`sizing.rs`)

Sizing is derived here from first principles — a fixed compact box for every node, and a taller header+rows box for entities when the diagram opts into attribute display. Values are our own choices, documented as constants; nothing is ported.

**Files:**
- Create: `crates/waml-editor/src/sizing.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod sizing;`)

**Interfaces:**
- Consumes: `waml::model::{Node, Diagram, DiagramDisplay, DiagramGroup}`, `waml::solve::{Size, SizeMap}`.
- Produces: `pub fn size_of(node: &waml::model::Node, display: &waml::model::DiagramDisplay) -> waml::solve::Size`.
- Produces: `pub fn size_map(model: &waml::model::Model, diagram: &waml::model::Diagram) -> waml::solve::SizeMap` — a `Size` for every diagram member key that resolves to a classifier node.

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add under `mod load;`:

```rust
mod sizing;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/waml-editor/src/sizing.rs`:

```rust
//! Node sizing for the solver. Derived from first principles (see constants);
//! not ported from any prior implementation.

use waml::model::{Diagram, DiagramDisplay, DiagramGroup, Model, Node};
use waml::solve::{Size, SizeMap};

/// Compact box used for every node by default and for entities that show no rows.
pub const COMPACT_W: f64 = 200.0;
pub const COMPACT_H: f64 = 90.0;
/// ERD box (entity with attribute rows shown).
pub const ERD_W: f64 = 220.0;
pub const ERD_HEADER_H: f64 = 44.0;
pub const ERD_ROW_H: f64 = 22.0;
/// Row cap when the diagram does not set `max_attributes`.
pub const ERD_DEFAULT_ROW_CAP: u32 = 10;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn node_with_attrs(n: usize) -> Node {
        let bundle = vec![(
            "e.md".to_string(),
            format!(
                "---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n{}",
                (0..n).map(|i| format!("- f{i}: String {{1}}\n")).collect::<String>()
            ),
        )];
        waml::parse::build_model(&bundle).nodes.into_iter().next().unwrap()
    }

    #[test]
    fn compact_when_attributes_hidden() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay::default(); // show_attributes = None => hidden
        assert_eq!(size_of(&node, &display), Size { w: COMPACT_W, h: COMPACT_H });
    }

    #[test]
    fn erd_size_scales_with_capped_rows() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay { show_attributes: Some(true), ..Default::default() };
        assert_eq!(
            size_of(&node, &display),
            Size { w: ERD_W, h: ERD_HEADER_H + 3.0 * ERD_ROW_H }
        );
    }

    #[test]
    fn erd_rows_capped_by_max_attributes() {
        let node = node_with_attrs(20);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            max_attributes: Some(4),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size { w: ERD_W, h: ERD_HEADER_H + 4.0 * ERD_ROW_H }
        );
    }

    #[test]
    fn compact_when_entity_has_no_attributes() {
        let node = node_with_attrs(0);
        let display = DiagramDisplay { show_attributes: Some(true), ..Default::default() };
        assert_eq!(size_of(&node, &display), Size { w: COMPACT_W, h: COMPACT_H });
    }

    #[test]
    fn size_map_covers_every_resolved_member() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram);
        // Both classifiers get a compact size (fixture diagram shows no attributes).
        assert_eq!(map.len(), 2);
        for size in map.values() {
            assert_eq!(*size, Size { w: COMPACT_W, h: COMPACT_H });
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib sizing`
Expected: FAIL to compile — `size_of`/`size_map` not defined.

- [ ] **Step 4: Implement the sizing functions**

Add above the `#[cfg(test)]` block in `crates/waml-editor/src/sizing.rs`:

```rust
/// Size one node for the solver.
pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size {
    let show = display.show_attributes.unwrap_or(false);
    if show && !node.attributes.is_empty() {
        let cap = display.max_attributes.unwrap_or(ERD_DEFAULT_ROW_CAP).max(1) as usize;
        let rows = node.attributes.len().min(cap);
        Size { w: ERD_W, h: ERD_HEADER_H + rows as f64 * ERD_ROW_H }
    } else {
        Size { w: COMPACT_W, h: COMPACT_H }
    }
}

/// Build a `SizeMap` for every diagram member that resolves to a classifier node.
pub fn size_map(model: &Model, diagram: &Diagram) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> =
        model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut keys = Vec::new();
    collect_member_keys(&diagram.groups, &mut keys);

    let mut map = SizeMap::new();
    for key in keys {
        if let Some(node) = lookup.get(key.as_str()) {
            map.insert(key.clone(), size_of(node, &diagram.display));
        }
    }
    map
}

fn collect_member_keys(groups: &[DiagramGroup], out: &mut Vec<String>) {
    for group in groups {
        out.extend(group.members.iter().cloned());
        collect_member_keys(&group.children, out);
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib sizing`
Expected: PASS (all five tests).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/sizing.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): first-principles node sizing (compact + ERD)"
```

---

### Task 3: Scene builder (`scene.rs`) — the seam

**Files:**
- Create: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod scene;`)

**Interfaces:**
- Consumes: `sizing::size_map`, `waml::solve::{solve_diagram, SolveConfig, Rect, SolvedGroup}`, `waml::model::{Model, Diagram, RelationshipKind}`, `waml::diagnostic::Diagnostic`.
- Produces:
  - `pub struct SceneNode { pub key: String, pub title: String, pub rect: waml::solve::Rect, pub emphasized: bool, pub collapsed: bool }`
  - `pub struct SceneEdge { pub source: waml::solve::Rect, pub target: waml::solve::Rect, pub kind: waml::model::RelationshipKind }`
  - `pub struct Scene { pub nodes: Vec<SceneNode>, pub groups: Vec<waml::solve::SolvedGroup>, pub edges: Vec<SceneEdge> }`
  - `pub fn build_scene(model: &Model, diagram: &Diagram) -> (Scene, Vec<Diagnostic>)`
  - `pub fn bounding_box(scene: &Scene) -> Option<waml::solve::Rect>`

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add:

```rust
mod scene;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/waml-editor/src/scene.rs`:

```rust
//! The render seam: pick a diagram, solve it, and flatten to plain data.
//! Nothing below this module touches makepad; nothing here touches a GPU.

use waml::diagnostic::Diagnostic;
use waml::model::{Diagram, Model, RelationshipKind};
use waml::solve::{solve_diagram, Rect, SolveConfig, SolvedGroup};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub key: String,
    pub title: String,
    pub rect: Rect,
    pub emphasized: bool,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    #[test]
    fn scene_has_both_nodes_with_titles() {
        let model = mini();
        let (scene, diags) = build_scene(&model, &model.diagrams[0]);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

        let mut titles: Vec<(&str, &str)> =
            scene.nodes.iter().map(|n| (n.key.as_str(), n.title.as_str())).collect();
        titles.sort();
        assert_eq!(titles, [("customer", "Customer"), ("order", "Order")]);
    }

    #[test]
    fn scene_edge_endpoints_match_node_rects() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        assert_eq!(scene.edges.len(), 1);
        let edge = &scene.edges[0];
        assert_eq!(edge.kind, RelationshipKind::Associates);

        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // The associates edge runs order -> customer (see fixture order.md).
        assert_eq!(edge.source, order.rect);
        assert_eq!(edge.target, customer.rect);
    }

    #[test]
    fn layout_places_order_left_of_customer() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // "- [Order] left of [Customer]" => order's right edge is left of customer's left edge.
        assert!(order.rect.x + order.rect.w <= customer.rect.x);
    }

    #[test]
    fn bounding_box_covers_all_nodes() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let bbox = bounding_box(&scene).unwrap();
        for node in &scene.nodes {
            assert!(node.rect.x >= bbox.x);
            assert!(node.rect.y >= bbox.y);
            assert!(node.rect.x + node.rect.w <= bbox.x + bbox.w + 1e-6);
            assert!(node.rect.y + node.rect.h <= bbox.y + bbox.h + 1e-6);
        }
        assert!(bbox.w > 0.0 && bbox.h > 0.0);
    }

    #[test]
    fn bounding_box_none_for_empty_scene() {
        let scene = Scene { nodes: vec![], groups: vec![], edges: vec![] };
        assert!(bounding_box(&scene).is_none());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib scene`
Expected: FAIL to compile — `build_scene`/`bounding_box` not defined.

- [ ] **Step 4: Implement `build_scene` and `bounding_box`**

Add above the `#[cfg(test)]` block in `crates/waml-editor/src/scene.rs`:

```rust
/// Solve `diagram` against `model` and flatten the result into a `Scene`.
pub fn build_scene(model: &Model, diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    let sizes = crate::sizing::size_map(model, diagram);
    let (solved, diags) = solve_diagram(diagram, &sizes, &SolveConfig::default());

    let title_of: BTreeMap<&str, String> = model
        .nodes
        .iter()
        .map(|n| (n.key.as_str(), n.concept.title.clone().unwrap_or_else(|| n.key.clone())))
        .collect();

    let mut nodes = Vec::with_capacity(solved.nodes.len());
    for (key, rect) in &solved.nodes {
        let flags = solved.flags.get(key).copied().unwrap_or_default();
        nodes.push(SceneNode {
            key: key.clone(),
            title: title_of.get(key.as_str()).cloned().unwrap_or_else(|| key.clone()),
            rect: *rect,
            emphasized: flags.emphasized,
            collapsed: flags.collapsed,
        });
    }

    // Only edges whose endpoints both appear in the solved layout are drawable.
    let mut edges = Vec::new();
    for e in &model.edges {
        if let (Some(&source), Some(&target)) =
            (solved.nodes.get(&e.source), solved.nodes.get(&e.target))
        {
            edges.push(SceneEdge { source, target, kind: e.kind });
        }
    }

    (Scene { nodes, groups: solved.groups.clone(), edges }, diags)
}

/// Axis-aligned bounding box over all node and group rects, or `None` if empty.
pub fn bounding_box(scene: &Scene) -> Option<Rect> {
    let mut rects = scene
        .nodes
        .iter()
        .map(|n| n.rect)
        .chain(scene.groups.iter().map(|g| g.rect));
    let first = rects.next()?;
    let (mut min_x, mut min_y) = (first.x, first.y);
    let (mut max_x, mut max_y) = (first.x + first.w, first.y + first.h);
    for r in rects {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.w);
        max_y = max_y.max(r.y + r.h);
    }
    Some(Rect { x: min_x, y: min_y, w: max_x - min_x, h: max_y - min_y })
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib scene`
Expected: PASS (all five tests).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): scene builder — solve + flatten to plain render data"
```

---

### Task 4: Camera (`camera.rs`) — pure pan/zoom math

The camera holds pan (world offset) and zoom. All rendering transforms world (diagram-pixel) coordinates to canvas-local coordinates; the widget adds the canvas rect origin at draw time. Keeping the camera origin-agnostic makes it fully unit-testable.

**Files:**
- Create: `crates/waml-editor/src/camera.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod camera;`)

**Interfaces:**
- Consumes: `waml::solve::Rect` (as bbox input).
- Produces:
  - `pub struct Camera { pub pan_x: f64, pub pan_y: f64, pub zoom: f64 }`
  - `pub fn world_to_local(&self, wx: f64, wy: f64) -> (f64, f64)`
  - `pub fn local_to_world(&self, lx: f64, ly: f64) -> (f64, f64)`
  - `pub fn zoom_at(&mut self, local_x: f64, local_y: f64, factor: f64)` — zoom keeping the world point under the given local point fixed.
  - `pub fn fit(bbox: waml::solve::Rect, viewport_w: f64, viewport_h: f64, pad: f64) -> Camera`

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add:

```rust
mod camera;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/waml-editor/src/camera.rs`:

```rust
//! Pan/zoom camera. Pure math — no makepad types. `local` coordinates are
//! relative to the canvas rect's top-left; the widget adds the rect origin.

use waml::solve::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

/// Zoom is clamped to this range to avoid degenerate transforms.
pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: (f64, f64), b: (f64, f64)) {
        assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{a:?} != {b:?}");
    }

    #[test]
    fn world_local_round_trip() {
        let cam = Camera { pan_x: 30.0, pan_y: -10.0, zoom: 2.0 };
        let local = cam.world_to_local(100.0, 50.0);
        approx(local, ((100.0 - 30.0) * 2.0, (50.0 - -10.0) * 2.0));
        approx(cam.local_to_world(local.0, local.1), (100.0, 50.0));
    }

    #[test]
    fn zoom_at_keeps_point_under_cursor_fixed() {
        let mut cam = Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        let before = cam.local_to_world(400.0, 300.0);
        cam.zoom_at(400.0, 300.0, 1.5);
        let after = cam.local_to_world(400.0, 300.0);
        approx(before, after);
        assert!((cam.zoom - 1.5).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_clamps_to_bounds() {
        let mut cam = Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        cam.zoom_at(0.0, 0.0, 1000.0);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(0.0, 0.0, 0.0001);
        assert_eq!(cam.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_centers_bbox_in_viewport() {
        let bbox = Rect { x: 0.0, y: 0.0, w: 200.0, h: 100.0 };
        let cam = Camera::fit(bbox, 800.0, 600.0, 40.0);
        // Limiting axis: width. zoom = (800-80)/200 = 3.6.
        assert!((cam.zoom - 3.6).abs() < 1e-9);
        // The bbox center maps to the viewport center.
        let center = cam.world_to_local(100.0, 50.0);
        approx(center, (400.0, 300.0));
    }

    #[test]
    fn fit_of_empty_viewport_stays_positive() {
        let bbox = Rect { x: 0.0, y: 0.0, w: 200.0, h: 100.0 };
        let cam = Camera::fit(bbox, 0.0, 0.0, 40.0);
        assert!(cam.zoom >= MIN_ZOOM);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib camera`
Expected: FAIL to compile — methods not defined.

- [ ] **Step 4: Implement the camera**

Add above the `#[cfg(test)]` block in `crates/waml-editor/src/camera.rs`:

```rust
impl Camera {
    /// World (diagram-pixel) point -> canvas-local point.
    pub fn world_to_local(&self, wx: f64, wy: f64) -> (f64, f64) {
        ((wx - self.pan_x) * self.zoom, (wy - self.pan_y) * self.zoom)
    }

    /// Canvas-local point -> world point.
    pub fn local_to_world(&self, lx: f64, ly: f64) -> (f64, f64) {
        (lx / self.zoom + self.pan_x, ly / self.zoom + self.pan_y)
    }

    /// Multiply zoom by `factor`, keeping the world point under `(local_x, local_y)` fixed.
    pub fn zoom_at(&mut self, local_x: f64, local_y: f64, factor: f64) {
        let (wx, wy) = self.local_to_world(local_x, local_y);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = wx - local_x / self.zoom;
        self.pan_y = wy - local_y / self.zoom;
    }

    /// Fit `bbox` centered in a `viewport_w` x `viewport_h` canvas with `pad` px inset.
    pub fn fit(bbox: Rect, viewport_w: f64, viewport_h: f64, pad: f64) -> Camera {
        let avail_w = (viewport_w - 2.0 * pad).max(1.0);
        let avail_h = (viewport_h - 2.0 * pad).max(1.0);
        let zoom = if bbox.w > 0.0 && bbox.h > 0.0 {
            (avail_w / bbox.w).min(avail_h / bbox.h).clamp(MIN_ZOOM, MAX_ZOOM)
        } else {
            1.0
        };
        let (cx, cy) = (bbox.x + bbox.w * 0.5, bbox.y + bbox.h * 0.5);
        Camera {
            pan_x: cx - viewport_w * 0.5 / zoom,
            pan_y: cy - viewport_h * 0.5 / zoom,
            zoom,
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib camera`
Expected: PASS (all five tests).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/camera.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): pure pan/zoom camera with fit + zoom-to-cursor"
```

---

### Task 5: makepad build spike — a running window

This task de-risks the makepad path dependency, the app-entry macro, and the widget-registration DSL in THIS makepad fork before any drawing logic is written. It produces a window showing a colored background. It is verified by actually running the app (no unit test — makepad rendering is not unit-testable).

**Files:**
- Rewrite: `crates/waml-editor/src/main.rs`
- Create: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Produces: an `App` type wired via `app_main!(App)` that opens a `Window` with a `Root` and a full-fill `View`.
- Note for later tasks: this fork uses the `script_mod!` DSL, `#[derive(Script, ScriptHook)]`, and `AppMain`/`MatchEvent` — confirmed against `C:\dev\vendor\makepad\examples\hello_world\src\main.rs` and `widgets/src/map/view.rs`. Widgets register via `mod.widgets.X = #(MyWidget::register_widget(vm))`; custom widgets derive `#[derive(Script, Widget)]` and implement `impl Widget { fn handle_event; fn draw_walk }`.

- [ ] **Step 1: Write the app shell**

Create `crates/waml-editor/src/app.rs`:

```rust
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                pass.clear_color: vec4(0.10, 0.11, 0.13, 1.0)
                body +: {
                    View{
                        width: Fill
                        height: Fill
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

- [ ] **Step 2: Rewrite `main.rs` to launch the app**

Replace `crates/waml-editor/src/main.rs` with:

```rust
pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod camera;
mod load;
mod scene;
mod sizing;

app_main!(app::App);
```

- [ ] **Step 3: Build the crate**

Run: `cargo build -p waml-editor`
Expected: PASS. First build compiles a large makepad subtree — allow several minutes. If it fails on the makepad path dependency, STOP and resolve the path with the user (see Global Constraints) before continuing.

- [ ] **Step 4: Run the app and confirm a window opens**

Run: `cargo run -p waml-editor`
Expected: a 1280×840 window with a dark blue-grey background appears. Close it to end the run. (If the project provides a makepad Studio bridge runnable per the makepad `AGENTS.md`, use that instead of raw `cargo run` for the dev run; `cargo run` is acceptable for this spike.)

- [ ] **Step 5: Verify library tests still pass**

Run: `cargo test -p waml-editor --lib`
Expected: PASS — the four pure modules are unaffected.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/app.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): makepad app shell — opens a window"
```

---

### Task 6: `GraphCanvas` widget — draw nodes, pan, zoom, fit

**Files:**
- Create: `crates/waml-editor/src/canvas.rs`
- Modify: `crates/waml-editor/src/app.rs` (register `GraphCanvas`, put one in the window body, feed it the scene)
- Modify: `crates/waml-editor/src/main.rs` (add `mod canvas;`)

**Interfaces:**
- Consumes: `scene::{Scene, bounding_box}`, `camera::Camera`, makepad draw/event API.
- Produces: `pub struct GraphCanvas` (a `#[derive(Script, Widget)]` widget) with `pub fn set_scene(&mut self, cx: &mut Cx, scene: scene::Scene)`.
- Behavior: on first draw with a scene, `Camera::fit` to the canvas rect; left-drag pans; scroll zooms toward the cursor; each node draws as a filled rounded rect + its title text.

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add `mod canvas;` to the module list.

- [ ] **Step 2: Write the widget**

Create `crates/waml-editor/src/canvas.rs`. This mirrors the widget structure and hit-handling proven in `C:\dev\vendor\makepad\widgets\src\map\view.rs`:

```rust
use crate::camera::Camera;
use crate::scene::{bounding_box, Scene};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    mod.widgets.GraphCanvas = set_type_default() do mod.widgets.GraphCanvasBase{
        width: Fill
        height: Fill
        draw_bg +: { color: #x14161d }
        draw_node +: { color: #x2b3345 }
        draw_text +: {
            color: #xe6ebf5
            text_style: theme.font_regular{font_size: 11}
        }
    }
}

#[derive(Script, Widget)]
pub struct GraphCanvas {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[redraw]
    #[live]
    draw_node: DrawColor,
    #[redraw]
    #[live]
    draw_text: DrawText,

    #[rust]
    scene: Scene,
    #[rust]
    camera: Camera,
    #[rust]
    fitted: bool,
    #[rust]
    view_rect: Rect,
    #[rust]
    drag_start_abs: Option<DVec2>,
    #[rust]
    drag_start_pan: (f64, f64),
}

impl Default for Camera {
    fn default() -> Self {
        Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
    }
}

impl Widget for GraphCanvas {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.drag_start_abs = Some(fe.abs);
                self.drag_start_pan = (self.camera.pan_x, self.camera.pan_y);
                cx.set_cursor(MouseCursor::Grabbing);
            }
            Hit::FingerMove(fe) => {
                if let Some(start) = self.drag_start_abs {
                    let delta = fe.abs - start;
                    self.camera.pan_x = self.drag_start_pan.0 - delta.x / self.camera.zoom;
                    self.camera.pan_y = self.drag_start_pan.1 - delta.y / self.camera.zoom;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerUp(_) => {
                self.drag_start_abs = None;
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Grab),
            Hit::FingerScroll(fs) => {
                let scroll = if fs.scroll.y.abs() > f64::EPSILON { fs.scroll.y } else { fs.scroll.x };
                let factor = (-scroll / 240.0).exp2(); // smooth multiplicative zoom
                let local_x = fs.abs.x - self.view_rect.pos.x;
                let local_y = fs.abs.y - self.view_rect.pos.y;
                self.camera.zoom_at(local_x, local_y, factor);
                self.draw_bg.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.view_rect = rect;
        self.draw_bg.draw_abs(cx, rect);

        if !self.fitted {
            if let Some(bbox) = bounding_box(&self.scene) {
                self.camera = Camera::fit(bbox, rect.size.x, rect.size.y, 48.0);
                self.fitted = true;
            }
        }

        // Nodes (groups + edges added in Task 7).
        for node in &self.scene.nodes {
            let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(node.rect.w * self.camera.zoom, node.rect.h * self.camera.zoom),
            };
            self.draw_node.draw_abs(cx, screen);
            self.draw_text.draw_abs(
                cx,
                dvec2(screen.pos.x + 10.0, screen.pos.y + 10.0),
                &node.title,
            );
        }

        DrawStep::done()
    }
}

impl GraphCanvas {
    pub fn set_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.fitted = false;
        self.draw_bg.redraw(cx);
    }
}
```

- [ ] **Step 3: Register the canvas and feed it a scene in `app.rs`**

Replace `crates/waml-editor/src/app.rs` with:

```rust
use crate::load;
use crate::scene::build_scene;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(1280, 840)
                pass.clear_color: vec4(0.08, 0.09, 0.11, 1.0)
                body +: {
                    canvas := GraphCanvas{
                        width: Fill
                        height: Fill
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // Directory + optional diagram title come from argv (wired fully in Task 8).
        let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
        let model = match load::load_model(std::path::Path::new(&dir)) {
            Ok(m) => m,
            Err(e) => {
                error!("failed to load OKF dir {dir:?}: {e}");
                return;
            }
        };
        let Some(diagram) = model.diagrams.first() else {
            error!("no diagrams in {dir:?}");
            return;
        };
        let (scene, diags) = build_scene(&model, diagram);
        for d in &diags {
            warning!("diagnostic: {d:?}");
        }
        self.ui.widget(ids!(canvas)).borrow_mut::<crate::canvas::GraphCanvas>()
            .unwrap()
            .set_scene(cx, scene);
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        crate::canvas::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

Note: the exact `borrow_mut::<GraphCanvas>()` accessor and `error!`/`warning!` macros must be confirmed against the makepad version during the build; if the widget-ref downcast differs, follow the pattern the map example uses to reach a typed widget (`self.ui.<widget>(cx, ids!(canvas))`). This is the one place where the fork's API may require a small adjustment — resolve it against `widgets/src/map/` at build time, not by guessing further.

- [ ] **Step 4: Build and fix any API-surface mismatches**

Run: `cargo build -p waml-editor`
Expected: PASS. If the typed-widget accessor or a draw method name differs in this fork, correct it using `C:\dev\vendor\makepad\widgets\src\map\view.rs` as the reference, then rebuild.

- [ ] **Step 5: Run against the fixture and verify visually**

Run: `cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini`
Expected: two labeled boxes ("Order" left of "Customer") centered in the window. Left-drag pans both; scroll zooms toward the cursor. Close to end.

- [ ] **Step 6: Verify library tests still pass**

Run: `cargo test -p waml-editor --lib`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/canvas.rs crates/waml-editor/src/app.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): GraphCanvas draws nodes with pan/zoom/fit"
```

---

### Task 7: Render groups and edges

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs`

**Interfaces:**
- Consumes: `scene.groups` (`waml::solve::SolvedGroup` with `rect`, `shape: waml::syntax::Shape`, `title`, `depth`) and `scene.edges` (`SceneEdge` with `source`/`target` rects, `kind`).
- Produces: additional draw fields `draw_group`, `draw_edge` and helper `fn border_point(from: waml::solve::Rect, to: waml::solve::Rect) -> (f64, f64)` (center-to-center line clipped to `from`'s border, in world coordinates). Note: this operates on `waml::solve::Rect` (`x`/`y`/`w`/`h`), which is what `SceneEdge` carries — NOT the makepad `Rect` (`pos`/`size`) used for on-screen quads.

- [ ] **Step 1: Write the failing test for edge border clipping**

Add to the `#[cfg(test)]` section of `crates/waml-editor/src/canvas.rs` (create the block if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use waml::solve::Rect as WorldRect;

    #[test]
    fn border_point_exits_on_the_side_facing_the_target() {
        // 100x100 box at origin; target far to the right -> exit on right edge x=100.
        let from = WorldRect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
        let to = WorldRect { x: 500.0, y: 0.0, w: 100.0, h: 100.0 };
        let (x, y) = border_point(from, to);
        assert!((x - 100.0).abs() < 1e-6, "x = {x}");
        assert!((y - 50.0).abs() < 1e-6, "y = {y}");
    }

    #[test]
    fn border_point_handles_vertical_stack() {
        // Target directly below -> exit on bottom edge y=100, centered x=50.
        let from = WorldRect { x: 0.0, y: 0.0, w: 100.0, h: 100.0 };
        let to = WorldRect { x: 0.0, y: 400.0, w: 100.0, h: 100.0 };
        let (x, y) = border_point(from, to);
        assert!((x - 50.0).abs() < 1e-6, "x = {x}");
        assert!((y - 100.0).abs() < 1e-6, "y = {y}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor --lib canvas`
Expected: FAIL to compile — `border_point` not defined.

- [ ] **Step 3: Implement `border_point`**

Add as a free function in `crates/waml-editor/src/canvas.rs` (above `impl Widget`):

```rust
/// Intersection of the center-to-center line from `from` to `to` with `from`'s
/// border, in world coordinates. Operates on `waml::solve::Rect` (`x`/`y`/`w`/`h`),
/// the type `SceneEdge` carries. Used to clip edge endpoints to node borders.
fn border_point(from: waml::solve::Rect, to: waml::solve::Rect) -> (f64, f64) {
    let fcx = from.x + from.w * 0.5;
    let fcy = from.y + from.h * 0.5;
    let tcx = to.x + to.w * 0.5;
    let tcy = to.y + to.h * 0.5;
    let dx = tcx - fcx;
    let dy = tcy - fcy;
    if dx == 0.0 && dy == 0.0 {
        return (fcx, fcy);
    }
    let hw = from.w * 0.5;
    let hh = from.h * 0.5;
    // Scale the direction vector to the nearest border along x and y, take the closer.
    let tx = if dx != 0.0 { (hw / dx).abs() } else { f64::INFINITY };
    let ty = if dy != 0.0 { (hh / dy).abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    (fcx + dx * t, fcy + dy * t)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p waml-editor --lib canvas`
Expected: PASS.

- [ ] **Step 5: Add the group and edge draw fields**

In the `GraphCanvas` struct, add after `draw_node`:

```rust
    #[redraw]
    #[live]
    draw_group: DrawColor,
    #[redraw]
    #[live]
    draw_edge: DrawColor,
```

And in the `script_mod!` default component, add styling after `draw_node +: { ... }`:

```
        draw_group +: { color: #x1b2130 }
        draw_edge +: { color: #x5a6785 }
```

- [ ] **Step 6: Draw groups (behind nodes) and edges (between groups and nodes)**

In `draw_walk`, insert BEFORE the node loop — groups draw first (back):

```rust
        // Groups: framed rects, deeper nesting slightly lighter.
        for group in &self.scene.groups {
            let (lx, ly) = self.camera.world_to_local(group.rect.x, group.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(group.rect.w * self.camera.zoom, group.rect.h * self.camera.zoom),
            };
            self.draw_group.draw_abs(cx, screen);
            if let Some(title) = &group.title {
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0, screen.pos.y + 4.0),
                    title,
                );
            }
        }
```

Then, AFTER the group loop and BEFORE the node loop, draw edges as thin quads clipped to node borders:

```rust
        // Edges: straight segment from source border to target border, drawn as a
        // thin rotated quad. Target-end arrow styling is a fast-follow.
        for edge in &self.scene.edges {
            let (sx, sy) = border_point(edge.source, edge.target);
            let (tx, ty) = border_point(edge.target, edge.source);
            let (a0, a1) = self.camera.world_to_local(sx, sy);
            let (b0, b1) = self.camera.world_to_local(tx, ty);
            let a = dvec2(rect.pos.x + a0, rect.pos.y + a1);
            let b = dvec2(rect.pos.x + b0, rect.pos.y + b1);
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            if len < 1e-3 {
                continue;
            }
            let thickness = 2.0;
            // Midpoint rect approximation: draw a thin axis-aligned box bounding the
            // segment. Rotated-quad refinement is a fast-follow; this reads correctly
            // for orthogonal layouts (the common case for `## Layout` diagrams).
            let min = dvec2(a.x.min(b.x), a.y.min(b.y));
            let max = dvec2(a.x.max(b.x), a.y.max(b.y));
            let seg = Rect {
                pos: min,
                size: dvec2((max.x - min.x).max(thickness), (max.y - min.y).max(thickness)),
            };
            self.draw_edge.draw_abs(cx, seg);
        }
```

Note: the axis-aligned edge box is a deliberate MVP simplification for orthogonal `## Layout` arrangements. A proper thin rotated quad (or line shader) is the documented edge fast-follow; do not add arrowheads or dashing here.

- [ ] **Step 7: Build, run, and verify visually**

Run: `cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini`
Expected: two group frames ("Order", "Customer" membership groups from the fixture — the fixture has ungrouped members, so group count may be low; a richer fixture is fine), a connecting edge between the two nodes, and the two labeled nodes on top. Pan/zoom still work.

- [ ] **Step 8: Run library tests**

Run: `cargo test -p waml-editor --lib`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(editor): render membership groups and relationship edges"
```

---

### Task 8: CLI argument parsing + diagram selection

**Files:**
- Create: `crates/waml-editor/src/cli.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod cli;`)
- Modify: `crates/waml-editor/src/app.rs` (use `cli::Args` + diagram selection)

**Interfaces:**
- Produces:
  - `pub struct Args { pub dir: std::path::PathBuf, pub diagram: Option<String> }`
  - `pub fn parse(argv: &[String]) -> Result<Args, String>` — usage: `waml-editor <okf-dir> [--diagram <title-or-key>]`.
  - `pub fn select_diagram<'a>(model: &'a waml::model::Model, wanted: Option<&str>) -> Option<&'a waml::model::Diagram>` — match by title or key; else the first diagram.

- [ ] **Step 1: Add the module declaration**

In `crates/waml-editor/src/main.rs`, add `mod cli;`.

- [ ] **Step 2: Write the failing tests**

Create `crates/waml-editor/src/cli.rs`:

```rust
//! CLI argument parsing and diagram selection for the viewer.

use std::path::PathBuf;
use waml::model::{Diagram, Model};

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub dir: PathBuf,
    pub diagram: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_dir_only() {
        let a = parse(&argv(&["waml-editor", "some/dir"])).unwrap();
        assert_eq!(a.dir, PathBuf::from("some/dir"));
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn parses_dir_and_diagram_flag() {
        let a = parse(&argv(&["waml-editor", "some/dir", "--diagram", "Orders"])).unwrap();
        assert_eq!(a.diagram.as_deref(), Some("Orders"));
    }

    #[test]
    fn missing_dir_is_an_error() {
        assert!(parse(&argv(&["waml-editor"])).is_err());
    }

    #[test]
    fn select_matches_by_title_then_falls_back_to_first() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();

        let by_title = select_diagram(&model, Some("Orders")).unwrap();
        assert_eq!(by_title.title, "Orders");

        // Unknown name falls back to the first diagram rather than None.
        let fallback = select_diagram(&model, Some("nope")).unwrap();
        assert_eq!(fallback.title, "Orders");

        let default = select_diagram(&model, None).unwrap();
        assert_eq!(default.title, "Orders");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib cli`
Expected: FAIL to compile — `parse`/`select_diagram` not defined.

- [ ] **Step 4: Implement parsing and selection**

Add above the `#[cfg(test)]` block in `crates/waml-editor/src/cli.rs`:

```rust
/// Parse `argv` (including argv[0]). Usage: `waml-editor <okf-dir> [--diagram <name>]`.
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut dir: Option<PathBuf> = None;
    let mut diagram: Option<String> = None;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = Some(
                    argv.get(i).cloned().ok_or("--diagram requires a value")?,
                );
            }
            other if dir.is_none() => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument: {other}")),
        }
        i += 1;
    }
    Ok(Args {
        dir: dir.ok_or("usage: waml-editor <okf-dir> [--diagram <name>]")?,
        diagram,
    })
}

/// Pick a diagram by title or key; fall back to the first diagram.
pub fn select_diagram<'a>(model: &'a Model, wanted: Option<&str>) -> Option<&'a Diagram> {
    if let Some(w) = wanted {
        if let Some(d) = model.diagrams.iter().find(|d| d.title == w || d.key == w) {
            return Some(d);
        }
    }
    model.diagrams.first()
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml-editor --lib cli`
Expected: PASS (all four tests).

- [ ] **Step 6: Wire `cli` into the app startup**

In `crates/waml-editor/src/app.rs`, replace the body of `handle_startup` with:

```rust
    fn handle_startup(&mut self, cx: &mut Cx) {
        let argv: Vec<String> = std::env::args().collect();
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                error!("{e}");
                return;
            }
        };
        let model = match load::load_model(&args.dir) {
            Ok(m) => m,
            Err(e) => {
                error!("failed to load OKF dir {:?}: {e}", args.dir);
                return;
            }
        };
        let Some(diagram) = crate::cli::select_diagram(&model, args.diagram.as_deref()) else {
            error!("no diagrams in {:?}", args.dir);
            return;
        };
        let (scene, diags) = build_scene(&model, diagram);
        for d in &diags {
            warning!("diagnostic: {d:?}");
        }
        self.ui.widget(ids!(canvas)).borrow_mut::<crate::canvas::GraphCanvas>()
            .unwrap()
            .set_scene(cx, scene);
    }
```

- [ ] **Step 7: Build, run with the flag, and verify**

Run: `cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini --diagram Orders`
Expected: same rendering as Task 7; the `--diagram Orders` flag is accepted.

- [ ] **Step 8: Run all tests**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/cli.rs crates/waml-editor/src/app.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): CLI arg parsing + diagram selection"
```

---

### Task 9: Headless render regression check

Produce a headless PNG of the fixture diagram for eyeball verification and future regression. This uses makepad's headless CPU renderer; the exact headless entry point must be located in the vendored makepad at implementation time.

**Files:**
- Create: `crates/waml-editor/tests/headless_render.rs`
- Create: `crates/waml-editor/tests/README.md` (how to run + where the PNG lands)

**Interfaces:**
- Consumes: the makepad headless render API (locate in `C:\dev\vendor\makepad` — search for a headless/offscreen pass entry, e.g. under `platform/` or an example with a `--headless` path).

- [ ] **Step 1: Locate the headless render entry point**

Run: `rg -l "headless" C:\dev\vendor\makepad\platform C:\dev\vendor\makepad\draw`
Expected: one or more modules exposing an offscreen/CPU pass. Read the closest one and note the function that renders a pass to an image buffer. If NO headless entry exists in this fork, STOP: mark this task blocked, keep the manual `cargo run` visual check from Tasks 6–8 as the verification of record, and record the gap in `tests/README.md`. Do not fabricate a headless API.

- [ ] **Step 2: Write the headless test (only if Step 1 found an entry point)**

Create `crates/waml-editor/tests/headless_render.rs` that: loads `tests/fixtures/mini`, builds the scene, drives one headless frame of `GraphCanvas` at a fixed 800×600 size, writes the PNG to the scratchpad, and asserts the buffer is non-empty and not a single flat color (a cheap "something drew" check). Match the exact headless API discovered in Step 1 — the skeleton:

```rust
// Pseudocode skeleton — replace calls with the real headless API from Step 1.
#[test]
fn fixture_diagram_renders_non_empty() {
    // 1. let scene = build_scene(...) from the mini fixture.
    // 2. drive one headless GraphCanvas frame at 800x600 -> Vec<u8> RGBA.
    // 3. assert pixels contain at least two distinct colors (bg + a node).
    // 4. write PNG to the scratchpad dir for eyeball review.
}
```

- [ ] **Step 3: Document the check**

Create `crates/waml-editor/tests/README.md`:

```markdown
# waml-editor tests

- Unit tests (`cargo test -p waml-editor --lib`) cover the engine-agnostic
  modules: `load`, `sizing`, `scene`, `camera`, and the `canvas::border_point`
  geometry helper. No GPU required.
- Visual verification: `cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini`.
- Headless render: `cargo test -p waml-editor --test headless_render` writes a
  PNG of the fixture diagram to the scratchpad for eyeball review.
  (If the makepad fork exposes no headless entry point, this test is absent and
  the manual run above is the verification of record.)
```

- [ ] **Step 4: Run whatever check exists and confirm**

Run: `cargo test -p waml-editor`
Expected: PASS. If the headless test exists, confirm the PNG was written and eyeball it.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/tests/headless_render.rs crates/waml-editor/tests/README.md
git commit -m "test(editor): headless render check for the fixture diagram"
```

---

## Fast-follows (post-MVP, NOT in this plan)

Attribute-row rendering + true text-measured sizing via the makepad font atlas; richer `RelationshipKind` edge styling (arrowheads, dashing, rotated-quad edges); file-open dialog; live re-parse on file change; multi-diagram navigation; auto-layout for diagrams without a `## Layout` section; the inspector/property panel. All of these are additive and change only the sizing function, the node/edge renderer, or the app shell — never the `scene` seam.
