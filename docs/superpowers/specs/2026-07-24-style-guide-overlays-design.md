# Style-guide overlays: Fonts · Icons · Colors

## Purpose

The editor's chrome design system — typography scale (`mod.fonts`), the SDF
icon catalog (`icons.rs`), and the Atlas palette (`theme_atlas.rs`) — has no
in-app reference. This adds three built-in reference overlays reachable from the
wordmark (logo) menu:

- **Fonts** — every `mod.fonts` role, its live sample, and its spec.
- **Icons** — every glyph actually wired into the UI, grouped by where it's
  used.
- **Colors** — every Atlas token, a live swatch, its hex, and its purpose.

They are living style-guide surfaces: samples render in the *real* style tokens,
so a theme flip (`T`) or a token edit is reflected immediately — the page can't
drift from what the app actually draws.

A shared `OverlayShell` carries the scrim / panel / scroll / dismiss behavior so
each page provides content only; the existing `ShortcutsOverlay` is migrated onto
the same shell.

## Non-goals

- No editing of fonts/icons/colors — read-only reference.
- No new hotkeys — the pages are menu-driven only (dismiss keys aside).
- No change to the icon catalog, font scale, or palette themselves.
- Not user-facing "help" — this is a design/dev reference, mounted like
  `ShortcutsOverlay`.

## Architecture

### Shared `OverlayShell` (new `overlay_shell.rs`)

A plain embedded struct (NOT a `Widget`) owning all overlay style + behavior.
Consumers embed one and supply content only.

Responsibilities:

- **Scrim** — full-window fill, `atlas.scrim`.
- **Panel** — centered, fixed width (per-consumer), fill `atlas.surface`, edge
  `atlas.frame_hi`. Height = `min(content_height + vertical_pad, max_panel_h)`,
  where `max_panel_h` leaves a margin off the window edges.
- **Scroll** — when `content_height` exceeds the visible panel body, the content
  clips and wheel-scrolls; a thumb draws at the right inset. This is the
  `menu.rs` `LinearGeom` clamp idiom (`scroll ∈ [0, max_scroll]`,
  `set_scroll` clamps), lifted into the shell so every page inherits it.
- **Dismiss** — `Escape` or a click on the scrim (outside the panel) →
  `OverlayShellAction::Dismissed`. A click/scroll inside the panel is consumed,
  never dismisses.

The shell exposes a **procedural seam** (not a draw callback) so the owning
widget can draw content with `&mut self` without a borrow conflict against the
shell:

```rust
// draw
let h = self.content_height(cx);            // consumer measures its own content
if let Some(pass) = self.shell.begin(cx, h) {
    // pass.origin = top-left of the (already scroll-shifted, clipped) content
    // pass.width  = content column width (panel minus padding)
    self.draw_rows(cx, pass.origin, pass.width);
    self.shell.end(cx);
}

// event
match self.shell.handle_event(cx, event) {
    OverlayShellAction::Dismissed => { /* owner closes itself */ }
    OverlayShellAction::None => {}
}
```

`begin` returns `None` when the shell is closed (draws nothing). `begin` draws
the scrim + panel + sets up the content clip/translate; `end` closes the clip
and draws the scrollbar thumb. The shell is drawn into the **window overlay**
(`begin_overlay_reuse` + root turtle), the same idiom `MenuPopup` and the
current `ShortcutsOverlay` use, so it paints over the whole window including the
caption band.

Open/close state (whether the overlay is showing) lives on the **owning widget**,
not the shell — the shell only knows scroll offset + geometry. Owner toggles
visibility and, while open, feeds events to the shell.

Pure geometry (panel rect, content rect, scroll clamp, thumb rect, wheel delta →
scroll) is unit-tested directly on the shell, mirroring `LinearGeom`'s tests.

### Content overlays

Three thin `Widget`s, one per page, each:

- embeds an `OverlayShell`,
- owns its **data table** (hand-authored — see Data source),
- owns its **draw resources** (the `draw_*` DSL fields it needs),
- owns its **row/section geometry** (row height, section-heading height, the
  content-height sum),
- implements `content_height(cx)` and `draw_rows(cx, origin, width)`.

Files: `fonts_overlay.rs`, `icons_overlay.rs`, `colors_overlay.rs`.

`ShortcutsOverlay` is **migrated** onto `OverlayShell` too (its bespoke
scrim/panel/dismiss code deleted), making it the fourth consumer and validating
the abstraction against pre-existing behavior. Its `BINDINGS` table and layout
constants stay; only the shell plumbing changes. Its existing behavior (toggle
from the tool-dock `?` button / `?` hotkey, Esc/scrim dismiss) must be preserved
exactly.

### Surfacing (`app.rs`)

- `logo_menu_items()` gains three rows before the Exit danger row:
  **Fonts** (`Icon::Paintbrush` or similar type/brush glyph),
  **Icons** (a grid/catalog glyph),
  **Colors** (a swatch/palette glyph). Pick from the existing catalog; no new
  glyphs.
- `LogoCommand` gains `Fonts`, `Icons`, `Colors`; `logo_command_for` maps the
  new ids; `handle_actions` opens the matching overlay.
- The three overlays are mounted as children of the body `Overlay` wrapper in
  the App DSL, alongside `shortcuts_overlay`. **One open at a time** — opening
  one closes any other (and vice-versa with the shortcuts overlay).

## Page content

### Fonts

One row per `mod.fonts` role, in scale order:
Title, Heading, Body, Label, Menu, Eyebrow, Mono (7 rows).

Each row:

1. **Role name** — e.g. "Menu", drawn in `text_label`.
2. **Sample** — a fixed pangram, drawn in the **real** style: wire each
   `mod.fonts.text_X` to a `draw_sample_X: DrawText` field in the overlay DSL,
   so the sample is literally that token's rendering.
3. **Spec line** — `<family> <weight> · <size>px · <line-spacing>`, e.g.
   `IBM Plex Sans Regular · 10px · 1.2`, drawn in `text_mono`.

7 rows fit without scroll, but the shell handles it regardless.

### Icons

Only glyphs **actually referenced** in UI code (≈40 of the 92 catalog entries),
grouped by usage-area. The grouping IS the "used for" answer. Groups (final list
derived by grepping `Icon::` at implementation time; expected):

Tool dock · Doc tabs · Wordmark/burger menus · Node menu · Inspector ·
Tree panel · Conflict badge/list · Constraint toggle · Statusbar · Start screen.

Each group = an eyebrow heading (`text_eyebrow`) + its rows. Each row:
`glyph (IconSet SDF) · slug (Icon::label(), text_body) · one-line purpose
(text_label/dim)`.

Long → shell scroll. Panel wider than Fonts (~560px) for the three-column row.
A glyph used in multiple areas appears under its primary/most-salient area (one
row per glyph, not per usage), keeping the drift guard (every used icon present
exactly once) simple.

### Colors

Atlas tokens grouped by the `theme_atlas.rs` comment sections:
Backdrops (`ground`, `canvas_ground`, `group_fill`) · Surfaces (`surface`,
`surface_border`, `field_bg`) · Accent + frame (`accent`, `accent_soft`,
`selection`, `frame_hi`, `frame_lo`) · State (`scrim`, `danger`) · Text (`text`,
`text_dim`) · Logo ramp (`logo_hi/mid/lo`) · Node buckets (`bucket_*`, 8).

Each group = eyebrow heading + rows. Each row:

1. **Swatch** — live: wire each `atlas.X` to a `draw_swatch_X: DrawColor` field,
   so the swatch is the token's current value and **tracks the `T` theme flip**.
2. **Token name** — e.g. `accent`, `text_dim` (`text_mono`).
3. **Hex** — read back from the live swatch's `Vec4` at draw time and format
   `#rrggbb` (+ alpha when < 1), so it too tracks the theme (`text_mono`).
4. **Purpose** — one-liner (`text_label`).

Long → shell scroll.

## Data source + drift guards

Script-DSL values (font sizes, atlas colors) are not readable from Rust at
runtime, so each page's metadata table is **hand-authored** in Rust (the
`BINDINGS` precedent). Samples/swatches render the real token via wired `draw_*`
fields, so only the descriptive text is hand-authored — the *appearance* can't
drift. To stop the *tables* drifting, unit tests:

- **Fonts** — the roles table covers exactly the 7 `mod.fonts` roles (a const
  list the test asserts length/coverage against).
- **Icons** — every table entry is a real `Icon`; and every `Icon::<Variant>`
  referenced in `crates/waml-editor/src` UI code (excluding `icons.rs`
  definitions and `bin/`) appears in the table exactly once. This catches
  "wired a new icon, forgot the page" and "removed an icon, stale row".
- **Colors** — the tokens table covers every field name defined in
  `mod.themes.atlas_light` (parsed from `theme_atlas.rs` source, the same
  source-shape-assertion technique `fonts.rs`'s namespace gate uses).

## Testing

- **Geometry (unit)** — `OverlayShell` scroll clamp / panel rect / content rect
  / thumb rect / wheel-delta→scroll, plus each page's row/section layout +
  `content_height` sum, tested pure like `LinearGeom`.
- **Drift guards (unit)** — the three coverage tests above.
- **Migration** — `ShortcutsOverlay`'s existing tests stay green; behavior
  unchanged.
- **Visual** — per-pid screenshot verify each page (light + dark), confirming
  samples/swatches render in real styles and scroll works. Screenshot by
  specific pid in a single call — never kill-all (the editor session guard).

## Risks / notes

- **Borrow fight** — the procedural `begin`/`end` seam is deliberate; a
  draw-callback seam (`shell.draw(cx, &mut content)`) fails to borrow-check
  against `&mut self`. Do not "simplify" back to a callback.
- **`mod.X` namespace shape** — new overlay `script_mod!` blocks that add
  `mod.widgets.*` entries must be registered in `App::script_mod` BEFORE the
  consuming module, and any `mod.<ns>` namespace must be created by a single
  object-literal assignment, never field-by-field (the chrome-font-outage
  class). The whole cargo/pnpm gate is blind to this — visual verify each page
  boots with text + samples.
- **Custom-widget child registration** — if a page is mounted as a DSL child, its
  `script_mod(vm)` must register before the mounting module resolves it, or it's
  a dead/invisible node that green tests + review both miss (the IconButton
  trap). Per-pid visual verify is mandatory.
- **One-open-at-a-time** — the mutual-exclusion wiring must include the existing
  `shortcuts_overlay`, not just the three new pages.
