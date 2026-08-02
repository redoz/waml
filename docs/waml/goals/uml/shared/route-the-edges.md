# Route the Edges

**Goal:** Each edge connects its two endpoints. An edge does not cross or cover
an element that the router can avoid.

**Why:** The quality of the edges is the difference between a diagram that a
reader trusts and a diagram that a reader must examine closely.

**Done when:** Each edge in this bundle stops on the border of its target, does
not go through an unrelated node, has the minimum number of crossings, and
keeps a distance from a parallel edge.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The routes are orthogonal. The product does not use curves. This is a
  decision and not an open question. The target quality is equal to a
  best-in-class orthogonal router, written in Rust.
- A connection step with two passes keeps each endpoint on the border of its
  target.
- The solver gives the routes as polylines with the rectangles. The renderer
  draws the given path and makes no decision about it.
- A step in a route that a straight line can replace is a defect. If one
  straight segment can meet the facing borders of both nodes at a right angle,
  that segment is the route. The route becomes a stepped path only when the
  nodes move out of that shared band.
- The router prefers a path that has space for the label of its edge. Thus the
  quality of a route and the quality of a [label](./place-the-labels.md) are
  one problem.
- The minimum number of crossings and the distance between parallel edges do
  not operate.
