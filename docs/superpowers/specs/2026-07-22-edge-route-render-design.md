# Edge Route Rendering — wire `solved.routes` into the makepad renderer

## Problem

The orthogonal edge router (`crates/waml/src/solve/route.rs`) is fully
implemented and produces `Solved.routes: Vec<Route>` — one orthogonal polyline
per drawable edge. Nothing consumes it. The makepad canvas
(`crates/waml-editor/src/canvas.rs`) still draws every edge as a single straight
center-to-center segment clipped to node borders (`border_point` + the
`EdgeLine` pen), ignoring obstacles entirely.

This spec wires the routed polylines into the renderer so edges draw as the
router intends: orthogonal, obstacle-aware paths.

## Decisions

- **Routes are authoritative on every layout path.** Both the constraint solver
  (`solve_diagram`) and the native stress/grid default (`stress_default` in
  `scene.rs`) must populate `solved.routes`. `solve_diagram` already does;
  `stress_default` currently returns `routes: Vec::new()` and must be fixed to
  call `route::route(...)` over its own solved rects and edge pairs. The
  straight-line renderer path is removed, not kept as a fallback.
- **No edge ever renders nothing.** `route::route` already emits a `fallback_l`
  straight-L polyline for any edge A* cannot solve (e.g. a landlocked endpoint),
  so "route everywhere" cannot produce an empty edge — the worst case is a
  straight L, which is exactly today's look.
- **Orthogonal only.** Consistent with the router: straight + orthogonal
  segments, no splines.
- **Arrowheads / `RelationshipKind` adornments are out of scope.** `SceneEdge`
  keeps carrying `kind`, unstyled, exactly as today. Arrowheads remain a
  fast-follow.

## Architecture

Three seams change, each independently:

### 1. Solver seam — `stress_default` produces routes (`scene.rs`)

`stress_default(model, sizes) -> Solved` currently sets `routes: Vec::new()`.
Change it to build the same `Vec<(BoxId, BoxId)>` edge list that `build_scene`
already builds (node-node pairs among the laid-out members, self-edges dropped)
and call `route::route(&boxes, &rects, &edges, &cfg)` to fill `routes`.

- The stress path has no `Box` forest (it lays out a flat node set with no
  groups). `route::route` uses `boxes` **only** to build group membership;
  `leaf_obstacles` derives its obstacles from `rects`. So pass an empty `&[]`
  boxes slice — `build_membership(&[])` yields no groups, `group_obstacles`
  contributes nothing, and routing degrades to pure leaf-obstacle avoidance
  among the nodes. Correct for a group-less layout; no fabricated `Box` list
  needed.
- `rects` is the `BTreeMap<BoxId, Rect>` the stress solve already produces
  (keyed by `BoxId::Node`).

This keeps routing behind the existing native-only call seam; the wasm/web path
is untouched (web keeps dagre and its own edge rendering).

### 2. Scene seam — `SceneEdge` carries the polyline (`scene.rs`)

Add one field:

```rust
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
    pub points: Vec<(f64, f64)>, // routed orthogonal polyline, world coords
}
```

`source`/`target` rects stay (used for the scene bounding box and future
hit-testing). `points` is the world-space polyline the renderer strokes.

In `build_scene`, the drawable-edge loop (currently scene.rs:246-257) matches
each edge to its `Route`. **Match by consuming `solved.routes` in order**, not by
`(source, target)` key lookup: `route::route` emits exactly one `Route` per
node-node edge whose endpoints are both present, in the same order `build_scene`
filters `model.edges`; key-only lookup is ambiguous for parallel edges (same
source/target pair). A defensive fallback: if the ordered route's
`source`/`target` keys don't match the edge, fall back to a straight
`[source-center, target-center]` two-point polyline so a mismatch degrades
gracefully rather than mis-assigning a path.

### 3. Renderer seam — stroke the polyline (`canvas.rs`)

Replace the single-segment edge draw (canvas.rs:520-553) with a loop over
consecutive point pairs of `edge.points`, drawing each segment as its own
`EdgeLine` quad via `draw_edge_down` (whose thickness-inflated bounding-box quad
already renders axis-aligned segments correctly — the same code path today's
axis-aligned edges already fall to). Per segment: world→local transform both
endpoints, build the inflated segment `Rect`, `draw_abs`.

`border_point` becomes unused and **must be deleted** — the implement-plan
clippy `-D warnings` gate promotes `dead_code` to a hard error, so an orphaned
helper fails the build. The `draw_edge_up` pen (the opposite-diagonal variant)
also becomes unused for orthogonal segments; keep it only if still referenced,
otherwise remove it and its uniforms too. Verify with a dead-code check before
declaring done.

## Data flow

```
model.edges ─┐
             ├─> build_scene builds (BoxId,BoxId) pairs
sizes ───────┘        │
                      v
        solve_diagram OR stress_default  ── both now call route::route
                      │
                      v
              Solved { nodes, groups, routes }
                      │
   build_scene: zip drawable edges with routes in order
                      │
                      v
        Scene { nodes, groups, edges: [SceneEdge{..., points}] }
                      │
                      v
        canvas draw_walk: per edge, stroke points[i]..points[i+1]
```

## Testing

- `build_scene` test: a routed edge's `points` is non-empty, its first point
  lies on/near the source rect border and its last on/near the target border.
- Update `scene_edge_endpoints_match_node_rects` for the new `points` field
  (still assert `source`/`target` rects; add a `points` non-empty assertion).
- A stress-default scene (layout-less diagram) now yields edges with non-empty
  `points` — a test asserting routes reach the scene on the default path.
- Renderer `draw_walk` stays GPU-untestable, as today; correctness of segment
  stroking is covered by the router's own 17 passing tests plus the scene tests
  above.

## Out of scope

- Arrowheads and `RelationshipKind`-specific adornment styling (fast-follow).
- Web/wasm edge rendering (keeps dagre + existing web path).
- Per-edge OVG-rebuild performance (accepted limitation from the router spec).
- Route caching / incremental re-route on pan/zoom (routes are world-space and
  camera-independent; no re-solve needed for view changes).
