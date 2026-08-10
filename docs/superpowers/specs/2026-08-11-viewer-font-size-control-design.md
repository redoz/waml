# Viewer font-size (zoom) control

A `−  100%  +` cluster in the document header's trailing button row that scales
the *prose* of the active document view. The widget is shared chrome; each view
implements its own scaling behaviour.

Status: approved design, 2026-08-11. Supersedes nothing.

## Why

The markdown reading view now typesets to theory
(`docs/design/markdown-typesetting.md`), but its size is fixed at 12pt with a
38em measure. Readers on a 4K panel want it bigger; readers proofreading a long
document want more on screen. Every reader application solves this the same
way — a zoom control with a percentage — and that convention is what this
implements. The raw-markdown source editor gets the same affordance, because a
reader who toggles to source expects the control not to vanish.

## Scope

**In:** a shared header widget; a zoom ladder; per-view-kind persisted zoom for
the **reading view** and the **markdown source editor**; buttons, keyboard
chords, and Ctrl+wheel.

**Out:** diagram/canvas surfaces (they already have their own zoom in the status
bar and the ViewBar — this control stays hidden there); chrome/UI scaling (this
is prose zoom, not DPI); per-document zoom.

## Design

### The control

A new `crates/waml-editor/src/font_size_control.rs` widget, `FontSizeControl`,
mounted in `DocumentHeader`'s `content_row` immediately **before** `view_button`
(so the order left-to-right is: breadcrumbs … `[−] [100%] [+]` `[view]`
`[dock]`).

Composition — three children on a `Flow::Right`, `align.y: 0.5`:

| Child | Widget | Size | Notes |
|---|---|---|---|
| `zoom_out_button` | `IconButton` | 30×30 | `Icon::ZoomOut`, `action_tag: zoom_out` |
| `percent_label` | hand-drawn `DrawText` | 44×30 | `fonts.text_menu`, `atlas.text_mid`; `atlas.text` on hover; click = reset |
| `zoom_in_button` | `IconButton` | 30×30 | `Icon::ZoomIn`, `action_tag: zoom_in` |

`Icon::ZoomIn` / `Icon::ZoomOut` are **already** in the icon catalog (enum, DSL
block, `get`, `ALL`, label) — this feature adds no icons, which keeps it clear of
the icon-catalog ordering protocol entirely.

The label is drawn, not a `Label` widget, because it needs its own hover state
and click hit-rect for the reset affordance; `DocumentHeader` already
hand-draws and hit-tests its breadcrumb segments, so this follows the file's
existing idiom rather than introducing a button-with-text widget.

The control emits one action enum:

```rust
pub enum FontSizeControlAction { ZoomIn, ZoomOut, Reset }
```

It holds **no zoom state**. Its only setter is `set_percent(&mut self, cx, u32)`,
which updates the label text, and `set_enabled_directions(&mut self, cx, can_in:
bool, can_out: bool)`, which dims the button at a ladder end (`IconButton` already
has `set_dim`).

### The ladder

New headless module `crates/waml-editor/src/zoom.rs`:

```rust
pub const ZOOM_LADDER: [u32; 10] = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];
pub const ZOOM_DEFAULT: u32 = 100;

pub fn zoom_in(percent: u32) -> u32;    // next rung up, saturating at 200
pub fn zoom_out(percent: u32) -> u32;   // next rung down, saturating at 50
pub fn nearest_rung(percent: u32) -> u32;  // snaps an off-ladder persisted value
pub fn scale(percent: u32) -> f64;      // percent / 100.0
```

Browser-style discrete rungs rather than a linear step: a linear `+10%` spends
too many clicks getting anywhere useful from 100%, and a multiplicative step
lands on ugly percentages. `nearest_rung` exists because a config file can carry
any `u32` (hand-edited, or written by a future finer-grained control) and the
control must still show a rung it can step from.

### Persistence

`EditorConfig` in `crates/waml-editor/src/config.rs` gains two fields, following
the existing `theme` / `markdown_emphasis` pattern exactly (`#[serde(default)]`,
so files written before this feature still load):

```rust
#[serde(default = "default_zoom")] reading_zoom: u32,
#[serde(default = "default_zoom")] source_zoom: u32,
```

with accessors `reading_zoom()`, `source_zoom()`, `set_reading_zoom(u32)`,
`set_source_zoom(u32)` mirroring `theme()` / `set_theme()`, including the
best-effort "log and swallow a write failure" contract. Values are passed
through `nearest_rung` on read so a malformed file can't strand the control.

### Which view is being zoomed

`ZoomTarget` (in `zoom.rs`) names the two zoomable view kinds:

```rust
pub enum ZoomTarget { Reading, Source }
```

`App` derives the current target from the active document surface, the same way
it already derives the header's view-toggle action. `None` → the control is
hidden and reserves no width.

Routing on a `FontSizeControlAction`:

1. compute the next percent via `zoom::zoom_in` / `zoom_out` / `ZOOM_DEFAULT`
2. persist it for that target (`config::set_reading_zoom` / `set_source_zoom`)
3. apply it to the view
4. push the new percent + end-of-ladder dimming back to the control

### Applying the zoom — each view owns its behaviour

This is the constraint the design is built around: the widget is shared, the
*behaviour* is the view's.

**Reading view.** `MarkdownViewer` gains `set_zoom(&mut self, cx, scale: f64)`,
which sets its `TextFlow`'s `font_size` to `base_font_size * scale` and
redraws. Nothing else changes: the typesetting pass made every other dimension
em-derived off that one value (block gaps, heading ladder, measure clamp,
bullet size and centring, list gutters), so a single multiplier scales the whole
page coherently. `MarkdownViewer` stores `base_font_size` at install so repeated
zooms don't compound.

**Source editor.** The markdown editor's painter already carries a `font_scale`
on its `DrawText`, and its gutter-metrics cache is already rekeyed on that value
(`cache_is_rekeyed_when_font_scale_changes` in `widget.rs`). The source view
therefore applies zoom by setting that `font_scale`; the existing cache
machinery handles re-measurement. Line-height, gutter width and caret geometry
follow from the painter's metrics, so no separate plumbing is needed.

### Inputs

**Buttons** — as above.

**Keyboard** — `zoom_command_for(key, modifiers, macos) -> Option<ZoomCommand>`
added to `crates/waml-editor/src/shortcuts.rs`, mirroring the existing
`search_command_for` / `history_command_for` pure functions (and their test
module, which is where the chord-collision audit lives):

| Chord | Command |
|---|---|
| `Ctrl/Cmd` + `=` or `+` | `ZoomIn` |
| `Ctrl/Cmd` + `-` | `ZoomOut` |
| `Ctrl/Cmd` + `0` | `Reset` |

`Alt` disqualifies, matching the other two functions. Dispatched from
`App::handle_global_shortcuts` (`app/event.rs`) next to the history and search
blocks, and consumed (`return true`) only when a zoomable view is active — so
with a diagram focused the chord falls through untouched.

**Ctrl+wheel** — handled by each view's own scroll handling, not globally: the
reading view's `ScrollYView` and the source editor already own wheel events, and
intercepting `Scroll` app-wide would fight them. A wheel event with the primary
modifier held steps one rung and is consumed instead of scrolling.

### Header layout

`DocumentHeaderState` gains a `zoom: Option<u32>` (`Some` = control visible),
and `trailing_buttons_width()` adds `FONT_SIZE_CONTROL_W` (104.0 = 30 + 44 + 30)
when it is `Some`. Because breadcrumb elision already keys off that single
reserved width, narrow windows drop ancestor crumbs to make room with no further
change. `DocumentHeaderAction` gains a `Zoom(FontSizeControlAction)` variant so
the existing `DocumentHeader::action(&Actions)` funnel carries it to `App`.

## Testing

Headless, per unit:

- `zoom.rs` — ladder stepping, saturation at both ends, `nearest_rung` snapping
  (including off-ladder and absurd values), `scale`.
- `shortcuts.rs` — each chord maps on both platforms; `Alt` disqualifies; the new
  chords collide with nothing already claimed (extend the existing audit test).
- `config.rs` — round-trip of both fields; a file predating them loads defaults;
  an off-ladder stored value is snapped on read.
- `document_header.rs` — reserved width grows by the control's width when shown;
  breadcrumb elision honours it; hidden control reserves nothing.
- `font_size_control.rs` — label text tracks `set_percent`; the correct action is
  emitted per tagged button; dimming at ladder ends.
- `MarkdownViewer::set_zoom` — `TextFlow::font_size` equals `base × scale`, and
  two successive zooms do not compound.

**Visual verification is deferred, not waived**
(`.claude/review-dimensions/testability.md`, GUI Limits): a green gate is not
evidence for a drawing change. Every task that
draws lands gate-green and its visual check is recorded in the *Outstanding
visual verification* table at the foot of the implementation plan, to be walked
by a human before the plan is signed off.

## Risks

- **Ctrl+wheel vs. scroll.** If the modifier check is wrong the surface either
  never zooms or can't scroll. Mitigated by consuming the event only on a
  positive primary-modifier match, and by the deferred visual check.
- **Source-editor scale re-measurement.** `font_scale` is already cache-keyed, but
  the caret/IME geometry path is measured separately; if it doesn't follow, the
  source half is deferred rather than shipped half-scaled — the reading half is
  independently useful.
- **Header crowding.** 104px of trailing chrome on a narrow window eats crumbs.
  Accepted: the current segment is already guaranteed to survive elision by
  `layout_header`'s existing contract, which has tests.

## Out of scope / follow-ups

- A zoom entry in the command palette.
- Per-document zoom memory.
- Zooming diagram surfaces through this control (they have canvas zoom).
- A finer-grained (non-ladder) zoom via drag or entry field.
