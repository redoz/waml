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
