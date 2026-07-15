# Straighten Edges Within a Shared Border Band

**Date:** 2026-07-12
**Status:** Not implemented. `RelEdge.svelte`'s `edgePath` unconditionally calls
`getSmoothStepPath` (no straighten branch); `floating.ts` has no `straightPort`
helper. Matching plan `plans/2026-07-12-straighten-edges-shared-band.md` is
still unstarted.
**Product:** Model Canvas (`packages/web`, Svelte + XYFlow/Svelte + TypeScript)
**Scope:** Edge rendering only — `RelEdge.svelte`, `AnchorEdge.svelte`, `floating.ts`.

## Goal

When two connected nodes are aligned closely enough that a **single straight
line can hit both nodes' facing borders head-on**, draw that edge as a straight
line instead of a smooth-step jog. When the nodes slide out of that shared
alignment band, snap back to smooth-step. Applies on **both axes**:

- **Horizontal straight** — nodes side-by-side (source `Right` ↔ target `Left`)
  and their vertical extents overlap enough.
- **Vertical straight** — nodes stacked (source `Top`/`Bottom` ↔ opposite) and
  their horizontal extents overlap enough.

The side (Right/Left/Top/Bottom) is already chosen upstream by `edges.ts`
(`sourceSide`/`targetSide`), so the vertical case comes nearly free once the
horizontal case works — same math on the other axis.

A hub's fanned edges must **stay fanned**: straightening turns a hub's fan into
**parallel straight lines**, one per edge, each keeping its distributed slot
position but using the **same coordinate at both ends**.

## Context / current state (concrete)

- `RelEdge.svelte:51-75` — `geometry` derived computes the two port points and
  the two sides; `edgePath` derived **always** calls `getSmoothStepPath`. This is
  where the straight-vs-step branch goes.
- `floating.ts:86-99` — `portPoint(rect, side, slot)` places a point on `side`,
  offset along that side by the slot. Solo edge (`count 1`) → midpoint. `N` edges
  → the central **`band = 0.72`** fraction of the side split into `N` evenly
  spaced ports (`f = (index+1)/(count+1)`, `t = 0.5 + (f-0.5)*band`). Rounded
  corners stay clear. `Rect`/`Slot` types + `oppositeSide` also here.
- `edges.ts` — `planPlacements` assigns `sourceSide`/`targetSide` +
  `sourceSlot`/`targetSlot` per edge (fan ordering, kept as-is).
- `AnchorEdge.svelte` — dashed edge, same `useInternalNode` pattern; already
  draws straight but is not yet band-aligned.

## Design

### The slot strip

Each node's facing border is not the full edge but a **strip**: `portPoint`
confines ports to the central `band = 0.72`. So a node's usable y-range on a
vertical border is `[y + h*0.14, y + h*0.86]` (and the symmetric x-range on a
horizontal border). The overlap test uses these **strip** ranges, not full rects.

### Overlap band test (Right ↔ Left shown; Top ↔ Bottom symmetric on x)

For an edge whose sides are `Right`/`Left`:

1. `srcStrip = [srcRect.y + h_s*0.14, srcRect.y + h_s*0.86]`
2. `tgtStrip = [tgtRect.y + h_t*0.14, tgtRect.y + h_t*0.86]`
3. `overlap = [max(lo), min(hi)]`, height `H = hi - lo`.

### Straighten decision — threshold folds into stepped spacing

Fan spacing is the y-gap between adjacent slot ports. The group of `N` parallel
straights needs `groupHeight = g * (count - 1)` of room, where `g` is the
stepped-fan gap. **Straighten iff `H >= max(8, groupHeight)`** — i.e. the overlap
band is tall enough to hold the whole fan at its natural stepped spacing (with an
8px floor so a solo edge, `groupHeight = 0`, still needs a sane sliver). This
single rule satisfies both approved choices: *match stepped spacing* (no
compression) and *minimum overlap threshold* (no single-pixel flicker).

- **Straighten:** place the `N` ports at stepped spacing `g`, the group
  **centered in the overlap band**, ordered by slot index. Each edge sets
  `sy = ty = its port y` → a straight horizontal line. Draw with
  `getStraightPath`.
- **Else:** keep `getSmoothStepPath` (current behavior), unchanged.

Because the test is on live geometry, dragging a node out of extent shrinks `H`
until `H < groupHeight` → automatic snap back to stepped. No hysteresis needed;
the threshold's 8px/groupHeight floor absorbs boundary jitter.

### Fan spacing source (resolve in TDD)

A straight line needs **one** shared coordinate, but source and target node
heights differ, so their natural strip gaps differ. Default: drive `g` off the
**source** strip (`g = h_s * 0.72 / (count + 1)`), using the edge's group
`count`. If a both-ends-hub case looks wrong in practice, fall back to
`g = min(h_s, h_t) * 0.72 / (count + 1)`. Pick during implementation against the
visual.

### Both-ends-hub tie-break

If both endpoints fan on this pair (both `count > 1`), the two ends want
different slot distributions. Combine deterministically — **average the two slot
fractions** — so each end stays distributed and lines don't cross. Verify against
a two-hub layout during impl.

### AnchorEdge

`AnchorEdge.svelte` already draws straight, so it needs only the
**shared-coordinate alignment**: when the overlap test passes, set `sy = ty`
(resp. `sx = tx`) so it, too, snaps head-on instead of angling. Reuse the same
`floating.ts` helper. Confirm the anchor's slot handling matches during impl.

### Shared helper

Add one helper to `floating.ts` (e.g. `straightPort(srcRect, tgtRect, side,
slot)`) returning either `{ straight: true, coord }` (shared y or x) or
`{ straight: false }`. Both edge components consume it, keeping the overlap math
in one place next to `portPoint`.

## Edge cases

- **Solo edge (`count 1`):** `groupHeight = 0` → threshold is the 8px floor;
  straightens whenever strips share ≥8px. Port sits at the overlap midpoint.
- **Unmeasured node:** `geometry` already bails to `getEdgeParams` when a node
  lacks measured size — the straight branch only runs on the measured path, so no
  divide-by-zero.
- **Zero/negative overlap:** `H <= 0` → stepped. No special-casing.
- **Node much taller than the other:** the small node's strip governs `H`;
  straightens only while the small strip sits inside the big one.
- **Live drag:** geometry is `$derived`, recomputed per frame (see prior fix
  `d1211d3`), so straighten/snap happens live during drag with no extra wiring.

## Out of scope

- Changing `edges.ts` side/slot assignment (fan ordering stays as-is).
- Curved/bezier edges, self-loops, edge labels repositioning.
- Any change to smooth-step rendering when the test fails.

## Testing

- **Straighten (horizontal):** two connected nodes side-by-side with overlapping
  vertical extents → connector is a straight horizontal line (`sy == ty`).
- **Snap back:** drag one node up/down until vertical extents no longer share the
  fan's height → connector returns to smooth-step.
- **Straighten (vertical):** stacked nodes with overlapping horizontal extents →
  straight vertical line; slide apart → smooth-step.
- **Hub fan-out:** a hub with N edges to aligned targets → N **parallel** straight
  lines, evenly spaced, non-crossing, same order as the stepped fan.
- **Threshold:** grazing alignment (< 8px shared strip) stays stepped, no
  flicker.
- **AnchorEdge:** dashed anchor edge snaps head-on within the band, angles
  outside it.
- **Regression:** `pnpm --filter @uaml/web check` clean; smooth-step path
  unchanged when the test fails.
