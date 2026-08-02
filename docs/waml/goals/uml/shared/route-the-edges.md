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
- The solver returns routes as polylines alongside the rectangles. The renderer
  draws what it is given and decides nothing about the path.
- A jog that a straight line could have avoided is a defect. When both nodes'
  facing border strips can be hit head-on by one straight segment, that is the
  route; it reverts to a stepped path only when the nodes slide out of that
  shared band.
- Routing and labelling are one problem. The router prefers paths that leave
  room for the label the edge will carry, so route quality and
  [label quality](./place-the-labels.md) cannot be finished independently.
