# Narrow-viewport chrome

Status: designed, not implemented.

## Goal

waml-editor's chrome is built for a 1280x840 desktop window and has no
responsive behaviour of any kind: every caption and dock dimension is a fixed
constant. On a phone-width viewport (~390px) three things are broken at once.

1. The wordmark spans the full 66px caption band on the left, so it steals
   ~92px from *both* caption rows -- including the tab row, which does not need
   it.
2. The tab strip does not scroll. Overflow simply fades out (`EDGE_FADE`, 48px
   over 4 steps, `doc_tabs.rs:419`). A tab card measures ~165px (icon 15 +
   `ICON_GAP` 6 + up to `MAX_TITLE_CHARS` 18 of 10px label + `TEXT_PAD` 12 +
   `CLOSE_GAP` 10 + `CLOSE_W` 32), so a narrow viewport holds under two cards
   and any further doc is not merely cramped -- it is unreachable.
3. The docks are `Size::Fixed` slots driven by `ProjectTree::slot_width()` and
   `Inspector::slot_width()` (`app.rs:834`). At 390px, one open dock buries the
   canvas; both open leave it negative.

This spec introduces the editor's first breakpoint and reshapes the caption,
the tab strip and the docks below it.

## Decisions

- **Width threshold, not platform.** Narrow mode keys off viewport width alone,
  so a desktop window dragged narrow enters it. Platform-gating (wasm + touch)
  was rejected: a layout you can only observe on a phone is a layout you cannot
  iterate on.
- **Caption *and* docks in one pass.** The dock fix lands in
  `sync_dock_slots` -- the same function the caption's `tree_gap` sync already
  calls -- so splitting the work means editing it twice.
- **Tabs collapse to a chip + switcher, not a one-tab cap.** Capping narrow to
  a single open doc was the starting proposal and is rejected: the set of open
  documents would change meaning with window width, so dragging a desktop
  window narrow would have to close documents that then do not return when it
  is widened. That reads as a bug. A strip that can hold exactly one card is
  also not a tab strip; it is a title.
- **Docks overlay, mutually exclusive.** The canvas is the thing the user is
  looking at, so it keeps full width at all times. Full-viewport drawer panels
  and clamped-but-still-reserved slots were both rejected (the latter on
  arithmetic: a 40% clamp leaves ~78px of canvas at 390px).

## Design

### Breakpoint

`App` gains a `narrow: bool` derived from viewport width, recomputed on the
path that already runs `sync_dock_slots` so there is exactly one relayout
trigger.

Threshold **640px entering, 680px leaving**. The 640 comes from the desktop
caption's fixed chrome cost -- logo ~92 + `TREE_BTN_W` 32 + `INSPECTOR_BTN_W`
32 + `WINDOW_BUTTONS_W` 138 = 294px -- against the ~330px two tab cards need:
the layout stops working under ~624px. The 40px of hysteresis is not cosmetic.
Without it, a window resting on the boundary re-forks the entire chrome every
frame.

### Caption layout

The band stays 66px and keeps both rows. Only the horizontal composition
changes.

Makepad cannot re-parent a widget at runtime, so the logo is **two `LogoMark`
instances with one visible at a time**:

- `wordmark` -- the existing full-band instance, hidden on narrow.
- a new instance mounted as `title_row`'s first child at 44x25, shown on
  narrow. 44/25 holds the documented ~1.749 content aspect.

Both instances need their own `WindowDragQuery` arm and `logo_action` wiring.
That duplication is the price of the no-reparent constraint and should be
called out in the DSL comment so it is not later "cleaned up" into one.

Nesting the narrow logo inside `title_row` is safe. The caption comment block
(`app.rs:56-66`) reads as though interactive caption widgets must be direct
children, but `tree_btn` is already nested in `tab_row` and works, because the
rule is really "must be client-ized in the `WindowDragQuery` handler so its
rect is not an OS drag region". The narrow logo joins on those terms. A short
logo left as a direct caption child would not help regardless: in a `flow:
Right` band it still reserves its column for the full height, which is exactly
the space the tab row is trying to reclaim.

Narrow row composition:

- `title_row`: `logo(44) | agent_mark(0w) | menu_btn(30) | model_name(Fill)`
- `tab_row`: `[T](30) | tree_gap | chip(Fill) | [I](30)`

`tab_row` now begins at x=0, giving the chip ~318px on a 390px viewport.

### The top rule

`doc_tabs::rule_x_end` adds `WINDOW_BUTTONS_W` (138) to the rule's right
overshoot unconditionally. On narrow that overshoot must be 0.

Note while touching this: `windows_buttons` is declared `visible: false`
(`app.rs:236`) and no code path sets it true, so the 138px reservation may
already be wrong on the web build independently of this work. Establish which
before changing the constant's use -- this spec only requires the narrow case
to be correct, and should not silently alter desktop.

### Chip and switcher

On narrow, `DocTabs` draws **only the active document's card**, `Fill` up to a
320px cap, keeping its icon, title and close button. Because one card owns the
whole strip, `MAX_TITLE_CHARS` goes *up* on narrow rather than down.

- Tap the card body -> switcher opens.
- Tap the card's `x` -> close the active document.

Keeping close on the chip is what lets `PopupItem` stay unchanged: no per-row
secondary action is needed in the switcher, so no new field on the shared popup
item type.

The switcher is the existing `MenuPopup`, not a new surface: opened with one
row per open document (icon + title), the active row marked via
`open_marking`, anchored at the chip's bottom-left. Scrolling for long document
lists already exists in `LinearGeom` (`set_max_height`, `scroll`,
`thumb_rect`). Because the burger already owns a `MenuPopup`, the switcher's
`PopupResult` is disambiguated by stashing which menu opened it -- the pattern
the node context menu already uses.

### Overlay docks

On narrow, `sync_dock_slots` writes `Size::Fixed(0.0)` to both `left_slot` and
`right_slot`. Each panel instead draws `draw_abs` at its own `slot_width()`,
spanning caption-bottom to statusbar-top: the tree from x=0, the inspector
flush against the right edge. The canvas underneath keeps full width.

Opening one dock closes the other.

Dismissal: a tap on the canvas outside an open dock closes it. Containment must
be tracked off `MouseMove`, **not** `Hit::FingerHover` -- both panels have child
widgets that claim the hover first, which is the failure the scrim
hover-arbiter fix already caught once.

### The lit-state seam

`[T]` and `[I]` currently derive their lit state from `lw > 0.5` / `rw > 0.5`
(`app.rs:856`, `app.rs:877`), chosen deliberately so the glyph cannot disagree
with the layout. Under overlay docks those widths are 0 while the panel is
open, so that derivation breaks.

Lit state moves to an explicit panel-open flag, and that flag becomes the
shared source of truth for **both** modes -- wide mode's slot width is then
derived from it rather than the reverse. Keeping two different sources of truth
per mode would reintroduce exactly the disagreement the original comment was
guarding against.

## Non-goals

- Statusbar, tool dock and popup sizing are not audited for narrow. The popups
  already carry their own clamp logic and there is no evidence yet that they
  break.
- No touch-specific gestures. Narrow mode is a layout fork; the touch input
  work is separate and already landed.
- Horizontal tab-strip scrolling is not built. The chip replaces the need for
  it on narrow, and wide mode keeps today's fade behaviour.
- No change to how documents open, close or persist. Narrow changes how they
  are *reached*, not what is open.

## Risks

- **Two logo instances drift.** Two `LogoMark`s with separate drag-query arms
  can fall out of sync in hover, click routing or styling. Mitigated by the DSL
  comment and by driving both from the same `logo_action` handler.
- **Overlay hit routing versus `PopupRoot`.** A `draw_abs` panel over the canvas
  must not let clicks fall through, and `PopupRoot` has already needed an
  explicit underlay-swallow fix once. If overlay routing fights the popup
  authority, the fallback is full-viewport drawer panels, which remove the
  partial-width case entirely.
- **Hysteresis masking a layout bug.** A chrome that re-forks at a threshold can
  hide an error that only appears mid-transition. Verification must include
  dragging *through* the boundary, not only sampling each side.
- **`WINDOW_BUTTONS_W` may already be wrong.** See "The top rule". Investigating
  it could widen scope; if it proves wrong on web, fix it separately rather
  than folding it in here.

## Verification

Unit:

- breakpoint transitions, including hysteresis (enter at <640, leave at >680,
  no flap between).
- switcher item construction from the open-document set, active row marked.
- `rule_x_end` overshoot is 0 on narrow, unchanged on wide.
- lit-state flag drives both slot width (wide) and glyph state (both modes).

Interactive, at a forced 390px window, via the established pid-scoped
synthetic-click recipe:

- chip opens the switcher; selecting a row switches document.
- chip `x` closes the active document.
- `[T]` opens the tree over the canvas; `[I]` closes it and opens the
  inspector.
- canvas tap outside an open dock dismisses it.
- drag the window from 900px to 380px and back; chrome forks cleanly in both
  directions and nothing is left mid-state.

Web: playwright headless probe for console panics after the layout fork, since
the fork adds a code path the wasm build reaches first.
