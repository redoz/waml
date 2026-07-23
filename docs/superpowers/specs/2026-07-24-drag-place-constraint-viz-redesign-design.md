# Drag-Place Constraint Visualization — Redesign (Veil + Scrubber + Error List)

**Date:** 2026-07-24
**Status:** Design approved, ready for implementation planning
**Supersedes the viz of:** Stage 4 (`specs/2026-07-23-drag-place-stage4-constraint-viz-design.md`) — the always-on placeholder overlay (text `dir_word` at midpoint + thin connector + flat red conflict tint + debug group bounds).

## Motivation

Stage 4 shipped a deliberately-placeholder constraint overlay. In live use it is unreadable: the `dir_word` text glyphs collide into gibberish at shared midpoints, are painted *over* card body content, thin connectors vanish, and the flat red tint can't distinguish a solver bug from a real conflict. See the reference screenshot — two relations (`ORDER left of CUSTOMER`, `PAYMENTGATEWAY below ORDER`) render as a `beItqwof` blob over ORDER's `id : OrderId` row.

Two structural problems the placeholder never addressed:
1. **Always-on doesn't scale.** Showing every constraint at once on a dense diagram is incomprehensible regardless of glyph quality.
2. **Connector-based notation fights the diagram.** The canvas already draws relationship arrows between nodes; adding more lines per constraint overloads them.

## Goals

- A constraint's meaning reads **spatially and immediately** — you see *where* a node is allowed to be relative to another, without decoding a symbol.
- Visibility is **scoped and controllable**, not always-on.
- The design **scales to dense diagrams** — you can tell what constrains what.
- **Bug vs conflict** is unambiguous: the canvas shows what the solver *honored*; contradictions surface as an explicit, readable error list.
- Adds **no new connector lines** to the canvas.

## Non-Goals

- No 3D camera / depth buffer in the editor. Depth separation is achieved with parallax, not perspective projection.
- No change to the drag-to-author gesture, `Op::PlaceSet` semantics, or the one-relation-per-pair invariant.
- No web/wasm renderer changes (native-only, consistent with prior stages).
- Auto-collapsing non-intersecting veils onto a shared layer is explicitly deferred.

## System Overview

Four subsystems:

1. **Visibility toggle** — a toolbar segmented control gating what's drawn.
2. **Veil notation** — a grey hatched keep-out field per constraint, replacing the text-glyph overlay.
3. **Parallax layer scrubber** — one constraint per layer, depth-separated by parallax, for the dense/all case.
4. **Off-canvas conflict error list** — dropped constraints surfaced as a toolbar counter + popup, never as canvas paint.

---

## 1. Visibility Toggle

A three-state segmented control in the toolbar (`ToolDock`): **None / Selected / All**.

- **None** — no constraint marks drawn; pure diagram.
- **Selected** (default) — selecting a node lights *every* constraint touching it (whether the node is the constraint's subject or reference), each veil anchored to its own reference node. Selection is **sticky** (survives pointer moving away, survives drag-authoring). Nothing drawn until a node is selected.
- **All** — every constraint in the diagram, viewed through the layer scrubber (§3). The audit mode; only legible *because* of the veil notation + scrubber.

Mode persists in editor view state. Cycling via the segmented control (and optionally a hotkey, mirroring the existing `T` theme toggle mechanism).

**Touchpoints:** new segmented-control widget mounted in the toolbar alongside the existing tool buttons (see `iconbutton-extraction` memory for the `ToolDock` → child-button pattern and the `script_mod(vm)` registration-order gotcha). A `ConstraintVisibility { None, Selected, All }` field on the canvas/view drives `draw_walk`.

---

## 2. Veil Notation (grey keep-out field)

Replaces `draw_relation_connector` / the `dir_word` text glyph entirely.

A placement constraint carves out a **keep-out region** — the area its subject may *not* occupy relative to its reference. That region is rendered as a hatched grey **veil**:

- **Anchor:** the veil is anchored to the **reference node's near edge**. For `A left of B`, A may not cross B's **left edge**, so the keep-out starts at B's left edge and extends right (covering B's own column). Mapping:
  - `left of` → reference's left edge, extends right
  - `right of` → reference's right edge, extends left
  - `above` → reference's top edge, extends down
  - `below` → reference's bottom edge, extends up
  - diagonals (`above left of`, etc.) → the corresponding **corner**, both half-planes
- **Draw order — scrim over the top:** the hatch + a faint grey wash are drawn *on top* of everything in the keep-out; cards under it read "behind glass."
- **Desaturation:** cards inside the keep-out are drawn **desaturated** (greyscale), *except* the constraint's **participants** (subject + reference), which keep full colour. Colour — not a cutout hole — is what marks the participant; the hatch runs continuous over them.
- **Distance fade:** the hatch fades out with distance from the anchor edge, so a half-plane constraint does not flood the entire canvas.
- **No connector line.** The veil's position and anchor edge encode the direction; nothing is drawn between the two nodes.

This is the "visual implication" principle: the mark's *position* carries the meaning, so no per-constraint icon needs decoding.

**Touchpoints:** `scene.rs` `SceneRelation { subject, reference, dir, .. }` and `project_relations` already provide the projected pairs + rects. The `conflicting` field and the `attribute_conflicts` / leave-one-out pass in `build_scene` (~scene.rs:530) are **removed** — conflict attribution moves to the solver (§4). `canvas.rs` gains a veil renderer (hatched SDF/pattern fill anchored to a reference edge, distance-faded) replacing `draw_relation_connector` (~canvas.rs:489 `dir_word` and the shared connector helper). The debug group-bounds outline from Stage 4 (~canvas.rs:1076) is retired or gated (not part of this design). Desaturation is a per-card draw treatment applied when a card falls inside an active veil and is not a participant.

---

## 3. Parallax Layer Scrubber

Because a single veil can span most of the plane, **two veils cannot legibly share one plane** — so each constraint gets **its own layer**.

- **One constraint per layer.**
- **Parallax depth:** stacked layers are offset and shift at different rates as the view pans / the pointer drifts, so overlapping veils separate by *motion* rather than perspective. This delivers the "exploded / layered" read the flat overlay couldn't — cheaply, with no 3D camera.
- **Scrubber:** in **All** mode, a scrub control pages through the diagram's constraint layers (the audit path). In **Selected** mode with only a few constraints, the node's layers simply stack with parallax; the scrub control mainly earns its keep in All mode.
- **Parallax driver:** view pan / slight pointer drift — not a separate camera control.

**Deferred:** if two constraints' keep-out regions don't intersect, they could be collapsed onto one shared layer to reduce layer count. Not in v1.

**Touchpoints:** a layer index / scrub position in view state; the veil renderer draws the active/nearby layers with per-layer parallax offset derived from pan + layer depth.

---

## 4. Conflicts — Off-Canvas Error List

The canvas never turns red. Conflicts are an **error affordance**.

The solver keeps the constraints it *can* satisfy and drops those it can't (today `Potentials::union` fails and the placement is silently dropped in `geometry.rs` `solve_cluster`). The model:

- **Solver reports dropped constraints + conflicts-with.** New instrumentation: the solver emits, for each dropped placement, the relation it couldn't honor and the set of relations it conflicted with (its contradiction set). This is the honest "bug vs conflict" source of truth and **retires the Stage-4 leave-one-out best-effort attribution** entirely (which couldn't isolate N-way or duplicate-pair contradictions).
- **Global toolbar counter:** a red counter (`! N`) in the toolbar shows the number of unsatisfiable constraints. Click → **popup** listing the offending constraint statements as text (the raw `A left of B` DSL form + a one-line "these contradict" note).
- **Click a conflict row → fade the rest:** selecting a conflict in the popup **desaturates/fades everything except the nodes involved** in that contradiction (reusing the §2 desaturation focus mechanic), so you can locate it. (Camera-fly-to is a nice-to-have, secondary.)

**Touchpoints:** `waml/src/solve` (`geometry.rs` `solve_cluster`, `solve/mod.rs`) grows a dropped-constraint report threaded out through the solve result into the scene/model. `scene.rs` carries the report to the editor. A conflict-counter widget + popup in the toolbar; the "fade the rest" action drives the same desaturation path as veils.

---

## Data / Type Changes

- **Remove:** `SceneRelation.conflicting`, `attribute_conflicts`, the leave-one-out `solve_diags`-based attribution in `build_scene`, and `placement_would_conflict`'s attribution role (Stage 4).
- **Add:** a solver dropped-constraint report — e.g. `DroppedPlacement { relation, conflicts_with: Vec<relation> }` — produced by `solve_cluster`, surfaced through the solve result to the editor.
- **Add:** `ConstraintVisibility { None, Selected, All }` + current scrub-layer index in editor view state.
- **Unchanged:** `Op::PlaceSet`, `Direction` (incl. the 4 diagonals), `project_relations` pair projection, the drag gesture, one-relation-per-pair invariant.

## Testing

- **Pure/GPU-free unit tests** (mirroring existing `node_at` / `segment_quad` style):
  - Veil anchor-edge geometry: given a reference rect + `Direction`, the keep-out region starts on the correct edge and extends the correct way (all 8 directions incl. diagonals).
  - Participant-exemption: given a constraint + a set of card rects, the correct cards are marked desaturated (all non-participants inside the keep-out) and participants are exempt.
  - Distance-fade falloff is monotonic from the anchor edge.
  - Parallax offset: given pan delta + layer depth, the per-layer offset is computed correctly.
- **Solver tests:**
  - A satisfiable set produces an empty dropped report; each card honors its keep-out.
  - A contradiction (e.g. a 3-node cycle) produces a dropped report naming the dropped relation + its conflict set; satisfied relations are absent from the report.
- **Visibility:** None draws nothing; Selected draws only constraints touching the selected node; All enumerates every constraint into layers.
- **Interactive sign-off** (`scripts/run-native.ps1 -Optimized mini`, redoz@ drives — screenshots can't drive drag/scrub): veil legibility, desaturation focus, parallax separation, error-list popup + fade-the-rest.

## Deferred / Future Threads

- Collapse non-intersecting veils onto a shared layer.
- Camera-fly-to on conflict-row click.
- Hotkey for the visibility toggle.
- Group-scoped constraints / group veils.
- Web/wasm renderer parity.

## Verification (definition of done)

- redoz@ approves the new look/behavior interactively.
- A plan lands it green on origin/main (`cargo test --workspace`, editor suites, clippy clean).
- On a dense fixture (e.g. the 33-node domain model), selecting a node reads its constraints legibly; contradictions appear only in the error list and fade-focus locates them; the "bug vs conflict" read works at a glance.
