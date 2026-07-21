# Orthogonal Edge Router

Route relationship edges as straight/orthogonal (Manhattan) polylines through
the solved diagram, avoiding node and group obstacles. Lives entirely in the
Rust solver (`crates/waml/src/solve`); no spline routing, no web-frontend work.

See the diagram layout solver design
(`docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md`) for the
geometry pass this builds on.

## Goal

Today the solver emits `Solved { nodes, groups, flags }` with no edge geometry.
The editor draws each edge as a straight center-to-center line clipped to node
borders (`waml-editor/src/canvas.rs::border_point`), with zero obstacle
avoidance. This spec adds a routing pass that produces clean orthogonal
polylines that route around obstacles, so edges no longer cut through unrelated
boxes.

## Style and tier

Straight + orthogonal only. No splines, no curves. Straight is the degenerate
case (a direct segment when the endpoints share a clear axis-aligned line of
sight); everything else is Manhattan.

Target the **libavoid / Adaptagrams tier** (Wybrow, Marriott, Stuckey,
*"Orthogonal Connector Routing"*): orthogonal visibility graph, A* shortest
path with a bend penalty, then constraint-based nudging. The full three-stage
pipeline is in scope for this spec (not a staged MVP).

## Public API

**One public call.** The top-level solve entry point returns rects *and* routes
in a single result — the external caller never sees a second pass. `Solved`
gains a `routes` field:

```
pub struct Solved {
    pub nodes: BTreeMap<String, Rect>,
    pub groups: Vec<SolvedGroup>,
    pub flags: BTreeMap<String, FlagSet>,
    pub routes: Vec<Route>,   // NEW
}
```

Internally this is two passes: the existing geometry solve, then a new,
independently-testable routing module the entry point invokes with the solved
rects and the edge list.

`routes` is additive — existing consumers that ignore it are unaffected, and the
field is empty when a scene has no edges. The wasm/serde wire derives on `Solved`
carry the new field automatically.

## Route module

New file `crates/waml/src/solve/route.rs`, exposing a `pub(super)` entry the
top-level solve calls:

```
pub(super) fn route(
    nodes: &BTreeMap<String, Rect>,
    groups: &[SolvedGroup],
    edges: &[(BoxId, BoxId)],
    cfg: &SolveConfig,
) -> Vec<Route>
```

Output:

```
pub struct Route {
    pub points: Vec<(f64, f64)>,  // ordered orthogonal polyline
    pub source: String,           // node key, matches Solved.nodes keys
    pub target: String,
}
```

`Route` is geometry-first with an identity ride-along. Identity is the **node-key
string** (the same `String` that keys `Solved.nodes`), not a `BoxId`: edges are
leaf-to-leaf, so both endpoints are `BoxId::Node(key)`, and `Route` lives inside
the wasm/serde `wire` module alongside `Solved` where an internal IR enum like
`BoxId` (no wire derives) cannot go. `Route` therefore gets the same
`serde`/`Tsify` derives as the other wire types.

It carries **no** `RelationshipKind` — the router never branches on kind, and the
frontend already owns the edge record (with its kind, markers, dashing) and
re-joins on `source`/`target` identity. The router needs the endpoints as input
anyway, so returning them is free and spares the frontend a fragile index-join.

The internal `route()` pass takes `(BoxId, BoxId)` edge pairs (matching the
solver's IR) and maps each leaf `BoxId::Node(key)` to its `key` string on the way
out.

## Inputs

- **Node rects** — every leaf node rect from the geometry solve. Obstacles,
  hard, always.
- **Group rects + containment** — each `SolvedGroup.rect`, plus which leaf nodes
  fall inside it (derived from rect containment). Drives the containment-aware
  obstacle rule below.
- **Edges** — `(source: BoxId, target: BoxId)` leaf pairs. Leaf-to-leaf only.

## Obstacle rule (containment-aware)

- Leaf node rects are hard obstacles for every edge, always.
- A group rect is an obstacle for an edge **only when both endpoints lie outside
  that group**. An edge with at least one endpoint inside the group routes
  freely within and across that group's boundary.

Rationale: a line leaving a node inside a group naturally crosses the group
frame; unrelated lines detour around it. This matches how people read nested
diagrams.

## Pipeline

### 1. Orthogonal visibility graph (OVG)

Build candidate segments only through the "interesting" x and y coordinates —
obstacle-edge coordinates, each obstacle inflated by a routing margin — rather
than a dense pixel grid. This keeps the graph small and the search fast.

Endpoints join the OVG via **free-perimeter connection candidates**: candidate
attachment points along each of the box's four sides, from which an orthogonal
edge can exit perpendicular. The router is free to attach anywhere on the
perimeter — the exit side and border offset are chosen by the search, not fixed
in advance.

### 2. A* shortest path

Per edge, A* over the OVG with cost = segment length + a bend penalty. The bend
penalty biases toward fewer corners. Because attachment points are candidates in
the graph, A* selects the exit side + border offset that yields the shortest
bent path — the "free perimeter" is self-optimizing.

### 3. Nudging / ordering

Parallel segments that share a routing channel are ordered and separated with 1D
separation constraints (VPSC-style solve), so coincident runs split into
visually distinct parallel lines.

Hub borders (a node with many edges) **spread their attachment points** along the
border side, so edges fan out at the box instead of piling on one point. Nudging
still separates the channel runs further out.

## Determinism

The geometry solver is deterministic and has green tests; the router must match.
Every ordering and tie-break is stable — sort obstacles and candidates by
`BoxId` and by coordinate, break ties deterministically, never rely on hash-map
iteration order. Identical input yields byte-identical `routes`.

## Testing

Unit-tested in isolation against hand-built rect sets, mirroring the
`geometry.rs` test style (small fixtures, assert on resulting geometry). Cases:

- Two boxes, clear line of sight → straight degenerate segment.
- Two boxes with a third obstacle between them → detour, orthogonal, no overlap
  with the obstacle.
- Endpoint inside a group, endpoint outside → route crosses the group frame
  (group not treated as obstacle).
- Both endpoints outside a group that lies between them → route detours around
  the group.
- Hub node with several edges → attachment points spread along the border, no
  two edges share an attachment point.
- Parallel edges between the same box pair → nudged into distinct parallel runs.
- Determinism → same input twice yields identical `routes`.

## Out of scope

- **Self-edges** (node → itself). Deferred; isolated special case, clean
  bolt-on later.
- **Group-as-endpoint** (a rel whose source or target is a group). Leaf-to-leaf
  only for v1.
- **Splines / curves.** Orthogonal only.
- **Web-frontend wiring.** The web canvas keeps its own straight-path edges for
  now; this spec produces the Rust router only. Consuming `routes` in either
  frontend is future work.
