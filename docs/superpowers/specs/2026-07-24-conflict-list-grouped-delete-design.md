# Conflict List — Grouped, Deletable Constraints Design

**Date:** 2026-07-24
**Status:** Approved (redoz@), ready for planning.

## Motivation

The off-canvas conflict error list (drag-place viz redesign, spec
`2026-07-24-drag-place-constraint-viz-redesign-design.md` §4) currently renders
each unsatisfiable placement as **one flat menu row** whose label jams the
dropped constraint plus every constraint it contradicts into a single line
(`scene::conflict_statement` → `"order left of customer; customer left of order
— these contradict"`). With more than one or two conflicts this is unreadable,
and there is no way to act on an individual offending constraint.

redoz@ wants each **conflict** rendered as its **set of participating
constraints**, one per line, separated by a divider from the next conflict, with
a per-constraint **trash affordance** for quick deletion. The toolbar conflict
**marker** should carry a proper warning glyph instead of a bare `!`.

The underlying data already carries the grouping — `SceneConflict { dropped,
conflicts_with: Vec<SceneRelation> }` — so this is a presentation + one new
delete op, not a solver change.

## Goals

1. Toolbar conflict badge shows a `message-square-warning` glyph + count (drop
   the literal `! ` text; the glyph is the warning).
2. A dedicated, taller **`ConflictList`** popup surface renders conflicts as
   grouped blocks:
   ```
   order left of customer          🗑
   customer left of order          🗑
   ───────────────────────────────────
   payment-gateway above order     🗑
   order below payment-gateway     🗑
   ```
   Each constraint line is one `SceneRelation` (the conflict's `dropped` plus
   each entry in `conflicts_with`), rendered via the existing
   `scene::relation_statement`. A hairline divider separates conflicts.
3. Each constraint line has a trailing **trash** glyph. Clicking it deletes that
   placement from the diagram's `## Layout`, re-solves, and refreshes the list.
4. Clicking a constraint line's **body** (not the trash) fades the canvas to
   just that constraint's two nodes (tighter than the current whole-conflict
   fade).
5. Layout driven by the makepad **Turtle** (flow / Fill / Fit / spacing /
   padding), not hand-computed pixel arithmetic.
6. Dismiss (Esc / outside-click / supersede) handled by the existing
   `PopupRoot` authority.

## Non-Goals

- No solver / conflict-attribution change (`DroppedPlacement` report unchanged).
- No undo (already deferred project-wide); delete is immediate.
- No web/wasm change — this is the native editor's conflict inspector only.
- No delete confirmation dialog (a trash click deletes immediately; the
  constraint is re-authorable by drag).
- No scrolling in v1 — the panel is content-sized and position-clamped
  on-screen; conflicts rarely exceed a handful of blocks. Scroll is a documented
  follow-up if a real diagram ever overflows.

## Decisions (locked)

- **Row-body click → focus that single constraint's two nodes** (not the whole
  conflict).
- **Trash → delete immediately, no confirm.**
- **Dismiss via `PopupRoot`** — `ConflictList` is a `Popup`-trait surface.
- **Passive surface, Turtle-derived geometry** — see Architecture below.

## Architecture

### 1. Icon: `Icon::MessageSquareWarning` (waml-editor `icons.rs`)

Add a new catalog glyph following the established invariant
(`enum == field == DSL == get == ALL == label`, and bump every count/label
assertion — see memory `keep-unused-catalog-icons`).

- **SVG source** (deterministic, offline — the Lucide bubble from the existing
  `message-square-text.svg` with the text lines replaced by a warning mark).
  Write `crates/waml-editor/resources/icons/message-square-warning.svg`:
  ```svg
  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-message-square-warning"><path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/><path d="M12 8v3"/><path d="M12 15h.01"/></svg>
  ```
- Generate the SDF body: `python scripts/gen-icon.py resources/icons/message-square-warning.svg`
  (run from `crates/waml-editor/`). Paste the printed
  `mod.draw.IconMessageSquareWarning` block beside the other `mod.draw.Icon*`
  definitions in the `script_mod!` catalog.
- Wire it into every parallel site: the `IconSet` DSL field
  (`message_square_warning: mod.draw.IconMessageSquareWarning{ color: atlas.accent }`),
  the `pub message_square_warning: DrawColor` struct field, the `get_mut` match
  arm, the `Icon` enum variant, the `ALL` slice, the `label()` arm
  (`=> "message-square-warning"`), and the label unit test. Bump any catalog
  count assertion.
- The `.01` dot flattens to a round-capped near-zero stroke run (a dot), same as
  every other Lucide `h.01` in the catalog.

### 2. Badge glyph (waml-editor `conflict_badge.rs`)

`ConflictBadge` becomes a small Turtle row: `flow: Right`, `Align y:0.5`,
`spacing`, an `IconSet` glyph cell drawing `Icon::MessageSquareWarning` (danger
white tint on the red pill) followed by the count `Label`. `set_count(n)` sets
the label to `format!("{n}")` (no `! ` prefix) and hides the pill when `n == 0`.
Keep the red rounded `draw_bg`, the `Clicked` action on `FingerDown`, and the
hand cursor unchanged.

### 3. Delete op: `Op::PlaceRm` (waml crate `ops/mod.rs`)

```rust
Op::PlaceRm {
    diagram: String,
    subject_slug: String,
    reference_slug: String,
}
```

Handler `op_place_rm`: `edit_doc(work, diagram, "place.rm", |doc| { … })` →
`layout_mut(doc)` → `retain(|line| match line.parsed() { Some(item) =>
!placement_matches(&item.stmt, &subject_slug, &reference_slug), None => true })`.
This reuses the now **pair-symmetric** `placement_matches` (commit `6d2e949`), so
either operand order is removed.

- **Idempotent:** removing an absent placement (or from a doc with no `## Layout`
  section) is a **no-op**, not an error — the refresh loop may target a
  constraint another delete already cleared, and a stale double-click must not
  error.
- **DTO:** `waml-ops-dto`'s `from_op` gets a `PlaceRm` arm marked native-only
  (`unreachable!`), mirroring `PlaceSet` (this op never crosses the wasm ABI).

### 4. `ConflictList` popup surface (waml-editor, new `popup/conflict_list.rs`)

A **fourth `PopupRoot` surface** alongside `MenuPopup` / `RadialPopup` /
`SelectFlyout`, implementing the `popup::base::Popup` trait
(`handle(&mut self, cx, event) -> PopupVerdict`, `reset()`). It is **event-passive**
— `PopupRoot::route` drives it — matching every existing surface. It hosts **no
child widgets**; instead it draws with a real Turtle and records hit rects.

**State:**
```rust
struct ConflictRow {
    subject: String,      // slug
    reference: String,    // slug
    body_rect: Rect,      // recorded from the Turtle during draw
    trash_rect: Rect,     // recorded from the Turtle during draw
}
// plus: the ordered rows, divider positions, an armed (hover) target
// { RowBody(usize) | Trash(usize) | None }, and the placed origin/size.
```

**Layout (Turtle):** `draw_abs` into the window overlay (same
`begin_overlay_reuse` idiom as `MenuPopup`, so the card escapes the body clip).
Drive a `Flow::Down` outer Turtle with card padding. For each `SceneConflict`,
open a `Flow::Down` block; for each of its `SceneRelation`s (dropped first, then
`conflicts_with`) walk a `Flow::Right` row: a `Label`-style statement cell
(`width: Fill`) drawn via `draw_label`, then a fixed-size trash glyph cell
(`Icon::Trash` via the `IconSet`, tinted from an idle/armed/danger holder).
Capture the row's turtle rect as `body_rect` and the trash cell's rect as
`trash_rect`. After a block's rows, walk a thin divider `Rect` (skip after the
last conflict). Spacing / padding / row height come from Turtle layout params,
not literals scattered through arithmetic.

**Sizing / placement:** measure content height from the block/row/divider walk
(a pure helper, unit-testable: `content_height(conflicts) -> f64`). Width is a
fixed comfortable cap (wider than `MENU_MAX_W`; statements are longer than menu
labels). `PopupRoot::show_at` clamps the card on-screen via `Presenter::place`
(as the Select arm does). Taller than the old single-row menu by construction.

**`handle` (passive):**
- `MouseMove` / hover → set the armed target from whichever `trash_rect` (checked
  first, it sits inside the row) or `body_rect` contains the point; redraw;
  `Consumed` when over the card, else `Ignored`.
- Primary press inside a `trash_rect` → emit
  `ConflictListAction::Delete { subject, reference }`; return **`Consumed`**
  (the surface stays open — delete is a repeatable in-surface action, NOT a
  commit).
- Primary press inside a `body_rect` (and not the trash) → emit
  `ConflictListAction::Focus { subject, reference }`; return **`Consumed`**.
- Primary press outside every rect → `Ignored` (⇒ `PopupRoot` treats it as an
  outside-click dismiss).
- Everything else → `Consumed` while over the card, else `Ignored`.
- Esc / blur are light-dismiss, decided in `PopupRoot::route` before `handle`.

Because delete/focus return `Consumed`, they never produce a
`PopupResult::Invoked`; the intent reaches `App` through the separately-emitted
`ConflictListAction` widget action, read the same frame in `App::handle_actions`.
`reset()` clears the armed target and rows.

**Hit-order note:** `trash_rect` ⊂ `body_rect`, so always test trash before
body. Record rects in **overlay/window space** and translate for the aligned
draw if needed (memory `makepad-aligned-parent-hit-rect-offset`).

### 5. `PopupRoot` integration (waml-editor `popup/root.rs`)

- Add `conflict := ConflictList{ width: Fill height: Fill }` to the `body` DSL
  tree (fourth child).
- `ActiveKind::Conflict`.
- `PopupSpec::Conflict { tag, anchor, bounds, conflicts: Vec<SceneConflict> }`
  with a `show_at` arm: size the card from `ConflictList::content_size(&conflicts)`,
  `Presenter::place`, call `conflict.open(cx, placed, conflicts)`, set the active
  slot.
- `route` + `show_at`-supersede + `Close` arms call `conflict.handle` /
  `conflict.reset()`, mirroring the existing three surfaces.
- Register `ConflictList`'s `script_mod(vm)` in `app.rs` **before** the consuming
  module resolves `mod.widgets.*` (memory `iconbutton-child-needs-script-mod-order`).

### 6. `App` wiring (waml-editor `app.rs`)

- **Open (badge click):** replace the current
  `PopupSpec::Menu{ tag: conflict_list, items: … }` block. Gather
  `canvas.conflicts()` and, if non-empty, `PopupRoot::show_at(PopupSpec::Conflict
  { tag: live_id!(conflict_list), anchor, bounds, conflicts })`. Drop the
  per-row `PopupItem` construction and the `conflict_row_ids` map (superseded by
  `ConflictListAction`, which carries slugs directly).
- **Read actions** in `handle_actions`:
  - `ConflictListAction::Focus { subject, reference }` → canvas fade-the-rest with
    the 2-node set `{subject, reference}` (reuse the existing fade path that
    `conflict_participants` feeds today, passing just these two keys).
  - `ConflictListAction::Delete { subject, reference }` → build
    `Op::PlaceRm { diagram: active_key, subject_slug: subject, reference_slug:
    reference }`, apply via `waml::ops::apply(&self.bundle, &[op])`, rebuild the
    model + bundle, re-solve (`update_scene`, camera-hold), `sync_conflict_badge`.
    Then **refresh the open list**: read the new `canvas.conflicts()`; if
    non-empty, `show_at(PopupSpec::Conflict{…})` again (re-anchored to the badge)
    so the panel shows the shrunken set still open; if empty, the badge hides and
    the popup is dismissed (`PopupRoot` supersede / explicit close).

### 7. Focus scope helper (waml-editor `scene.rs`)

Keep `conflict_participants` for any All-mode use. `ConflictListAction::Focus`
carries the two slugs directly, so no new scene helper is required; the canvas
fade path already accepts a key set.

## Data Flow (delete round-trip)

```
badge click
  → App: canvas.conflicts() → PopupRoot.show_at(Conflict{ conflicts })
trash click on a row
  → PopupRoot.route → ConflictList.handle → emit Delete{subj,ref}, Consumed
  → App.handle_actions: Op::PlaceRm → apply → rebuild model → re-solve
  → sync_conflict_badge
  → if conflicts remain: show_at(Conflict{ new conflicts })  (stays open)
    else: dismissed, badge hidden
```

## Testing

**waml (`ops/mod.rs`):**
- `place_rm_removes_a_matching_placement` — pair given in stored order.
- `place_rm_removes_a_reversed_pair_placement` — pair-symmetry (mirrors
  `place_set_replaces_a_reversed_pair_placement`).
- `place_rm_is_a_noop_when_absent` — unknown pair leaves layout untouched, no
  error.
- `place_rm_is_a_noop_without_a_layout_section`.

**waml-editor:**
- `icons.rs`: `MessageSquareWarning.label() == "message-square-warning"`; catalog
  count assertion bumped.
- `conflict_list.rs` (pure helpers): `content_height` / `content_size` for
  0 / 1 / N conflicts; row/divider ordering from a `Vec<SceneConflict>`
  (dropped-first, then `conflicts_with`); a hit-classify unit over recorded
  rects (`trash before body`, outside → none).
- `root.rs`: the `decide` table already covers the verdict mapping; add a
  `Conflict` supersede/reset smoke if cheap.
- `app.rs`: `ConflictListAction::Delete` maps to a correct `Op::PlaceRm`
  (diagram = active key, slugs passed through); after apply the conflict count
  drops.

**Gate:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.

## Interactive sign-off (redoz@, `run-native.ps1 -Optimized`)

Screenshots can't drive the drag that authors conflicts, so this needs live
hands: author two contradictory placements, open the badge popup, confirm the
grouped rendering + divider, trash one constraint and watch the list shrink /
badge decrement, row-body click fades to the two nodes, Esc / outside-click
dismiss. Use the dense fixture (`domain-model.md`, 33 nodes) plus a hand-built
multi-conflict layout.

## File Touchpoints

- `crates/waml/src/ops/mod.rs` — `Op::PlaceRm` variant + `op_place_rm` + tests.
- `crates/waml-ops-dto/…` — native-only `PlaceRm` `from_op` arm.
- `crates/waml-editor/resources/icons/message-square-warning.svg` — new glyph.
- `crates/waml-editor/src/icons.rs` — `MessageSquareWarning` (all invariant sites).
- `crates/waml-editor/src/conflict_badge.rs` — glyph + count row.
- `crates/waml-editor/src/popup/conflict_list.rs` — **new** surface.
- `crates/waml-editor/src/popup/mod.rs` — module export.
- `crates/waml-editor/src/popup/root.rs` — `ActiveKind::Conflict`,
  `PopupSpec::Conflict`, surface wiring.
- `crates/waml-editor/src/app.rs` — badge-open + `ConflictListAction` handling;
  register `conflict_list::script_mod`; drop `conflict_row_ids`.

## Deferred / follow-ups

- Vertical scroll when conflicts overflow the window.
- Undo for `Op::PlaceRm` (project-wide undo still deferred).
- "Amber will-rewrite vs red contradiction" visual (tracked in
  `drag-place-constraints` memory) is independent of this work.
- Optional: hover-trace linking a constraint row to its glyph on canvas.
