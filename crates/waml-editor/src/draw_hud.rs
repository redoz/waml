//! `HudFrame`: the one reusable Atlas "HUD" frame primitive, used app-wide.
//!
//! A `DrawColor` whose interior is a flat fill (`color`) ringed by the Atlas
//! source-bright accent stroke -- a thin border whose color fades along a
//! 150deg diagonal, bright top-left (`border_hi`) to dim bottom-right
//! (`border_lo`). This reproduces the svelte `.hud-surface::before` masked
//! gradient border (see `docs/superpowers/specs/2026-07-18-draw-hud-frame-design.md`):
//! the "fade" is the stroke's alpha gradient, NOT a blur.
//!
//! Reuse follows the fork's own gradient-border pattern (`widgets/src/button.rs`
//! declares its shader inline on a `DrawColor` rather than a bespoke Rust draw
//! struct). Any widget declares a field `draw_x: DrawColor`, points its DSL at
//! `mod.draw.HudFrame{ ... }`, and calls `draw_abs`; the caller owns layout.
//!
//! Phase 1 draws stroke + flat fill only. The full `.hud-surface` material
//! (frost-gradient fill + depth shadow + bloom glow, with panel/node/button
//! knob variants) is a later phase that adds uniforms to this same prototype.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // Knobs default to the Atlas tokens; a consumer overrides only what differs
    // (usually just `color`). `angle` is a CSS linear-gradient angle in degrees;
    // the shader converts it with *pi/180 (radians() is avoided so the shader
    // leans only on sin/cos, which the fork's own shaders use). `span` normalizes
    // the two gradient stops to the box corners, matching CSS behavior. The
    // projection is written out longhand to avoid depending on dot().
    mod.draw.HudFrame = mod.draw.DrawColor{
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        border_width: uniform(1.5)
        radius: uniform(0.0)
        angle: uniform(150.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let bw = self.border_width
            sdf.box(bw, bw, self.rect_size.x - bw * 2.0, self.rect_size.y - bw * 2.0, self.radius)
            sdf.fill_keep(self.color)
            let a = self.angle * 0.017453293
            let dir = vec2(sin(a), -cos(a))
            let span = abs(dir.x) + abs(dir.y)
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
            sdf.stroke(mix(self.border_hi, self.border_lo, t), bw)
            return sdf.result
        }
    }
}
