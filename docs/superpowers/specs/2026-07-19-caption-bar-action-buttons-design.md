# Caption-bar action buttons

## Goal

Add two icon buttons to the right edge of the window caption bar:

- **Save** (left) and **hamburger menu** (right).
- Both are **placeholders** this pass: a click fires a `log!` line, nothing more.
  Real save / real menu contents land later.

Also: rename the stale `pkg_name` caption label id to `model_name` (the
project→model rename left it behind). Its shown value is unchanged.

## Context

The caption bar is a `script_mod!` `View` tree in `app.rs` (`caption_bar:
SolidView{ flow: Right ... }`), laid out as:

```
nav (width 292) | doc_tabs (width Fill) | windows_buttons (min/max/close)
```

- `nav` holds the wordmark logo, a `/` separator, and the `pkg_name` label
  (already populated with `self.model.path` / `"bundle"` at `app.rs:474` and
  `:561`).
- `doc_tabs` takes `width: Fill`, so it consumes all slack between the nav
  cluster and the window controls.

The whole caption bar (minus the OS min/max/close buttons) is an OS
window-drag region. `App::handle_event` (`app.rs:1039`) re-answers the
`WindowDragQuery` as `Client` over the doc-tab cards so tab clicks/hover reach
the widget instead of being swallowed by the drag. Any new interactive element
in the caption bar needs the same treatment.

Clickable-widget pattern in this codebase: `ActionLink` (`action_link.rs`) is a
`#[deref] View` hybrid — `handle_event` hit-tests its own `view.area()`, emits a
`Clicked` action on `FingerUp` when `is_over`, and drives a `hovered` flag from
`FingerHoverIn/Out` that feeds a `hover` uniform on the root `draw_bg`.

Icon-drawing pattern: hand-authored SDF shaders on `DrawColor`
(`icons.rs`, `icon.rs`). `icon.rs`'s `DrawIcon` switches shape inside one
`pixel: fn()` on a `shape` uniform (an if-chain over shader indices). SVG
sources are ported to SDF centerlines (`sdf.line_to` / `sdf.arc_to`), the way
`IconPin` was ported from `pin.svg`.

## Component: `CaptionButton` widget

New file `crates/waml-editor/src/caption_button.rs`, modeled on `ActionLink`.

- `#[deref] View`. Square, ~30×30, vertically centered in the 44px caption bar.
- `#[live] shape: f32` — selects the glyph in the root `draw_bg` shader:
  `0.0 = hamburger`, `1.0 = save`. Set per DSL instance (a root scalar prop,
  the same mechanism `ActionLink` uses for `kind`/`text` — this fork's DSL has
  no per-instance child override).
- Root `draw_bg` `pixel: fn()`:
  - Draws the glyph selected by `shape` (if-chain, `DrawIcon` idiom). Geometry
    is ported from the user-supplied SVGs (pending) — hamburger = three
    full-width bars; save = floppy body (rounded square, clipped corner, label
    slot + shutter). Sharp HUD language: `sdf.rect` / paths, thin stroke,
    `sdf.box` only for a real corner radius (never `sdf.box(..,0.0)` — floods
    on this fork).
  - Idle stroke tint = `atlas.text_dim`; on hover the stroke goes `atlas.accent`
    plus a subtle premultiplied accent wash square behind the icon (the
    `ActionLink` wash formula). Colors are chosen inside the shader from atlas
    tokens via a `hover` uniform — no RGBA crosses Rust.
- `handle_event`: `Hit::FingerUp` with `is_primary_hit() && is_over` →
  `cx.widget_action(uid, CaptionButtonAction::Clicked)`. `FingerHoverIn` sets
  the Hand cursor + `hovered = true`; `FingerHoverOut` clears it. Each toggles a
  redraw.
- `draw_walk`: push `hover` and `shape` uniforms, then delegate to `view`.
- `CaptionButtonAction { None, Clicked }` + a `clicked(&Actions) -> bool` helper
  and a `CaptionButtonRef::clicked`, mirroring `ActionLink`.

## Placement

Caption-bar flow becomes:

```
nav (292) | doc_tabs (Fill) | save_btn | menu_btn | windows_buttons
```

Two `CaptionButton` instances added after `doc_tabs`, before `windows_buttons`:

```
save_btn := CaptionButton{ shape: 1.0 }
menu_btn := CaptionButton{ shape: 0.0 }
```

`doc_tabs`'s `Fill` pushes both to the far right. A small right margin / inter-
button gap keeps them off the window controls.

## Drag-region seam

Extend the `WindowDragQuery` override in `App::handle_event` (`app.rs:1039`):
in addition to `over_tab`, answer `WindowDragQueryResponse::Client` when the
pointer is over either button's rect. Get each rect from the widget's
`area().rect(cx)` (or a small `hits(abs)` accessor on `CaptionButton`, matching
`DocTabs::hits_any_tab`). Without this, clicks over the buttons are swallowed by
the OS drag and the placeholder never fires.

## Wiring

In `App::handle_actions`, read each button ref's `clicked(actions)`:

- `save_btn.clicked()`  → `log!("caption: save clicked")`
- `menu_btn.clicked()`  → `log!("caption: menu clicked")`

(Placeholder targets; replaced with real save / menu-open later.)

## Rename

`pkg_name` → `model_name`:

- DSL: `pkg_name := Label{...}` → `model_name := Label{...}` (and the enclosing
  `pkg_name_view` may stay or follow suit).
- The two `self.ui.label(cx, ids!(pkg_name))` sites (`app.rs:474`, `:561`) →
  `ids!(model_name)`.
- Shown value (`root_name`) unchanged.

## Testing / verification

`CaptionButton` carries no unit-testable state logic (same as `ActionLink`,
which ships without unit tests). Verification is by running the app:

- Both buttons render dim at the far right of the caption bar, save left of
  menu.
- Hover: stroke goes accent + wash, cursor is a hand.
- Click: the expected `log!` line prints.
- The empty stretch of the caption bar still drags the OS window; the button
  rects do not.
- Existing suites stay green: `cargo test -p waml-editor`, `cargo test -p waml`.

## Out of scope

- Real save behavior and menu contents/popup surface.
- Any change to the window min/max/close controls.
- Dark-mode-specific tuning beyond the atlas tokens the shader already reads.
