# View-owned right dock — design

**Date:** 2026-07-25
**Status:** approved (design), plan pending
**Branch:** `title-tab-two-row`

## Problem

The project tree has been reintegrated into the two-row Zed-style caption: it is a
flush `left_slot` column with a binary `Flag`/`Pinned` dock state, toggled by an
anchored `[T]` `IconButton` in the tab row. The inspector has not followed. It
still floats in `peek_layer.right_peek_wrap` with all three dock states, its own
flag spine, a pin button, a peek auto-collapse timer and a scrim.

Two things need to happen, and they are not the same thing:

1. **Mechanically**, the inspector should become the right-hand twin of the tree:
   a flush column in `right_slot`, binary open/closed, driven by a caption toggle
   pinned to the right of the tab row.
2. **Structurally**, the inspector is not app chrome — it belongs to the document
   view. A `DocView` should be able to declare *that it has a right-hand docked
   panel* and *which glyph its toggle wears*, and to ask for that panel to be
   opened in response to a user action. The app orchestrates everything else.

The second point is the reason this is a spec rather than a patch. The tree is
app chrome and will stay that way; the right dock is per-view, and the seam for
that already exists (`BodyChrome`, `ViewOutcome`) and just needs a third member.

Separately, the shared `IconButton`'s active state currently reads as a wash,
which is the same visual as hover. Toggles want a distinct resting-on state.

## Decisions

Settled in brainstorming, recorded here so the plan does not relitigate them:

- **Dock open/closed state is app-global.** One `DockState`, shared by every
  view. Switching tabs leaves the panel as the user left it. Rejected: per-tab
  state (same button meaning different things per document, a state map to
  maintain, no caller that wants it yet). KISS.
- **`[I]` is the last child of `tab_row`.** A literal mirror of `[T]` at the
  other end, with the `Fill` tab strip absorbing the slack between them, so the
  button never moves when tabs open or the tree column expands.
  - *Known cosmetic:* `tab_row`'s right edge is 138px inboard of the window's
    right edge, because `windows_buttons` (3 × 46) sits after `caption_col` in
    the caption bar's `flow: Right`. So `[I]` does not line up with the
    inspector column it toggles, and a bar-coloured void sits to its right in
    the lower band. Accepted for now. The fix, if it reads wrong on screen, is
    to lift `windows_buttons` out of the `Right` flow into an Overlay wrapper
    (`align x:1, y:0`) so `caption_col` spans the full width — deferred, because
    it also means reworking the top rule's `WINDOW_BUTTONS_W` overshoot and its
    hand-tuned edge fade.
- **The `IconButton` active-style change is global**, not an opt-in mode. One
  widget, one rule. It changes `tool_dock`, `view_bar` and the `menu_btn` glow
  as well as the two caption toggles. If a selected tool then reads too weakly
  in the dock's five-glyph column, the fix is a dock-side selection marker, not
  a second button mode.
- **Tab overflow is out of scope.** With enough open tabs the strip will run
  into `[I]`. Handled later.

## Unit 1 — `IconButton`: split the wash from the tint

`icon_button.rs:154-170` currently computes one flag and uses it for both the
16% accent wash and the accent glyph tint:

```rust
let lit = (self.hovered || self.active) && !self.dim;
```

Split it:

```rust
let hot = self.hovered && !self.dim;                     // the wash
let ink = (self.hovered || self.active) && !self.dim;    // the glyph tint
```

- The `lit` uniform pushed onto `draw_bg` takes `hot`.
- The glyph tint takes `ink`: `ink → draw_icon_lit`, else `dim → draw_icon_dim`,
  else `draw_icon_idle`.

Resulting read: resting-active is a bare full-accent glyph with no wash; hover
adds the wash; an active button that is also hovered looks like any other
hovered button. Disabled still never lights, on either channel.

The file's module doc and the `active` field doc both state the old
"OR'd into the `lit` uniform + accent glyph tint" rule verbatim; they move with
the code.

**Blast radius** — every `set_active` caller: `tree_btn` and the new `[I]`
(the targets), `tool_dock.rs:185` (selected tool), `view_bar.rs:249` (lit
toggle), `app.rs:1569` (transient burger glow), and the inspector's
`element_bar.pin_btn`, which Unit 3 deletes.

## Unit 2 — the `DocView` right-dock seam

### Declaration

`BodyChrome` (`doc_view.rs:216`) is already exactly this pattern: per-view chrome
declared by the view, computed from the active tab, applied by the app. It grows
a third member.

```rust
// on the DocView trait
/// The right-hand docked panel this view drives, and the glyph its caption
/// toggle wears. `None` -> no right dock; the shell hides the toggle.
fn right_dock(&self) -> Option<Icon> {
    None
}
```

```rust
pub struct BodyChrome {
    pub tool_dock: bool,
    pub view_bar: bool,
    pub right_dock: Option<Icon>,
}
```

`body_chrome(None)` — no active tab at all — returns `None` for it, exactly as it
returns `false` for the other two.

All three concrete views drive the shared `inspector` widget today
(`class_diagram_view.rs`, `classifier_preview_view.rs`, `source_view.rs`), so all
three return `Some(Icon::InspectionPanel)` — already in the catalog at
`icons.rs:4119`. The seam earns its keep on the `None` path (no open tab → no
toggle) and on views yet to be written.

The view declares *whether* and *which glyph*, and nothing else. Open/closed
state, slot width, button placement and lit state are all the app's.

### Request-to-open

A view also needs to open the panel in response to a user action in the body.
That is `ViewOutcome`, the existing upward intent channel:

```rust
/// Ask the shell to open the right-hand docked panel — a view-side user
/// action that needs the panel visible (select a node, hit a body control).
/// Request-only: a view never asks for a collapse, so a user who closed the
/// panel isn't fought by the next click. Ignored when the active view
/// declares no right dock.
pub open_right_dock: bool,
```

A bare `bool`, matching the existing `close_active` / `statusbar_dirty` fields
rather than an `Option<bool>`. No view wants to force a collapse, and an
open-only request is the one that cannot surprise the user.

Applied in the outcome block (`app.rs:2222-2355`), alongside `statusbar_dirty`:
when the flag is set *and* the active view's `right_dock().is_some()`, drive the
inspector to `Pinned`. Idempotent — an already-open panel is a no-op, so there is
no redraw churn. Because it rides `ViewOutcome`, `handle`, `on_popup_result` and
`on_popup_armed` all get it for free.

Nothing sets the flag in this change. Like `ops` and `open_preview` before it, it
lands as a wired and tested channel whose first real caller comes later.

## Unit 3 — the inspector becomes a binary right column

The right-hand mirror of what `ProjectTree` already became.

**Delete:**

- `flag_btn` and the `FLAG_SQUARE` branch of `draw_walk` (the flag spine).
- `PeekTimer`, its dedicated `NextFrame` clock, and the pointer-left /
  geometric-containment handling that arms it.
- `element_bar.pin_btn` and the pin half of `sync_bar_buttons`.
- The floating card's frame ring and its `margin: Inset{right: 28, top: 12,
  bottom: 12}`.

**Keep, narrowed:**

- `dock: DockState`, but binary: only `DockEvent::Toggle`, `Flag` ⇄ `Pinned`,
  never `Peek`.
- `slot_width()` at 320.0, unchanged.
- Add `toggle_dock(&mut self, cx: &mut Cx)`, the twin of
  `ProjectTree::toggle_dock` (`tree_panel.rs:936`).

`DockState::default()` is `Flag` and stays that way — `tree_panel.rs:405-408`
notes the default is spelled `Flag` on the enum precisely because the inspector
depends on it, and the inspector still wants to start collapsed. The tree keeps
its own `#[rust(DockState::Pinned)]` seed.

**Layout** (`app.rs`): `inspector` moves out of `peek_layer.right_peek_wrap` and
into `right_slot` as a flush `Fill`/`Fill` child, sized by the `sync_dock_slots`
wiring that already exists at `app.rs:809-821`. `peek_layer` and
`right_peek_wrap` then hold nothing and are deleted, collapsing `dock_body`'s
`flow: Overlay` down to `dock_row` alone.

**Trap to respect.** A docked panel's `Flag` draw branch must loop its inner
draw to completion:

```rust
while view.draw_walk(cx, scope, walk).step().is_some() {}
```

A one-shot `let _ = view.draw_walk(..)` leaves the turtle begun and never ended,
unbalancing the window's turtle stack and blanking the caption and both side
panels. It fails silently — clean stderr, the whole test gate blind to it. This
exact bug shipped once already on the tree side (fixed in e62ad58).

## Unit 4 — the caption `[I]` toggle

Last child of `tab_row`, after `doc_tabs`:

```
inspector_btn := IconButton{
    width: 30.0 height: 30.0 icon_size: 18.0
    margin: Inset{right: 2.0, top: 1.0}
    visible: false
}
```

The same 30px box / 18px glyph as `menu_btn` and `tree_btn`, so all three caption
glyphs read as one set, with the 2px inset mirrored to the right edge.

Wiring, each piece the mirror of the `tree_btn` equivalent:

- **Visibility and glyph** are driven by `BodyChrome.right_dock` — *not* by
  `show_editor`/`show_start_screen` the way `tree_btn` is. That is the whole
  point of the seam: the button exists because the active view says it does.
- **Click** → `Inspector::toggle_dock` (mirrors `app.rs:1577`).
- **Lit state** ← `slot_width() > 0.5`, set in `sync_dock_slots` beside the
  tree's (`app.rs:799-807`). Same source of truth as the layout, so the glyph
  cannot disagree with the pixels.
- **`WindowDragQuery`** (`app.rs:2543`) gains a fourth arm. Without it the button
  sits inside the OS caption drag region, every press becomes a window drag, and
  the toggle is simply dead.
- **`INSPECTOR_BTN_W`** const = 32 (30 + the 2px margin), the right-hand twin of
  `TREE_BTN_W`. `doc_tabs`' top-rule right overshoot becomes
  `WINDOW_BUTTONS_W + INSPECTOR_BTN_W`, so the rule still reaches the window's
  right edge now that the strip's turtle is shorter by the button.

## Testing

Unit-testable (pure, no `Cx`):

- `doc_view.rs`: extend `body_chrome_follows_the_active_view` and
  `no_active_tab_hides_every_piece_of_body_chrome` for the new `right_dock`
  member; assert each of the three tab kinds reports `Some(InspectionPanel)`.
- `ViewOutcome::default()` leaves `open_right_dock` false (the existing
  `view_outcome_default_is_all_empty` case).
- Inspector dock transitions: `Toggle` moves `Flag` ⇄ `Pinned` and never yields
  `Peek`; `slot_width()` is 320 open / 0 closed.

Not unit-testable, and the gate is blind to all of it — the caption blanking bug
above proves the point. Interactive sign-off is owed on:

- `[I]` pinned at the tab row's right edge, unmoved by opening tabs or expanding
  the tree column.
- The button toggles the column; the column is flush, full-height, and shrinks
  the canvas rather than overlapping it.
- Active-state read across all five `IconButton` consumers, the tool dock
  especially.
- The caption and both side panels still draw after the `peek_layer` deletion.

## Out of scope

- Tab overflow against `[I]`.
- Moving `windows_buttons` so `[I]` reaches the true window edge.
- Any per-tab dock memory.
- A first real caller for `open_right_dock`.
- Replacing the shared `inspector` widget with a per-view panel widget. The seam
  is shaped to allow it; this change does not do it.
