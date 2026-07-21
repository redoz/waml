# Semi-Smart Default Layout via Stress Majorization

## Problem

A diagram with **no layout statements** renders as a single horizontal strip:
every node becomes its own component in `geometry::solve`, X packs them
left-to-right, Y pins every component top to `0`. With ~20 ER tables the camera
fits width and everything shrinks to unreadable slivers (observed on
`ocuro/docs/kd/domain/`).

Root cause: `crates/waml/src/solve/geometry.rs` `solve()` assembles unconstrained
roots as a flat left-to-right clump, and **edges never reach the solver** —
`solve_diagram` only receives a `&Diagram` (nodes + groups + layout statements),
while relationships live on `Model.edges`. The default is edge-blind.

The web frontend sidesteps this by running dagre; the native path has no
auto-layout at all.

## Goal

When a diagram has no explicit layout, produce a **semi-smart** default that
reflects relationships: connected nodes sit near each other, the result is
readable without manual layout, and it is **deterministic** (same input → same
pixels, for golden tests + stable screenshots).

Chosen algorithm: **stress majorization (SMACOF)**. Rationale (see session
research 2026-07-21): ER/domain models are associative, not strictly directed;
SMACOF fits that shape, degrades gracefully on arbitrary connectivity, is fully
deterministic with a fixed seed, is ~200 lines of pure numeric Rust with zero
new dependencies, and composes with the existing solver (positions in, positions
out). The Rust ELK-port ecosystem was rejected: `elkrs` vanished from crates.io
and GitHub mid-evaluation; `openedges/elk-rs` is EPL-2.0, JS-first, undocumented
for Rust consumers, and 1-star unproven.

## Non-Goals (v1)

- No group/container awareness in the stress pass — flat layout of all leaf
  nodes. The no-constraint case has no authored groups anyway.
- No edge routing — nodes only; edges are drawn by existing render code.
- No replacement of the constraint solver. When layout statements **are**
  present, the existing `geometry::solve` path is unchanged.
- No web-frontend change. Web keeps dagre.

## Architecture

### Layout dispatch

`solve_diagram` gains an edge input and branches:

```
solve_diagram(diagram, edges, sizes, cfg):
    if diagram has layout statements or groups:
        -> existing resolve + geometry::solve   (unchanged)
    else if edges (filtered to members) non-empty:
        -> stress::layout                        (new)
    else:
        -> grid_pack                             (new, edgeless fallback)
```

- **Stress** needs edges to be meaningful; with none it has no distances to
  honor, so it degenerates. `grid_pack` is the edgeless fallback (also the
  fallback for a fully-disconnected node set).
- Group presence still routes to the constraint solver — authored structure wins
  over auto-layout.

### `stress` module — `crates/waml/src/solve/stress.rs`

Concrete, Model-free signature (decoupled by plain slices, **not** a trait —
YAGNI with a single eager backend; a trait is a near-zero-cost refactor if a
second graph source ever appears):

```rust
pub struct StressConfig {
    pub edge_len: f64,   // ideal px per hop (L), default ~120
    pub max_iter: u32,   // default 300
    pub epsilon: f64,    // stress-delta convergence threshold
    pub gap: f64,        // min px between node boxes after overlap removal
}

/// `edges` are index pairs into `ids`/`sizes` (undirected, symmetric).
/// Returns one Rect per input id, in the same order.
pub fn layout(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    cfg: &StressConfig,
) -> Vec<Rect>;
```

Adapter (call seam, not in `stress`): `Model`/`Diagram` → `(ids, sizes, edge
index pairs)`, filtering `Model.edges` to endpoints that are both diagram
members, mapping node keys to indices.

### Algorithm

1. **Adjacency** from undirected edge pairs.
2. **All-pairs shortest path** via BFS per node → hop matrix. Target distance
   `d_ij = hops * edge_len`.
3. **Disconnected components**: split by connectivity. Each component is stressed
   independently, then components are packed left-to-right (row) with `gap`.
   Cross-component `d_ij` is undefined and never entered into a single solve.
4. **Weights** `w_ij = 1 / d_ij^2` (standard SMACOF weighting).
5. **Deterministic init**: circular seed — node `k` of `n` at
   `(cos(2πk/n), sin(2πk/n)) * radius`. No RNG.
6. **Guttman-transform iteration** until `|stress_prev - stress| < epsilon` or
   `max_iter` reached. Standard majorization update.
7. **Node-size overlap removal**: inflate `d_ij` targets by combined node
   half-extents before solving; then a final deterministic scan-line push-apart
   pass guarantees no rectangle overlap (min `gap` separation).
8. **Emit** `Rect { x, y, w, h }` per node; translate so the min corner sits at
   origin (matches `assemble`'s normalization).

### `grid_pack` — edgeless fallback

Wrap the flat node list into a grid of `ceil(sqrt(n))` columns, row heights =
max node height in row, column widths = per-column max, `gap` between cells.
Deterministic, trivial, kills the strip when there are no edges.

## Determinism

- Circular seed (no RNG), fixed iteration order (`ids` order), fixed component
  ordering (by smallest member index), fixed `epsilon`/`max_iter`.
- Golden test asserts exact pixel output for a small fixture, same style as
  `crates/waml/tests/solver_golden.rs`.

## Edge Cases

| Case | Behavior |
|---|---|
| 0 nodes | empty output |
| 1 node | single rect at origin |
| No edges | `grid_pack` |
| Fully disconnected | per-node components → `grid_pack`-equiv packing |
| Duplicate / self edges | dedup; self-edges dropped |
| Degenerate coincident positions | epsilon guard in Guttman denom avoids div0 |

## Testing

- Unit: BFS distances on a known graph; circular seed positions; convergence
  monotonically decreases stress; overlap-removal leaves no overlaps.
- Golden: small fixed graph → exact pixel dump (`solve::pretty`-style).
- Visual: harness bin renders `ocuro/docs/kd/domain/`; screenshot compared
  against the current strip. Kill/ship decision on the image.

## Rollout

1. Build `stress` + `grid_pack` + unit/golden tests (standalone, no wiring).
2. Harness bin, screenshot the real domain model, eyeball quality.
3. If good: add `edges` param to `solve_diagram`, wire dispatch, adapter,
   plumb `Model.edges` at the native canvas call site.
4. Web unaffected.

## Open Questions

- `edge_len` / `gap` defaults — tune from the domain-model screenshot, not
  guessed up front.
- Whether disconnected-component packing should grid rather than row when there
  are many components — defer until observed.
