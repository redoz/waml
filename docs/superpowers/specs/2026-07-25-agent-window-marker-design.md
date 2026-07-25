# Agent window marker — design

**Date:** 2026-07-25
**Status:** approved, ready for planning

## Problem

Several agents run `waml-editor` at once, plus the user's own session. On screen the
windows are identical, so telling "which agent launched this one" apart is guesswork.

Scope is deliberately narrow: **eyeball identification only**. Making an agent able to
find its own window programmatically (window title, taskbar text, `MainWindowTitle`
matching) is a different feature and is *not* part of this design — per-pid capture
already covers the tooling case.

## Surface

Two independent launch flags, either usable alone, both composable with the existing
positional dir and `--diagram`:

- `--title <text>` — text shown in a badge in the caption bar.
- `--color <hex>` — a colour that tints both the badge and the caption's title row.

```
waml-editor crates/waml-editor/tests/fixtures/mini --title veil-fix --color '#e91e63'
waml-editor --color 2b8            # start screen, tint only
waml-editor . --title opus-3       # badge only, default chrome colours
```

`scripts/run-native.ps1` gains `-Title` / `-Color` params that pass straight through.

## Components

### 1. `cli.rs` — parsing

`Args` gains two fields:

```rust
pub struct Args {
    pub dir: Option<PathBuf>,
    pub diagram: Option<String>,
    pub badge: Option<String>,
    pub tint: Option<[f32; 3]>,
}
```

`--title` takes the next argv element verbatim (any string, including empty).

`--color` takes a hex colour and parses it **in `cli.rs`**, to sRGB components in
`0.0..=1.0`. Keeping the parse here rather than in `app.rs` keeps this module free of
makepad types, so it stays unit-testable without a `Cx`. Accepted forms, leading `#`
optional and case-insensitive:

- `rgb` / `#rgb` — each nibble doubled (`f0a` → `ff00aa`)
- `rrggbb` / `#rrggbb`

Anything else is an error, in the same class as the existing `--diagram requires a
value`: `parse` returns `Err`. Alpha (`rgba`/`rrggbbaa`) is not accepted — the blend
factors are fixed by the design, so a caller-supplied alpha would have no meaning.

**Error-path fix.** `App::handle_startup` currently logs a parse error and bare-`return`s,
which leaves a blank window with no chrome and no explanation. It changes to log then
`show_start_screen(cx)`, so a typo'd flag yields a usable window. This is a pre-existing
bug that a mistyped `--color` would otherwise make routine.

### 2. Both marks are one widget: `AgentMark`

A new `crates/waml-editor/src/agent_mark.rs` owns the wash and the badge together, in a
single widget mounted in the two-row caption's upper row (`app.rs`, `title_row`).

**Why a custom widget rather than tinting a `SolidView`.** The obvious shape — make
`title_row` a `SolidView` and set its `draw_bg.color` at runtime — cannot be built on
this fork. `View::draw_bg` is a `DrawQuad`, and `SolidView`'s `color` is a shader
`instance` (`view_ui.rs:85`). The fork exposes `set_uniform` for *uniforms*
(`canvas.rs:2359`) but nothing for instance vars, and it has no `apply_over`-style
setter — `app.rs:789` records exactly this, which is why `sync_dock_slots` pokes public
`walk` fields instead. A runtime-settable colour therefore requires a Rust-typed draw
struct: `DrawColor` (`draw_quad.rs:103`, `{ draw_super: DrawQuad, color: Vec4f }`), the
pattern `canvas.rs` already uses for `draw_rule` / `draw_veil`.

`AgentMark` holds:

- `draw_wash: DrawColor` — the title-row band
- `draw_chip: DrawColor` — the badge fill
- `draw_label: DrawText` — the badge text
- `#[rust] badge: Option<String>`, `#[rust] tint: Option<Vec4>`, `#[rust] row_w: f64`

**Mounting: zero layout footprint.** It is declared as the **first** child of
`title_row` with `width: 0.0, height: Fill`. Zero width means it consumes no space in
the `flow: Right` row, so `menu_btn` and `model_name` keep their exact current
positions — no `width: Fill` change to `model_name`, no restructuring, and critically
no extra nesting for `menu_btn`, whose interactivity is depth-sensitive and gate-blind.
First position also fixes paint order: the wash paints *under* the burger and the model
name rather than gelling over them.

Everything is then drawn with `draw_abs` from the widget's own origin, over a width the
App supplies. This is the established trick in this file: `DocTabs::left_overshoot`
(`doc_tabs.rs:823`, driven from `app.rs:856`) already draws outside its own turtle rect
using an App-measured delta, and `title_row`'s existing `clip_x: true` bounds the result
to the row.

- **Wash:** a `row_w × row_h` quad at the widget's origin, filled with `field_bg`
  blended 15% toward the tint. Drawn only when a tint is set.
- **Badge:** the label measured, then a chip right-aligned at `row_w - chip_w - 6`,
  padded 6px horizontal / 2px vertical. Drawn only when `--title` was given.
- The label uses the existing `fonts.text_caption` role — the same role `model_name`
  uses. Reusing it avoids adding a font role, which would require `fonts.rs`,
  `script_gate.rs` and `fonts_overlay.rs` to move together.
- Chip fill is `--color`; label colour is picked by the fill's relative luminance
  (near-white on dark fills, near-black on light ones) so any hex stays readable. With
  `--title` and no `--color`, the chip falls back to Atlas tokens (`selection` fill,
  `atlas.text` ink) so it stays legible in both themes.

`App` supplies `row_w` by measuring `ids!(title_row)`'s drawn rect, change-guarded and
pushed via `set_row_width(cx, px)` — the same measure-and-push shape as `sync_tree_gap`.
Zero until the row has been laid out once, which draws nothing; the next pass fills in.

### 3. Why the wash covers the title row and not the whole caption

The tint deliberately does **not** go on `caption_bar` itself, for two reasons:

1. `caption_bar` is a `Window` field with no id, so `ids!(...)` cannot reach it.
2. `DocTabs` repaints `field_bg` across the tab row itself. Tinting the bar would wash
   the title row and leave the tab band at the original colour — a half-tinted bar reads
   as a rendering bug, not as a marker.

Washing one clean 34px band and leaving the tabs untouched avoids both.

15% is low enough that the window still reads as Atlas in light and dark rather than as
a broken theme, and high enough to identify at a glance across a monitor.

The wash is **static**. An animated/pulsing tint was considered and rejected: it would
keep the app redrawing forever, and canvas draws are already expensive.

`agent_mark::script_mod(vm)` must be registered **before** `app.rs`'s own in
`main.rs` — a custom widget mounted as a DSL child is a dead, invisible node if its
module resolves late, and neither tests nor review catch it.

### 4. State and reapplication

`App` holds `agent_badge: Option<String>` and `agent_tint: Option<Vec4>`, populated in
`handle_startup` from the parsed `Args`.

A single `apply_agent_marks(&mut self, cx)` pushes both into the `AgentMark` widget. It
is called from:

- `handle_startup`, after parsing; and
- `rehydrate`, **before** its start-screen early-return.

The second call is required, not defensive. The `T` theme toggle goes through
`cx.request_live_edit()` → `Apply::Reload`, which resets `#[live]`/`#[rust]` widget state
— including `AgentMark`'s `badge` and `tint`. Without the `rehydrate` call, both marks
vanish the first time an agent toggles the theme, and the window silently becomes
indistinguishable again.

Reaching the widget follows the pattern already in `sync_dock_slots`: `borrow_mut::<T>()`
on the widget and call its setter, since this fork's widget API has no `apply_over`-style
setter.

## Data flow

```
argv ──> cli::parse ──> Args{badge, tint}
                          │
                          ├─ App.agent_badge / App.agent_tint   (handle_startup)
                          │
                          └─ apply_agent_marks(cx)
                                 └─ AgentMark::set_marks(cx, badge, tint)
                                        │      ▲
                                        │      └─ rehydrate() re-invokes after every
                                        │         theme reload
                                        └─ draw_walk (draw_abs over App-supplied row_w)
                                               ├─ draw_wash  = mix(field_bg, tint, 0.15)
                                               ├─ draw_chip  = tint
                                               └─ draw_label = luminance pick

sync_agent_row(cx): measure ids!(title_row) rect ──> AgentMark::set_row_width(cx, px)
```

## Error handling

| Case | Behaviour |
|---|---|
| `--title` with no following value | `Err("--title requires a value")`, start screen |
| `--color` with no following value | `Err("--color requires a value")`, start screen |
| `--color` with unparseable hex | `Err` naming the bad value, start screen |
| Neither flag given | No badge, no wash — today's appearance exactly |
| `--color` only | Wash applied, no badge |
| `--title` only | Badge in Atlas fallback colours, no wash |

Every error path logs and lands on the start screen. No path produces a blank window.

## Testing

Unit tests in `cli.rs` (no `Cx`, runs in the headless gate):

- both flags parse, alone and together
- flags compose with the positional dir and with `--diagram`, in any order
- each accepted hex form, with and without `#`, mixed case
- unparseable hex, wrong-length hex, and missing values each return `Err`
- absent flags leave both fields `None`

Plus pure, tested helpers in `agent_mark.rs`: luminance → label-colour, and
`mix(field_bg, tint, 0.15)`.

No UI test: the gate is headless and cannot assert on drawn pixels. Verification is an
interactive launch, captured per-pid, of two windows with different `--title`/`--color`
values, confirming:

- the badge sits right-floated in the title row and the burger and model name have not
  moved a pixel from their current positions;
- `menu_btn` still opens its drop-down and `tree_btn` still toggles the column — the
  caption is the known gate-blind failure class, so the click test is mandatory, not
  optional;
- the wash reads at a glance and does not gel the burger glyph or the model name;
- both marks survive a `T` theme toggle;
- a no-dir `--color`-only launch shows the wash on the start screen.

## Resolved: caption visibility on the start screen

`caption_bar`'s `visible: false` is the makepad default and does **not** hide it — the
`Window` widget flips it on Windows, which is why nothing in `app.rs` touches it. The
caption, and therefore both marks, render on the start screen as well as with a model
open. A `--color`-only, no-dir launch is a valid way to tag a window.

`menu_btn` and `tree_btn` *are* app-hidden until a model opens, but `AgentMark` is not
gated on that — it shows whenever it has something to draw.

## Explicitly out of scope

- Window-title / taskbar suffix for programmatic window lookup.
- Env-var fallback (`WAML_AGENT_TITLE` etc.). Flags only.
- Animated or pulsing tint.
- Hashing an agent name into a colour automatically; the caller picks the hex.
