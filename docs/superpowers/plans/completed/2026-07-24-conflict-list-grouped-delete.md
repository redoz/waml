# Conflict List — Grouped, Deletable Constraints — Implementation Plan

**Spec (source of truth, locked):**
`docs/superpowers/specs/2026-07-24-conflict-list-grouped-delete-design.md`
**Date:** 2026-07-24

## Goal

Turn the off-canvas drag-place conflict error list from one flat menu row per
conflict into GROUPED blocks: one `SceneRelation` per line (dropped-first, then
each `conflicts_with`), a hairline divider between conflicts, and a trailing
**trash** glyph on every line that deletes that placement immediately and
re-solves. The toolbar conflict badge gains a `message-square-warning` glyph in
place of the bare `!`. Native editor only — **no web/wasm change**.

## Gate (every task must pass this on its own before commit)

```
cargo test --workspace && pnpm -r test && pnpm lint && pnpm build
```

## Constraints / gotchas (bake these into the named task)

- **Workspace-wide gate:** adding `Op::PlaceRm` to the `waml` crate makes the
  `waml-ops-dto` `from_op` match non-exhaustive and *break the workspace build*.
  The DTO `PlaceRm` arm therefore ships in the SAME unit as the op (Task 1), not
  a later one. (Native-only `unreachable!`, mirroring `PlaceSet`; do NOT touch
  the frozen wasm solve ABI.)
- **Idempotent `PlaceRm`:** removing an absent placement, or from a doc with no
  `## Layout` section, is a **no-op, not an error** (the refresh loop may target
  a constraint another delete already cleared; a stale double-click must not
  error). Reuses the pair-symmetric `placement_matches` (commit `6d2e949`).
- **Icon invariant** (memory `keep-unused-catalog-icons`): a new catalog glyph
  must be added at EVERY parallel site and every count/label assertion bumped —
  `IconSet` DSL field, `pub …: DrawColor` struct field, `get` match arm, `Icon`
  enum variant, `ALL` slice, `label()` arm, AND the label unit test + `ALL.len()`
  / `seen.len()` count assertions. Spelled out as sub-steps in Task 2. The
  catalog currently holds **90** glyphs → **91**. Append the new glyph at the END
  of every site (after `Search`) so the existing edge tests (`ALL[85..=88]`) are
  untouched.
- **`script_mod` order** (memory `iconbutton-child-needs-script-mod-order`):
  `conflict_list::script_mod(vm)` MUST be registered in `app.rs` BEFORE
  `crate::popup::root::script_mod(vm)` (currently `app.rs:1719`), else the DSL
  child `mod.widgets.ConflictList` resolves to a dead invisible node. Register it
  in Task 4 (module lands) so it is already in place when Task 5 adds the child.
- **Aligned-parent hit-rect offset** (memory
  `makepad-aligned-parent-hit-rect-offset`): the `ConflictList` surface draws into
  the **window overlay** via the `begin_overlay_reuse` + `begin_root_turtle`
  idiom (exactly like `MenuPopup::draw_walk`), so recorded turtle rects are in
  window/overlay space and match `MouseMove.abs` / `MouseDown.abs` DIRECTLY — no
  translation. Do NOT record hit rects from a plain aligned child turtle (those
  are pre-alignment and would silently miss). Test `trash_rect` BEFORE `body_rect`
  (trash ⊂ body).
- **Trash keeps the popup OPEN:** the trash and body presses return
  `PopupVerdict::Consumed` and emit a separate `ConflictListAction` widget action;
  they NEVER return `PopupVerdict::Closed`/`PopupResult::Invoked`. Only Esc /
  outside-click / supersede close the surface (via `PopupRoot`).
- **Non-goals (do not scope-creep):** no scroll, no undo, no confirm dialog, no
  solver/attribution change. All deferred per spec §Non-Goals.
- **Worktree:** `implement-plan` runs each unit in its own worktree; write no step
  that assumes the main checkout.

## Key facts verified from the code (use these exact names)

- `crates/waml/src/ops/mod.rs`: `enum Op` at `:65`; `Op::PlaceSet` variant `:168`;
  dispatch arm `:275`; `placement_matches` (pair-symmetric) `:1017`; `op_place_set`
  `:1032`; `edit_doc` `:335`; `layout_mut` `:367`. PlaceSet tests + helpers
  (`layout_diagram`, `diagram_no_layout`, `placeset`) at `:2023+`.
- `crates/waml-ops-dto/src/lib.rs`: `from_op` `:571`; `Op::PlaceSet { .. } =>
  unreachable!("place.set no web DTO yet (native-only)")` `:765`.
- `crates/waml-editor/src/icons.rs`: `IconSet` DSL `:3081`; struct fields
  `:3178`; `get` match `:3368`; `IconSet::draw(cx, icon, rect, color)` `:3466`;
  `enum Icon` `:3477`; `ALL` slice ends `…Icon::Search,` `:3664` (`[Icon; 90]`
  `:3574`); `label()` `:3669`; count tests `:3770`/`:3793` + label test `:3797`.
  `Icon::Trash` already exists (`mod.draw.IconTrash`).
- `crates/waml-editor/src/conflict_badge.rs`: `ConflictBadge` (`#[deref] View` +
  `label`), `set_count` (currently `"! {n}"`) `:79`, `clicked` `:88`.
- `crates/waml-editor/src/popup/base.rs`: `Popup` trait (`handle -> PopupVerdict`,
  `reset`), `PopupVerdict::{Consumed,Ignored,Closed}`, `PopupResult`.
- `crates/waml-editor/src/popup/menu.rs`: `MenuPopup` — the overlay-draw idiom to
  mirror (`draw_walk` `:399`, `begin_overlay_reuse` + `begin_root_turtle`), the
  `IconSet` glyph-tint holders, `Popup for MenuPopup` `:525`. Constants
  `MENU_GAP`, `CAPTION_H`, `PAD_V`, `PAD_H`, `ROW_H`.
- `crates/waml-editor/src/popup/root.rs`: `PopupSpec` `:39`, `ActiveKind` `:79`
  (`Menu/Radial/Select`), `show_at` `:171`, `route` `:298`, DSL body children
  `:121`, `decide` + tests `:95`/`:372`. Three surfaces (`menu/radial/select`) —
  ConflictList is the 4th, wired identically.
- `crates/waml-editor/src/popup/mod.rs`: module list (`pub mod …`).
- `crates/waml-editor/src/scene.rs`: `SceneRelation { subject, reference, dir }`
  `:87`; `SceneConflict { dropped, conflicts_with }` `:96`; `relation_statement`
  (**private**, `:287`) → make `pub`; `dir_keyword` (pub) `:272`;
  `conflict_statement` `:294`; `conflict_participants` `:304`.
- `crates/waml-editor/src/canvas.rs`: `conflicts()` `:1999`; `conflict_count()`
  `:1995`; index-based focus `conflict_focus: Option<usize>` field `:338`, draw
  block `:1478` (builds a `HashSet<String>` from `conflict_participants`),
  `set_conflict_focus` `:2005`, reset sites `:1946`/`:1969`.
- `crates/waml-editor/src/app.rs`: badge-open block `:1383` (builds flat
  `PopupItem`s + `conflict_row_ids`); `conflict_row_ids` field `:358`;
  `conflict_closed` read `:1033` + index focus handler `:1125`; `sync_conflict_badge`
  `:537`; op-apply/re-solve pattern `:1451` (`waml::ops::apply` → rebuild
  `self.bundle`/`self.model` → `v.resolve_active` → `sync_conflict_badge`);
  `script_mod` registration list `:1704+` (root at `:1719`); `window_bounds`,
  `self.tabs.active_tab()` (`.id/.key/.title`).

---

### Task 1: `Op::PlaceRm` + native-only DTO arm (waml + waml-ops-dto)

Pure model/op change, no UI. Ships the DTO arm in the same unit so the workspace
build stays green.

**Files:**
- `crates/waml/src/ops/mod.rs`
- `crates/waml-ops-dto/src/lib.rs`

**Steps:**
1. Add the variant to `enum Op` (`crates/waml/src/ops/mod.rs:168`, right after
   `PlaceSet`):
   ```rust
   PlaceRm {
       diagram: String,
       subject_slug: String,
       reference_slug: String,
   },
   ```
2. Add the dispatch arm in `apply_one` (after the `Op::PlaceSet {…}` arm, `:275`):
   ```rust
   Op::PlaceRm {
       diagram,
       subject_slug,
       reference_slug,
   } => op_place_rm(work, diagram, subject_slug, reference_slug),
   ```
3. Add the handler beside `op_place_set` (`:1032`). It reuses the pair-symmetric
   `placement_matches` and is a no-op when nothing matches / no `## Layout`
   exists (`layout_mut` creates an empty section, `retain` over an empty vec is a
   no-op, and canonical serialize drops an empty section — so an absent target
   round-trips unchanged):
   ```rust
   fn op_place_rm(
       work: &mut Bundle,
       diagram: &str,
       subject_slug: &str,
       reference_slug: &str,
   ) -> Result<(), OpError> {
       let subject_slug = subject_slug.to_string();
       let reference_slug = reference_slug.to_string();
       edit_doc(work, diagram, "place.rm", |doc| {
           let layout = layout_mut(doc);
           layout.retain(|line| match line.parsed() {
               Some(item) => !placement_matches(&item.stmt, &subject_slug, &reference_slug),
               None => true,
           });
           Ok(())
       })
   }
   ```
4. In `crates/waml-ops-dto/src/lib.rs`, add the native-only arm to `from_op`
   directly after the `Op::PlaceSet { .. }` arm (`:765`):
   ```rust
   Op::PlaceRm { .. } => {
       unreachable!("place.rm no web DTO yet (native-only)")
   }
   ```
   (Confirm no other exhaustive `match op` over `Op` exists in the DTO crate; if
   `to_op` or a serde enum needs a mirror, `PlaceRm` is authoring-only and never
   crosses the wire — match the existing `PlaceSet` treatment exactly.)

**Tests (add to `crates/waml/src/ops/mod.rs` `#[cfg(test)]`, reuse the existing
`layout_diagram` / `diagram_no_layout` helpers at `:2026`):**
- `place_rm_removes_a_matching_placement` — `layout_diagram("- [Order](./order.md)
  left of [PaymentGateway](./payment-gateway.md)\n")`, apply
  `Op::PlaceRm { diagram: "dia", subject_slug: "order", reference_slug:
  "payment-gateway" }`, assert the line is gone (`!out[0].1.contains("left of")`).
- `place_rm_removes_a_reversed_pair_placement` — stored order
  `PaymentGateway left of Order`, remove with `subject=order, reference=
  payment-gateway` (swapped) → line gone (pair symmetry; mirrors
  `place_set_replaces_a_reversed_pair_placement`).
- `place_rm_is_a_noop_when_absent` — a layout with one UNRELATED placement,
  `PlaceRm` of a pair not present → `apply` is `Ok` and the existing line is kept.
- `place_rm_is_a_noop_without_a_layout_section` — `diagram_no_layout()`,
  `PlaceRm` → `Ok`, output has no `## Layout` (or an empty/no placement); no error.

**Green:** `cargo test --workspace` (waml + waml-ops-dto compile & pass), pnpm gate
unaffected (no web change).

---

### Task 2: `Icon::MessageSquareWarning` catalog glyph (waml-editor `icons.rs`)

Independent leaf. Adds one Lucide glyph across every invariant site. Append at the
END of every parallel list so the existing edge tests stay valid.

**Files:**
- `crates/waml-editor/resources/icons/message-square-warning.svg` (new)
- `crates/waml-editor/src/icons.rs`

**Steps:**
1. Write the SVG source exactly as spec §Architecture.1:
   `crates/waml-editor/resources/icons/message-square-warning.svg`:
   ```svg
   <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-message-square-warning"><path d="M22 17a2 2 0 0 1-2 2H6.828a2 2 0 0 0-1.414.586l-2.202 2.202A.71.71 0 0 1 2 21.286V5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2z"/><path d="M12 8v3"/><path d="M12 15h.01"/></svg>
   ```
2. From `crates/waml-editor/`, run
   `python scripts/gen-icon.py resources/icons/message-square-warning.svg`.
   Paste the printed `mod.draw.IconMessageSquareWarning = mod.draw.DrawColor{ … }`
   block into the `script_mod!` catalog beside the other `mod.draw.Icon*` defs
   (e.g. right after `mod.draw.IconMessage`). The `h.01` dot flattens to a
   round-capped near-zero stroke run, same as every other Lucide `.01`.
3. Wire the SEVEN parallel sites, appending at the END of each list:
   - `IconSet` DSL (`:3172`, after `search: mod.draw.IconSearch{ color: atlas.accent }`):
     `message_square_warning: mod.draw.IconMessageSquareWarning{ color: atlas.accent }`
   - `pub message_square_warning: DrawColor,` `#[live]` struct field (after
     `pub search: DrawColor,` `:3358`).
   - `get` arm (after `Icon::Search => &mut self.search,` `:3459`):
     `Icon::MessageSquareWarning => &mut self.message_square_warning,`
   - `enum Icon` variant (after `Search,`): `MessageSquareWarning,`
   - `ALL` slice (after `Icon::Search,` `:3664`): `Icon::MessageSquareWarning,`
     and bump the array type `[Icon; 90]` → `[Icon; 91]` (`:3574`).
   - `label()` arm (after `Icon::Search => "search",` `:3760`):
     `Icon::MessageSquareWarning => "message-square-warning",`
4. Bump the count/label tests in the same file:
   - `icon_all_has_90_entries` → `assert_eq!(Icon::ALL.len(), 91);` (rename to
     `icon_all_has_91_entries`).
   - `icon_labels_are_unique_and_nonempty`: `assert_eq!(seen.len(), 91);`
   - Add to `label_reflects_lucide_slugs_not_field_names` (or a new test):
     `assert_eq!(Icon::MessageSquareWarning.label(), "message-square-warning");`

**Green:** `cargo test -p waml-editor` (label/count tests), full gate.

---

### Task 3: Badge glyph row (waml-editor `conflict_badge.rs`)

Depends on Task 2 (`Icon::MessageSquareWarning`). Replace the `! ` text with a
leading `message-square-warning` glyph + bare count.

**Files:**
- `crates/waml-editor/src/conflict_badge.rs`

**Steps:**
1. Give `ConflictBadge` an `IconSet` + a danger-tint color holder, and a leading
   glyph cell. In the DSL (`mod.widgets.ConflictBadge`): keep `flow: Right`,
   `align: Align{x:0.5,y:0.5}`, `padding` and the red `draw_bg`; add a color
   holder `draw_icon +: { color: #FFF }` (danger-white on the red pill) and keep
   `label`. In the struct add `#[live] icons: IconSet,` and `#[redraw] #[live]
   draw_icon: DrawColor,` (mirror the `MenuPopup` glyph-holder pattern).
2. Draw the glyph in `draw_walk` after the view paints, in immediate mode at a
   rect computed from the badge's OWN area rect (robust against first-frame child
   area timing — the same reason `MenuPopup` computes rects rather than reading a
   child area):
   ```rust
   fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
       let step = self.view.draw_walk(cx, scope, walk);
       let r = self.view.area().rect(cx);
       if r.size.x > 0.0 {
           let d = 16.0;
           let icon = Rect {
               pos: dvec2(r.pos.x + 10.0, r.pos.y + (r.size.y - d) * 0.5),
               size: dvec2(d, d),
           };
           self.icons.draw(cx, crate::icons::Icon::MessageSquareWarning, icon, self.draw_icon.color);
       }
       step
   }
   ```
   Bump the `padding.left` (e.g. `Inset{left: 30.0, right: 10.0}`) so the label
   clears the glyph gutter; keep `width: Fit`.
3. `set_count`: drop the `! ` prefix — `.set_text(cx, &format!("{n}"))`; keep the
   `set_visible(cx, n > 0)` hide-on-zero and `redraw`. Leave `clicked` and the
   `FingerDown → Clicked` / hand-cursor handling unchanged.

**Tests:** the badge is view/draw glue with no pure helper to unit-test; rely on
the label-format change compiling + the shared gate. (No new unit test required;
the icon glyph is covered by Task 2's catalog tests. Interactive appearance is in
the deferred sign-off.)

**Green:** full gate (`cargo test --workspace` compiles the badge; pnpm untouched).

---

### Task 4: `ConflictList` popup surface + pure helpers (waml-editor `popup/conflict_list.rs`)

New 4th `PopupRoot` surface. Ships standalone (module + `script_mod` registration
+ pure helpers with tests), NOT yet a `PopupRoot` child (that is Task 5). Marked
`#[allow(dead_code)]` like `MenuPopup` so the not-yet-instantiated widget passes
the dead-code gate.

**Files:**
- `crates/waml-editor/src/popup/conflict_list.rs` (new)
- `crates/waml-editor/src/popup/mod.rs` (add `pub mod conflict_list;`)
- `crates/waml-editor/src/scene.rs` (make `relation_statement` `pub`)
- `crates/waml-editor/src/app.rs` (register `script_mod` BEFORE root — gotcha)

**Steps:**
1. `scene.rs`: change `fn relation_statement` (`:287`) to `pub fn relation_statement`
   so the surface can render each line. (`dir_keyword` is already `pub`.)
2. `popup/mod.rs`: add `pub mod conflict_list;`.
3. New `popup/conflict_list.rs`, mirroring `menu.rs` structure:
   - Layout constants (lpx): `CONFLICT_MAX_W = 380.0` (wider than `MENU_MAX_W`;
     statements are longer than menu labels), `PAD_V`, `PAD_H`, `ROW_H = 30.0`,
     `DIVIDER_H = 1.0`, `TRASH_W = 22.0`, `TRASH_INSET`.
   - `#[derive(Clone)] pub struct ConflictRow { pub subject: String, pub
     reference: String, pub statement: String, pub body_rect: Rect, pub
     trash_rect: Rect }`.
   - Pure helper `pub fn rows_of(conflicts: &[SceneConflict]) -> (Vec<(String,
     String, String)>, Vec<usize>)` producing, in order, each conflict's rows —
     `dropped` first, then each `conflicts_with` — as `(subject, reference,
     relation_statement)`, plus the flat row-index at which a divider precedes a
     new conflict group (the group boundaries). Uses
     `scene::relation_statement`.
   - Pure sizing: `pub fn content_height(conflicts: &[SceneConflict]) -> f64`
     (`PAD_V*2 + total_rows*ROW_H + (num_conflicts-1)*DIVIDER_H`, `0` conflicts →
     just `PAD_V*2`) and `pub fn content_size(conflicts) -> DVec2`
     (`dvec2(CONFLICT_MAX_W, content_height(conflicts))`).
   - Hit-classify: `pub enum ConflictHit { Trash(usize), Body(usize), None }` and
     `pub fn classify(point: DVec2, rows: &[ConflictRow]) -> ConflictHit`, testing
     each row's `trash_rect` BEFORE its `body_rect` (trash ⊂ body), returning
     `None` when outside all.
   - `#[derive(Clone, Debug, Default)] pub enum ConflictListAction { #[default]
     None, Focus { subject: String, reference: String }, Delete { subject:
     String, reference: String } }`.
   - The widget struct `ConflictList` (event-passive, `#[allow(dead_code)]`):
     own `#[live] draw_list: DrawList2d`, an `AccentFrame` `draw_frame`, a
     `draw_label: DrawText`, divider + hover/danger `DrawColor` holders, and
     `#[live] icons: IconSet`; `#[rust] rows: Vec<ConflictRow>`, `#[rust] armed:
     ConflictHit`, `#[rust] placed: Rect` (origin+size), `#[rust] open: bool`.
   - `pub fn open(&mut self, cx, placed: Rect, conflicts: Vec<SceneConflict>)`:
     store `placed`, build the flat row list via `rows_of` (rects filled at
     draw), set `open = true`, redraw.
   - `draw_walk`: early-return when closed; otherwise use the EXACT overlay idiom
     from `MenuPopup::draw_walk` (`self.draw_list.begin_overlay_reuse(cx)` →
     `cx.begin_root_turtle(cx.current_pass_size(), Layout::flow_overlay())` →
     `self.draw(cx)` → `cx.end_pass_sized_turtle()` → `self.draw_list.end(cx)`).
   - `draw(&mut self, cx)`: draw the `AccentFrame` card at `self.placed`. Drive a
     `Flow::Down` turtle inside the card padding; for each conflict, for each of
     its `SceneRelation`s (dropped first, then `conflicts_with`) advance a
     `Flow::Right` row of height `ROW_H`: a `width: Fill` label cell drawn via
     `self.draw_label.draw_abs`, then a fixed `TRASH_W` trash cell drawn via
     `self.icons.draw(cx, Icon::Trash, trash_cell, tint)` where `tint` picks the
     danger/armed/idle holder from `self.armed`. Record the row's produced Rect as
     `body_rect` and the trash cell's Rect as `trash_rect` into `self.rows[k]`
     (window/overlay space — matches `e.abs`, no translation). Between conflict
     groups draw a `DIVIDER_H` hairline `DrawColor` inset off both frame edges
     (skip after the last conflict). Spacing/padding/row height come from the
     turtle params + the constants, not scattered arithmetic.
   - `impl Popup for ConflictList`:
     - `handle(&mut self, cx, event) -> PopupVerdict`:
       - `Event::MouseMove(e)` → `self.armed = classify(e.abs, &self.rows)`;
         redraw; `Consumed` when over `self.placed` (i.e. `armed != None` OR
         `placed.contains(e.abs)`), else `Ignored`.
       - `Event::MouseDown(e) if e.button.is_primary()` → `match classify(e.abs,
         &self.rows)`: `Trash(i)` → `cx.widget_action(self.widget_uid(),
         ConflictListAction::Delete { subject, reference })` (from `self.rows[i]`),
         return **`Consumed`**; `Body(i)` → emit `ConflictListAction::Focus {…}`,
         return **`Consumed`**; `None` → return **`Ignored`** (⇒ `PopupRoot`
         outside-click dismiss).
       - everything else → `Consumed` when `placed.contains` the pointer (if the
         event carries one) else `Consumed` (match `MenuPopup`'s default).
       - Esc/blur are decided in `PopupRoot::route` BEFORE `handle` — do not
         handle them here.
     - `reset(&mut self)`: `self.open = false; self.rows.clear(); self.armed =
       ConflictHit::None;`
   - A `pub fn action(actions: &Actions) -> Option<ConflictListAction>` reader for
     `App` (find its widget action, cast), OR expose the `widget_uid()` — pick the
     idiom `App` already uses; a dedicated reader mirrors `ConflictBadge::clicked`.
   - `script_mod!` DSL block `mod.widgets.ConflictList = … do
     mod.widgets.ConflictListBase{ … }` giving the frame/label/divider/icon-tint
     holders their atlas tokens (danger = `atlas.danger`, hover = `atlas.accent`,
     idle = `atlas.text`, divider = `atlas.accent_soft`), copying `MenuPopup`'s
     token wiring. `pub fn script_mod(vm)` via `register_widget`.
4. `app.rs`: register the surface BEFORE root (gotcha). Insert
   `crate::popup::conflict_list::script_mod(vm);` immediately before
   `crate::popup::root::script_mod(vm);` (`:1719`).

**Tests (`#[cfg(test)]` in `conflict_list.rs`, pure — no `Cx`):**
- `content_height_for_zero_one_and_n_conflicts` — build `Vec<SceneConflict>`
  fixtures (0 conflicts; 1 conflict with dropped + 1 `conflicts_with` = 2 rows;
  N=2 conflicts) and assert `content_height` / `content_size.y` equal the
  `PAD_V*2 + rows*ROW_H + (n-1)*DIVIDER_H` formula; `content_size.x ==
  CONFLICT_MAX_W`.
- `rows_are_dropped_first_then_conflicts_with_with_group_dividers` — assert the
  emitted `(subject, reference, statement)` order is `dropped` then each
  `conflicts_with` per conflict, and the divider boundary indices land between
  groups (not after the last).
- `classify_tests_trash_before_body_and_outside_is_none` — hand-build a
  `Vec<ConflictRow>` with synthetic `body_rect` + nested `trash_rect`; assert a
  point in the trash → `Trash(i)`, a point in the body-but-not-trash → `Body(i)`,
  and a point outside all rows → `None`.

**Green:** `cargo test -p waml-editor` (pure helpers), full gate. Widget is
registered but not yet shown — `#[allow(dead_code)]` keeps the gate green.

---

### Task 5: `PopupRoot` integration (waml-editor `popup/root.rs`)

Wire `ConflictList` as the 4th surface, identical shape to `Menu/Radial/Select`.
Depends on Task 4 (surface + `content_size` + registered `script_mod`).

**Files:**
- `crates/waml-editor/src/popup/root.rs`

**Steps:**
1. `use crate::popup::conflict_list::ConflictList;` and
   `use crate::scene::SceneConflict;`.
2. Add the DSL child (`:126`, after `select := …`):
   `conflict := ConflictList{ width: Fill height: Fill }`.
3. `enum ActiveKind` (`:79`): add `Conflict`.
4. `enum PopupSpec` (`:39`): add
   ```rust
   Conflict {
       tag: LiveId,
       anchor: DVec2,
       bounds: Rect,
       conflicts: Vec<SceneConflict>,
   },
   ```
5. In `show_at` supersede (`:173`) add the `ActiveKind::Conflict` reset arm
   (`self.body.widget(cx, ids!(conflict)).borrow_mut::<ConflictList>() → c.reset()`).
6. Add the `PopupSpec::Conflict { … }` match arm in `show_at` (`:208`), mirroring
   the `Select` arm's clamp-then-place:
   ```rust
   PopupSpec::Conflict { tag, anchor, bounds, conflicts } => {
       let size = crate::popup::conflict_list::content_size(&conflicts);
       let placed = Presenter::place(anchor, size, bounds);
       if let Some(mut c) = self.body.widget(cx, ids!(conflict)).borrow_mut::<ConflictList>() {
           c.open(cx, Rect { pos: placed, size }, conflicts);
       }
       self.active = Some((ActiveKind::Conflict, tag));
   }
   ```
   (Match the exact `Presenter::place` signature the `Select`/`Menu` arms use —
   it returns the clamped origin; pass `size` through to `open` so the card draws
   at the placed rect.)
7. In `route` (`:308`) add the `ActiveKind::Conflict` verdict arm
   (`… .borrow_mut::<ConflictList>().map(|mut c| c.handle(cx, ev))`), and in the
   `RouteStep::Close` reset block (`:331`) add the `ActiveKind::Conflict` reset
   arm. The existing `decide` mapping already turns an `Ignored` primary press
   into an outside-click dismiss and a `Consumed` into keep-open — no `decide`
   change needed.

**Tests (`#[cfg(test)]` in `root.rs`):**
- The `decide` table is already covered. Add a cheap smoke if inexpensive:
  `conflict_supersede_resets_prior` is hard without a `Cx`; instead assert the
  pure `decide` path a `ConflictList` relies on: a `Consumed` verdict keeps open
  (covered by `a_consumed_event_keeps_it_open`) and an `Ignored` primary press
  dismisses (covered). No new `decide` variant → the existing tests suffice; add
  a one-line comment noting `Conflict` rides the same table. (Do not fabricate a
  `Cx`-driven test.)

**Green:** full gate (root compiles with the 4th arm; `ConflictList` now
instantiated as a child so it is no longer dead).

---

### Task 6: `App` wiring + `ConflictListAction` + canvas key-focus (waml-editor `app.rs`, `canvas.rs`)

Replace the badge-open flat-menu block with `PopupSpec::Conflict`, handle the new
actions (Focus fades to the two nodes; Delete applies `Op::PlaceRm`, re-solves,
and refreshes the open list), and drop the superseded `conflict_row_ids` +
index-focus path. Depends on Tasks 1, 4, 5.

**Files:**
- `crates/waml-editor/src/canvas.rs`
- `crates/waml-editor/src/app.rs`

**Steps:**
1. **Canvas: focus by key set (replaces index focus).** In `canvas.rs`:
   - Replace the `conflict_focus: Option<usize>` field (`:338`) with
     `conflict_focus_keys: Option<std::collections::HashSet<String>>`.
   - Replace `set_conflict_focus(&mut self, cx, idx: Option<usize>)` (`:2005`)
     with:
     ```rust
     pub fn set_conflict_focus_keys(&mut self, cx: &mut Cx, keys: Option<Vec<String>>) {
         self.conflict_focus_keys = keys.map(|v| v.into_iter().collect());
         self.draw_bg.redraw(cx);
     }
     ```
   - Rewrite the draw block (`:1478`) to fade every card whose key is NOT in
     `conflict_focus_keys` (drop the `conflict_participants`/`scene.conflicts.get`
     lookup — the key set is now passed in directly):
     ```rust
     if let Some(keep) = &self.conflict_focus_keys {
         for idx in 0..self.scene.nodes.len() {
             if !keep.contains(&self.scene.nodes[idx].key) {
                 let s = self.node_screen_rect(idx);
                 self.fill_rect(cx, s.pos.x, s.pos.y, s.size.x, s.size.y, vec4(0.62, 0.65, 0.70, 0.55));
             }
         }
     }
     ```
   - Update the two reset sites (`:1946`, `:1969`) `self.conflict_focus = None;`
     → `self.conflict_focus_keys = None;`.
   (`conflict_count`, `conflicts()`, `conflict_participants` stay — the latter is
   still used for any All-mode use per spec §7.)
2. **App: drop the dead index plumbing.** Remove the `conflict_row_ids` field
   (`:358`) and the `conflict_closed` index-focus handler block (`:1125-1136`).
   Keep the `conflict_closed = pr.closed(actions, live_id!(conflict_list))` read
   only if still needed — the `Conflict` surface never `Invoked`-closes, so a
   `Closed` for that tag is only ever `Dismissed`; drop the read + its handler
   entirely (the badge simply hides via `sync_conflict_badge` on delete).
3. **App: open the grouped surface.** Replace the badge-click block (`:1383-1431`):
   ```rust
   if badge_clicked {
       let conflicts = self.ui.widget(cx, ids!(canvas))
           .borrow::<crate::canvas::GraphCanvas>()
           .map(|c| c.conflicts()).unwrap_or_default();
       if !conflicts.is_empty() {
           self.open_conflict_list(cx, conflicts); // helper below
       }
       return;
   }
   ```
   Add a private helper (re-used by the delete-refresh path) that anchors under
   the badge and shows the surface:
   ```rust
   fn open_conflict_list(&mut self, cx: &mut Cx, conflicts: Vec<crate::scene::SceneConflict>) {
       let btn = self.ui.widget(cx, ids!(conflict_badge)).area().rect(cx);
       let anchor = dvec2(btn.pos.x, btn.pos.y + btn.size.y + crate::popup::menu::MENU_GAP);
       let bounds = self.window_bounds(cx);
       if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
           pr.show_at(cx, PopupSpec::Conflict {
               tag: live_id!(conflict_list), anchor, bounds, conflicts,
           });
       }
   }
   ```
4. **App: read `ConflictListAction`.** In `handle_actions`, after the popup-outcome
   block, read the surface's action (via the `conflict_list::action(actions)`
   reader from Task 4, borrowing the `conflict` child through `popup_root`), and:
   - `Focus { subject, reference }` → `canvas.set_conflict_focus_keys(cx,
     Some(vec![subject, reference]))`.
   - `Delete { subject, reference }`:
     ```rust
     let diagram = self.tabs.active_tab().map(|t| t.key.clone()).unwrap_or_default();
     let op = waml::ops::Op::PlaceRm {
         diagram,
         subject_slug: subject,
         reference_slug: reference,
     };
     match waml::ops::apply(&self.bundle, &[op]) {
         Ok(new_bundle) => {
             self.bundle = new_bundle;
             self.model = waml::parse::build_model(&self.bundle);
             // re-solve the active diagram view in place (camera held) — same
             // path as the drag-place apply at app.rs:1456
             if let Some(active) = self.tabs.active_tab().cloned() {
                 if let Some(v) = self.views.get_mut(&active.id).and_then(|v| v.downcast_diagram()) {
                     v.resolve_active(cx, &body, &self.model);
                 }
             }
             self.sync_conflict_badge(cx);
             // Refresh the OPEN list (stays open) or dismiss if now empty.
             let conflicts = self.ui.widget(cx, ids!(canvas))
                 .borrow::<crate::canvas::GraphCanvas>()
                 .map(|c| c.conflicts()).unwrap_or_default();
             if conflicts.is_empty() {
                 if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
                     pr.close(cx); // or show_at-supersede; use PopupRoot's existing dismiss
                 }
             } else {
                 self.open_conflict_list(cx, conflicts); // supersede re-anchored, stays open
             }
         }
         Err(e) => log!("place.rm failed: {e:?}"),
     }
     ```
     (If `PopupRoot` has no public `close`, dismiss by letting the badge hide and
     calling `show_at` only when non-empty — an empty set simply leaves the old
     surface up until the next event; PREFER adding a small `PopupRoot::close(cx)`
     that resets the active surface + emits `Closed{Dismissed}` if none exists,
     mirroring the supersede path. Verify against `root.rs` before choosing.)
   - Confirm `&body` is the same `WidgetRef` the drag-apply path uses at `:1461`
     (`resolve_active(cx, &body, &self.model)`); reuse it.

**Tests (`#[cfg(test)]` in `app.rs`, or a pure helper module if `App` is not
unit-constructable — check how existing app tests are shaped; if `App` needs a
live `Cx`, extract a pure mapping helper and test THAT):**
- `conflict_delete_maps_to_place_rm` — a pure helper `place_rm_for(diagram:
  &str, action: &ConflictListAction) -> Option<Op>` (or inline construction)
  that, given `ConflictListAction::Delete { subject: "order", reference:
  "payment-gateway" }` and `diagram = "dia"`, yields
  `Op::PlaceRm { diagram: "dia", subject_slug: "order", reference_slug:
  "payment-gateway" }`. Assert field-for-field.
- `conflict_count_drops_after_place_rm` — a pure end-to-end at the ops layer
  (does not need `App`): a `layout_diagram` with two contradictory placements →
  `solve`/`project_conflicts` reports N conflicts → `apply` the `Op::PlaceRm`
  built from a `Delete` action → re-solve → conflict count < N. (If wiring a full
  solve into a unit test is heavy, assert instead that the removed placement is
  gone from the re-serialized bundle — the solver behaviour is already covered by
  Task 1 + scene tests.)

**Green:** full gate. Interactive behaviour is deferred (below).

---

## Interactive sign-off (deferred to redoz@)

Screenshots cannot drive the drag that authors a conflict, so the end-to-end
behaviour is NOT machine-verifiable in this plan and is explicitly left for the
user to validate live with `scripts/run-native.ps1 -Optimized` (per spec
§"Interactive sign-off"):

1. Load the dense fixture (`domain-model.md`, 33 nodes); author two contradictory
   placements by drag.
2. Open the toolbar conflict badge — confirm the `message-square-warning` glyph +
   bare count (no `! `), and the grouped rendering: one `SceneRelation` per line,
   dropped-first then `conflicts_with`, a hairline divider between conflicts.
3. Trash one constraint → the line + its conflict shrink, the list stays open,
   the badge count decrements, the diagram re-solves.
4. Click a row body (not the trash) → the canvas fades to just that constraint's
   two nodes.
5. Esc / outside-click → dismiss; deleting the last conflict hides the badge and
   closes the panel.

Do NOT claim visual verification in the implementation commits — only the gate is
verified per unit.

## Task summary

- Task 1: `Op::PlaceRm` + native-only DTO arm (waml + waml-ops-dto)
- Task 2: `Icon::MessageSquareWarning` catalog glyph (icons.rs, all invariant sites)
- Task 3: Badge glyph row (conflict_badge.rs)
- Task 4: `ConflictList` popup surface + pure helpers (popup/conflict_list.rs)
- Task 5: `PopupRoot` integration (popup/root.rs)
- Task 6: `App` wiring + `ConflictListAction` + canvas key-focus (app.rs, canvas.rs)
