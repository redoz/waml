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

    // The gradient stops default to the Atlas tokens; a consumer overrides only
    // the per-instance `color` fill. `zoom` scales the 1.5px border inset +
    // stroke width so a canvas node's frame thickens with its zoomed box instead
    // of staying a fixed screen-pixel hairline; the canvas pushes it per frame
    // via set_uniform. Panels leave it at the default 1.0 (screen-space, no zoom).
    //
    // Sharp corners use `sdf.rect`, NOT `sdf.box(..., 0.0)`: a zero corner radius
    // degenerates `box` and floods the fill (rounded variants get their own
    // primitive). The 150deg CSS gradient direction is precomputed:
    // (sin150, -cos150) = (0.5, 0.866), y-down; `span` = |x|+|y| normalizes the
    // stops to the box corners (CSS behavior); projection is longhand (no dot()).
    mod.draw.HudFrame = mod.draw.DrawColor{
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        zoom: uniform(1.0)
        pixel: fn() {
            let inset = 1.5 * self.zoom
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
            sdf.fill_keep(self.color)
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
            sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
            return sdf.result
        }
    }
}
