# DrawHudFrame — reusable Atlas HUD frame (phase 1)

**Date:** 2026-07-18
**Status:** approved, ready for plan

## Goal

A reusable Makepad draw primitive that renders the Atlas "HUD" frame — the
thin source-bright accent stroke whose color fades diagonally across the box —
around arbitrary content. Today the frame is hand-inlined inside
`GraphCanvas`'s `draw_node` shader; it is not reusable, and its gradient angle
is wrong (45° instead of the svelte reference's 150°). This phase extracts a
single `DrawHudFrame` draw struct, fixes the angle to match svelte, and
rewires `draw_node` onto it as the first consumer and fidelity checkpoint.

Full HUD material (frost-gradient fill + depth shadow + bloom glow, with
panel/node/button knob variants — the complete svelte `.hud-surface`) is
**phase C**, a later change that adds uniforms to this same struct. Out of
scope here.

## Reference (svelte `.hud-surface::before`)

The effect being reproduced, from `packages/web/src/atlas-components.css:26-34`
and tokens in `packages/web/src/atlas.css`:

- Border ring via a masked pseudo-element, `padding: var(--bw)` = **1.5px**.
- Fill: `linear-gradient(150deg, rgba(accent,.95), rgba(accent,.5))` — a
  directional **opacity** ramp along a 150° line, bright top-left → dim
  bottom-right. The "fade" is this alpha gradient, not a blur.
- `--round: 0` → surfaces are **sharp-cornered** (chips use `--round-chip: 2px`).
- `--accent: 20,150,220` (#1496DC).

Makepad theme tokens already encode the two stops correctly:
`frame_hi #x1496dcf2` (α≈.95), `frame_lo #x1496dc80` (α≈.50) in
`theme_atlas.rs`. So the only stroke-fidelity gap is the **gradient angle**.

## Unit: `DrawHudFrame`

New file `crates/waml-editor/src/draw_hud.rs`. A `DrawQuad`-derived draw
struct — the makepad-idiomatic reusable shader primitive (mirrors the fork's
own gradient-border button in `widgets/src/button.rs`). Any widget declares it
as a field and calls `draw_abs(cx, rect)`; the caller owns layout.

```rust
#[derive(Live, LiveHook, LiveRegister)]
#[repr(C)]
struct DrawHudFrame {
    #[deref] draw_super:  DrawQuad,
    #[live] fill:         Vec4,   // interior fill (field_bg today; -> transparent/frost in phase C)
    #[live] border_hi:    Vec4,   // bright gradient stop (frame_hi)
    #[live] border_lo:    Vec4,   // dim gradient stop    (frame_lo)
    #[live] border_width: f32,    // 1.5
    #[live] radius:       f32,    // 0 for surfaces; 2 for chips later
    #[live] angle:        f32,    // CSS gradient angle in degrees (150)
}
```

### Shader (`pixel`)

Angle math stays in the shader — it's a single trig call on a constant
uniform; a Rust precompute seam was considered and rejected as low-value
(it would only test that `sin`/`cos` were typed correctly; the real
correctness — does 150° match svelte — is eyeballed regardless).

```
let sdf = Sdf2d.viewport(self.pos * self.rect_size)
let bw  = self.border_width
sdf.box(bw * 0.5, bw * 0.5, self.rect_size.x - bw, self.rect_size.y - bw, self.radius)
sdf.fill_keep(self.fill)
let a    = radians(self.angle)
let dir  = vec2(sin(a), -cos(a))          // y-down: 0->(0,-1) top, 90->(1,0) right, 150->(0.5,0.866)
let span = abs(dir.x) + abs(dir.y)        // corner-projection span; normalizes stops to the box (CSS behavior)
let t    = clamp(dot(self.pos, dir) / span, 0.0, 1.0)
sdf.stroke(mix(self.border_hi, self.border_lo, t), bw)
return sdf.result
```

`sdf.box` with `radius: 0` renders sharp corners, matching `--round: 0`.
`radius` is a live uniform so chips/future rounded surfaces reuse the struct.

## Consumer / fidelity checkpoint: `draw_node`

Rewire the existing node frame onto `DrawHudFrame` — same tokens, corrected
angle, now reusable.

- `live_design!` in `canvas.rs` registers `DrawHudFrame` (`LiveRegister`).
- The `GraphCanvasBase` Rust struct's `draw_node` field changes type
  `DrawQuad` → `DrawHudFrame`. Its `draw_abs` call site is unchanged.
- The DSL `draw_node +: { pixel: fn(){...} }` block collapses to value
  overrides:
  ```
  draw_node: <DrawHudFrame>{
      fill: atlas.field_bg
      border_hi: atlas.frame_hi
      border_lo: atlas.frame_lo
      border_width: 1.5
      radius: 0.0
      angle: 150.0
  }
  ```

## Scope guard

Phase 1 = frame stroke + flat fill passthrough only. **No** depth shadow, **no**
bloom glow, **no** frost-gradient fill — those are phase C, adding uniforms to
this same struct. `draw_node` keeps its flat `field_bg` fill.

## Verification

Shaders are not unit-testable in this harness, so verification is build + run +
visual compare (no new `cargo test`):

1. `cargo build -p waml-editor` — compiles clean.
2. `cargo run -p waml-editor <sample diagram>` — load a diagram, eyeball a
   node frame against the svelte render: 150° tilt (more vertical bias than the
   old 45°), sharp corners, bright top-left → dim bottom-right.
