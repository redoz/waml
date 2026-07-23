# Drag-to-place Stage 3 — diagonal placement + drag-time constraint visibility

**Status:** design approved 2026-07-23 · builds on Stage 1 (compass gesture) + Stage 2 (Layout write-back), both landed on origin/main (`e662220`, `88b849b`).

## Problem

Drag-to-place lets a user drag a node onto a compass zone of a target to author a
placement relation. Three gaps remain:

1. **Corner drops are inert.** A corner zone tries to author *two* same-pair placements
   (e.g. `PG above Customer` + `PG left of Customer`). The solver center-aligns the cross
   axis of each `Place`, so the two relations mutually contradict — 4 `LayoutConflict`
   diagnostics, node does not move. There is no primitive that separates a pair on *both*
   axes at once.
2. **The drag is blind.** The canvas Scene carries geometry only; existing `LayoutStatement`s
   are never threaded in. While dragging you cannot see the constraints you are working
   against.
3. **Conflicts are silent.** A drop that the solver would reject gives no feedback until
   after the write-back re-solves and the node fails to move.

## Goals

- A corner drop authors a single diagonal relation that moves the node diagonally with **zero**
  `LayoutConflict` diagnostics.
- While dragging, the placement relations touching the **dragged node** and the **hovered
  target** are drawn on the canvas.
- Hovering a zone whose drop the solver would reject paints that zone **red**, predicted
  before the drop commits.

## Non-goals

- Disk write-back (Stage 2 is in-memory only; disk is a separate later follow).
- General N-axis placement primitives beyond the 4 corners (YAGNI — see Approach A rejected).
- Drawing *all* diagram relations during a drag (readability; see Viz scope decision).

---

## Feature 1 — Diagonal placement primitive

### Shape (decided: Approach B — diagonal `Direction` variants)

Extend `syntax::Direction` with four diagonal variants:

```rust
pub enum Direction {
    LeftOf, RightOf, Above, Below,
    AboveLeft, AboveRight, BelowLeft, BelowRight,   // new
}
```

Rejected — **Approach A** (`Constraint::Place2D{a,b,x,y}`): a general 2D primitive is heavier
(new `Constraint` variant, solver arm, serialize, DTO/op plumbing) and over-general for exactly
four corners. Approach B reuses every existing seam.

### Solver arm (`solve/geometry.rs`, the `Constraint::Place` match)

Each existing cardinal arm pins primary-axis *separation* + cross-axis *center-align*. The
diagonal arms pin **separation on both axes, no center-align on either** — that is the entire
point (freeing the cross axis is what lets a pair hold a diagonal without self-conflict):

```rust
Direction::AboveLeft => {           // a is above-and-left of b
    eq(&mut py, ia, ib, sa.h + gap, diags);   // a bottom gap above b top
    eq(&mut px, ia, ib, sa.w + gap, diags);   // a right  gap left  of b left
}
Direction::AboveRight => {
    eq(&mut py, ia, ib, sa.h + gap, diags);
    eq(&mut px, ia, ib, -(sb.w + gap), diags);
}
Direction::BelowLeft => {
    eq(&mut py, ia, ib, -(sb.h + gap), diags);
    eq(&mut px, ia, ib, sa.w + gap, diags);
}
Direction::BelowRight => {
    eq(&mut py, ia, ib, -(sb.h + gap), diags);
    eq(&mut px, ia, ib, -(sb.w + gap), diags);
}
```

The `gap` (margin + optional `MIN_ASSOC` for connected pairs) is reused verbatim from the
cardinal arms.

### DSL surface (`layout.rs` parse + serialize)

- Serialize: `AboveLeft => "above left of"`, `AboveRight => "above right of"`,
  `BelowLeft => "below left of"`, `BelowRight => "below right of"`.
- Parse (`eat_direction`): on `above`/`below`, peek the next word — if `left`/`right` follows,
  consume the diagonal (still requiring the trailing `of` that `left`/`right` already demand);
  otherwise fall back to the plain cardinal. Order matters: try the diagonal extension before
  committing to the cardinal, with `pos` save/restore on miss (same pattern the `left`/`right`
  arms already use for the `of` requirement).
- Round-trips: `PG above left of Order` parses to one `Placement` with a single
  `AboveLeft` direction and serializes back identically.

### Op / write-back — no change needed

`op_place_set` already loops `for dir in &directions` and pushes one `Placement` per direction;
`placement_matches` already requires `directions.len() == 1`. A diagonal is a *single* direction,
so a corner drop passes exactly one diagonal `Direction` and pair-scoped replace works unchanged.
The gesture stops emitting two cardinals for a corner and emits one diagonal instead.

### validate.rs

`Direction` is matched in `validate.rs`; the new variants need arms (mirror the cardinal
validation — a diagonal is valid wherever a cardinal is).

---

## Feature 2 — Drag-time constraint visibility

### Scope (decided: dragged-node + hover-target only)

During a drag, draw only the placement relations that (a) touch the dragged node, or (b) belong
to the currently hovered target pair. Rejected: drawing all relations (noisy on dense diagrams);
all-dimmed-with-emphasis (extra draw cost + two-tier styling for little gain at this stage).

### Plumbing

The Scene currently carries geometry only. Thread the diagram's `LayoutStatement`s (or a reduced
projection: `(subject_slug, reference_slug, Direction)` triples resolved to on-screen node rects)
into the canvas so the gesture layer can query "which relations touch node X". This is the same
data the conflict oracle needs (Feature 3), so plumb it once.

### Draw

For each in-scope relation, draw a light indicator between the two node rects expressing the
`Direction` (an orthogonal connector / arrow from reference to subject, styled distinctly from
edges). Exact visual — line vs arrow, weight, tint — is an implementation detail validated
interactively (`-Optimized`, user drags), since a screenshot cannot drive a drag.

---

## Feature 3 — Conflict-red highlight

### Oracle (decided: speculative solve)

On zone-hover, author the hypothetical placement into a **scratch** model, run the real solver,
and paint the zone red iff it emits a `LayoutConflict` diagnostic. The solver is the ground truth
— it catches transitive / cycle conflicts a hand-rolled rule would miss. Rejected: a rule
heuristic reading only the pair's existing relation (blind to third-node contradictions). Drag is
not perf-critical and a single-cluster solve is fast.

### Mechanics

- Reuse the same `LayoutStatement` projection plumbed for Feature 2.
- Speculative apply = clone the layout, run `op_place_set` semantics (pair-scoped retain + push
  the hovered direction) against the clone, re-solve, inspect `diags` for `DiagCode::LayoutConflict`.
- Cache per hovered-zone within a drag so we solve once per zone-change, not per frame.
- Red styling composes over the existing compass-zone draw (`draw_compass`), gated by the
  per-zone conflict verdict.

---

## Architecture / seams touched

- `crates/waml/src/syntax.rs` — `Direction` + 4 variants.
- `crates/waml/src/solve/geometry.rs` — 4 diagonal solver arms.
- `crates/waml/src/layout.rs` — parse + serialize diagonals.
- `crates/waml/src/validate.rs` — validation arms for the new variants.
- `crates/waml-editor/src/canvas.rs` — corner-zone → diagonal `Direction`; relation viz;
  conflict-red on `draw_compass`; Scene relation-projection intake.
- `crates/waml-editor/src/class_diagram_view.rs` — owns canvas + gesture; provides the
  relation projection + speculative-solve oracle; stays inside the view (view-seam: emit ops via
  `ViewOutcome`, shell applies).

No `waml-ops-dto` change: the DTO's exhaustive match is over `Op` variants, and no new `Op` is
introduced — `PlaceSet` already carries `Vec<Direction>`. New `Direction` variants ride inside
the existing `PlaceSet` payload.

## Testing

- **Diagonal (clean TDD):** unit tests in `waml` — a corner-authored `AboveLeft` placement
  produces both-axis separation and **zero** `LayoutConflict` diags; parse/serialize round-trip
  for all four diagonals; `op_place_set` pair-scoped replace across a cardinal→diagonal re-drag.
  `cargo test -p waml` green.
- **Viz + conflict-red:** interactive only — no screenshot can drive a drag. Validated by the
  user running `scripts/run-native.ps1 -Optimized <bundle>` and dragging.
- Full gate: `cargo test --workspace` (+ web gate if touched).

## Verify (done = all true)

1. Corner drop moves the node diagonally, 0 `LayoutConflict`.
2. Solver unit test asserts both-axis separation for each diagonal.
3. During a drag, dragged-node + hover-target relations are drawn.
4. Hovering a would-conflict zone paints it red (speculative-solve verdict).
5. `cargo test --workspace` green; user signs off the live drag.
