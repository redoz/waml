# Edge Route Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the orthogonal router's `Solved.routes` into the makepad canvas so edges draw as obstacle-aware orthogonal polylines instead of straight center-to-center segments.

**Architecture:** Three independent seams. (1) `stress_default` in `scene.rs` calls the router so layout-less diagrams populate `solved.routes` (this requires exposing `route::route` from the `waml` crate). (2) `SceneEdge` gains a `points` polyline; `build_scene` matches each drawable edge to its `Route` by consuming `solved.routes` in order, with a straight-line defensive fallback. (3) `canvas.rs` strokes `edge.points` per segment via `draw_edge_down`, and the now-dead `border_point` helper and `draw_edge_up` pen are removed.

**Tech Stack:** Rust, makepad (fork on branch `waml-svg-stroked-bounds`), `cargo test` / `cargo clippy`.

## Global Constraints

- **Clippy `-D warnings` gate promotes `dead_code` to a hard build error.** Any orphaned helper/field/pen left after a deletion fails the build. Every deletion task must end with a clean `cargo clippy ... -- -D warnings`.
- **Orthogonal only.** Straight + orthogonal segments, no splines. Matches the router.
- **Native-only seam.** Routing lives behind the existing native `stress_default` / `solve_diagram` call sites. The wasm/web path (dagre + its own edge rendering) is untouched.
- **No edge ever renders nothing.** `route::route` already emits a straight `fallback_l` L-polyline for any edge A* cannot solve, so "route everywhere" never yields an empty polyline.

**Out of scope — do NOT plan or implement:**
- Arrowheads and `RelationshipKind`-specific adornment styling (fast-follow). `SceneEdge` keeps carrying `kind`, unstyled, exactly as today.
- Web/wasm edge rendering (keeps dagre + existing web path).
- Route caching / incremental re-route on pan/zoom (routes are world-space, camera-independent).

---

### Task 1: Expose `route::route` and populate `stress_default` routes

The router entry `route()` is currently `pub(super)` inside a private `mod route;` in the `waml` crate, so `waml-editor` cannot call it. Widen its visibility, then make `stress_default` build its edge list + rects map and call it, replacing `routes: Vec::new()`.

**Files:**
- Modify: `crates/waml/src/solve/mod.rs:11` (`mod route;` → `pub mod route;`)
- Modify: `crates/waml/src/solve/route.rs:21` (`pub(super) fn route` → `pub fn route`)
- Modify: `crates/waml-editor/src/scene.rs:6-8` (add `route` to the `waml::solve` import)
- Modify: `crates/waml-editor/src/scene.rs:144-185` (`stress_default`)
- Test: `crates/waml-editor/src/scene.rs` (`mod tests`)

**Interfaces:**
- Consumes: `waml::solve::route::route(boxes: &[waml::solve::Box], rects: &BTreeMap<BoxId, Rect>, edges: &[(BoxId, BoxId)], cfg: &SolveConfig) -> Vec<Route>`. `boxes` is used ONLY for group membership; obstacles derive from `rects`. `Route { points: Vec<(f64,f64)>, source: String, target: String }`.
- Produces: `stress_default(model, sizes) -> Solved` with `Solved.routes` populated (one `Route` per non-self model edge whose endpoints are both in the laid-out set, in `model.edges` order).

- [ ] **Step 1: Widen router visibility in the `waml` crate**

In `crates/waml/src/solve/mod.rs` change line 11:

```rust
pub mod route;
```

In `crates/waml/src/solve/route.rs` change the `route` fn signature (line 21) from `pub(super) fn route(` to:

```rust
/// Route every leaf-to-leaf edge as an orthogonal polyline avoiding obstacles.
pub fn route(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
```

- [ ] **Step 2: Confirm the `waml` crate still builds**

Run: `cargo build -p waml`
Expected: builds clean (widening visibility is non-breaking; `solve_diagram` still calls `route::route`).

- [ ] **Step 3: Import the `route` module in `scene.rs`**

Change the `use waml::solve::{...}` block (currently lines 6-8) to add `route`:

```rust
use waml::solve::{
    route, solve_diagram, stress, BoxId, Rect, Size, SizeMap, SolveConfig, Solved, SolvedGroup,
};
```

- [ ] **Step 4: Build the edge list + rects map and call the router in `stress_default`**

Replace the body of `stress_default` (lines 144-185) so it builds a `BTreeMap<BoxId, Rect>` and a `Vec<(BoxId, BoxId)>` edge list (mirroring `build_scene`'s drawable-edge construction: node-node pairs, self-edges dropped) and fills `routes` via `route::route` with an empty `boxes` slice:

```rust
fn stress_default(model: &Model, sizes: &SizeMap) -> Solved {
    use std::collections::{BTreeMap, BTreeSet};

    let keys: Vec<String> = sizes.keys().cloned().collect();
    let ids: Vec<BoxId> = keys.iter().cloned().map(BoxId::Node).collect();
    let dims: Vec<Size> = keys.iter().map(|k| sizes[k]).collect();
    let index: BTreeMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    // Undirected edge index pairs among members; drop self-loops and duplicates.
    let mut seen = BTreeSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for e in &model.edges {
        let (Some(&a), Some(&b)) = (index.get(e.source.as_str()), index.get(e.target.as_str()))
        else {
            continue;
        };
        if a == b {
            continue;
        }
        if seen.insert((a.min(b), a.max(b))) {
            pairs.push((a, b));
        }
    }

    let cfg = stress::StressConfig::default();
    let rects = if pairs.is_empty() {
        stress::grid_pack(&ids, &dims, &cfg)
    } else {
        stress::layout(&ids, &dims, &pairs, &cfg)
    };

    // Rects keyed by BoxId for the router (obstacles derive from these rects).
    let rect_map: BTreeMap<BoxId, Rect> = ids.iter().cloned().zip(rects.iter().copied()).collect();

    // Directed (BoxId, BoxId) edge list in model.edges order, self-edges dropped
    // — same construction build_scene uses, so routes come out in the order
    // build_scene consumes them. route::route presence-filters internally.
    let route_edges: Vec<(BoxId, BoxId)> = model
        .edges
        .iter()
        .filter(|e| e.source != e.target)
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();

    // Empty boxes slice: the stress layout is group-less, so build_membership(&[])
    // yields no groups and routing degrades to pure leaf-obstacle avoidance.
    let routes = route::route(&[], &rect_map, &route_edges, &SolveConfig::default());

    Solved {
        nodes: keys.into_iter().zip(rects).collect(),
        groups: Vec::new(),
        flags: BTreeMap::new(),
        routes,
    }
}
```

- [ ] **Step 5: Write the failing test for `stress_default` routes**

Add to `crates/waml-editor/src/scene.rs`'s `mod tests`:

```rust
    #[test]
    fn stress_default_populates_routes() {
        let model = mini();
        // stress_default is layout-agnostic (it reads model + sizes, not the
        // diagram's layout block), so any sized diagram exercises it.
        let sizes = crate::sizing::size_map(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let solved = stress_default(&model, &sizes);
        // mini declares one associates edge order -> customer.
        assert_eq!(solved.routes.len(), 1);
        assert!(!solved.routes[0].points.is_empty());
        let r = &solved.routes[0];
        assert!(
            (r.source == "order" && r.target == "customer")
                || (r.source == "customer" && r.target == "order"),
            "unexpected route endpoints: {} -> {}",
            r.source,
            r.target
        );
    }
```

- [ ] **Step 6: Run the test and the crate build**

Run: `cargo test -p waml-editor scene`
Expected: `stress_default_populates_routes` PASSES; all existing `scene` tests still pass.

- [ ] **Step 7: Commit**

```bash
git add crates/waml/src/solve/mod.rs crates/waml/src/solve/route.rs crates/waml-editor/src/scene.rs
git commit -m "feat(scene): stress_default populates solved.routes via router"
```

---

### Task 2: `SceneEdge` carries the routed polyline

Add `points` to `SceneEdge` and wire `build_scene` to match each drawable edge to its `Route` by consuming `solved.routes` in order (parallel edges make key-only lookup ambiguous), falling back to a straight center-to-center polyline on a key mismatch so the cursor stays aligned.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs:63-68` (`SceneEdge` struct)
- Modify: `crates/waml-editor/src/scene.rs:245-257` (drawable-edge loop in `build_scene`)
- Test: `crates/waml-editor/src/scene.rs` (`mod tests`) — update one test, add two

**Interfaces:**
- Consumes: `Solved.routes: Vec<Route>` from Task 1 / `solve_diagram`. `Route { points: Vec<(f64,f64)>, source: String, target: String }`.
- Produces: `SceneEdge { source: Rect, target: Rect, kind: RelationshipKind, points: Vec<(f64, f64)> }`. `points` is always non-empty (≥2 points).

- [ ] **Step 1: Add the `points` field to `SceneEdge`**

Replace the struct (lines 63-68):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
    /// Routed orthogonal polyline in world coordinates; the renderer strokes it
    /// segment-by-segment. Always non-empty (router emits ≥2 points; a defensive
    /// straight [source-center, target-center] fallback is used on route
    /// mismatch).
    pub points: Vec<(f64, f64)>,
}
```

- [ ] **Step 2: Match edges to routes in order in `build_scene`**

Replace the drawable-edge loop (lines 245-257):

```rust
    // Only edges whose endpoints both appear in the solved layout are drawable.
    // Match each to its Route by consuming solved.routes IN ORDER — route::route
    // emits one Route per drawable edge in the same order build_scene filters
    // model.edges, and key-only lookup is ambiguous for parallel edges. On a key
    // mismatch (e.g. a drawable self-edge the router skipped, desyncing the
    // stream) fall back to a straight center-to-center polyline WITHOUT advancing
    // the cursor, so later edges stay aligned.
    let mut edges = Vec::new();
    let mut route_cursor = 0usize;
    for e in &model.edges {
        if let (Some(&source), Some(&target)) =
            (solved.nodes.get(&e.source), solved.nodes.get(&e.target))
        {
            let points = match solved.routes.get(route_cursor) {
                Some(r) if r.source == e.source && r.target == e.target => {
                    route_cursor += 1;
                    r.points.clone()
                }
                _ => {
                    let sc = (source.x + source.w / 2.0, source.y + source.h / 2.0);
                    let tc = (target.x + target.w / 2.0, target.y + target.h / 2.0);
                    vec![sc, tc]
                }
            };
            edges.push(SceneEdge {
                source,
                target,
                kind: e.kind,
                points,
            });
        }
    }
```

- [ ] **Step 3: Update the existing endpoint test for the new field**

In `scene_edge_endpoints_match_node_rects` (currently lines 478-495) keep the `source`/`target`/`kind` assertions and add a non-empty `points` assertion. The full updated test:

```rust
    #[test]
    fn scene_edge_endpoints_match_node_rects() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        assert_eq!(scene.edges.len(), 1);
        let edge = &scene.edges[0];
        assert_eq!(edge.kind, RelationshipKind::Associates);
        assert!(!edge.points.is_empty(), "routed edge must carry a polyline");

        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // The associates edge runs order -> customer (see fixture order.md).
        assert_eq!(edge.source, order.rect);
        assert_eq!(edge.target, customer.rect);
    }
```

- [ ] **Step 4: Add a test that routed points are anchored near node borders**

The mini fixture has an authored `## Layout`, so this exercises the `solve_diagram` route path. Add to `mod tests`:

```rust
    #[test]
    fn routed_edge_points_anchor_near_node_borders() {
        // A point is "at" a rect when it lies within `tol` of the rect's bounds;
        // router endpoints attach to box-perimeter ports, so both ends land on
        // (or within a route-margin of) their node.
        fn near_rect(p: (f64, f64), r: Rect, tol: f64) -> bool {
            p.0 >= r.x - tol && p.0 <= r.x + r.w + tol && p.1 >= r.y - tol && p.1 <= r.y + r.h + tol
        }

        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let edge = &scene.edges[0];
        assert!(edge.points.len() >= 2, "polyline needs both endpoints");

        // edge.source is order's rect, edge.target is customer's rect.
        let first = *edge.points.first().unwrap();
        let last = *edge.points.last().unwrap();
        assert!(
            near_rect(first, edge.source, 12.0),
            "first point {first:?} not anchored to source {:?}",
            edge.source
        );
        assert!(
            near_rect(last, edge.target, 12.0),
            "last point {last:?} not anchored to target {:?}",
            edge.target
        );
    }
```

- [ ] **Step 5: Add a test that the stress-default path yields non-empty edge points**

Force the layout-less path by clearing the diagram's `layout` (public `Vec<LayoutStatement>` field). Add to `mod tests`:

```rust
    #[test]
    fn stress_default_scene_edges_carry_points() {
        let model = mini();
        // Clearing `layout` routes build_scene through stress_default (see
        // use_stress_default: layout.is_empty()).
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        assert!(super::use_stress_default(&diagram), "expected stress path");

        let (scene, _) =
            build_scene(&model, &diagram, &std::collections::HashSet::new());
        assert_eq!(scene.edges.len(), 1, "mini has one drawable edge");
        assert!(
            !scene.edges[0].points.is_empty(),
            "stress-default edges must carry a routed polyline"
        );
    }
```

- [ ] **Step 6: Run the scene tests**

Run: `cargo test -p waml-editor scene`
Expected: `scene_edge_endpoints_match_node_rects`, `routed_edge_points_anchor_near_node_borders`, and `stress_default_scene_edges_carry_points` PASS; all other `scene` tests still pass.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/scene.rs
git commit -m "feat(scene): SceneEdge carries routed polyline points"
```

---

### Task 3: Stroke the polyline in the canvas; delete `border_point` and `draw_edge_up`

Replace the single center-to-center segment draw with a loop over consecutive `edge.points` pairs, each stroked as its own axis-aligned `EdgeLine` quad via `draw_edge_down`. Delete the now-unused `border_point` helper (and its two tests) and the `draw_edge_up` pen (field, DSL, uniform pushes, pen-selection) — the clippy `-D warnings` gate turns any orphan into a hard build error.

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs:516-554` (edge draw loop)
- Modify: `crates/waml-editor/src/canvas.rs:507-515` (edge-pen uniform pushes)
- Delete: `crates/waml-editor/src/canvas.rs:209-237` (`border_point` fn)
- Delete: `crates/waml-editor/src/canvas.rs:746-784` (`border_point_*` tests)
- Modify: `crates/waml-editor/src/canvas.rs:60-61` and `:146-148` (`draw_edge_up` DSL + field)
- Modify: `crates/waml-editor/src/canvas.rs:20-29` (stale two-pen EdgeLine doc comment)

**Interfaces:**
- Consumes: `SceneEdge.points: Vec<(f64, f64)>` from Task 2.
- Produces: no new public API. Removes the `border_point` fn and the `draw_edge_up` field.

- [ ] **Step 1: Replace the edge draw loop with a per-segment polyline stroke**

Replace lines 517-554 (the comment + `for edge in &self.scene.edges { ... }` block) with:

```rust
        // Edges: stroke each consecutive point pair of the routed orthogonal
        // polyline as its own axis-aligned EdgeLine quad. Orthogonal segments
        // are axis-aligned, so a single down-diagonal pen renders them all; the
        // thickness-inflated bounding-box quad hides the (zero) slope. Arrow/
        // adornment styling is a fast-follow.
        let thickness = 2.0 * zoom;
        for edge in &self.scene.edges {
            for pair in edge.points.windows(2) {
                let (a0, a1) = self.camera.world_to_local(pair[0].0, pair[0].1);
                let (b0, b1) = self.camera.world_to_local(pair[1].0, pair[1].1);
                let a = dvec2(rect.pos.x + a0, rect.pos.y + a1);
                let b = dvec2(rect.pos.x + b0, rect.pos.y + b1);
                let min = dvec2(a.x.min(b.x), a.y.min(b.y));
                let max = dvec2(a.x.max(b.x), a.y.max(b.y));
                let seg = Rect {
                    pos: min,
                    size: dvec2(
                        (max.x - min.x).max(thickness),
                        (max.y - min.y).max(thickness),
                    ),
                };
                self.draw_edge_down.draw_abs(cx, seg);
            }
        }
```

- [ ] **Step 2: Drop the `draw_edge_up` uniform pushes**

Replace the edge-pen uniform block (lines 507-515) with the `draw_edge_down`-only version:

```rust
        // Edge pen: feed zoom so the stroke thickens with the box, and bake the
        // down-diagonal direction (per-instance uniforms batch-collapse on this
        // fork). Every routed segment is axis-aligned, so one pen suffices.
        self.draw_edge_down.set_uniform(cx, live_id!(flip), &[0.0]);
        self.draw_edge_down
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);
```

- [ ] **Step 3: Delete the `border_point` helper**

Delete the entire `border_point` fn and its doc comment (lines 209-237):

```rust
/// Intersection of the center-to-center line from `from` to `to` with `from`'s
/// border, in world coordinates. Operates on `waml::solve::Rect` ...
fn border_point(from: waml::solve::Rect, to: waml::solve::Rect) -> (f64, f64) {
    ...
}
```

- [ ] **Step 4: Delete the `border_point` tests**

In `mod tests`, delete `border_point_exits_on_the_side_facing_the_target` and `border_point_handles_vertical_stack` (lines 746-784). Leave `node_at_hits_the_topmost_node_under_the_point` and the `use waml::solve::Rect as WorldRect;` import (still used by the remaining `node_at` tests).

- [ ] **Step 5: Delete the `draw_edge_up` field and DSL entry**

Remove the DSL line (line 61):

```rust
        draw_edge_up: mod.draw.EdgeLine{ color: atlas.text_dim }
```

Remove the struct field with its attributes (lines 146-148):

```rust
    #[redraw]
    #[live]
    draw_edge_up: DrawColor,
```

- [ ] **Step 6: Update the stale EdgeLine doc comment**

The comment at lines 20-29 describes "two pens" routed by slope sign. Trim it to reflect the single orthogonal pen (keep the rationale for stroking a line vs. filling the AABB):

```rust
    // Edge pen: stroke the segment as an actual line, NOT a filled rect. The
    // segment's axis-aligned bounding box has the two endpoints at opposite
    // corners, so the edge is one of the AABB's two diagonals; `flip` selects
    // which (0 = top-left->bottom-right). Routed segments are axis-aligned, so
    // the single `draw_edge_down` pen (flip = 0) draws them all; the
    // thickness-inflated quad hides the zero slope. Filling the whole AABB (the
    // old `draw_edge: DrawColor`) painted a solid grey blob for diagonal edges.
```

- [ ] **Step 7: Build the editor crate**

Run: `cargo build -p waml-editor`
Expected: builds clean — no "cannot find function `border_point`" / "no field `draw_edge_up`" errors.

- [ ] **Step 8: Run the dead-code / clippy gate**

Run: `cargo clippy -p waml-editor --all-targets -- -D warnings`
Expected: no warnings — confirms `border_point` and `draw_edge_up` left no `dead_code` orphans (the gate would hard-fail otherwise).

- [ ] **Step 9: Run the editor tests**

Run: `cargo test -p waml-editor`
Expected: all tests pass (the deleted `border_point_*` tests are gone; `node_at` / `scene` tests still pass).

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/canvas.rs
git commit -m "feat(canvas): stroke routed edge polylines; drop border_point + draw_edge_up"
```

---

## Self-Review

**Spec coverage:**
- Seam 1 (`stress_default` produces routes, empty `boxes` slice) → Task 1. ✓
- Seam 2 (`SceneEdge.points`, ordered route consumption + straight fallback) → Task 2. ✓
- Seam 3 (per-segment stroke via `draw_edge_down`, delete `border_point`, remove `draw_edge_up` + uniforms) → Task 3. ✓
- Router visibility prerequisite (`pub(super)` → `pub`, `mod` → `pub mod`) folded into Task 1 (its deliverable needs it). ✓
- Tests: updated `scene_edge_endpoints_match_node_rects` (Task 2 Step 3); routed points non-empty + border-anchored (Task 2 Step 4); stress-default scene non-empty points (Task 2 Step 5); `stress_default` routes non-empty (Task 1 Step 5). ✓
- Out-of-scope items (arrowheads, web/wasm, route caching) stated in Global Constraints, not planned. ✓

**Type consistency:** `route::route(&[], &rect_map, &route_edges, &SolveConfig::default())` matches `pub fn route(boxes: &[Box], rects: &BTreeMap<BoxId, Rect>, edges: &[(BoxId, BoxId)], _cfg: &SolveConfig)`. `SceneEdge.points: Vec<(f64, f64)>` matches `Route.points`. `route.source`/`route.target` are `String`, compared against `e.source`/`e.target` (`String`). `draw_edge_down` field/DSL name consistent across canvas edits.

**Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to Task N" — every code step carries full code.
