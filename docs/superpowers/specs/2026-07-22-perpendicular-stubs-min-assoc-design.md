# Perpendicular connector stubs + minimum association length

Two related layout/routing refinements to `crates/waml/src/solve`:

1. **Perpendicular stubs** — every routed connector leaves and enters a box
   perpendicular to the border it attaches to, via a short mandatory stub. No
   more border-hugging (parallel) exits.
2. **Minimum association length** — two boxes joined by an edge are pushed far
   enough apart that the connector between them can carry arrowheads and a
   label. Unconnected neighbours stay tight.

Follows the orthogonal edge router (`2026-07-22-orthogonal-edge-router-design.md`)
and the rigid union-find layout solver (`2026-07-12-diagram-layout-solver-design.md`).

## Background

`route.rs` builds an orthogonal visibility graph (OVG), runs A* with a bend
penalty, then post-processes with `hub_spread` (spread coincident endpoints
along a border) and `nudge` (separate parallel channel segments).

`geometry.rs` (`solve_cluster`) places boxes with a weighted union-find
(`Potentials`): every constraint is an **exact equality** delta between two
boxes. There is no inequality / minimum-distance primitive. All inter-box gaps
trace to a `Place` delta (`geometry.rs:89`, `sa.w + gap`). Group-axis flow is
also emitted as a chain of `Place` constraints (`axis_constraints`,
`geometry.rs:246`), so the same gap lever covers explicit placement and group
flow alike.

Two problems today:

- A* is free to leave a border **parallel** (hug it, then turn). `hub_spread`
  tolerates this with a special "neighbour on border" branch
  (`route.rs:605-620`) rather than preventing it. Connectors are not guaranteed
  perpendicular at their attachment.
- The layout gap between two placed boxes is `cfg.margin(...)` (≤ 32px, Large),
  independent of whether an edge connects them. Two associated boxes can sit
  8px apart — a connector too short to hold a label.

## Part 1 — Perpendicular stubs

### Approach: structural (stub vertices in the OVG)

Make perpendicularity an invariant of the graph, not a post-hoc repair. A*
never considers a parallel exit because none exists in the adjacency.

Stub length = `ROUTE_MARGIN` (12px). This is exactly the clearance the router
already inflates around every *other* box, so a stub reserves the room the
router already guarantees; no new constant.

### `attach()` change (`route.rs:265`)

For each border candidate point `p` on side *S* of the endpoint box:

- Emit the **on-border vertex** `p` and a **stub vertex** `p'` at
  `p + 12px · outward_normal(S)`.
- Wire `p ↔ p'` (the mandatory perpendicular stub segment).
- Only the **stub vertex** `p'` joins the grid (the aligned/unblocked-neighbour
  wiring that `attach` does today). The on-border vertex `p` has exactly one
  neighbour: its stub.
- Return the **on-border vertices** as the attachment index list (A* sources /
  targets).

A path therefore starts on the border, and its first hop is forced through the
perpendicular stub before any grid movement. Same for the last hop into the
target. First and last route segments are always perpendicular, length ≥ 12px.

`outward_normal(S)`: Left → `(-1,0)`, Right → `(+1,0)`, Top → `(0,-1)`,
Bottom → `(0,+1)`. Side is determined the same way `side_of` already classifies
a border point.

### Self-consistency (no new collision logic)

The endpoint's own box is excluded from obstacles; every *other* box is inflated
by `ROUTE_MARGIN`. A stub that would poke into a neighbour's inflated zone is
rejected by the existing `segment_blocked` check when wiring `p'` into the grid —
if `p'` cannot reach any grid vertex, that candidate contributes nothing and
another side/candidate is used. If a box is boxed in on all sides (every stub
blocked), A* finds no path and `route()` falls back to `fallback_l` as today.

### `hub_spread` simplification (`route.rs:543`)

With parallel exits impossible, the "neighbour on border" branch
(`route.rs:605-620`) is dead. `hub_spread` always:

- moves the endpoint along the border to its evenly-spaced slot, and
- rewrites the adjacent stub vertex's perpendicular coordinate to match, keeping
  the stub perpendicular.

The conditional `if (nb.1 - fixed).abs() > 1e-6 { ... }` guard (and its vertical
twin) is removed — the neighbour is always off-border now, so the rewrite always
applies.

### Tests (Part 1)

- **`every_route_leaves_and_enters_perpendicular`** — for a spread of layouts
  (clear LOS, detour, hub fan-out), assert the first segment of every route is
  axis-perpendicular to the source border and the last to the target border, and
  each has length ≥ `ROUTE_MARGIN`.
- **`stub_blocked_side_falls_back_to_open_side`** — a box tightly flanked on one
  side still routes out an open side (no panic, orthogonal result).
- Keep `hub_spread_keeps_every_segment_orthogonal` — now holds structurally.
- Update `astar_clear_line_of_sight_is_two_point_straight`: a clear horizontal
  neighbour now yields stub-out + straight + stub-in. Assert the invariants
  (perpendicular ends, all-orthogonal) rather than the exact point count, OR
  re-derive the expected count (source stub, run, target stub → simplify).
  Pick invariant-style assertions to avoid brittleness.

## Part 2 — Minimum association length

### Approach: floor the Place gap for edge-connected pairs

Thread the edge list into layout. Where the solver computes a `Place` gap
between two boxes that are also edge-connected, floor it at `MIN_ASSOC`. Rigid
union-find is untouched — a floored gap is still an exact equality delta,
so determinism holds.

### Constant

`MIN_ASSOC: f64 = 72.0` — fixed, tunable. Budget: 12 (source stub) + ~10
(arrowhead) + ~40 (short label run) + ~10 + slack. This is the minimum
**facing-border gap**; since a Manhattan path length is ≥ the axis gap it spans,
flooring the gap floors the association length.

### Plumbing

- `solve_diagram` already has `edges: &[(BoxId, BoxId)]` — pass it down through
  `geometry::solve_with_rects` into `solve_cluster` (and any intermediate
  subtree-layout helper that owns a constraint list).
- Build a `connected: BTreeSet<(BoxId, BoxId)>` of unordered `BoxId::Node`
  pairs (store with `min/max` ordering so lookup is order-independent).
  Group-as-endpoint edges are ignored (already out of routing scope).

### Gap floor (`geometry.rs:89`)

```rust
let gap = cfg.margin(max_margin(ma, mb));
let gap = if connected.contains(&pair(a, b)) { gap.max(MIN_ASSOC) } else { gap };
```

Applied in the `Place` arm only. It covers:

- explicit `a left of b` (etc.) where an `a→b` edge exists, and
- group-axis-flow adjacency (`axis_constraints` Place chain) where two adjacent
  siblings are edge-connected.

The floor uses the separation axis the `Place` already governs (the `sa.w + gap`
/ `sa.h + gap` term), which is the correct axis for that pair.

### Scope limit (accepted)

A rigid equality solver cannot enforce a *global* minimum distance — only gaps
that exist as a `Place` delta can be floored. This is every realizable
too-close case: two connected boxes only land ~20px apart when a `Place`
(explicit or group-flow adjacency) put them there. A connected pair that is not
Place-adjacent is already far — its distance is a sum of intervening box widths
plus gaps. True global min-distance would require replacing union-find with an
inequality-capable solver (LP / stress majorization); out of scope, no real
benefit here.

### Tests (Part 2)

- **`connected_adjacent_pair_gets_min_assoc_gap`** — `a left of b` with an
  `a→b` edge: facing-border gap ≥ `MIN_ASSOC`.
- **`unconnected_adjacent_pair_keeps_margin_gap`** — same placement, no edge:
  gap stays at the margin value.
- **`group_flow_connected_siblings_spread`** — two adjacent column-flow siblings
  with an edge between them get the floored gap.
- **`min_assoc_layout_is_deterministic`** — identical input → identical rects.

## Interaction between the two parts

Part 1 consumes 12px of stub at each end. Part 2's 72px floor guarantees a
connected pair's facing gap comfortably exceeds `2 × 12` stub, leaving a real
straight run for the arrowhead and label between the stubs. Unconnected
neighbours may sit closer than 12px; Part 1 handles that safely by dropping the
blocked stub candidate and using an open side.

## Out of scope

- Measured (text-width-aware) label spacing — `MIN_ASSOC` is a fixed constant.
- Global inequality-based layout (LP / stress majorization) for non-adjacent
  connected pairs.
- Self-edges and group-as-endpoint edges (already unrouted).
- Arrowhead / label rendering itself (this reserves room; drawing is elsewhere).

## Files touched

- `crates/waml/src/solve/route.rs` — `attach()` stub vertices, `hub_spread`
  simplification, tests.
- `crates/waml/src/solve/geometry.rs` — `edges`/`connected` plumbing, gap floor,
  `MIN_ASSOC`, tests.
- `crates/waml/src/solve/mod.rs` — pass `edges` into `solve_with_rects`.
