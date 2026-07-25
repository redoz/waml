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

### 2. Badge — right-floated in `title_row`

Lives in the two-row caption's upper row (`app.rs`, `title_row`), which today holds the
burger and the model name.

- `model_name` changes from its default `Fit` to `width: Fill`, absorbing the row's
  slack so the badge floats hard right, landing just left of the min/max/close cluster.
  The label still starts at the same x and still left-aligns, so short names look
  unchanged; long paths clip on the row's existing `clip_x` exactly as now.
- A new `agent_badge := SolidView{ visible: false }` follows it: `Fit`/`Fit`, padding
  6px horizontal / 2px vertical, 6px right margin, holding a single `Label`.
- The label uses the existing `fonts.text_caption` role — the same role `model_name`
  uses. Reusing it avoids adding a font role, which would require `fonts.rs`,
  `script_gate.rs` and `fonts_overlay.rs` to move together.
- Fill colour is `--color`. Label colour is chosen by the fill's relative luminance
  (near-white on dark fills, near-black on light ones) so any hex an agent picks stays
  readable.
- With `--title` but no `--color`, the chip falls back to Atlas tokens (a dim chrome
  fill with `atlas.text`), so the badge is still legible in both themes.
- `visible` is driven by whether `--title` was given. `--color` alone shows no badge.

### 3. Wash — the title row only

`title_row` changes from `View` to `SolidView` with `draw_bg.color: atlas.field_bg`.
That is pixel-identical to today (the caption bar behind it already paints
`atlas.field_bg`); it exists so the colour is addressable at runtime. With `--color` set,
its `draw_bg.color` is blended 15% toward the tint.

The tint deliberately does **not** go on `caption_bar` itself, for two reasons:

1. `caption_bar` is a `Window` field with no id, so `ids!(...)` cannot reach it.
2. `DocTabs` repaints `field_bg` across the tab row itself. Tinting the bar would wash
   the title row and leave the tab band at the original colour — a half-tinted bar reads
   as a rendering bug, not as a marker.

Washing one clean 34px band and leaving the tabs untouched avoids both.

15% is low enough that the window still reads as Atlas in light and dark rather than as
a broken theme, and high enough to identify at a glance across a monitor.

The strip is **static**. An animated/pulsing tint was considered and rejected: it would
keep the app redrawing forever, and canvas draws are already expensive.

### 4. State and reapplication

`App` holds `agent_badge: Option<String>` and `agent_tint: Option<Vec4>`, populated in
`handle_startup` from the parsed `Args`.

A single `apply_agent_marks(&mut self, cx)` sets the badge text, the badge visibility,
the badge fill/label colours, and the title-row wash. It is called from:

- `handle_startup`, after parsing; and
- `rehydrate`, **before** its start-screen early-return.

The second call is required, not defensive. The `T` theme toggle goes through
`cx.request_live_edit()` → `Apply::Reload`, which resets DSL-declared values — including
`title_row`'s `draw_bg.color` and the badge's text and visibility. Without the
`rehydrate` call, both marks vanish the first time an agent toggles the theme, and the
window silently becomes indistinguishable again.

Runtime mutation follows the pattern already in `sync_dock_slots`: `borrow_mut::<T>()`
on the widget and poke the public field, since this fork's widget API has no
`apply_over`-style setter.

## Data flow

```
argv ──> cli::parse ──> Args{badge, tint}
                          │
                          ├─ App.agent_badge / App.agent_tint   (handle_startup)
                          │
                          └─ apply_agent_marks(cx)
                                 ├─ agent_badge label text + visible
                                 ├─ agent_badge draw_bg.color   = tint
                                 ├─ agent_badge label colour    = luminance pick
                                 └─ title_row  draw_bg.color    = mix(field_bg, tint, 0.15)
                                        ▲
                                 rehydrate() re-invokes after every theme reload
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

Plus a pure, tested luminance → label-colour function.

No UI test: the gate is headless and cannot assert on drawn pixels. Verification is an
interactive launch, captured per-pid, of two windows with different `--title`/`--color`
values, confirming the badge sits right-floated in the title row without displacing the
model name, the wash reads at a glance, and both survive a `T` theme toggle.

## Open item

Whether the caption bar is visible on the start screen (pre-model) is unconfirmed —
`caption_bar` declares `visible: false` and nothing in `app.rs` flips it, so makepad's
`Window` appears to own that. If the caption is hidden there, a `--title`-only launch of
the start screen shows no marker at all. Resolve during implementation; if confirmed,
note it in the flags' help text rather than growing the design.

## Explicitly out of scope

- Window-title / taskbar suffix for programmatic window lookup.
- Env-var fallback (`WAML_AGENT_TITLE` etc.). Flags only.
- Animated or pulsing tint.
- Hashing an agent name into a colour automatically; the caller picks the hex.
