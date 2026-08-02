# Edge label placement as a layout stage

Status: approved design, not yet implemented
Date: 2026-08-02

## Problem

Edge labels (terminal role/multiplicity, mid-route relationship name) are placed
at draw time in `waml-editor/src/edge_labels.rs`, in screen space, with no
knowledge of anything else on the canvas. Consequences:

- Labels overlap each other on short edges.
- Labels are painted before node cards, so any label that lands on a card is
  silently swallowed.
- A screen-space fix would change its own results with zoom, so the same diagram
  would lay its labels out differently at 0.5x and at 2x.

A prior commit (`75e129db`) fixed the narrower defect that labels sat *on* the
stroke and inside the endpoint adornment. It moved clearance to be perpendicular
to the route and taught terminal labels to grow away from their endpoint. That
fix stands; this design builds on it and does not revisit it.

## Decisions

| Question | Decision |
| --- | --- |
| Where is placement decided? | In the solver (`waml/src/solve/`), world space |
| What must a label avoid? | Hard: cards, group title bands, other labels. Soft: foreign strokes |
| Degrees of freedom | Slide along route + flip side, **and** the router may reroute to make room |
| When nothing fits | Leader line to free space |
| Unconnected node spacing | New `MIN_SEP` floor, 40px |

## Architecture

New module `crates/waml/src/solve/label.rs` owns world-space label placement.
`waml-editor/src/edge_labels.rs` shrinks to an adapter reading placed rects off
the scene.

### Two spaces, deliberately

- **Solver, world space** — decides where the label box sits. Everything it
  avoids (cards, groups, labels) is world-space, so the result is
  zoom-independent by construction.
- **Renderer, screen space** — keeps only the head clearance from `75e129db`,
  because in CAD linework mode (`stroke_scale = 1.0`, the default) adornments are
  a constant screen size at every zoom while label text is `8.0 * zoom`. The two
  genuinely live in different spaces and cannot be unified. Pre-inflating the
  world reservation to cover the worst case is not viable: `MIN_ZOOM` is 0.05, so
  it would mean a 20x inflation.

A **legibility cutoff** completes this: below a drawn font size of 5px — i.e.
zoom below ~0.63, given the `8.0 * zoom` label size — labels do not draw at all.
At zoom 0.05 the text is 0.4px and unreadable regardless, and the cutoff removes
clutter when zoomed out.

### Pipeline

```
stress/geometry layout  -> node rects              (existing)
route                   -> polylines               (existing)
measure                 -> label sizes             (sizing.rs, new caller)
place                   -> candidates + scoring    (label.rs, new)
reroute                 -> only edges whose labels failed, bounded rounds
leader                  -> anything still unplaced
```

`Scene` gains placed labels as world rects, so both frontends consume identical
geometry.

## Candidate model

Each label gets a small enumerable candidate set — no continuous optimisation, so
placement is deterministic and golden-testable. Two degrees of freedom: slide
position along the route, and side of the stroke.

- **Terminal** (`from`/`to`): slides along its terminal segment, starting past the
  adornment, bounded to a short band — beyond that it stops reading as belonging
  to that end. 4 slide positions x 2 sides.
- **Mid-route** (relationship name): slides by arc length in a band around the
  midpoint (~35-65%). 5 x 2.

The rect is derived, not authored: near edge one gap off the stroke; grows away
from its endpoint for terminals, centred for mid-route.

## Obstacles

- **Hard — node cards, other placed labels.** Never violated.
- **Hard — group *title bands* only.** A group box is a large translucent
  container that legitimately holds edges and labels; treating its whole rect as
  solid would forbid every label inside a group.
- **Soft — foreign edge strokes crossing the rect.** Counted and penalised, never
  fatal.
- **Not an obstacle — the label's own stroke.** The perpendicular gap handles it.

## Scoring and assignment

Score, lower wins, rejecting any candidate with a hard collision:

- distance from the ideal slide position (terminals prefer close to their
  endpoint; mid-route prefers true middle)
- a side-preference term favouring the canonical side (above for horizontal,
  right for vertical), so labels do not flip sides between nearly-tied layouts
- count of foreign strokes crossed

Assignment is greedy in a fixed order — scene edge order, then `from`, `to`,
mid — so results are reproducible.

Greedy is order-dependent: a label placed early can force a later one into a
worse slot. Mitigation is one retry pass over the failures after everything else
is placed, before escalating to reroute. If golden dumps show that pass mattering
a lot, that is the signal to consider a real assignment algorithm. Not now.

## Rerouting

`route.rs` today bakes length and bend penalties into `astar` as constants.
Replace with a `RouteCost` struct carrying named weights — `length`, `bend`,
`label_pressure` — threaded through `route_keyed`. Future layout work extends
that struct rather than inventing a new mechanism each time.

`label_pressure` is a real cost term, not a geometry hack: for an OVG edge under
consideration, does a band of label-height alongside that segment collide with a
hard obstacle? Penalty is proportional to the blocked fraction. The router
*prefers* paths with room for their labels, continuously.

An obstacle-inflation alternative was considered and rejected: it can only say
"this box is bigger", never "this path is mildly worse", so it does not compose
with further layout tuning.

Two properties keep this from destabilising current routes:

- The term is weighted zero unless the edge has a label to place, so unlabelled
  edges route byte-identically to today.
- Default `length`/`bend` weights are the current constants, so the refactor
  alone is a no-op — provable against existing route goldens before the new term
  is switched on.

The band check per OVG edge is the real cost; it is a rect-vs-obstacle query
against the inflated obstacle list A* already holds, cached per segment. If it
proves too slow on large diagrams, evaluate the term only on the reroute pass
rather than the first. That is a knob, not a redesign.

Bounded to 2 rounds of (reroute failures -> re-place affected edges), then stop.

## Leader lines

Whatever survives rerouting gets a leader line. Placement searches an expanding
ring around the label's ideal anchor, stepping by label height, taking the first
position with no hard collision. This always terminates, because space outside
the content bounding box is by definition empty — which is what makes leader
lines a *total* strategy needing no further fallback.

The leader is a 2-point line from the anchor on the route to the nearest edge of
the placed rect.

Leader lines are **soft** obstacles for later labels, not hard. Hard would
cascade, each leader shrinking free space for the next, and they are hairlines —
a crossing is untidy rather than unreadable.

## Instrumentation

The scene carries counts of labels that needed a reroute and labels that needed a
leader, and both appear in golden dumps. Without those numbers there is no way to
tell whether the reroute stage earns its complexity, and with a tunable cost
function there is no way to see what a weight change did.

## Spacing

`geometry.rs:191` floors the facing-border gap for *connected* pairs at
`MIN_ASSOC = 72`, "so the connector can carry arrowheads and a short label".
Nothing floors unconnected pairs, which fall back to the plain margin
(`Medium = 16`). Unconnected neighbours are therefore 4.5x tighter than connected
ones — an inversion, since unrelated boxes should not read as a pair.

1. **New `MIN_SEP = 40` floor for unconnected neighbours.** Over twice the current
   16, well under the connected floor, so connectedness stays visually legible.
2. **`MIN_ASSOC` becomes label-aware**: for a connected pair the floor becomes
   `max(72, from_label_width + to_label_width + 24)`, where 24 is the slack that
   keeps the two terminal labels from touching each other in the middle. This is
   the payoff from
   measuring labels in the solver. In the motivating screenshot `order {1}` and
   `customer {1}` are ~90px each with 72px of gap to share — no placement
   strategy could have won, and rerouting would have papered over a gap that was
   never wide enough.
3. **`MIN_SEP`, `MIN_ASSOC` and `StressConfig::gap` move into `SolveConfig`**, so
   they are tunable without hunting constants.

Raising these moves every layout golden. That is expected, and the diffs are the
review artifact.

## Implementation order

The pieces are interlocking but independently landable, and the order matters —
spacing first, because it may reduce how much work the later stages have to do:

1. **Spacing floors** (`MIN_SEP`, label-aware `MIN_ASSOC`, config move). Standalone
   and immediately visible. Note that step 2's measurement is a prerequisite for
   the label-aware half of `MIN_ASSOC`, so land the plain `MIN_SEP` floor first
   and the label-aware floor with step 2.
2. **Measure + place, no rerouting** — `label.rs`, candidates, scoring, greedy
   assignment, placed rects on the `Scene`, editor adapter. At this point labels
   already stop hiding under cards and stop overlapping each other, which is the
   whole of the original complaint.
3. **`RouteCost` refactor**, weights defaulted to today's constants, proven a
   no-op against existing route goldens.
4. **`label_pressure` term + bounded reroute loop.**
5. **Leader lines** for the residue.

Steps 4 and 5 are the ones whose value the instrumentation from step 2 can
actually measure. If the counts come back near zero on real diagrams, that is
real evidence, not a reason to skip them silently.

## Testing

- Unit tests per stage in `label.rs`: candidate generation bounds, hard-collision
  rejection, score ordering, greedy determinism, leader-ring termination.
- Golden dumps in the style of `tests/stress_golden.rs`, covering placed label
  rects plus the reroute/leader counts.
- Existing route goldens must be byte-identical after the `RouteCost` refactor
  and before `label_pressure` is enabled — this is the regression gate for the
  riskiest part of the change.
- Per-pid screenshot sign-off on the native editor, since occlusion is exactly
  the class of defect that unit tests cannot see.

## Out of scope

- Assignment beyond greedy + one retry pass.
- Any change to how labels are *styled* (font, background chip, colour).
