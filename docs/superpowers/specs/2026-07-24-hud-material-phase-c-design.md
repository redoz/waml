# HUD material phase C — depth shadow, bloom, frost fill

**Date:** 2026-07-24
**Status:** approved, ready for plan
**Supersedes nothing.** Completes the work deferred by
`2026-07-18-draw-hud-frame-design.md` ("phase C").

## Goal

The rust editor's surfaces render flatter than the svelte reference. The svelte
`.hud-surface` material is four layers; `AccentFrame` implements one of them.
This spec adds the missing three — depth shadow, accent bloom, frost-gradient
fill — to the same primitive, and gives every consumer (present and future) a
one-call seam that gets them automatically.

## The gap

Svelte `.hud-surface` (`packages/web/src/atlas-components.css:7-39`):

1. **frost fill** — `linear-gradient(180deg, rgba(255,255,255,var(--frost-top)),
   rgba(255,255,255,var(--frost-bot)))` composited over
   `rgba(var(--accent), var(--frost-tint))`
2. **masked accent frame** — 150deg alpha ramp, `padding: var(--bw)` = 1.5px
3. **depth shadow** — `0 var(--depth-y) var(--depth-blur) rgba(40,70,110,
   var(--depth-a))`
4. **bloom** — `0 0 calc(14px * var(--glow)) rgba(var(--accent),
   calc(var(--bloom) * var(--glow)))`, with `--glow: .4` (`atlas.css:23`)

Rust `AccentFrame` (`crates/waml-editor/src/frame.rs`) implements **layer 2
only**, over a flat `atlas.field_bg` fill. Its own module doc says so:

> Phase 1 draws stroke + flat fill only. The full `.hud-surface` material
> (frost-gradient fill + depth shadow + bloom glow, with panel/node/button knob
> variants) is a later phase that adds uniforms to this same prototype.

Knob presets in svelte, which this spec ports verbatim:

| preset | frost top/bot/tint | depth y/blur/a | bloom |
| --- | --- | --- | --- |
| panel (`.hud-surface`) | .95 / .82 / .06 | 12px / 30px / .20 | .16 |
| node (`--node`) | .94 / .80 / .06 | 8px / 22px / .14 | .18 |
| button (`--btn`) | .92 / .74 / .10 | 6px / 18px / .14 | .22 |

Selected node (`packages/web/src/components/canvas/canvas.css:21-26`) overrides
`--bw` to 2.5px plus `0 8px 22px rgba(40,70,110,.14)` and
`0 0 26px rgba(var(--accent),.28)`.

## Scope

**In:** canvas nodes (`canvas.rs`), the selection toolbar
(`selection_toolbar.rs`), and the three popup surfaces
(`popup/select.rs`, `popup/menu.rs`, `popup/conflict_list.rs`) — every current
`mod.draw.AccentFrame` consumer that draws into an overlay or an explicitly
computed rect.

**Out:** docked panels and the project tree. They butt against window edges
where a drop shadow largely clips, so the payoff does not justify reworking
their View-walk-driven backgrounds.

## Unit 1 — bleed seam + depth shadow

### The bleed problem

Shadow and bloom paint *outside* the box. `AccentFrame`'s SDF is clamped to
`rect_size`, so the drawn quad must be larger than the surface it frames.
Rather than make each call site inflate its own rect, the pen does it.

### `bleed` uniform

`AccentFrame` gains `bleed: uniform(0.0)` — the padding, in pixels, that the
caller added on every side. The shader offsets its geometry inward by `bleed`
so the frame still lands on the true rect:

```
let inset = 1.5 * self.zoom * mix(1.0, 1.5, self.selected)
let x0 = self.bleed + inset
let y0 = self.bleed + inset
let w  = self.rect_size.x - (self.bleed + inset) * 2.0
let h  = self.rect_size.y - (self.bleed + inset) * 2.0
```

At the default `bleed = 0.0` this is byte-for-byte today's geometry, so an
un-migrated consumer is visually unchanged.

### `draw_hud_abs` helper — the generic seam

An extension trait in `frame.rs`:

```rust
/// Draw an `AccentFrame`-derived pen at `rect`, padding the drawn quad so the
/// depth shadow and bloom have room to fall outside the surface.
pub trait HudFrameExt {
    fn draw_hud_abs(&mut self, cx: &mut Cx2d, rect: Rect);
}

impl HudFrameExt for DrawColor { /* … */ }
```

It reads the pen's own knob uniforms, computes the bleed, pushes it, inflates
the rect, and calls `draw_abs`. Every consumer's diff is one word:
`draw_abs` -> `draw_hud_abs`. Any future popup that points its `draw_frame` at
`mod.draw.AccentFrame` and calls `draw_hud_abs` gets the shadow with no
geometry math of its own. That is the whole point of the seam.

### Bleed math — the testable unit

Extracted as a pure function so it is unit-testable without a GPU:

```rust
/// Pixels of padding a HUD surface needs on every side for its shadow and
/// bloom to fall outside the frame without clipping.
pub fn hud_bleed(depth_y: f64, depth_blur: f64, bloom_px: f64, zoom: f64) -> f64
```

Returns `(max(depth_blur + depth_y, bloom_px) * zoom).max(0.0) + 2.0`. The
`depth_blur + depth_y` term covers the downward-offset shadow's far edge;
`bloom_px` is the un-offset halo radius (`14.0 * glow`, or `26.0` when
selected); the `+ 2.0` is antialias slack. It must scale by `zoom` for the same
reason the shader does (below) — otherwise the padded quad and the drawn shadow
disagree and the shadow clips.

### Shadow layer

`depth_y`, `depth_blur`, `depth_a` uniforms, all defaulting to `0.0`. The
shadow is a box distance field offset down by `depth_y * zoom`, its alpha a
`smoothstep` falloff over `depth_blur * zoom`, tinted by a new theme token
(below) at `depth_a`. Composited under everything else.

### Shadow color is a theme token

Svelte hardcodes `rgba(40,70,110,…)`, a cool slate that reads correctly only on
the light ground. The rust editor also ships a dark theme (`theme_atlas.rs`
defines a second palette), where a blue-slate shadow would glow rather than
recede. Add a token:

```
shadow: #x28466e   // light — the svelte 40,70,110
shadow: #x1a0a17   // dark  — near-black, deepened off the plum ground
```

The pen takes it as `shadow_col: uniform(atlas.shadow)`; `depth_a` supplies the
alpha.

### Zoom

The canvas already pushes a `zoom` uniform per frame; popups push a fixed
`zoom: 0.6` as a stroke-thickness device. Shadow offset and blur scale with
`zoom` — the shadow belongs to the node in world space, exactly as the border
inset already does at `frame.rs:47`. Floor the result so the shadow does not
vanish at fit-zoom, mirroring the existing `max(1.25, inset)` at `frame.rs:52`.

### Sign-off

Canvas nodes and the menu popup cast a soft downward shadow. Nothing else has
moved: no halo, no fill change, frame stroke identical.

## Unit 2 — accent bloom

`bloom: uniform(0.0)`, `accent_col: uniform(atlas.accent)`, plus a `glow`
constant of `0.4` (the `--glow` token). A zero-offset falloff of `accent_col`
over `14.0 * glow` pixels at alpha `bloom * glow`, composited above the shadow
and below the fill. `accent_col` is reused by unit 3's frost tint.

`hud_bleed`'s `bloom_px` argument starts carrying `14.0 * glow`.

**Sign-off:** the halo reads as accent-tinted, not as a second grey shadow.

## Unit 3 — frost fill

`frost_top`, `frost_bot`, `frost_tint` uniforms replace the flat `self.color`
fill:

```
// accent tint sits behind the white ramp (CSS layers the gradient over
// `rgba(var(--accent), var(--frost-tint))`)
let base  = mix(self.color, self.accent_col, self.frost_tint)
// vertical white ramp, opaque-ish at the top, thinner at the bottom
let frost = mix(self.frost_top, self.frost_bot, self.pos.y)
let fill  = mix(base, vec4(1.0, 1.0, 1.0, 1.0), frost)
```

Defaults `frost_top = frost_bot = 1.0`, `frost_tint = 0.0` reproduce today's
flat `atlas.field_bg` exactly, so this unit is inert until unit 4 sets the
knobs.

Isolated as its own unit because the interior fill is the layer most likely to
need a taste pass, and it should not be entangled with the shadow work.

**Sign-off:** card interiors read top-lit rather than flat white.

## Unit 4 — knob presets and the selected lift

Three DSL objects in `frame.rs`, carrying the svelte numbers from the table
above:

```
mod.draw.HudPanel = mod.draw.AccentFrame{ frost_top: 0.95 frost_bot: 0.82 frost_tint: 0.06 depth_y: 12.0 depth_blur: 30.0 depth_a: 0.20 bloom: 0.16 }
mod.draw.HudNode  = mod.draw.AccentFrame{ frost_top: 0.94 frost_bot: 0.80 frost_tint: 0.06 depth_y:  8.0 depth_blur: 22.0 depth_a: 0.14 bloom: 0.18 }
mod.draw.HudBtn   = mod.draw.AccentFrame{ frost_top: 0.92 frost_bot: 0.74 frost_tint: 0.10 depth_y:  6.0 depth_blur: 18.0 depth_a: 0.14 bloom: 0.22 }
```

Consumers repoint: `canvas.rs:172` -> `HudNode`; `selection_toolbar.rs:26`,
`popup/select.rs:83`, `popup/menu.rs:316`, `popup/conflict_list.rs:144` ->
`HudPanel`. `HudBtn` is declared for `icon_button`/`ToolDock` to adopt later;
declaring it now keeps the three presets together and readable as one table.

**Selected lift.** The existing `selected` uniform is already 0.0/1.0 and
already widens the stroke. It additionally drives depth toward
`0 8px 22px @ .14` and bloom to `26px @ .28`, per `canvas.css:21-26`. The
canvas pushes the same uniform it pushes today; no new plumbing.

**Sign-off:** panel, node, and button depths read distinctly; selecting a node
visibly lifts it off the canvas.

## Fork constraints the shader must respect

Both are already documented in-tree and both have burned this codebase before:

- **`sdf.box(..., 0.0)` degenerates and floods the fill** (`frame.rs:35`).
  Sharp corners stay `sdf.rect`. The shadow and bloom falloffs are computed
  longhand from a box distance expression rather than via a rounded `box`.
- **`if` on a uniform silently no-ops in this fork's shader VM**
  (`canvas.rs:106`). Every knob must be branchless `mix`/`clamp` arithmetic —
  no early-out for "shadow disabled". A zeroed `depth_a` must fall out of the
  math, not out of a branch.

## Non-goals

- Docked panels and the project tree (see Scope).
- Rounded-corner surfaces. `--round: 0`; chips' `--round-chip: 2px` is a
  separate concern and stays out.
- The svelte `nodeglow` / `glowpulse` keyframe animations. Static material only.
- Any change to hit-testing or event routing (see below — there is none).

## Hit-testing is unaffected

Inflating the drawn quad grows `draw_frame.area()`. This was checked against
every routing path before choosing the approach, and changes nothing:

- All three popups decide containment on their own geometry, not the draw area
  — e.g. `self.geom.panel_rect().contains(e.abs)` at `popup/select.rs:449`.
- `draw_frame.area()` appears only as an opaque "claimed" token stamped into
  `e.handled` (`popup/select.rs:445,454`), where makepad needs a non-empty
  `Area` and never reads it as a rect.
- `PopupRoot::route`'s underlay swallowing is verdict-driven, and its
  `handled` stamp is the same opaque token.

No unit may change a hit rect. If one appears to need to, that is a defect in
the unit, not a licensed change.

## Clipping

Node shadows clip against the canvas viewport edge. This matches DOM `overflow`
behaviour in the svelte reference and is deliberately not special-cased.

## Verification

Shaders are not unit-testable in this harness (same constraint the phase-1 spec
recorded), so verification is split:

- **`cargo test`** covers `hud_bleed` — zero knobs give the `+2.0` floor, blur
  and offset add, bloom dominates when larger, and the result scales with zoom.
- **Build gate** — `cargo test --workspace` green per unit.
- **Visual** — launch the worktree's *own* `scripts/run-native.ps1` (it builds
  the checkout the script lives in, not the cwd), screenshot by specific pid,
  and compare against the svelte render. Never capture or kill by process name;
  that hits the user's own open editor.

## Delivery

One unit at a time on the `hud-material` worktree. Each unit gets its own
commit, its own visual verification, and an explicit sign-off from redoz@
before it is integrated to main. Nothing auto-pushes to `origin/main` — the
default `implement-plan` flow, which fast-forward-pushes each green unit, is
not used here.
