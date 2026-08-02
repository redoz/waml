# Route the Edges

**Goal:** Edges connect their endpoints without crossing or overlapping
anything they could have avoided.

**Why:** Edge quality is what separates a diagram a reader trusts from one they
squint at.

**Done when:** Every edge in this bundle lands on its target's border, avoids
passing through unrelated nodes, minimises crossings, and separates parallel
runs.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Routing is orthogonal only. No splines — that is a settled decision, not an
  open question. The target quality tier is a libavoid-class router, written in
  Rust.
- The endpoint-on-border invariant is enforced by a two-pass connect step.
- Crossing minimisation and parallel-run separation are the unbuilt parts.
- Planned in `docs/superpowers/plans/2026-07-22-orthogonal-edge-router.md` — a
  hand-rolled Manhattan router returning obstacle-avoiding polylines as
  `Solved.routes` beside the rectangles — and
  `2026-07-12-straighten-edges-shared-band.md`, which draws a straight line
  whenever both nodes' facing border strips can be hit head-on.
- `2026-08-03-edge-label-route-pressure.md` couples routing to labelling: the
  router should prefer paths that leave room for their labels. Route and label
  quality cannot be finished independently.
