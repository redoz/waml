# A waml-owned tree row list

**Date:** 2026-08-05
**Status:** Approved design, ready for planning

## Problem

`tree_panel.rs` (2220 lines) is built on makepad's `FileTree`, but almost
everything visible in the panel is layered *over* that widget rather than
provided by it:

- **Selection is entirely ours.** The fork exposes no public API to select or
  highlight a row, so the app drives it (`sync_document_shell` ->
  `set_selected_document`) and the panel paints `draw_selection` as an
  immediate-mode overlay.
- **Fold is ours.** `auto_toggle_folders: false` disables the built-in
  fold-on-click; we hand-draw `DrawChevron`, cache each row's chevron rect at
  draw time (`chevron_rects`), and hit-test chevron-vs-row-body ourselves.
- **Fold animation is read back, not owned.** We poll `ft.folder_opened(id)` and
  `ft.current_scale()` per row to rotate the chevron and derive a `scale` that
  every hand-drawn mark multiplies in by hand.
- **Tap counts are ours.** `pending_tap_count` / `pending_click_abs` come from a
  raw `FingerDown`, because the fork's click actions don't carry them.
- **Icons are ours.** The built-in folder box is zeroed to transparent and
  `draw_row_icon` paints a per-`TreeKind` glyph in the reclaimed slot.
- **Identity is ours.** `id_to_key`, `id_to_concept`, `openable_ids` and
  `directory_addresses` exist solely to round-trip our `RowId` keys through
  makepad's `LiveId` row identity.

The result is two sources of truth -- fork-held row state versus our overlay --
kept in agreement by convention. That convention is what fails. The bug fixed in
`ddd4fb66` is the archetype: `NavigationTarget::Directory` opened a folder tab
without running `sync_document_shell`, so the tree kept highlighting the
previously active file. The highlight lives outside the widget, so any code path
that forgets the sync goes stale silently.

What we still genuinely use from `FileTree` is small: `begin_folder` /
`end_folder` / `file`, `last_node_drawn`, `folder_opened`, `current_scale`,
`set_folder_is_open`, three click actions, scroll, and the fold clock. Row
geometry is already ours -- `ROW_HEIGHT: f64 = 27.0` is our constant, and every
overlay mark positions itself from `cx.turtle().pos()` captured *before* the
fork draws the row (`tree_panel.rs:813`).

`FileTree` is used by `tree_panel.rs` only. Nothing else in waml touches it,
apart from test scaffolding in `app/tests/` and one stale comment in
`app/navigation.rs:500`. This is a clean single-consumer removal.

## Goal

Replace `FileTree` with a waml-owned immediate-mode tree row list, so that
selection, fold state, scroll and hit-testing live in one place that also
computes the rects used to draw. The dual source of truth becomes structurally
impossible rather than merely well-commented.

waml keeps its zero-upstream-lines property: this **removes** dependence on
forked upstream code rather than vendoring any. No relicensing, no
`THIRD-PARTY-NOTICES`, waml stays plain MPL-2.0. We continue to depend on
`makepad-widgets` as a normal dependency (`ScrollBars`, `View`, the widget
system) -- that is unchanged and not at issue.

## Decisions

These were settled during brainstorming and are not open in planning:

| Decision | Choice | Why |
| --- | --- | --- |
| Fold animation | **Animated, ours** | Keep today's feel; we own the clock instead of reading it back. |
| Scope | **Strict parity, plus hover** | A regression and a new interaction model must not land together. Keyboard navigation is a separate spec, cheap once we own the list. |
| Fork commits | **Revert after waml lands** | Reducing fork surface is the point; three bespoke APIs left behind are future rebase conflicts. |
| Structure | **Pure geometry core + thin shell** | Proven by `popup/menu.rs`; makes the core `Cx`-free and unit-testable. |
| Virtualization | **Out of scope** | Parity means same rows, same pixels. The tree build is already bounded (`5b0098a1`). |

## Architecture

Three units replace today's single `tree_panel.rs`.

### `tree_layout.rs` -- pure, no `Cx`, no makepad draw types

Owns all state:

- the flattened visible-row list (key, depth, kind, title, flags), derived from
  `TreeNode` roots plus the open-set
- per-folder fold amount (0..1)
- scroll offset, with clamping
- the selected key
- the hovered key
- geometry: `row_rect(i)`, `row_at(pos)`, `chevron_at(pos)`, `content_height()`
- `hit(pos) -> Chevron(key) | Row(key) | None`

One method advances all fold amounts given a `dt` and reports whether another
frame is needed.

`tree.rs` is already makepad-free (`TreeNode` carries key, title, kind,
`is_directory`, `openable`, `view_degraded`, nesting), so the core consumes it
directly. Rows are addressed by `RowId` key throughout; the `LiveId` bridge
(`id_to_key`, `id_to_concept`, `openable_ids`, `directory_addresses`) is
deleted.

### `tree_row_draw.rs` -- stateless drawing

Functions painting one row's marks into a rect the core computed: highlight,
hover tint, chevron, icon, label, diagnostic marker. Today's `draw_row_*`
functions move here nearly unchanged -- they already take `row_top` + `scale`,
so the signature becomes `rect` + `scale`. One addition: the row label itself
(`DrawText`), previously the fork's job.

### `tree_panel.rs` -- the widget

Shrinks to: hold a `TreeLayout`, own `ScrollBars` and a clipped view, run the
draw loop, route events into the core, emit the existing actions.

The public API the app already calls -- `set_view_with_fold_reset`,
`set_selected_document`, `set_selected_key`, `set_scope_title`,
`set_view_mode`, `dock_state` -- stays identical, so `app/shell.rs` and
`app/navigation.rs` do not change.

## Fold animation

The fork animates `opened` 0<->1 over **0.2s** with `Ease.ExpDecay`
(`d1: 0.80, d2: 0.97` closing; `0.82 / 0.95` opening --
`makepad/widgets/src/file_tree.rs:218-236`), and a row's effective scale is the
product down the ancestor stack, culled at `opened <= 0.001`
(`file_tree.rs:801`).

We reproduce that: `TreeLayout` holds `fold: HashMap<RowId, f32>`, advanced by a
`NextFrame` tick, and the flatten walk multiplies down the stack with the same
cull threshold. Parity target is the same 0.2s ExpDecay curve. If that exact
ease is not directly available, use the closest available ease at 0.2s and
**flag it as a visual-check item** -- do not silently substitute a different
feel.

Two things improve by construction:

1. Today a *culled* folder is forgotten by the fork, so `folder_opened` reports
   it closed -- a caveat documented at `tree_panel.rs:849`. In the new core the
   fold amount is authoritative and never forgotten.
2. The cull becomes a real skip: rows under a fully-closed folder are never
   flattened, rather than drawn and discarded.

## Scroll

Keep `ScrollBars`. It is a plain makepad widget already instantiated directly in
`inspector_panel.rs:109`, `diagram_properties.rs:26` and `app.rs:399`, so it
costs no vendored lines and preserves the panel's scroll feel -- including the
flush-right bar that the padding comment at `tree_panel.rs:150` is deliberately
tuned for.

`ScrollBars` owns wheel/drag input and bar geometry. The core keeps the scroll
offset for hit-testing: `row_at` subtracts it and rejects rows outside the
clipped viewport, mirroring `popup/menu.rs:152-172`.

## Events and actions

The outward contract is unchanged. `ProjectTreeAction::{Navigate, ContextMenu,
ToggleViewMode}` stay as they are, `row_navigation` (`tree_panel.rs:374`) moves
across untouched, and `app.rs`'s handlers do not change.

Preserved semantics:

- **Folder chevron** -> fold/unfold locally, no action emitted.
- **Folder body** -> `Navigate(Directory, Preview)`.
- **File row**, openable -> `Navigate(Document, ...)`; `Preview` on single
  click, `Persistent` on `tap_count == 2`. Non-openable rows emit nothing.
- **Right-click**, openable rows only -> `ContextMenu { key, anchor }`.
- Reveal pulse, scope title, degraded-chain marker, projected/raw toggle:
  unchanged.
- Hit-testing keys on **`FingerDown`**, and rows scrolled outside the clipped
  viewport are not hittable even though their band maps -- same rule as
  `popup/menu.rs:152`.

What simplifies: today the panel stashes `pending_tap_count` and
`pending_click_abs` on `FingerDown` (`tree_panel.rs:936-939`), then replays that
position against `chevron_rects` when the fork's action arrives. Two hops, two
pieces of retained state, plus a documented fallback for a missing cached rect.
The new core does it in one hop via `hit(pos)`, so `pending_tap_count`,
`pending_click_abs`, `chevron_rects` and `chevron_hit` all delete, and the
missing-rect caveat disappears with them.

## Hover

The one addition beyond parity.

- Hovered row tracked in `TreeLayout` as `hover: Option<RowId>`, updated from
  **`MouseMove` containment**, not `Hit::FingerHover`. Containment survives an
  arbiter handing the hit elsewhere (lesson from `bc53c22`).
- Painted as a tint *beneath* the selection highlight, so a hovered-and-selected
  row still reads as selected. New `draw_hover` token off the atlas, weaker than
  `atlas.selection`.
- Clears on `MouseMove` outside the panel and on any scroll -- a row that slides
  out from under a stationary cursor must not stay lit.
- Rows set a pointer cursor on enter and reset on leave via the `crate::cursor`
  helpers. This closes a known gap: file-tree rows are recorded as still
  uncovered for cursor reset in the fork, precisely because we could not reach
  them.

Hover keys on `RowId`, so a row shrinking mid-collapse keeps or loses hover by
its current rect with no special case.

## Sequencing

Strict ordering. Step 2 is the gate that keeps a mistake from being masked by a
simultaneous fork change.

1. **Land the waml side** -- `tree_layout.rs`, `tree_row_draw.rs`, rewritten
   `tree_panel.rs` -- still on the current fork pin. `FileTree` stops being
   referenced; the `LiveId` bridge and the click-stash state delete. Remove the
   stale `FileTree` comment at `app/navigation.rs:500`, which documents an
   immediate-mode quirk of a widget we no longer use.
2. **Visual verification** against the current build, before anything on the
   fork moves. If it is wrong, the fix is in waml with the fork untouched.
3. **Revert the three fork commits** on `redoz/makepad`'s `waml` branch --
   `fbb881c5` (app-owned folder toggles), `2ad35404` (animated folder open
   amount), `92df3316` (fold scale + `last_node_drawn`) -- returning
   `file_tree.rs` to stock.
4. **Bump the waml pin to the new fork SHA.** A SHA, never a branch tip.
5. **Re-gate and re-verify visually**, since step 4 rebuilds against different
   fork code.

## Testing

### Unit tests -- `tree_layout.rs`, no `Cx`

This is where the design pays off: most of these cases are untestable today
because the state lives in the fork.

- flatten respects the open-set and the `<= 0.001` cull
- fold amounts advance and settle at exactly 0 / 1
- a row's scale is the product of its ancestors' fold amounts
- `row_at` / `chevron_at` map bands correctly and reject positions outside the
  clipped viewport
- scroll clamps to `[0, max_scroll]`
- hover clears on scroll and on leaving the panel
- selection survives a re-projection that keeps the key (the
  `set_view_with_fold_reset` path)
- fold state survives a mode flip -- `RowId` is stable across re-projection by
  design (`tree.rs:12-16`)

### Widget tests

Keep today's coverage in `app/tests/navigation.rs`, including
`opening_a_folder_highlights_its_row`. The synthetic-action scaffolding changes
shape: tests currently fabricate `FileTreeAction::FolderClicked(LiveId)`
(`tree_panel.rs:1918`) and mount a real `FileTree`
(`mounted_project_tree_test_context`, `tree_panel.rs:1465`). These become direct
`TreeLayout` hits or `ProjectTreeAction` assertions -- simpler, and no longer
dependent on fork internals.

### Gate

`cargo test --workspace`, plus the vscode extension's test / lint / build.

### Visual verification -- deferred to the user, not automated

Nothing in the gate can see pixels, and a GUI check embedded as an
implementation task stalls indefinitely. The implementation plan must carry
these as **owed sign-off items**, run via `run.ps1 -Title tree-owned-list`:

1. Rows, indent, glyphs and text baseline identical to the current build
   (side-by-side, same fixture).
2. Fold/unfold motion matches the 0.2s ExpDecay feel; no popping, no stuck
   partially-folded row.
3. Selection tracks the active tab -- file rows *and* folder rows.
4. Hover tint reads correctly, including hovered-and-selected, and clears on
   scroll-out.
5. Cursor resets when leaving the panel (no leak).
6. Scrollbar sits flush right, scrolls at the same feel, and rows scrolled out
   are not clickable.
7. Projected/raw toggle, reveal pulse, degraded-chain marker and right-click
   menu all still work.
8. Re-run 1-7 after the fork pin bump (step 5).

## Out of scope

- Keyboard navigation (arrow keys, Enter, type-ahead). Separate spec; cheap once
  the list is ours.
- Row virtualization. A natural follow-up once the core hands out rects.
- Any change to `tree.rs`, the projection, or the folder-view chain.
