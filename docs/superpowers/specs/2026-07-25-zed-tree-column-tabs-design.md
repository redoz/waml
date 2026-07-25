# Zed-style tree column + content-width tabs

Date: 2026-07-25
Branch: `title-tab-two-row`

## Goal

Today the tree panel sits *under* the doc tabs: the tab strip spans the full window
width and the tree floats beneath it as a HUD card. Make the tree a real left column
whose top-right corner is marked by a toggle button in the caption's second row, so the
tab cards begin where the content area begins — the Zed reading.

Target layout, expanded:

```
+---------------------------------------------------------------+
|#[LOGO]#=##models/orders.waml####################_###[]###X#####|  row 1, 34px
|#[LOGO]#--------------------------------------------------~~####|  rule at y=34, FULL width
|#[LOGO]#########(gap)###########[T]#| orders |x| schema |x|#####|  row 2, 32px
|####################################+--------------------------+  y=66
|#  PROJECT TREE                     |                          |
|#   > src                           |     C A N V A S          |
+------------------------------------+--------------------------+
   0                              280
```

Collapsed:

```
+---------------------------------------------------------------+
|#[LOGO]#=##models/orders.waml####################_###[]###X#####|
|#[LOGO]#--------------------------------------------------~~####|
|#[LOGO]#[T]#| orders |x| schema |x|############################|
+---------------------------------------------------------------+
|                                                               |
|                       C A N V A S                             |
```

`#` = `atlas.field_bg`. `[LOGO]` spans both rows and is a keep-out zone. `[T]` is the
tree toggle, always immediately left of the first tab card. `~~` = the existing 48px
alpha fade before the window's right edge.

The invariant that makes this coherent: `[T]` marks the point where the `field_bg`
chrome mass steps in from full-width to column-width. One number — the tree's slot
width — positions the gap, `[T]`, and every tab card.

## Decisions

Settled during brainstorming, with the rejected alternative noted where it was close:

1. **The two-row caption stays.** Tabs remain inside the caption bar widget tree rather
   than moving into the body. Rejected: relocating the tab strip below the caption
   (truest to Zed, but discards the reviewed two-row work, the top-rule-to-window-edge
   overshoot, and the 66px band).
2. **The toggle is binary: `Pinned` <-> `Flag`.** The tree never enters `Peek` again.
   Rejected: cycling all three states, which would leave the tab strip anchored left
   while a peeking tree body overlapped the content beneath it.
3. **The pinned tree is a flush column**, top edge at y=66, full height, flush left.
   Rejected: keeping the floating inset card, which preserves the exact "floats under
   the tabs" quality this change exists to remove.
4. **The flag spine is deleted.** Collapsed means zero pixels — no 28px gutter, no
   sideways label. `[T]` is the only affordance.
5. **The tab band and its top rule stay full width.** Only the tab cards and `[T]`
   indent. Rejected: starting the rule at the column's right edge, which is more
   literally Zed but introduces a rule/divider corner to solve.
6. **No divider between column and canvas.** Separation is carried by `field_bg` versus
   canvas ground. Because the tree card is already `field_bg` (`tree_panel.rs:73`), the
   caption band and the tree column merge into one continuous chrome mass.
7. **Expand/collapse snaps.** No easing for now — prove the layout first, animate later.
8. **The glyph is the existing `Icon::ListTree`.** No catalog work. This is the same
   glyph the flag spine already used (`tree_panel.rs:614`), so `[T]` inherits the
   spine's visual identity rather than inventing one. Rejected: adding a `panel-left`
   glyph, which reads better as "sidebar" but costs the catalog-invariant dance.
9. **The tree starts expanded** when a model opens. Deleting the spine removes the only
   ambient hint the tree exists, so opening expanded pays that back.
10. **`[T]` is hidden until a model opens**, matching the burger (`app.rs:118`).
11. **The inspector is untouched.** It keeps its floating card and all three dock
    states. The asymmetry is deliberate.

## Design

### Layout

**`project_tree` moves out of `peek_layer` and into `left_slot`.** The tree never peeks
now, so it stops being an overlay and becomes a real layout child: `width: Fill,
height: Fill`, no margins. Its top lands at the body's top (y=66) and its bottom at the
window bottom for free, because `main_column` already begins below the caption. The
`left_peek_wrap` wrapper is removed; `right_peek_wrap` (inspector) stays.

`left_slot` keeps its runtime-driven width (`sync_dock_slots`, `app.rs:707`), which
already reads `ProjectTree::slot_width()` and yields 280 when `Pinned`, 0 otherwise.
Expanded panels already zero their own margins at draw time (`tree_panel.rs:654-657`),
so the reserved 280 and the drawn 280 already agree; nothing to reconcile.

**`tab_row` gains two children ahead of `doc_tabs`:**

```
tab_row := View{
    tree_gap := View{ width: 0.0, height: Fill }   // runtime-driven
    tree_btn := IconButton{ ... visible: false }   // Icon::ListTree
    doc_tabs := DocTabs{ width: Fill, height: Fill }
}
```

`tree_gap.walk.width` is driven from the same tree slot width that drives `left_slot`,
minus `[T]`'s own width and its inset, so that `[T]`'s right edge lands on the column's
right edge. Because `tab_row` is `flow: Right`, `[T]` and every tab card follow
automatically; no tab-side offset arithmetic is needed.

`TAB_LEFT_INSET` (currently 10, aligning the first card with the burger glyph above)
becomes the gap between `[T]` and the first card instead. The burger alignment it
encoded is superseded by this change and is not preserved.

### The top rule

`DocTabs::draw` paints its band bg and top rule off its own turtle rect
(`doc_tabs.rs:593-616`). With `doc_tabs` now starting to the right of `[T]`, that turtle
no longer begins at the window's left edge.

- **Band bg** needs nothing. `caption_bar` already fills `field_bg` (`app.rs:66`), which
  is the same ink, so the region left of `doc_tabs` in row 2 is already correct.
- **Top rule** gains a `left_overshoot: f64` field on `DocTabs`, the twin of the
  existing `WINDOW_BUTTONS_W = 138` right overshoot. `x0` becomes
  `(rect.pos.x - left_overshoot).round()`. The app sets it to the strip's distance from
  the window's left edge, so the rule spans full width in both states. This works for
  the same reason the right overshoot does: `caption_col` and `tab_row` both set
  `clip_x: false`.

The rule stays a pixel-snapped plain `DrawColor` quad with its fade faked as
`EDGE_FADE_STEPS` stacked 1px segments. An SDF fill under ~2px has zero AA coverage on
this fork, and a 3px SDF rule was already rejected as too thick.

### State model

`dock.rs` gains one transition so the tree can toggle directly, leaving the inspector's
three-state cycle untouched:

```rust
pub enum DockEvent {
    FlagActivate,
    PointerLeft,
    PinToggle,
    /// Caption tree toggle: Flag <-> Pinned, skipping Peek entirely.
    Toggle,
}

// in `next`:
(Flag, Toggle) => Pinned,
(Pinned, Toggle) => Flag,
(Peek, Toggle) => Flag,   // unreachable for the tree; defined for totality
```

`ProjectTree` then sheds its `Peek` machinery: the flag-spine draw branch and its
`flag_btn` child, the `MouseMove` peek-hover containment arm
(`tree_panel.rs:867-890`), the `PeekTimer` field and its `dock_frame` advance loop, and
the `DockState::Peek` match arms in `draw_walk`. `dock.rs` keeps `PeekTimer` and
`peek_hover_span` — the inspector still uses both.

`ProjectTree` initialises its own `dock` field to `Pinned` on model open. `DockState`'s
`#[derive(Default)]` stays `Flag` — the inspector depends on it.

### Wiring seams

- `App::sync_dock_slots` already computes the tree's slot width each `handle_event` and
  writes `left_slot.walk.width`. It gains a second consumer: `tree_gap.walk.width` and
  `DocTabs::left_overshoot`, from the same value, behind the same change-threshold
  guard so the redraw cost is unchanged.
- `[T]`'s click is read from `Event::Actions` via `IconButton::clicked`, and applies
  `DockEvent::Toggle` to the tree.
- `[T]`'s lit state uses the existing `IconButton::set_active` (accent wash at 16%) —
  lit when expanded. One glyph, not a pair.
- `[T]`'s visibility is toggled alongside the burger in `show_editor` /
  `show_start_screen`.

### The tree header's pin button

With the toggle in the caption, the tree header's `pin_btn` (`tree_panel.rs:111`) is a
second control for the same binary state. **This design removes it.** `[T]` is the sole
affordance, which is what Zed does — its project panel header carries no pin.

This is the one decision made without explicit sign-off. It is cheap to reverse: keeping
`pin_btn` as an in-panel collapse button means fixing its glyph to a single icon and
pointing its click at `DockEvent::Toggle`.

## Non-goals

- Animating the width change. Snap now; ease later if it reads badly.
- Any change to the inspector's card styling or dock states.
- A divider line, or a rule/divider corner treatment.
- Using the dead gap between the logo and `[T]` when expanded. It stays empty.
- Any change to the web renderer.

## Risks

- **Caption chrome breakage is gate-blind.** Blanked or dead caption chrome produces
  clean stderr and green tests. Only launching and screenshotting catches it. This is
  the primary risk in the whole change.
- **`clip_x` and the left overshoot.** If any ancestor of `doc_tabs` clips on x, the
  rule's leftward overshoot is silently truncated and the rule appears to start at the
  tab strip. Verify visually in both dock states.
- **Moving `project_tree` between parents.** A custom widget mounted as a DSL child is a
  dead, invisible node unless its `script_mod(vm)` registers before the consuming
  module's. `ProjectTree` is already registered, but the reparent must not disturb that
  order in `app.rs`.
- **Turtle balance on the tree's draw path.** The docked-panel `Flag` branch previously
  caused an unbalanced turtle stack by not looping `draw_walk` to completion, which
  blanked the entire caption and both panels. Deleting the `Flag` branch must leave
  every remaining `draw_walk` looped to `None`.
- **Inline `font_size:` / `FontMember` in chrome sources fails `chrome_typography_gate`.**
  No new type is planned here, so no new `mod.fonts` role should be needed.

## Verification

1. `cargo test --workspace` — baseline is 372 passed / 0 failed.
2. `cargo clippy --workspace --all-targets -- -D warnings` — clean. Two benign
   fork dup-package warnings are expected. Note that `dead_code` is promoted to a hard
   error here, so code orphaned by the `Peek` removal must actually be deleted.
3. Unit tests in `dock.rs` for the new `Toggle` transitions, alongside the existing
   transition-table tests.
4. Launch `scripts/run-native.ps1` **from this worktree** (it builds the checkout the
   script lives in, not the cwd) and screenshot by specific pid — capturing or killing
   by process name hits the user's own editor session. Confirm, in both dock states:
   - caption text renders at all (the real smoke test),
   - the top rule reaches both window edges,
   - `[T]` sits flush against the first tab card and against the column's right edge,
   - the tree column is flush at top, left, and bottom with no window-bg frame,
   - collapsing leaves zero tree pixels.
5. Interactive click-test, still owed from the prior two-row caption task and not yet
   done: burger drop-down, tab hover/press, and now `[T]` toggle round-trips.
