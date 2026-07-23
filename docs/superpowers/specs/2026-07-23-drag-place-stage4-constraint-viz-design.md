# Drag-to-place Stage 4 — persistent constraint visibility (CAD-inspired)

**Status:** design approved (direction) 2026-07-23 by redoz@ · builds on Stage 3 (diagonal primitive + drag-time viz + conflict-red), landed origin/main. This is a **playable-tonight v1**: always-on relation + group visibility with a **placeholder glyph**, not final art.

## Problem

You cannot see the placement constraints you are authoring. Relations are drawn only *during* a drag and *only* once a target is armed (`draw_drag_overlay`, canvas.rs). With nothing visible at rest, organizing a diagram is guesswork, and — the sharp pain — when the solver reports a `LayoutConflict` there is **no way to tell a bug from a genuinely-contradictory constraint set**: the conflicting relations are invisible.

Parametric CAD sketchers (SolidWorks, Fusion, Onshape, FreeCAD) solve the same class of problem by **state-coloring constraints persistently** (under/fully/over-constrained), **glyphing each constraint on the geometry**, and **highlighting the whole conflicting set** — not just the last edit. This spec borrows their *visualization*, not their *manipulation* model (their drag moves a body within remaining DOF; ours **authors a relation** and the solver picks pixels).

## Goals (v1)

1. **Persistent relation glyphs.** Every projected 2-node placement relation is drawn always-on (at rest, not only mid-drag), with a placeholder direction glyph at the relation midpoint and a thin connector between the two node rects.
2. **Conflict state-color.** When the solver emits a `LayoutConflict`, the relations responsible paint **red**; all others neutral. This is the bug-vs-conflict signal — a real contradiction lights up its culprit relation(s), a solver bug lights up something unexpected.
3. **Group bounds viz.** Group rectangles are drawn (debug-grade: outline + title label), so groups are visible for organizing and debugging.

## Non-goals (explicitly deferred to later stages)

- **Hover-trace** (hover a node → its relations + partner nodes glow). Next stage.
- **Conflict *ring* attribution** — highlighting the full transitive cycle. v1 uses best-effort per-relation leave-one-out attribution (below); ring-tracing the union-find is a follow-up.
- **Override affordance** — same-pair re-drop = amber "will rewrite" (unordered replace) vs red true-conflict. Separate thread; needs the unordered-replace fix (see Appendix).
- **Group-scoped drag** — restricting drop targets to intra-group / whole-group placement.
- **Final glyph art** — v1 is a deliberate placeholder shape/char; visual polish is a later pass.
- **Viz on/off toggle** — always-on for v1; a hotkey toggle can come later if clutter warrants.

---

## Feature 1 — Persistent relation glyphs

### Data (already present)

`Scene.relations: Vec<SceneRelation>` (subject slug, reference slug, `Direction`) is already
projected by `project_relations` (scene.rs) and node rects live on `SceneNode.rect`. No new
projection needed.

### Draw

Add an always-on relation-overlay draw invoked from `draw_walk` (canvas.rs, near the existing
`draw_drag_overlay` call ~:1263), independent of drag state:

- For each `SceneRelation`, resolve subject/reference to their `SceneNode` indices, take
  `node_screen_center` of each, stroke a thin connector, and stamp a **placeholder glyph** at the
  midpoint. The glyph crudely encodes the `Direction` (reuse `dir_word` for a text placeholder, or
  a simple directional chevron) — final art is out of scope.
- Reuse/extract the connector+glyph drawing currently inlined in `draw_drag_overlay` so the armed-
  drag overlay and the persistent overlay share one draw helper (avoid two divergent code paths).
  The drag overlay keeps its *scoping* (dragged-node + hover-target) and *emphasis*; the persistent
  overlay draws the *full* relation set at a calmer weight.
- World→screen: use the same camera transform the node/edge draws use (`node_screen_center` already
  encapsulates it).

### Verify

A diagram with placement relations shows a glyph + connector between each related node pair at
rest (no drag). Interactive: `run-native.ps1 -Optimized mini` shows the `order left of customer`
and `paymentgateway below order` relations always-on.

---

## Feature 2 — Conflict state-color

### Attribution (leave-one-out, best-effort)

The solver reports `LayoutConflict` diagnostics but does not attribute them to specific relations.
v1 attributes best-effort, cheaply, and only when needed:

- In `build_scene` (scene.rs), after the solve, inspect `diags` for `DiagCode::LayoutConflict`.
- **If none:** every relation is `conflicting: false`. No extra work (the common path).
- **If present:** for each projected relation, speculatively **remove** just that relation from a
  scratch clone, re-solve, and mark the relation `conflicting: true` iff removing it reduces the
  `LayoutConflict` count (i.e. it participates in a contradiction). This is O(relations) solves but
  fires only on already-conflicted diagrams and only at scene-build time (not per frame).
- Add a `conflicting: bool` field to `SceneRelation` (default false), set by this pass.

### Draw

The persistent relation overlay (Feature 1) colors a relation **red** when `conflicting`, neutral
otherwise. Same placeholder glyph, red tint.

### Testing

Unit-testable in the `waml-editor` scene layer: build a fixture whose layout is genuinely
contradictory (e.g. `A left of B` + `B left of A`), assert `build_scene` marks the involved
relations `conflicting: true` and a non-conflicting relation `false`. A conflict-free fixture marks
none. (Mirrors the existing `placement_would_conflict` test style.)

---

## Feature 3 — Group bounds viz

Groups already partially draw (`for group in &self.scene.groups`, canvas.rs ~:1076). v1 makes them
legibly debuggable:

- Ensure each `SolvedGroup.rect` is stroked with a visible debug outline and its `title` (when
  `Some`) labeled. Use `depth` for a subtle nesting tint if cheap.
- This is debug-grade, not final chrome — enough to *see* group extents while organizing.

### Verify

A diagram with a `### Group` heading shows the group's bounding rect + label at rest.

---

## Architecture / seams touched

- `crates/waml-editor/src/scene.rs` — `SceneRelation.conflicting: bool`; leave-one-out attribution
  pass in `build_scene`; reuse existing `project_relations` + `build_scene` clone/solve machinery.
- `crates/waml-editor/src/canvas.rs` — extract a shared relation connector+glyph draw helper from
  `draw_drag_overlay`; add an always-on relation overlay in `draw_walk`; red tint on `conflicting`;
  strengthen group-rect debug draw (~:1076).
- No `waml` crate change (relations + conflict diags already exist). No `waml-ops-dto` change (no
  new `Op`, no new `Direction`). No web/wasm change (native canvas only).

## Testing

- **Scene attribution (unit, green):** conflicting fixture marks culprit relations; clean fixture
  marks none. `cargo test --workspace` green.
- **Persistent draw + group viz:** interactive only — `run-native.ps1 -Optimized mini` (and a
  grouped fixture). A screenshot cannot show hover, but *can* confirm always-on glyphs + group
  rects render at rest; final sign-off is redoz@ playing with it.

## Verify (done = all true)

1. Placement relations draw always-on (at rest) with a placeholder glyph + connector.
2. A contradictory fixture paints the culprit relation(s) red; a clean one paints none.
3. Group rects + labels render at rest.
4. `cargo test --workspace` green; redoz@ signs off interactively.

---

## Appendix — deferred override semantics (context, not v1 scope)

`placement_would_conflict` / `op_place_set` replace is **ordered** (`operand[0]==subject &&
operand[1]==reference`). A relation authored in reversed operand order (`B above A` vs `A below B`,
semantically identical) is missed by the ordered retain, so a same-pair re-drop can coexist with
the reversed form and register as a **conflict** rather than an **override**. The fix — unordered
pair replace + an amber "will rewrite" affordance distinct from red conflict — is a separate thread,
tracked for after this viz foundation lands.
