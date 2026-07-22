# Orthogonal Edge Router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a hand-rolled orthogonal (Manhattan) edge router to the Rust solver so `solve_diagram` returns clean, obstacle-avoiding polylines (`Solved.routes`) alongside the existing rects.

**Architecture:** A new `crates/waml/src/solve/route.rs` module implements the libavoid-tier pipeline — orthogonal visibility graph (OVG) → A* shortest path with a bend penalty → a 1D separation "nudge" sweep (plus hub attachment spreading). The top-level `solve_diagram` runs the existing geometry pass, then invokes `route()` with the internal `Box` forest, `BoxId`-keyed solved rects, and the leaf-to-leaf edge list. `Route` is a new wire type carrying `points` + `source`/`target` node-key strings. Everything is hand-rolled; zero new crate dependencies.

**Tech Stack:** Rust (crate `waml`, dep-lean, wasm-targeted). `serde` + `tsify_next` derives on wire types (feature-gated). Colocated `#[cfg(test)]` unit tests mirroring `geometry.rs` style.

## Global Constraints

- **Zero new crate dependencies.** No `petgraph`, `cassowary`, `casuarius`, or FFI. Hand-roll OVG, A*, and the separation sweep. (Crate deps stay `regex`, `pulldown-cmark`, `ttf-parser`.)
- **Determinism is a hard requirement.** Identical input yields **byte-identical** `routes`. Sort obstacles and candidates by `BoxId` and by coordinate; break every tie deterministically; **never** rely on hash-map iteration order (use `BTreeMap`/`BTreeSet`/`Vec` and `f64::total_cmp`).
- **Leaf-to-leaf only.** Self-edges (source == target), group-as-endpoint, splines/curves, and web-frontend consumption of `routes` are OUT of scope — skip them defensively where they can appear.
- **`Route` lives in the wasm `wire` module** in `crates/waml/src/solve/mod.rs`, with the same `serde`/`Tsify` derives as the sibling wire types. Identity is the **node-key `String`** (keys of `Solved.nodes`), never a `BoxId` and never a `RelationshipKind`.
- **Containment-aware group obstacle rule:** a group is an obstacle for an edge **only when BOTH endpoints are non-members** of that group. Membership is the transitive child-list closure (`Box { kind: Group, children, .. }`), NEVER inferred from rect overlap.
- **`routes` is additive:** empty when a scene has no edges; existing consumers that ignore it are unaffected; `pretty()` is NOT extended (keeps the golden test stable).
- **Spec:** `docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md`. Builds on `docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md`.

---

## File Structure

- `crates/waml/src/solve/mod.rs` — MODIFY. Add `Route` to the `wire` module + re-export; add `routes: Vec<Route>` to `Solved`; declare `mod route;`; change `solve_diagram` to accept an `edges` slice and call `route::route`.
- `crates/waml/src/solve/geometry.rs` — MODIFY. Key group rects by `BoxId` in the internal rect map; split `solve` into `solve_with_rects` (returns the `BoxId`-keyed rects) + a thin `solve` wrapper; add `routes: Vec::new()` to the one `Solved` literal.
- `crates/waml/src/solve/route.rs` — CREATE. The whole router: obstacles, OVG, A*, nudge, hub spread, membership, and the `pub(super) fn route` entry. All unit tests colocated.
- `crates/waml/tests/solver_golden.rs` — MODIFY. Pass `&[]` for the new `edges` argument (golden output unchanged; `pretty()` omits routes).
- `crates/waml-wasm/src/lib.rs` — MODIFY. In `solve_bundle`, build the `(BoxId, BoxId)` edge list from `model.edges` and pass it to `solve_diagram`.
- `crates/waml-editor/src/scene.rs` — MODIFY. In `build_scene`, build the edge list from `model.edges` and pass it to `solve_diagram`; add `routes: Vec::new()` to the `stress_default` `Solved` literal.

Route pass runs on the internal IR (before wire projection flattens the layout), so it consumes the `Box` forest and `BoxId`-keyed rects, not the wire `Solved`.

---

### Task 1: Wire type `Route`, `Solved.routes`, and end-to-end plumbing (empty routes)

Establishes the public surface and threads an (initially empty) route pass through `solve_diagram` and every caller, so the crate + workspace compile and all existing tests stay green. No routing logic yet.

**Files:**
- Modify: `crates/waml/src/solve/mod.rs` (wire `Route`, `Solved.routes`, `mod route;`, `solve_diagram` signature + call)
- Create: `crates/waml/src/solve/route.rs` (stub entry only)
- Modify: `crates/waml/src/solve/geometry.rs` (group-keyed rects; `solve_with_rects` split; `Solved` literal gains `routes`)
- Modify: `crates/waml/tests/solver_golden.rs` (pass `&[]`)
- Modify: `crates/waml-wasm/src/lib.rs` (build + pass edges)
- Modify: `crates/waml-editor/src/scene.rs` (build + pass edges; `stress_default` literal gains `routes`)

**Interfaces:**
- Produces:
  - `pub struct Route { pub points: Vec<(f64, f64)>, pub source: String, pub target: String }` (re-exported from `solve`), derives `Debug, Clone, PartialEq` + feature-gated `serde`/`Tsify`.
  - `Solved` gains `pub routes: Vec<Route>`.
  - `pub(super) fn route(boxes: &[Box], rects: &BTreeMap<BoxId, Rect>, edges: &[(BoxId, BoxId)], cfg: &SolveConfig) -> Vec<Route>` — stub returns `Vec::new()`.
  - `geometry::solve_with_rects(scene, sizes, cfg) -> (Solved, BTreeMap<BoxId, Rect>, Vec<Diagnostic>)` where the rect map covers **every** box (`BoxId::Node` leaves AND `BoxId::Group` frames).
  - `solve_diagram(diagram, edges: &[(BoxId, BoxId)], sizes, cfg) -> (Solved, Vec<Diagnostic>)` — new `edges` parameter (second position).

- [ ] **Step 1: Write the failing test — group rects are keyed by `BoxId` in the internal map**

Add to the `tests` module in `crates/waml/src/solve/geometry.rs` (helpers `leaf`, `group`, `sizes` already exist there):

```rust
#[test]
fn solve_with_rects_keys_group_frames_by_boxid() {
    use crate::syntax::Axis;
    let scene = Scene {
        boxes: vec![
            leaf("a"),
            leaf("b"),
            group(
                0,
                vec![BoxId::Node("a".into()), BoxId::Node("b".into())],
                Some(Axis::Column),
                Shape::Frame,
                "Users",
            ),
        ],
        constraints: vec![],
    };
    let (_solved, rects, diags) = solve_with_rects(
        &scene,
        &sizes(&["a", "b"], 200.0, 90.0),
        &SolveConfig::default(),
    );
    assert!(diags.is_empty());
    // Leaf rects present.
    assert!(rects.contains_key(&BoxId::Node("a".into())));
    assert!(rects.contains_key(&BoxId::Node("b".into())));
    // The group frame is keyed by its BoxId and equals the "Users" frame @ 0,0 232x228.
    let g = rects[&BoxId::Group(0)];
    assert_eq!((g.x, g.y, g.w, g.h), (0.0, 0.0, 232.0, 228.0));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p waml --lib solve::geometry::tests::solve_with_rects_keys_group_frames_by_boxid`
Expected: FAIL — `solve_with_rects` does not exist.

- [ ] **Step 3: Key group rects by `BoxId` and split `solve` into `solve_with_rects`**

In `crates/waml/src/solve/geometry.rs`, in `fn solve_box`, after the `assemble(...)` call that builds a group's `Laid`, insert the group's own frame rect (local `0,0,outer`) into the returned `rects` keyed by its `BoxId`. The simplest place is to capture the `assemble` result and add the key before returning:

```rust
    let inset = cfg.margin(b.margin);
    let mut laid = assemble(
        &b.children,
        &child_laid,
        &child_margins,
        &cons,
        inset,
        Some((b.shape, b.title.clone(), b.depth)),
        cfg,
        diags,
    );
    // Key this group's own frame by its BoxId so the router can use group rects
    // as containment-aware obstacles. Behavior-preserving: Solved.nodes only ever
    // extracts BoxId::Node entries, so group keys are invisible to existing output.
    laid.rects.insert(
        id.clone(),
        Rect { x: 0.0, y: 0.0, w: laid.size.w, h: laid.size.h },
    );
    laid
```

(`assemble` translates a child subtree's entire `rects` map into the parent frame, so these group keys ride along and end up correctly positioned in the final `laid.rects`.)

Now split the public `solve`. Rename the current body to `solve_with_rects` returning the rect map, and add a thin wrapper so the ~dozen existing `solve(...)` test/callsites are unchanged:

```rust
pub fn solve(scene: &Scene, sizes: &SizeMap, cfg: &SolveConfig) -> (Solved, Vec<Diagnostic>) {
    let (solved, _rects, diags) = solve_with_rects(scene, sizes, cfg);
    (solved, diags)
}

pub(super) fn solve_with_rects(
    scene: &Scene,
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, BTreeMap<BoxId, Rect>, Vec<Diagnostic>) {
    // ... existing body of `solve` verbatim, EXCEPT:
    //   * keep `laid.rects` intact (do NOT move it out before use);
    //   * the flattening loop clones keys instead of consuming, e.g.
    //         for (id, r) in &laid.rects {
    //             if let BoxId::Node(key) = id { nodes.insert(key.clone(), *r); }
    //         }
    //   * add `routes: Vec::new()` to the returned `Solved` literal (see Step 5);
    //   * return `(Solved { .. }, laid.rects, diags)`.
}
```

- [ ] **Step 4: Run the group-rects test to verify it passes**

Run: `cargo test -p waml --lib solve::geometry::tests::solve_with_rects_keys_group_frames_by_boxid`
Expected: PASS.

- [ ] **Step 5: Add the `Route` wire type + `Solved.routes` field**

In `crates/waml/src/solve/mod.rs`, inside `mod wire { .. }`, add (mirroring the existing wire structs' derives):

```rust
    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct Route {
        pub points: Vec<(f64, f64)>,
        pub source: String,
        pub target: String,
    }
```

Add the field to `Solved` (after `flags`):

```rust
        pub flags: BTreeMap<String, FlagSet>,
        #[cfg_attr(feature = "serde", serde(default))]
        pub routes: Vec<Route>,
```

Extend the re-export line:

```rust
pub use wire::{FlagSet, Rect, Route, Size, SolveConfig, Solved, SolvedGroup};
```

Update the two `Solved { .. }` literals in the `mod tests` of `mod.rs` (lines ~230 and ~271) to add `routes: vec![],`. Update the `Solved` literal in `geometry.rs::solve_with_rects` to add `routes: Vec::new(),`.

- [ ] **Step 6: Declare the route module + stub, and wire `solve_diagram`**

Create `crates/waml/src/solve/route.rs`:

```rust
//! Orthogonal (Manhattan) edge router: OVG -> A* (bend penalty) -> nudge.
//! See docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md.

use super::{Box, BoxId, Rect, Route, SolveConfig};
use std::collections::BTreeMap;

/// Route every leaf-to-leaf edge as an orthogonal polyline avoiding obstacles.
pub(super) fn route(
    _boxes: &[Box],
    _rects: &BTreeMap<BoxId, Rect>,
    _edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    Vec::new()
}
```

In `crates/waml/src/solve/mod.rs`, add `mod route;` beside the other `pub mod` lines (plain `mod` — the entry is `pub(super)`), and rewrite `solve_diagram`:

```rust
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let (mut solved, rects, mut geo_diags) = geometry::solve_with_rects(&scene, sizes, cfg);
    diags.append(&mut geo_diags);
    solved.routes = route::route(&scene.boxes, &rects, edges, cfg);
    (solved, diags)
}
```

- [ ] **Step 7: Update the three `solve_diagram` callers to compile**

`crates/waml/tests/solver_golden.rs` (~line 54): pass an empty edge slice:

```rust
    let (solved, diags) = solve_diagram(&diagram, &[], &sizes, &SolveConfig::default());
```

`crates/waml-wasm/src/lib.rs` (`solve_bundle`, ~line 84) — replace the `solve_diagram` call with a version that builds the leaf edge list from the model pool:

```rust
    let edges: Vec<(waml::solve::BoxId, waml::solve::BoxId)> = model
        .edges
        .iter()
        .filter(|e| e.source != e.target) // self-edges out of scope
        .map(|e| {
            (
                waml::solve::BoxId::Node(e.source.clone()),
                waml::solve::BoxId::Node(e.target.clone()),
            )
        })
        .collect();
    let (solved, diagnostics) = waml::solve::solve_diagram(diagram, &edges, &sizes, &cfg);
```

`crates/waml-editor/src/scene.rs` (`build_scene`, ~line 187) — `BoxId` is already imported:

```rust
    let edges: Vec<(BoxId, BoxId)> = model
        .edges
        .iter()
        .filter(|e| e.source != e.target)
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    let (solved, diags) = if use_stress_default(diagram) {
        (stress_default(model, &sizes), Vec::new())
    } else {
        solve_diagram(diagram, &edges, &sizes, &SolveConfig::default())
    };
```

Also add `routes: Vec::new(),` to the `Solved { .. }` literal in `stress_default` (`scene.rs` ~line 172).

> Note (input plumbing only): passing all `model.edges` mapped to `BoxId::Node` is safe — `route()` skips any edge whose endpoint is absent from `rects`. Consuming `solved.routes` in the canvas is future work (out of scope).

- [ ] **Step 8: Run to verify the whole workspace is green**

Run: `cargo test -p waml`
Expected: PASS (all existing solver tests + the new group-rects test).
Run: `cargo build -p waml-wasm -p waml-editor`
Expected: builds clean.

- [ ] **Step 9: Commit**

```bash
git add crates/waml/src/solve/mod.rs crates/waml/src/solve/geometry.rs crates/waml/src/solve/route.rs crates/waml/tests/solver_golden.rs crates/waml-wasm/src/lib.rs crates/waml-editor/src/scene.rs
git commit -m "feat(solve): add Route wire type + empty route pass plumbing"
```

---

### Task 2: Orthogonal visibility graph (OVG) construction

Build the sparse candidate graph from obstacle-edge coordinates (each obstacle inflated by a routing margin) plus per-endpoint free-perimeter attachment candidates. Standalone and unit-tested; not yet wired into `route()`.

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `Route`, `BoxId`, `Rect` (Task 1).
- Produces (all module-private in `route.rs`):
  - `const ROUTE_MARGIN: f64 = 12.0;` — obstacle inflation / perimeter standoff. (Value not fixed by spec; a module const, tunable.)
  - `type P = (f64, f64);`
  - `struct Obstacle { id: BoxId, rect: Rect }`
  - `struct Ovg { verts: Vec<P>, adj: Vec<Vec<(usize, f64)>> }` — adjacency: `adj[i]` lists `(neighbor_index, segment_length)`; only orthogonal, obstacle-free segments.
  - `fn inflate(r: Rect, m: f64) -> Rect`, `fn strictly_inside(r: &Rect, x: f64, y: f64) -> bool`, `fn segment_blocked(inflated: &[Rect], a: P, b: P) -> bool` — geometry helpers (used again in Tasks 3/6/7 tests).
  - `fn leaf_obstacles(rects: &BTreeMap<BoxId, Rect>, exclude: &[BoxId]) -> Vec<Obstacle>` — every `BoxId::Node` rect except `exclude`, sorted by `BoxId`.
  - `fn build_ovg(obstacles: &[Obstacle], src: Rect, tgt: Rect) -> (Ovg, Vec<usize>, Vec<usize>)` — returns the graph plus the source-attachment and target-attachment vertex indices.

- [ ] **Step 1: Write the failing tests**

Add a `#[cfg(test)] mod tests` to `crates/waml/src/solve/route.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::BoxId;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect { Rect { x, y, w, h } }

    #[test]
    fn ovg_has_attachments_on_all_four_sides_and_is_obstacle_free() {
        // Two boxes, clear gap; no third obstacle.
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(300.0, 0.0, 100.0, 60.0);
        let (ovg, srcv, tgtv) = build_ovg(&[], src, tgt);
        assert!(!srcv.is_empty(), "source has attachment candidates");
        assert!(!tgtv.is_empty(), "target has attachment candidates");
        // Every adjacency segment is axis-aligned (orthogonal).
        for (i, nbrs) in ovg.adj.iter().enumerate() {
            for &(j, _len) in nbrs {
                let (ax, ay) = ovg.verts[i];
                let (bx, by) = ovg.verts[j];
                assert!(
                    (ax - bx).abs() < 1e-9 || (ay - by).abs() < 1e-9,
                    "segment {i}->{j} must be orthogonal"
                );
            }
        }
    }

    #[test]
    fn ovg_vertices_avoid_inflated_obstacle_interior() {
        // An obstacle sitting between src and tgt.
        let mid = Obstacle { id: BoxId::Node("m".into()), rect: r(150.0, -20.0, 80.0, 100.0) };
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(350.0, 0.0, 100.0, 60.0);
        let (ovg, _s, _t) = build_ovg(&[mid.clone()], src, tgt);
        let inflated = inflate(mid.rect, ROUTE_MARGIN);
        for &(x, y) in &ovg.verts {
            assert!(!strictly_inside(&inflated, x, y),
                "vertex ({x},{y}) must not be strictly inside the inflated obstacle");
        }
    }

    #[test]
    fn leaf_obstacles_excludes_endpoints_and_sorts_by_boxid() {
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("b".into()), r(0.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Node("a".into()), r(20.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Node("c".into()), r(40.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Group(0), r(0.0, 0.0, 60.0, 20.0)); // groups excluded here
        let obs = leaf_obstacles(&rects, &[BoxId::Node("a".into())]);
        let ids: Vec<_> = obs.iter().map(|o| o.id.clone()).collect();
        assert_eq!(ids, vec![BoxId::Node("b".into()), BoxId::Node("c".into())]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests`
Expected: FAIL — `build_ovg` / `leaf_obstacles` / `ROUTE_MARGIN` / `P` / `Ovg` / `Obstacle` not defined.

- [ ] **Step 3: Implement the OVG**

Add to `crates/waml/src/solve/route.rs` (above the `tests` module). Algorithm: collect the "interesting" x and y coordinates (each obstacle's inflated left/right and top/bottom, plus the four sides of `src`/`tgt`), form the grid of their intersections, drop any vertex strictly inside an inflated obstacle, connect grid-adjacent survivors along each shared coordinate line when the connecting segment does not cross an inflated obstacle interior, and add perpendicular perimeter attachment candidates for `src`/`tgt`.

```rust
const ROUTE_MARGIN: f64 = 12.0;

type P = (f64, f64);

#[derive(Debug, Clone, PartialEq)]
struct Obstacle {
    id: BoxId,
    rect: Rect,
}

#[derive(Debug, Clone)]
struct Ovg {
    verts: Vec<P>,
    adj: Vec<Vec<(usize, f64)>>,
}

fn inflate(r: Rect, m: f64) -> Rect {
    Rect { x: r.x - m, y: r.y - m, w: r.w + 2.0 * m, h: r.h + 2.0 * m }
}

/// Strictly inside (edges are allowed — a vertex may sit on an inflated border).
fn strictly_inside(r: &Rect, x: f64, y: f64) -> bool {
    x > r.x + 1e-9 && x < r.x + r.w - 1e-9 && y > r.y + 1e-9 && y < r.y + r.h - 1e-9
}

/// True if the axis-aligned segment (a..b) passes through any inflated obstacle interior.
fn segment_blocked(inflated: &[Rect], a: P, b: P) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    inflated.iter().any(|r| {
        let ox0 = r.x.max(x0);
        let ox1 = (r.x + r.w).min(x1);
        let oy0 = r.y.max(y0);
        let oy1 = (r.y + r.h).min(y1);
        // Positive overlap on BOTH axes => the segment cuts the interior.
        (ox1 - ox0) > 1e-9 && (oy1 - oy0) > 1e-9
    })
}

fn leaf_obstacles(rects: &BTreeMap<BoxId, Rect>, exclude: &[BoxId]) -> Vec<Obstacle> {
    let mut out: Vec<Obstacle> = rects
        .iter()
        .filter(|(id, _)| matches!(id, BoxId::Node(_)) && !exclude.contains(id))
        .map(|(id, r)| Obstacle { id: id.clone(), rect: *r })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Deterministic sorted-unique coordinate list.
fn axis_coords(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

fn build_ovg(obstacles: &[Obstacle], src: Rect, tgt: Rect) -> (Ovg, Vec<usize>, Vec<usize>) {
    let inflated: Vec<Rect> = obstacles.iter().map(|o| inflate(o.rect, ROUTE_MARGIN)).collect();

    // Interesting coordinates: inflated obstacle borders + endpoint box borders.
    let mut xs = vec![src.x, src.x + src.w, tgt.x, tgt.x + tgt.w];
    let mut ys = vec![src.y, src.y + src.h, tgt.y, tgt.y + tgt.h];
    for r in &inflated {
        xs.push(r.x);
        xs.push(r.x + r.w);
        ys.push(r.y);
        ys.push(r.y + r.h);
    }
    let xs = axis_coords(xs);
    let ys = axis_coords(ys);

    // Grid intersections that are not strictly inside any inflated obstacle.
    let mut verts: Vec<P> = Vec::new();
    let mut at: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (xi, &x) in xs.iter().enumerate() {
        for (yi, &y) in ys.iter().enumerate() {
            if inflated.iter().any(|r| strictly_inside(r, x, y)) {
                continue;
            }
            at.insert((xi, yi), verts.len());
            verts.push((x, y));
        }
    }

    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); verts.len()];
    let connect = |verts: &Vec<P>, adj: &mut Vec<Vec<(usize, f64)>>, i: usize, j: usize| {
        let (a, b) = (verts[i], verts[j]);
        if segment_blocked(&inflated, a, b) {
            return;
        }
        let len = (a.0 - b.0).abs() + (a.1 - b.1).abs();
        adj[i].push((j, len));
        adj[j].push((i, len));
    };
    // Horizontal neighbours: same yi, next present xi.
    for yi in 0..ys.len() {
        let mut prev: Option<usize> = None;
        for xi in 0..xs.len() {
            if let Some(&idx) = at.get(&(xi, yi)) {
                if let Some(p) = prev {
                    connect(&verts, &mut adj, p, idx);
                }
                prev = Some(idx);
            }
        }
    }
    // Vertical neighbours: same xi, next present yi.
    for xi in 0..xs.len() {
        let mut prev: Option<usize> = None;
        for yi in 0..ys.len() {
            if let Some(&idx) = at.get(&(xi, yi)) {
                if let Some(p) = prev {
                    connect(&verts, &mut adj, p, idx);
                }
                prev = Some(idx);
            }
        }
    }

    // Free-perimeter attachment candidates for one endpoint box: a vertex at
    // every interesting coordinate on its four sides (plus side midpoints),
    // joined perpendicular into any aligned, unblocked grid vertex.
    let attach = |verts: &mut Vec<P>, adj: &mut Vec<Vec<(usize, f64)>>, bx: Rect| -> Vec<usize> {
        let mut points: Vec<P> = Vec::new();
        for &y in &ys {
            if y >= bx.y - 1e-9 && y <= bx.y + bx.h + 1e-9 {
                points.push((bx.x, y));
                points.push((bx.x + bx.w, y));
            }
        }
        for &x in &xs {
            if x >= bx.x - 1e-9 && x <= bx.x + bx.w + 1e-9 {
                points.push((x, bx.y));
                points.push((x, bx.y + bx.h));
            }
        }
        // Side midpoints guarantee at least one candidate per side.
        points.push((bx.x, bx.y + bx.h / 2.0));
        points.push((bx.x + bx.w, bx.y + bx.h / 2.0));
        points.push((bx.x + bx.w / 2.0, bx.y));
        points.push((bx.x + bx.w / 2.0, bx.y + bx.h));
        points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);

        let mut idxs = Vec::new();
        for pt in points {
            let ai = verts.len();
            verts.push(pt);
            adj.push(Vec::new());
            idxs.push(ai);
            for gi in 0..ai {
                let g = verts[gi];
                let aligned = (g.0 - pt.0).abs() < 1e-9 || (g.1 - pt.1).abs() < 1e-9;
                if aligned && !segment_blocked(&inflated, pt, g) {
                    let len = (g.0 - pt.0).abs() + (g.1 - pt.1).abs();
                    adj[ai].push((gi, len));
                    adj[gi].push((ai, len));
                }
            }
        }
        idxs
    };

    let srcv = attach(&mut verts, &mut adj, src);
    let tgtv = attach(&mut verts, &mut adj, tgt);
    (Ovg { verts, adj }, srcv, tgtv)
}
```

- [ ] **Step 4: Run to verify the tests pass**

Run: `cargo test -p waml --lib solve::route::tests`
Expected: PASS (all three OVG tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): orthogonal visibility graph construction"
```

---

### Task 3: A* shortest path with bend penalty (single-edge polyline)

Search the OVG for the least-cost orthogonal path (segment length + per-corner bend penalty), reconstruct and simplify the polyline. Straight line-of-sight emerges as the zero-bend degenerate.

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `Ovg`, `P`, `build_ovg`, `inflate`, `strictly_inside` (Task 2).
- Produces:
  - `const BEND_PENALTY: f64 = 20.0;` (magnitude not fixed by spec; a module const, tunable).
  - `fn astar(ovg: &Ovg, sources: &[usize], targets: &[usize], goal: P) -> Option<Vec<P>>` — least-cost polyline from any source-attachment to any target-attachment; `goal` is the target box center, used only for the admissible Manhattan heuristic.
  - `fn simplify(pts: Vec<P>) -> Vec<P>` — collapse consecutive collinear points and exact duplicates.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `route.rs`:

```rust
    #[test]
    fn astar_clear_line_of_sight_is_two_point_straight() {
        // Boxes sharing a y-band with a clear horizontal gap.
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(300.0, 0.0, 100.0, 60.0);
        let (ovg, srcv, tgtv) = build_ovg(&[], src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let path = astar(&ovg, &srcv, &tgtv, goal).expect("path exists");
        // Straight degenerate: a single horizontal segment => two points, equal y.
        assert_eq!(path.len(), 2, "straight route is two points, got {path:?}");
        assert!((path[0].1 - path[1].1).abs() < 1e-6, "same y => horizontal");
    }

    #[test]
    fn astar_detours_around_blocking_obstacle_orthogonally() {
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(350.0, 0.0, 100.0, 60.0);
        let mid = Obstacle { id: BoxId::Node("m".into()), rect: r(150.0, -30.0, 80.0, 120.0) };
        let (ovg, srcv, tgtv) = build_ovg(&[mid.clone()], src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let path = astar(&ovg, &srcv, &tgtv, goal).expect("path exists");
        assert!(path.len() >= 4, "a detour has >= 4 points, got {path:?}");
        for w in path.windows(2) {
            assert!(
                (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                "segment {:?}->{:?} not orthogonal", w[0], w[1]
            );
        }
        let inf = inflate(mid.rect, ROUTE_MARGIN);
        for &(x, y) in &path {
            assert!(!strictly_inside(&inf, x, y), "path pierces obstacle at ({x},{y})");
        }
    }

    #[test]
    fn simplify_collapses_collinear_and_duplicates() {
        let pts = vec![(0.0, 0.0), (0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (20.0, 10.0)];
        assert_eq!(simplify(pts), vec![(0.0, 0.0), (20.0, 0.0), (20.0, 10.0)]);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests::astar`
Expected: FAIL — `astar` / `BEND_PENALTY` / `simplify` not defined.

- [ ] **Step 3: Implement A* + simplify**

State = `(vertex, direction)` where direction ∈ {0=none, 1=horizontal, 2=vertical}; a bend is charged when the incoming direction is non-none and differs from the outgoing. Frontier is a `BinaryHeap` of `Reverse((Ord64(f), state))`; `Ord64` wraps `f64` with `total_cmp` (no new deps); ties broken by state index for determinism.

```rust
const BEND_PENALTY: f64 = 20.0;

#[derive(Clone, Copy, PartialEq)]
struct Ord64(f64);
impl Eq for Ord64 {}
impl PartialOrd for Ord64 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}
impl Ord for Ord64 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering { self.0.total_cmp(&o.0) }
}

fn dir_of(a: P, b: P) -> u8 {
    if (a.1 - b.1).abs() < 1e-9 { 1 } else { 2 } // horizontal else vertical
}

fn simplify(pts: Vec<P>) -> Vec<P> {
    let mut out: Vec<P> = Vec::new();
    for p in pts {
        if out.last().is_some_and(|&l| (l.0 - p.0).abs() < 1e-9 && (l.1 - p.1).abs() < 1e-9) {
            continue; // duplicate
        }
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let colinear_x = (a.0 - b.0).abs() < 1e-9 && (b.0 - p.0).abs() < 1e-9;
            let colinear_y = (a.1 - b.1).abs() < 1e-9 && (b.1 - p.1).abs() < 1e-9;
            if colinear_x || colinear_y {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

fn astar(ovg: &Ovg, sources: &[usize], targets: &[usize], goal: P) -> Option<Vec<P>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    let n = ovg.verts.len();
    let state = |v: usize, d: u8| v * 3 + d as usize;
    let mut dist = vec![f64::INFINITY; n * 3];
    let mut prev: Vec<Option<usize>> = vec![None; n * 3]; // predecessor STATE
    let is_target = |v: usize| targets.contains(&v);
    let h = |v: usize| {
        let (x, y) = ovg.verts[v];
        (x - goal.0).abs() + (y - goal.1).abs()
    };

    let mut srt = sources.to_vec();
    srt.sort_unstable();
    let mut heap: BinaryHeap<Reverse<(Ord64, usize)>> = BinaryHeap::new();
    for &s in &srt {
        let st = state(s, 0);
        if dist[st] > 0.0 {
            dist[st] = 0.0;
            heap.push(Reverse((Ord64(h(s)), st)));
        }
    }

    let mut goal_state: Option<usize> = None;
    while let Some(Reverse((_f, st))) = heap.pop() {
        let v = st / 3;
        let d = (st % 3) as u8;
        let g = dist[st];
        if is_target(v) {
            goal_state = Some(st);
            break;
        }
        for &(w, len) in &ovg.adj[v] {
            let nd = dir_of(ovg.verts[v], ovg.verts[w]);
            let bend = if d != 0 && d != nd { BEND_PENALTY } else { 0.0 };
            let ng = g + len + bend;
            let ns = state(w, nd);
            if ng + 1e-9 < dist[ns] {
                dist[ns] = ng;
                prev[ns] = Some(st);
                heap.push(Reverse((Ord64(ng + h(w)), ns)));
            }
        }
    }

    let mut cur = goal_state?;
    let mut rev: Vec<P> = Vec::new();
    loop {
        rev.push(ovg.verts[cur / 3]);
        match prev[cur] {
            Some(p) => cur = p,
            None => break,
        }
    }
    rev.reverse();
    Some(simplify(rev))
}
```

- [ ] **Step 4: Run to verify the tests pass**

Run: `cargo test -p waml --lib solve::route::tests`
Expected: PASS (OVG tests + the three A* tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): A* orthogonal path with bend penalty"
```

---

### Task 4: Nudging — 1D separation sweep for coincident channel runs

Split parallel segments that share a routing channel (same axis + coincident coordinate) into distinct parallel lines via an order-then-push sweep (the specialized VPSC separation, O(n log n)).

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `Route`, `P` (Task 1/2).
- Produces:
  - `const NUDGE_GAP: f64 = 8.0;` (minimum channel gap; not fixed by spec; a module const).
  - `fn nudge(routes: &mut [Route])` — mutates polyline interior coordinates in place so coincident co-linear interior segments separate to at least `NUDGE_GAP`. Endpoints (first/last point of each route) are never moved.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn nudge_separates_coincident_parallel_segments() {
        // Two routes both running horizontally along y = 50 via an INTERIOR
        // segment (first/last segments are anchored and excluded from nudging).
        let mk = |src: &str| Route {
            points: vec![(0.0, 0.0), (0.0, 50.0), (100.0, 50.0), (100.0, 0.0)],
            source: src.into(),
            target: "t".into(),
        };
        let mut routes = vec![mk("a"), mk("b")];
        nudge(&mut routes);
        let y0 = routes[0].points[1].1;
        let y1 = routes[1].points[1].1;
        assert!((y0 - y1).abs() >= NUDGE_GAP - 1e-6, "runs must separate: {y0} vs {y1}");
        // Endpoints untouched.
        assert_eq!(routes[0].points[0], (0.0, 0.0));
        assert_eq!(routes[0].points[3], (100.0, 0.0));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests::nudge_separates`
Expected: FAIL — `nudge` / `NUDGE_GAP` not defined.

- [ ] **Step 3: Implement the separation sweep**

Bucket every interior segment (a segment touching neither the first nor last route point) by channel key `(axis, coord quantized to 1e-6)`. Within a channel of >1 member, sort deterministically (by the segment's other-axis midpoint, then `source`, then `target`), then push each member to a slot `base + k * NUDGE_GAP` centered on the original coordinate, rewriting the shared coordinate on both endpoints of that segment.

```rust
fn nudge(routes: &mut [Route]) {
    #[derive(Clone)]
    struct Seg { ri: usize, a: usize, b: usize, other_mid: f64, src: String, tgt: String }
    let mut chan_h: BTreeMap<i64, Vec<Seg>> = BTreeMap::new(); // key = quantized y
    let mut chan_v: BTreeMap<i64, Vec<Seg>> = BTreeMap::new(); // key = quantized x
    let q = |c: f64| (c * 1e6).round() as i64;

    for (ri, route) in routes.iter().enumerate() {
        let n = route.points.len();
        for i in 0..n.saturating_sub(1) {
            // Skip first/last segment: keep route endpoints anchored to their box.
            if i == 0 || i + 1 == n - 1 {
                continue;
            }
            let a = route.points[i];
            let b = route.points[i + 1];
            if (a.1 - b.1).abs() < 1e-9 {
                chan_h.entry(q(a.1)).or_default().push(Seg {
                    ri, a: i, b: i + 1, other_mid: (a.0 + b.0) / 2.0,
                    src: route.source.clone(), tgt: route.target.clone(),
                });
            } else if (a.0 - b.0).abs() < 1e-9 {
                chan_v.entry(q(a.0)).or_default().push(Seg {
                    ri, a: i, b: i + 1, other_mid: (a.1 + b.1) / 2.0,
                    src: route.source.clone(), tgt: route.target.clone(),
                });
            }
        }
    }

    fn sweep(chan: BTreeMap<i64, Vec<Seg>>, routes: &mut [Route], horizontal: bool) {
        for (key, mut segs) in chan {
            if segs.len() < 2 {
                continue;
            }
            segs.sort_by(|p, r| {
                p.other_mid.total_cmp(&r.other_mid).then(p.src.cmp(&r.src)).then(p.tgt.cmp(&r.tgt))
            });
            let base = key as f64 / 1e6;
            let m = segs.len();
            let start = base - (m as f64 - 1.0) * NUDGE_GAP / 2.0;
            for (k, s) in segs.iter().enumerate() {
                let coord = start + k as f64 * NUDGE_GAP;
                if horizontal {
                    routes[s.ri].points[s.a].1 = coord;
                    routes[s.ri].points[s.b].1 = coord;
                } else {
                    routes[s.ri].points[s.a].0 = coord;
                    routes[s.ri].points[s.b].0 = coord;
                }
            }
        }
    }
    sweep(chan_h, routes, true);
    sweep(chan_v, routes, false);
}

const NUDGE_GAP: f64 = 8.0;
```

(`Seg` must be visible to the free `fn sweep`; declare `NUDGE_GAP` at module scope and lift `Seg` out of `nudge` to a module-private struct if the compiler requires it — a local struct in `nudge` cannot be named by a sibling `fn`, so define `struct Seg { .. }` at module scope just above `fn nudge`.)

- [ ] **Step 4: Run to verify the test passes**

Run: `cargo test -p waml --lib solve::route::tests::nudge_separates`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): 1D separation nudge for parallel channel runs"
```

---

### Task 5: Hub attachment spreading

A node with many edges spreads its attachment points along the border side so edges fan out at the box instead of piling on one point (no two edges share an attachment point).

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `Route`, `Rect`, `BoxId`, `P` (Task 1/2).
- Produces:
  - `fn hub_spread(routes: &mut [Route], rects: &BTreeMap<BoxId, Rect>)` — groups the route ENDPOINTS (first point = source box, last point = target box) that land on the same side of the same box and reassigns evenly spaced distinct offsets along that side, rewriting the endpoint and the adjacent interior point that shares its coordinate (so the first/last segment stays perpendicular).

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn hub_spread_gives_distinct_attachment_points() {
        // Hub `h`: three edges all attaching at the same right-side midpoint.
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("h".into()), r(0.0, 0.0, 100.0, 90.0));
        rects.insert(BoxId::Node("t1".into()), r(300.0, 0.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t2".into()), r(300.0, 40.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t3".into()), r(300.0, 80.0, 60.0, 30.0));
        let mk = |t: &str, ty: f64| Route {
            points: vec![(100.0, 45.0), (300.0, ty)],
            source: "h".into(),
            target: t.into(),
        };
        let mut routes = vec![mk("t1", 15.0), mk("t2", 55.0), mk("t3", 95.0)];
        hub_spread(&mut routes, &rects);
        let ys: Vec<f64> = routes.iter().map(|rt| rt.points[0].1).collect();
        for rt in &routes { assert!((rt.points[0].0 - 100.0).abs() < 1e-6, "stay on right border"); }
        assert!(
            (ys[0] - ys[1]).abs() > 1e-6 && (ys[1] - ys[2]).abs() > 1e-6 && (ys[0] - ys[2]).abs() > 1e-6,
            "attachments must be distinct: {ys:?}"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests::hub_spread`
Expected: FAIL — `hub_spread` not defined.

- [ ] **Step 3: Implement hub spreading**

```rust
#[derive(Clone, Copy, PartialEq)]
enum Side { Left, Right, Top, Bottom }

fn side_of(bx: &Rect, p: P) -> Option<Side> {
    let e = 1e-6;
    if (p.0 - bx.x).abs() < e { Some(Side::Left) }
    else if (p.0 - (bx.x + bx.w)).abs() < e { Some(Side::Right) }
    else if (p.1 - bx.y).abs() < e { Some(Side::Top) }
    else if (p.1 - (bx.y + bx.h)).abs() < e { Some(Side::Bottom) }
    else { None }
}

struct End { ri: usize, ep: usize, nb: usize, along: f64 }

fn hub_spread(routes: &mut [Route], rects: &BTreeMap<BoxId, Rect>) {
    let mut groups: BTreeMap<(String, u8), Vec<End>> = BTreeMap::new();
    let sd = |s: Side| match s { Side::Left => 0u8, Side::Right => 1, Side::Top => 2, Side::Bottom => 3 };

    for (ri, route) in routes.iter().enumerate() {
        if route.points.len() < 2 { continue; }
        let last = route.points.len() - 1;
        for (key, ep, nb) in [
            (route.source.clone(), 0usize, 1usize),
            (route.target.clone(), last, last - 1),
        ] {
            let Some(bx) = rects.get(&BoxId::Node(key.clone())) else { continue };
            let p = route.points[ep];
            let Some(side) = side_of(bx, p) else { continue };
            let neighbour = route.points[nb];
            let along = match side {
                Side::Left | Side::Right => neighbour.1,
                Side::Top | Side::Bottom => neighbour.0,
            };
            groups.entry((key, sd(side))).or_default().push(End { ri, ep, nb, along });
        }
    }

    for ((key, sdisc), mut ends) in groups {
        if ends.len() < 2 { continue; }
        let bx = rects[&BoxId::Node(key)];
        ends.sort_by(|a, b| a.along.total_cmp(&b.along).then(a.ri.cmp(&b.ri)));
        let m = ends.len();
        let horizontal_side = sdisc == 2 || sdisc == 3; // Top/Bottom spread along x
        let (span_lo, span_hi, fixed) = if horizontal_side {
            (bx.x, bx.x + bx.w, if sdisc == 2 { bx.y } else { bx.y + bx.h })
        } else {
            (bx.y, bx.y + bx.h, if sdisc == 0 { bx.x } else { bx.x + bx.w })
        };
        for (k, e) in ends.iter().enumerate() {
            let t = (k as f64 + 1.0) / (m as f64 + 1.0); // interior fraction, no corners
            let along = span_lo + t * (span_hi - span_lo);
            if horizontal_side {
                routes[e.ri].points[e.ep] = (along, fixed);
                routes[e.ri].points[e.nb].0 = along; // keep first/last segment perpendicular
            } else {
                routes[e.ri].points[e.ep] = (fixed, along);
                routes[e.ri].points[e.nb].1 = along;
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify the test passes**

Run: `cargo test -p waml --lib solve::route::tests::hub_spread`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): spread hub attachment points along the border"
```

---

### Task 6: Wire the pipeline into `route()` (leaf obstacles) + integration tests

Replace the Task 1 stub `route()` body with the full pipeline over leaf obstacles: per edge build OVG → A* → collect polyline; then hub-spread and nudge across all routes; map each leaf `BoxId::Node(key)` to its `key` string. Group obstacles are added in Task 7.

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `leaf_obstacles`, `build_ovg`, `astar`, `hub_spread`, `nudge`, `simplify` (Tasks 2-5).
- Produces:
  - Final `route()` body (leaf obstacles only): iterates `edges`, skips self-edges and non-`Node` endpoints, skips edges whose endpoints are absent from `rects`, produces one `Route` per remaining edge; deterministic.
  - `fn key_of(id: &BoxId) -> Option<String>` — `BoxId::Node(k) => Some(k.clone())`, else `None`.
  - `fn fallback_l(src: Rect, tgt: Rect) -> Vec<P>` — center-to-center L used only if A* finds no path (keeps output total).

- [ ] **Step 1: Write the failing integration tests**

Add to the `tests` module in `route.rs`. Note the extra `use` of IR types the fixtures build:

```rust
    use crate::solve::{BoxKind, FlagSet, SolveConfig};
    use crate::syntax::{Axis, Margin, Shape};

    fn nrect(x: f64, y: f64, w: f64, h: f64) -> Rect { Rect { x, y, w, h } }

    fn leafbox(k: &str) -> Box {
        Box {
            id: BoxId::Node(k.into()),
            kind: BoxKind::Leaf,
            children: vec![],
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: 0,
        }
    }

    #[test]
    fn route_two_clear_boxes_is_straight_segment() {
        let boxes = vec![leafbox("a"), leafbox("b")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(300.0, 0.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "a");
        assert_eq!(out[0].target, "b");
        assert_eq!(out[0].points.len(), 2, "clear LOS => straight: {:?}", out[0].points);
    }

    #[test]
    fn route_detours_around_third_box() {
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert!(out[0].points.len() >= 4, "detour: {:?}", out[0].points);
        let inf = inflate(nrect(150.0, -30.0, 80.0, 120.0), ROUTE_MARGIN);
        for &(x, y) in &out[0].points {
            assert!(!strictly_inside(&inf, x, y));
        }
    }

    #[test]
    fn route_skips_self_edges_and_unknown_endpoints() {
        let boxes = vec![leafbox("a")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        let edges = vec![
            (BoxId::Node("a".into()), BoxId::Node("a".into())),      // self
            (BoxId::Node("a".into()), BoxId::Node("ghost".into())),  // unknown target
        ];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert!(out.is_empty(), "self + unknown edges produce no routes: {out:?}");
    }

    #[test]
    fn route_is_deterministic() {
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
        let edges = vec![
            (BoxId::Node("a".into()), BoxId::Node("b".into())),
            (BoxId::Node("a".into()), BoxId::Node("b".into())), // parallel
        ];
        let a = route(&boxes, &rects, &edges, &SolveConfig::default());
        let b = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(a, b, "identical input => identical routes");
        assert_ne!(a[0].points, a[1].points, "parallels separated");
        // silence unused import warning in this fixture-heavy module:
        let _ = Axis::Row;
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests::route_`
Expected: FAIL — `route` still returns empty (`out.len()` == 0).

- [ ] **Step 3: Implement the full `route()` body**

Replace the Task 1 stub:

```rust
fn key_of(id: &BoxId) -> Option<String> {
    match id {
        BoxId::Node(k) => Some(k.clone()),
        _ => None,
    }
}

fn fallback_l(src: Rect, tgt: Rect) -> Vec<P> {
    let s = (src.x + src.w / 2.0, src.y + src.h / 2.0);
    let t = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
    simplify(vec![s, (t.0, s.1), t])
}

pub(super) fn route(
    _boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    let mut routes: Vec<Route> = Vec::new();
    for (s, t) in edges {
        if s == t {
            continue; // self-edge: out of scope
        }
        let (Some(source), Some(target)) = (key_of(s), key_of(t)) else {
            continue; // group-as-endpoint: out of scope
        };
        let (Some(&src), Some(&tgt)) = (rects.get(s), rects.get(t)) else {
            continue; // endpoint not in this diagram
        };
        let obstacles = leaf_obstacles(rects, &[s.clone(), t.clone()]);
        let (ovg, srcv, tgtv) = build_ovg(&obstacles, src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let points = astar(&ovg, &srcv, &tgtv, goal).unwrap_or_else(|| fallback_l(src, tgt));
        routes.push(Route { points, source, target });
    }
    hub_spread(&mut routes, rects);
    nudge(&mut routes);
    routes
}
```

- [ ] **Step 4: Run to verify the integration tests pass**

Run: `cargo test -p waml --lib solve::route`
Expected: PASS (all route unit + integration tests).
Run: `cargo test -p waml`
Expected: PASS (whole crate still green; golden unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): wire OVG+A*+hub+nudge into route() over leaf obstacles"
```

---

### Task 7: Containment-aware group obstacle rule (exact membership via child lists)

A group rect becomes an obstacle for an edge **only when both endpoints are non-members** of that group (membership = transitive child-list closure, never rect overlap). Adds group rects to the obstacle set per edge accordingly.

**Files:**
- Modify: `crates/waml/src/solve/route.rs`

**Interfaces:**
- Consumes: `route()` (Task 6), `Box`, `BoxId`, `leaf_obstacles`, `build_ovg`.
- Produces:
  - `struct Membership { members: BTreeMap<BoxId, BTreeSet<BoxId>> }` — for each `BoxId::Group`, the transitive set of descendant leaf `BoxId`s.
  - `fn build_membership(boxes: &[Box]) -> Membership`
  - `impl Membership { fn is_member(&self, group: &BoxId, leaf: &BoxId) -> bool }`
  - `fn group_obstacles(rects, membership, s, t) -> Vec<Obstacle>` — each `BoxId::Group` rect included only when NEITHER `s` nor `t` is a member.
  - `route()` extended to build membership once and append `group_obstacles(..)` to the per-edge obstacle set (and `_boxes` renamed `boxes`).

- [ ] **Step 1: Write the failing tests**

```rust
    fn groupbox(id: u32, children: Vec<BoxId>) -> Box {
        Box {
            id: BoxId::Group(id),
            kind: BoxKind::Group,
            children,
            axis: Some(Axis::Column),
            shape: Shape::Frame,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: Some("G".into()),
            depth: 0,
        }
    }

    #[test]
    fn membership_is_transitive_via_child_lists() {
        let boxes = vec![
            leafbox("a"),
            groupbox(1, vec![BoxId::Node("a".into())]),
            groupbox(0, vec![BoxId::Group(1)]),
        ];
        let m = build_membership(&boxes);
        assert!(m.is_member(&BoxId::Group(0), &BoxId::Node("a".into())));
        assert!(m.is_member(&BoxId::Group(1), &BoxId::Node("a".into())));
        assert!(!m.is_member(&BoxId::Group(0), &BoxId::Node("b".into())));
    }

    #[test]
    fn member_edge_crosses_group_frame_freely() {
        // "a" inside g0, "b" outside; the group is transparent to a->b.
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            groupbox(0, vec![BoxId::Node("a".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(20.0, 20.0, 100.0, 60.0));
        rects.insert(BoxId::Group(0), nrect(0.0, 0.0, 140.0, 100.0));
        rects.insert(BoxId::Node("b".into()), nrect(300.0, 20.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].points.len(), 2, "member edge is straight: {:?}", out[0].points);
    }

    #[test]
    fn non_member_edge_detours_around_group() {
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("x"),
            groupbox(0, vec![BoxId::Node("x".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(400.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("x".into()), nrect(200.0, -10.0, 80.0, 40.0));
        rects.insert(BoxId::Group(0), nrect(180.0, -40.0, 120.0, 140.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert!(out[0].points.len() >= 4, "non-member edge detours: {:?}", out[0].points);
        let inf = inflate(nrect(180.0, -40.0, 120.0, 140.0), ROUTE_MARGIN);
        for &(px, py) in &out[0].points {
            assert!(!strictly_inside(&inf, px, py), "pierces group at ({px},{py})");
        }
    }

    #[test]
    fn membership_by_child_list_not_rect_overlap() {
        // "a"'s rect sits INSIDE g0's rect but "a" is NOT a child of g0, so
        // membership-by-child-list must keep g0 a solid obstacle for the a->b
        // edge. Asserted directly on membership + group_obstacles (NOT on a
        // route point count): a non-member endpoint whose rect is deep inside a
        // group's rect is geometrically landlocked, so route() correctly falls
        // back to a straight segment — the invariant under test is *containment
        // decided by child list, never rect overlap*, which is what we check.
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("x"),
            groupbox(0, vec![BoxId::Node("x".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Group(0), nrect(0.0, 0.0, 260.0, 200.0));
        rects.insert(BoxId::Node("x".into()), nrect(10.0, 10.0, 60.0, 40.0));
        rects.insert(BoxId::Node("a".into()), nrect(90.0, 80.0, 60.0, 40.0)); // rect inside g0
        rects.insert(BoxId::Node("b".into()), nrect(500.0, 80.0, 60.0, 40.0));
        let membership = build_membership(&boxes);
        // Rect overlap does NOT make "a" a member of g0.
        assert!(!membership.is_member(&BoxId::Group(0), &BoxId::Node("a".into())));
        // Therefore g0 stays an obstacle for the a->b edge (child list decides).
        let obs = group_obstacles(
            &rects,
            &membership,
            &BoxId::Node("a".into()),
            &BoxId::Node("b".into()),
        );
        assert!(
            obs.iter().any(|o| o.id == BoxId::Group(0)),
            "g0 must remain an obstacle: membership is by child list, not rect overlap"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml --lib solve::route::tests::membership`
Expected: FAIL — `build_membership` / `Membership` / `group_obstacles` not defined; group tests fail (groups not yet obstacles).

- [ ] **Step 3: Implement membership + group obstacles and extend `route()`**

Add to `route.rs` (extend the module `use` to add `std::collections::BTreeSet`):

```rust
use std::collections::BTreeSet;

struct Membership {
    members: BTreeMap<BoxId, BTreeSet<BoxId>>,
}

impl Membership {
    fn is_member(&self, group: &BoxId, leaf: &BoxId) -> bool {
        self.members.get(group).is_some_and(|s| s.contains(leaf))
    }
}

fn build_membership(boxes: &[Box]) -> Membership {
    let by_id: BTreeMap<BoxId, &Box> = boxes.iter().map(|b| (b.id.clone(), b)).collect();
    fn leaves(id: &BoxId, by_id: &BTreeMap<BoxId, &Box>, out: &mut BTreeSet<BoxId>) {
        let Some(b) = by_id.get(id) else { return };
        for c in &b.children {
            if matches!(c, BoxId::Node(_)) {
                out.insert(c.clone());
            }
            leaves(c, by_id, out);
        }
    }
    let mut members = BTreeMap::new();
    for b in boxes {
        if matches!(b.id, BoxId::Group(_)) {
            let mut set = BTreeSet::new();
            leaves(&b.id, &by_id, &mut set);
            members.insert(b.id.clone(), set);
        }
    }
    Membership { members }
}

/// Group rects that block THIS edge: a group is an obstacle only when neither
/// endpoint is one of its (transitive) members.
fn group_obstacles(
    rects: &BTreeMap<BoxId, Rect>,
    membership: &Membership,
    s: &BoxId,
    t: &BoxId,
) -> Vec<Obstacle> {
    let mut out: Vec<Obstacle> = rects
        .iter()
        .filter(|(id, _)| matches!(id, BoxId::Group(_)))
        .filter(|(id, _)| !membership.is_member(id, s) && !membership.is_member(id, t))
        .map(|(id, r)| Obstacle { id: id.clone(), rect: *r })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
```

Extend `route()` (rename `_boxes` to `boxes`; build membership once; append + re-sort obstacles):

```rust
pub(super) fn route(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    let membership = build_membership(boxes);
    let mut routes: Vec<Route> = Vec::new();
    for (s, t) in edges {
        if s == t { continue; }
        let (Some(source), Some(target)) = (key_of(s), key_of(t)) else { continue; };
        let (Some(&src), Some(&tgt)) = (rects.get(s), rects.get(t)) else { continue; };
        let mut obstacles = leaf_obstacles(rects, &[s.clone(), t.clone()]);
        obstacles.extend(group_obstacles(rects, &membership, s, t));
        obstacles.sort_by(|a, b| a.id.cmp(&b.id)); // deterministic order
        let (ovg, srcv, tgtv) = build_ovg(&obstacles, src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let points = astar(&ovg, &srcv, &tgtv, goal).unwrap_or_else(|| fallback_l(src, tgt));
        routes.push(Route { points, source, target });
    }
    hub_spread(&mut routes, rects);
    nudge(&mut routes);
    routes
}
```

- [ ] **Step 4: Run to verify the group tests pass**

Run: `cargo test -p waml --lib solve::route`
Expected: PASS (membership + all four group tests + earlier route tests).
Run: `cargo test -p waml`
Expected: PASS (whole crate).

- [ ] **Step 5: Full workspace gate**

Run: `cargo test --workspace`
Expected: PASS.
Run: `cargo build -p waml-wasm -p waml-editor`
Expected: builds clean.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/solve/route.rs
git commit -m "feat(route): containment-aware group obstacle rule via child-list membership"
```

---

## Self-Review Notes

- **Spec coverage:** OVG (Task 2), A* + bend penalty + straight degenerate (Task 3), nudging (Task 4), hub spreading (Task 5), one public call returning `Solved.routes` (Tasks 1 + 6), containment-aware group rule + all five group/membership test cases (Task 7), determinism test (Task 6), no new deps (Global Constraints), `Route` in wire module with derives + node-key identity + no `RelationshipKind` (Task 1). All eight spec Testing bullets map to tasks.
- **Type consistency:** `route(boxes, rects, edges, cfg)`, `build_ovg`, `astar(.., goal)`, `nudge`, `hub_spread(routes, rects)`, `build_membership`/`group_obstacles`, `Route { points, source, target }` used identically across tasks.
- **Out of scope honored:** self-edges skipped, group-as-endpoint skipped, no splines, no web-frontend route consumption (callers only SUPPLY edges).

## Known Limitations (accepted for v1)

- **Per-edge OVG rebuild = quadratic-ish scaling.** `route()` rebuilds the whole
  visibility graph (all obstacles) for every edge, so cost grows roughly with
  `edges × vertices²`. Fine for moderate diagrams; on `stress.rs`-scale graphs
  (hundreds of nodes × many edges) it will drag. The spec sets no performance
  target, so this is an accepted limitation, not a defect — revisit (shared/
  incremental OVG, obstacle culling per edge) only if stress-scale diagrams route
  slowly.

## Open Ambiguities Surfaced (do NOT change the design — flagged for the reviewer)

1. **Edge source into `solve_diagram`:** `model::Diagram` carries no edges (relationships live in `Model.edges`), so `solve_diagram` MUST gain an `edges: &[(BoxId, BoxId)]` parameter and callers must supply them. The spec specifies `route()`'s edge input but not how the top-level entry obtains it. Chosen: signature change + callers map all `model.edges` to `BoxId::Node` pairs (route skips unknown/self endpoints). This lightly touches `waml-wasm` and `waml-editor` call sites — input plumbing only, not route consumption.
2. **Group rects lacked `BoxId` keys:** the current geometry pass keeps group frames in a separate `Vec<SolvedGroup>` with no `BoxId`. The spec's `rects` map "covers every box (leaf nodes AND group frames)", so Task 1 adds group rects to the internal `BoxId`-keyed map (behavior-preserving; `Solved.nodes` unchanged).
3. **Unspecified numeric constants:** routing margin (`ROUTE_MARGIN`), bend penalty (`BEND_PENALTY`), and nudge gap (`NUDGE_GAP`) have no values in the spec. Chosen as module consts (12.0 / 20.0 / 8.0), tunable; they do not affect correctness of the test assertions (which check topology, orthogonality, distinctness, determinism — not exact pixel positions).
4. **`pretty()` not extended:** routes are not dumped by `pretty()`, keeping `solver_golden` stable. The spec's testing section asserts on router geometry via dedicated unit tests, not the golden dump.
