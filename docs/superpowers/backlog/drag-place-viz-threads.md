# Drag-place constraint-viz — deferred design threads (backlog)

Living backlog of design ideas raised while building drag-to-place Stages 1–4 (all landed
origin/main 2026-07-23). These are **not yet specced** — each is a candidate next stage. Captured
so the reasoning survives; brainstorm one properly (`superpowers:brainstorming`) before building.

**Context:** drag a node in the class-diagram canvas → author a WAML placement relation
(`A left of B`) → solver re-runs. Stage 4 added persistent, CAD-inspired constraint *visibility*
(always-on relation glyphs, conflict state-color, group bounds) with a **placeholder glyph**. The
north star (redoz@) is parametric-CAD constraint UX: state-color constraints, glyph-on-constraint,
hover-to-trace, highlight the whole conflicting *set*. Borrow their **visualization**, not their
**manipulation** (CAD drag moves a body in remaining DOF; ours *authors* a relation, solver picks
pixels).

Key code today:
- `crates/waml-editor/src/scene.rs` — `SceneRelation{subject,reference,dir,conflicting}`,
  `project_relations`, `attribute_conflicts`/`solve_diags` (~:530 leave-one-out conflict marking),
  `placement_would_conflict` (~:495 speculative-solve oracle).
- `crates/waml-editor/src/canvas.rs` — `draw_relation_connector` (shared glyph+connector helper),
  always-on overlay in `draw_walk`, `draw_drag_overlay` (~:1292 armed-drag scoped overlay), group
  draw (~:1076), `dir_word` (~:489, placeholder glyph text), `node_screen_center` (~:1283).
- `crates/waml/src/ops/mod.rs` — `op_place_set` (~:1034 pair-scoped replace), `placement_matches`
  (~:1014). `crates/waml/src/solve/geometry.rs` — `Constraint::Place` solver arms.

Rough ranking by design leverage (redoz@ 2026-07-23): hover-trace and override-amber are the two
biggest daily-use wins; conflict-ring is the deepest correctness fix; groups + art are polish.

---

## 1. Hover-trace (highlight relations on hover)

**What.** Hover a node → its placement relations + the partner nodes glow. Hover a relation glyph
→ its two endpoint nodes glow. Pure read-only highlight, no authoring.

**Why.** Highest clarity-per-effort. On a dense diagram the always-on overlay (Stage 4) shows
*all* relations at once; hover lets you isolate "what touches THIS node" without reading the DSL.
Directly the CAD hover pattern. redoz@: "can't organize what you can't see" — this is the cheap
half of the answer.

**Design sketch.**
- Canvas already tracks a hovered node for other interactions — reuse that hit-test, don't add a
  second. On hover, compute the set of `SceneRelation`s where `subject`/`reference` == hovered key
  (cheap linear scan of `scene.relations`).
- Draw those relations + partner node rects at an emphasized weight/tint; dim the rest (or leave
  neutral). Reuse `draw_relation_connector` with an emphasis flag.
- Hovering a relation glyph needs per-glyph hit-rects at the midpoint — store them during
  `draw_relation_connector` (mirror the compass-zone hit-rect pattern), then reverse-highlight the
  endpoints.

**Open questions.** Dim-others vs emphasize-hovered-only (two-tier styling cost)? Does hover fight
the existing node hover/select affordance? Glyph hit-rects are small — pick a forgiving radius.

**Risk/effort.** Low-medium. No model/solver change; pure canvas. Interactive sign-off only (no
screenshot drives hover) — TDD the relation-set computation, eyeball the draw.

---

## 2. Conflict-RING attribution (highlight the whole contradiction cycle)

**What.** When the solver reports a `LayoutConflict`, light up **every** relation in the
contradicting cycle red — not just relations whose single removal clears it.

**Why.** Fixes the core "bug vs conflict" signal properly. Stage 4's leave-one-out attribution is
best-effort and has two holes (see Gotchas): an N-way contradiction needing ≥2 removals marks
**nothing** red, and duplicate ordered-pair placements get removed all-at-once so no single culprit
shows. CAD shows the full over-constrained set and offers to delete one — that's the target.

**Design sketch.**
- The solver's `Potentials` union-find (`crates/waml/src/solve/geometry.rs`, `eq()`/`union`)
  already *knows* which union failed — that's where `LayoutConflict` is pushed today (blindly,
  with no operand info). Thread the failing edge's `(a, b, axis)` out of `union` into the
  diagnostic instead of dropping it.
- Then map failing solver edges back to the `LayoutStatement`s / `SceneRelation`s that produced
  them (the constraint list is built from layout stmts — keep provenance: which stmt emitted which
  `Constraint`). Mark all relations on the failing cycle `conflicting`.
- This likely REPLACES `attribute_conflicts`'s leave-one-out loop with real provenance — cheaper
  (no re-solve-per-relation) AND correct.

**Open questions.** How much provenance to carry through `solve_cluster` (constraint → source
stmt)? Difference-constraint cycles: is the "ring" the full negative cycle, or just the two edges
that closed it? Decide what "the conflicting set" means for the user.

**Risk/effort.** Medium-high — touches the `waml` solver core (careful, shared with web via wasm).
But strongly TDD-able (contradictory fixtures, assert the marked set). Highest correctness payoff.

---

## 3. Override amber vs conflict red (unordered pair replace)

**What.** Distinguish two "occupied" drop states: (a) **override** — the pair already has a
relation, re-dropping REWRITES it → amber "will rewrite" affordance; (b) **conflict** — the drop
genuinely contradicts a *third-node* constraint → red. Today both look red.

**Why.** redoz@ explicitly flagged: "if A is below B and I drag A above B, that's a conflict I
should be able to OVERRIDE by dropping." Pair-scoped replace already rewrites — but only when the
existing relation was stored in the SAME operand order. A reversed-storage relation (`B above A`
semantically == `A below B`) is missed by the **ordered** retain and coexists → false red instead
of a clean override.

**Design sketch.**
- Make pair replace **unordered**: `placement_matches` (ops/mod.rs ~:1014) and `placement_is_pair`
  (scene.rs ~:483) currently require `operand[0]==subject && operand[1]==reference`. Add an
  unordered variant that drops any placement touching BOTH slugs in either order. Careful:
  `A left of B` and `B left of A` are *opposite* — unordered replace of both is the intended
  "one spatial relation per visual pair" semantics; confirm with redoz@.
- Split the drop-zone verdict into three: free (neutral), override (amber — same visual pair
  already related, will rewrite), conflict (red — speculative solve still errors after the
  unordered replace, i.e. a genuine third-node contradiction). The oracle
  `placement_would_conflict` (scene.rs ~:495) already does the speculative solve; feed it the
  unordered replace, and separately detect "pair already related" for the amber case.

**Open questions.** Unordered replace changes op semantics — is dropping `A left of B` supposed to
silently kill an existing `B left of A`? (Probably yes — one relation per visual pair.) Amber vs
red on a zone that is BOTH an override AND still-conflicting after replace — which wins?

**Risk/effort.** Medium. Touches `waml` op + the editor oracle + compass draw. TDD the unordered
replace + the three-way verdict.

---

## 4. Group-scoped drag (constrain drop targets by group)

**What.** When you drag a node that lives inside a group, either (a) restrict valid drop targets to
**other members of that group**, or (b) let you place the **whole group** relative to an outside
element. redoz@'s framing: "if you drag an item in a group, only other items in that group should
be available for placement, or you place the whole group in relation to another element."

**Why.** Keeps intra-group layout coherent and prevents authoring cross-group placements that
fight the group's own layout. Groups are a first-class structure; drag should respect them.

**Design sketch.**
- Needs group membership per node in the scene. `SolvedGroup` (waml solve/mod.rs:67) carries
  rect/shape/title/depth but NOT its member node keys — either add members to `SolvedGroup`, or
  resolve membership from the model's group headings at scene-build.
- Drag target filter (canvas gesture): when the dragged node has a group, the target hit-test
  excludes non-members (mode a). Mode b = a modifier/zone that switches the subject from the node
  to its whole group (authoring a group-level placement — needs a group operand in the layout DSL;
  check whether `## Layout` already supports group names as operands).
- Depends on **group viz** (Stage 4 debug bounds) being legible so the user sees the scope.

**Open questions.** Does the layout DSL support a GROUP as a placement operand? (Check
`crates/waml/src/layout.rs` operand parsing.) Mode a vs b — one gesture or a modifier to pick?
What about ungrouped nodes (no restriction)? Nested groups (depth)?

**Risk/effort.** Medium-high — likely needs a DSL/model extension (group-as-operand) for mode b,
plus membership plumbing. Mode a (target filter) alone is lighter.

---

## 5. Real glyph art (retire the placeholder)

**What.** Replace Stage 4's placeholder glyph (`dir_word` text at midpoint) with proper directional
constraint glyphs — a designed marker per `Direction` (cardinal + 4 diagonals), legible at any
zoom, distinct from edge adornments.

**Why.** v1 shipped text-as-glyph deliberately to be playable. Real art makes the overlay scan as
*constraints* not *labels*, and reads at a glance like CAD's constraint badges (⊥, //, etc.).

**Design sketch.**
- 8 directional glyphs (LeftOf/RightOf/Above/Below + AboveLeft/AboveRight/BelowLeft/BelowRight).
  Likely SDF glyphs via the existing IconSet/IconSpline machinery (see the picker-popup per-row
  icon pattern already in the codebase) so they tint (neutral/red/amber) and scale cleanly.
- Draw at the connector midpoint (where `dir_word` sits now, in `draw_relation_connector`).
- Zoom-independent sizing like the compass handles (Stage 3 used fixed-px, zoom-independent
  handles — reuse that reasoning so glyphs don't vanish when zoomed out).

**Open questions.** Glyph vocabulary — arrow/chevron showing direction, or an abstract badge?
Should the glyph also encode override(amber)/conflict(red) state, or is that the connector's job?
Interaction with hover-trace emphasis (thread 1)?

**Risk/effort.** Low-medium, pure canvas/art. Needs a visual-companion / mockup pass with redoz@
(inherently visual — offer the browser companion when brainstorming this one).

---

## 6. Viz on/off toggle

**What.** A hotkey to toggle the always-on constraint overlay (relations + glyphs, maybe groups).

**Why.** Always-on is great for organizing, noisy for reading the diagram content. A toggle lets
the user flip between "layout-editing" and "reading" modes. Smallest thread — bundle into whichever
stage ships next rather than its own.

**Design sketch.** A view-state bool on the canvas/view, flipped by a hotkey (follow the existing
theme-toggle `T` hotkey pattern — request_live_edit/rehydrate mechanism), gating the overlay draw
in `draw_walk`. No model change.

**Open questions.** Which key? Toggle relations + groups together or separately? Persist per-view?

**Risk/effort.** Trivial. Ride-along with another stage.
