# Reducing unnecessary edge crossings in the orthogonal router

**Status:** design, awaiting implementation plan
**Builds on:** group hulls as routing obstacles (`f2c7b6f0`) and the
border-landing invariant (`073ffc4`, the two-pass `connect_ends`). Routing style
is orthogonal only — splines are rejected, and stay rejected here.

## Why

Clustering by group made the router's blind spot visible. In the WAML Domain
Model view (`docs/waml/architecture/views/domain-model.md`, 13 nodes / 4 groups)
two edges that run roughly parallel between the same pair of regions cross each
other a few pixels before their endpoints: one arriving rightward into
*Behavioral View*, and *Behavioral View*'s own `views` edge leaving leftward and
turning up into *WAML Model*. In open space their order is one way; at the
attachment points it is the other way; so they must cross. Nothing about the
graph requires that crossing — it is purely an artefact of which border point
each edge chose.

Group hulls did not cause this. They made more edges run in parallel bundles
between clusters, which is exactly the population in which the defect shows.

## Current behaviour, verified

**The router places each edge completely independently.** Confirmed by reading
the pipeline, not inferred:

`route_keyed_with` (`crates/waml/src/solve/route.rs:125`) loops edge by edge
(`route.rs:147`). Per edge it masks the shared obstacle list — every leaf rect
except this edge's own endpoints, plus every group hull neither endpoint is a
member of (`route.rs:154`) — inflates by `ROUTE_MARGIN` (`route.rs:164`, const at
`route.rs:254`), builds a fresh orthogonal visibility graph over that obstacle
set (`build_ovg`, `route.rs:399`), and runs A* (`route.rs:170`, `astar` at
`route.rs:769`). The other edges are not in scope: no route already produced is
passed into `build_ovg` or `astar`, and no route is an obstacle. On failure it
falls back to an L (`fallback_l`, `route.rs:19`).

The cost function is `RouteCost` (`route.rs:62`): `length`, `bend`,
`label_pressure`. There is no crossing term and no place to put one, because the
A* has no other-route input. Every predicate whose name suggests crossing —
`segment_blocked` (`route.rs:299`), `segment_cuts` (`route.rs:306`) — tests a
segment against an obstacle *rect*, never against another edge. **The claim that
the router has no edge-to-edge crossing awareness whatsoever is confirmed.**

Two cross-edge passes run after the loop, and they are the only places where
routes see each other at all:

- `hub_spread` (`route.rs:1076`, called at `route.rs:179`) groups route endpoints
  by `(node key, Side)` (`route.rs:1077`), sorts each group by `along` — the
  coordinate of the *adjacent bend* of the already-routed polyline
  (`route.rs:1096`, sort at `route.rs:1116`) — and re-lays them at even interior
  fractions of that side (`route.rs:1136`). Multi-point routes are moved in place
  with `realign_interior` keeping the stub perpendicular (`route.rs:1148`);
  2-point routes are only recorded, then rebuilt whole in pass 2 via
  `connect_ends` (`route.rs:1153`–`1176`, `connect_ends` at `route.rs:1047`).
  Pass 2 is the border-landing fix from `073ffc4`: both endpoints of a 2-point
  route may be spread on different boxes, and each must stay on its own border.
- `nudge` (`route.rs:873`, called at `route.rs:180`) separates *exactly
  coincident* parallel interior segments into distinct lines, ordered by segment
  midpoint (`route.rs:922`). Endpoints and the first/last segment are never moved
  (`route.rs:889`).

So a weak, local form of port ordering already exists — but only among edges
landing on the **same side of the same node**, and it orders them by a bend
coordinate of a path that was itself chosen with no knowledge of its siblings.
The reported crossing is between an edge *arriving* at *Behavioral View* and an
edge *leaving* it toward a different node; if those land on different sides, or
if the `along` proxy disagrees with where the far endpoint actually is,
`hub_spread` never compares them. That is precisely the gap.

Which side an edge attaches to is not decided anywhere explicitly. `attach`
inside `build_ovg` (`route.rs:536`) offers *every* grid-aligned border point on
all four sides (minus the `CORNER_INSET` band, `route.rs:261`) plus the four side
midpoints, each wired to a mandatory perpendicular stub. A* picks whichever
candidate minimises length + bends. Side choice is therefore an emergent property
of a single-edge shortest path.

## Which crossings are unnecessary

This is the load-bearing distinction, and it should be written into the code as
the definition the tests assert against.

A crossing between routes A and B is **removable** when it can be eliminated by
changing only *which border point on which side* each edge attaches to, and/or
the *order of parallel runs within a shared corridor* — with node and group
geometry fixed. Two recognisable families:

1. **Port-order inversion (the reported case).** A and B both attach to the same
   node side (or to two facing sides of the same corridor), and their order along
   that side is the reverse of their order in the open space they arrive from.
   Swapping the two attachment points removes the crossing and changes nothing
   else. The swap is always available, because both points are already valid
   candidates on the same side.
2. **Channel-order inversion.** A and B run parallel down a shared corridor and
   `nudge` (or the A* tie-break) assigns them lane offsets in the opposite order
   to the order in which they must leave the corridor. Reordering lanes within
   the corridor removes the crossing; the routes' shapes are otherwise identical.

A crossing is **inherent** when no assignment of ports and lanes removes it with
nodes where they are: the planar-embedding obstruction. Canonically, four nodes
placed so that edges A–C and B–D must intersect regardless of ports (a K4-like
sub-configuration, or an edge that must pass through a corridor another edge
already occupies end-to-end). Removing these requires moving nodes — i.e. layout,
not routing — and is out of scope here.

There is a third, honest category: **crossings removable only by re-pathing**,
where a different A* route (a detour on the other side of an obstacle) has equal
or near-equal cost and no crossing. These are real but are neither a port choice
nor a lane choice, and they need the router itself to know about other routes.

I cannot currently measure the split between these three families on the domain
model, because nothing counts crossings today. The first implementation step
should therefore be **the counter, not the fix** — see Testing.

## Candidate approaches, cheapest first

### (a) Port-order assignment — sort attachment points by the far endpoint

Extend `hub_spread`'s ordering so that the sort key stops being the local bend
coordinate and becomes the *direction/position of the other end of the edge*
(e.g. the angle of the far endpoint's centre about this node's centre, projected
onto the side's along-axis, with a deterministic tie-break on `(source,target)`
as `nudge` already does at `route.rs:925`). Optionally also let a node's four
sides be assigned as one cyclic sequence rather than four independent groups, so
an edge whose far end lies up-and-left is never given a port below an edge whose
far end lies down-and-left.

- **Fixes:** family 1, with no search — it is a sort. Directly kills the reported
  *Behavioral View* case if and only if the two edges are compared, which the
  cyclic (whole-node) variant guarantees and the per-side variant does not.
- **Does not fix:** families 2 and 3, or anything inherent.
- **Cost — and this is where I decline to call it cheap.** The sort itself is
  trivial. The entanglement is not:
  - `hub_spread` runs *after* A*, so changing the port changes the path shape,
    but only the adjacent bend is repaired (`realign_interior`, `route.rs:1032`).
    Moving an endpoint further than the current even-spread does today can drag a
    segment across a node or another route. The existing code gets away with it
    because the spread stays within one side; a whole-node cyclic assignment can
    move a port to a *different side*, which `realign_interior` cannot repair —
    the perpendicular direction changes. Cross-side reassignment therefore needs
    the route re-pathed, not patched, and that is a materially bigger change than
    a sort.
  - The border-landing invariant (`073ffc4`) must hold. Any endpoint written must
    still lie exactly on its box border, and any 2-point route touched must still
    go through the `connect_ends` rebuild (`route.rs:1153`) rather than being
    edited in place. A same-side reordering preserves this by construction —
    it only permutes points already generated by the same span formula.
  - **Recommended scope: same-side permutation only.** Keep the existing even
    spread and the existing spans; change only *which edge gets which slot*, from
    the bend-coordinate order to the far-endpoint order. That is a pure
    permutation of already-valid points, so the border invariant is preserved by
    construction and `realign_interior` stays sufficient. Cross-side reassignment
    is a separate, later, more expensive step.

### (b) Channel/bundle ordering for edges sharing a corridor

Generalise `nudge` from "exactly coincident segments" to "segments in the same
corridor within a tolerance", and order lanes by where each edge must exit the
corridor rather than by segment midpoint (`route.rs:922`).

- **Fixes:** family 2, and tidies bundles between clusters — the population group
  hulls just made larger.
- **Does not fix:** family 1, or inherent crossings.
- **Cost:** moderate. `nudge` already owns the sweep and the tie-break; the work
  is corridor detection with a tolerance, and an exit-order key. Risk: widening
  the coincidence test can move segments that today are deliberately left alone,
  and lane offsets interact with `hub_spread`'s port positions (b after a, always
  — a route's lane should follow its port, not fight it).

### (c) Global crossing-reduction pass over finished routes

A layer-sweep / barycentre-style pass that counts pairwise crossings and applies
local improvement moves (swap two ports, swap two lanes, flip a route's detour
side), iterating to a fixed point or a round cap.

- **Fixes:** families 1 and 2 together, plus some of family 3 where the improving
  move is a detour flip.
- **Does not fix:** inherent crossings; cannot beat the layout it is given.
- **Cost:** high. Needs a crossing counter, a move set, an acceptance rule, and a
  determinism story (the router is deterministic today by construction — sorted
  obstacles at `route.rs:145`, sorted candidates at `route.rs:561`, `BTreeMap`
  everywhere — and an iterative pass must not break that). Also needs a round cap
  in the style of `MAX_REROUTE_ROUNDS` (`crates/waml/src/solve/mod.rs:421`).

### (d) Crossing count in the A* cost function

Add a `crossing` weight to `RouteCost` (`route.rs:62`) and charge a path for
crossing already-placed routes.

- **Fixes:** in principle the most, including family 3.
- **Cost:** highest, and it changes the router's contract. Today each edge's A*
  is independent of every other edge; charging for existing routes makes the
  result **order-dependent** — edge *i* pays for edges *0..i* and not the
  reverse. That is a real, permanent asymmetry, and it makes the whole solve
  sensitive to authored edge order in a way it currently is not. It also costs an
  extra geometric query per graph edge inside the A* inner loop, on a graph that
  is rebuilt per edge. Not worth it until (a) and (b) have been measured.

### Recommendation

**Do (a) restricted to same-side permutation, then (b), and gate both behind a
crossing counter landed first.** (a) addresses the reported symptom with a
change that is a permutation of points the current code already produces, so the
border-landing invariant is preserved by construction. (b) addresses the bundle
population that group hulls created. (c) is the natural follow-up once a counter
exists to prove it helps; (d) should be considered rejected for now, because
order-dependence is a worse property than the crossings it removes.

## Interaction with the two layout paths

Both paths converge on the same `route_keyed_with`, so a fix in `hub_spread` /
`nudge` reaches both automatically — and can regress both automatically.

- **Stress default (native).** `stress_default`
  (`crates/waml-editor/src/scene.rs:622`) solves positions and hulls, then calls
  `route_with_groups` (`scene.rs:565`, invoked at `scene.rs:684`), which inserts
  each hull as `BoxId::Group(i)` into the rect map and builds one `Box` per group
  for membership — never inferred from rect overlap (`scene.rs:591`).
- **Constraint path.** `solve_diagram_routed`
  (`crates/waml/src/solve/mod.rs:332`, called from `scene.rs:798`) routes against
  `geometry`-solved rects with the same group-obstacle rule.

The one shared hazard is **the label reroute loop**.
`place_labels_with_reroute` (`mod.rs:494`) replays the *entire* edge set through
`route_keyed_with` (`mod.rs:570`) with `label_pressure` weighted on blocked
edges, precisely because `hub_spread` and `nudge` are cross-edge passes and
running them over a one-edge slice strips the spread (`mod.rs:462`). Any new
cross-edge pass must live inside `route_keyed_with` alongside those two, for the
same reason — a pass added at a caller would be silently dropped on replay. It
must also be deterministic and idempotent, since the loop compares polylines for
equality to decide whether to stop (`mod.rs:576`, `mod.rs:581`); a pass that
oscillates would burn every round and could keep a worse result.

## Testing

**Land the counter first.** A `fn crossings(routes: &[Route]) -> usize` (test
support at minimum, ideally `pub(crate)` so both the golden test and any future
pass share one definition): count unordered pairs of routes with at least one
pair of properly-intersecting segments, excluding segments that merely touch at
a shared endpoint on a common node border. Without it, no claim in this spec is
checkable.

Golden tests, then:

- **Regression, hand-placed.** A four-rect fixture staging the exact port-order
  inversion — two nodes on the left, one shared node on the right, one edge in
  and one edge out whose free-space order is the reverse of their border order.
  Assert `crossings == 0` after routing, and assert it currently fails before the
  fix. Hand-placed geometry, in the style of the existing hull test at
  `scene.rs:1940`, because the stress solve's placement is not controllable
  enough to stage a crossing (that is already stated at `scene.rs:562`).
- **Invariant, unchanged.** Every route endpoint still lies on its box border —
  the `073ffc4` invariant, already covered by
  `routed_edge_points_anchor_near_node_borders` (`scene.rs:1974`). It must stay
  green untouched.
- **No obstacle regression.** `member_edge_crosses_group_frame_freely`
  (`route.rs:1737`) and the hull-obstacle test at `scene.rs:1940` must stay
  green: reducing edge crossings must never buy it by cutting a hull.
- **Small end-to-end fixture.** `crates/waml-editor/tests/fixtures/groups-linked/`
  — groups plus relations with no `## Layout` section, already driven by
  `groups_linked_fixture_clusters_and_never_overlaps` (`scene.rs:2362`). Assert a
  crossing-count ceiling here, not zero.
- **Big fixture, ratchet.** The WAML Domain Model view. Record today's crossing
  count as a ceiling and assert it never rises. A ratchet rather than an absolute
  number, because some of its crossings are inherent and the spec explicitly does
  not promise to remove those.
- **Determinism.** Routing the same input twice yields byte-identical polylines,
  and the label reroute loop still terminates (`mod.rs:530`).

## Non-goals

- **Splines or any curved routing.** Orthogonal only, permanently.
- **Adopting libavoid or any C++ dependency.** libavoid-tier *quality* is the
  target; the implementation stays a Rust solver in-tree.
- **Moving nodes.** Crossings inherent to the placement are a layout problem.
  This spec never perturbs a rect or a hull.
- **Guaranteeing zero crossings.** Not achievable without layout changes, and
  promising it would make the domain-model test a lie.
- **Changing which edges are routed at all.** `routable` (`route.rs:87`) and its
  skip rule — self-edges, group endpoints, missing rects — are untouched, along
  with `routable_edge_indices` (`route.rs:106`), which the label reroute loop
  depends on for its route-to-edge mapping.
- **Cross-side port reassignment**, in this pass. It needs re-pathing rather than
  bend repair; if the same-side permutation proves insufficient it earns its own
  spec.

## Uncertainties, stated rather than invented

- I have not measured how many of the domain model's crossings are removable
  versus inherent. The counter must land before anyone claims a number.
- I have not confirmed from the code that the two *Behavioral View* edges land on
  the same node side. If they do not, the same-side permutation in (a) will not
  reach them and the fix escalates to the cyclic whole-node variant, with the
  re-pathing cost that carries. The regression test above should be written
  against the real fixture geometry so this is settled by evidence, not by this
  document.
- Whether `nudge`'s corridor tolerance can be widened without disturbing routes
  it deliberately leaves alone today is untested.
