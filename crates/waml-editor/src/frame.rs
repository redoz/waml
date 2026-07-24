//! `AccentFrame`: the one reusable Atlas "HUD" frame primitive, used app-wide.
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
//! `mod.draw.AccentFrame{ ... }`, and calls `draw_abs`; the caller owns layout.
//!
//! Phase C fills in the rest of the material one layer at a time (see
//! `docs/superpowers/specs/2026-07-24-hud-material-phase-c-design.md`). Layer 3,
//! the depth shadow, is in. Bloom glow, frost-gradient fill and the
//! panel/node/button knob presets still land on this same prototype.
//!
//! Shadow and bloom paint OUTSIDE the surface, but the SDF is clamped to
//! `rect_size` -- so the drawn quad has to be bigger than the surface it frames.
//! `bleed` is that padding: the shader offsets its geometry inward by it so the
//! frame still lands on the true rect, and [`HudFrameExt::draw_hud_abs`] does the
//! padding for the caller. Consumers call `draw_hud_abs` instead of `draw_abs`
//! and get the shadow with no geometry math of their own.

use makepad_widgets::*;

/// Floor applied to `zoom` before it scales the depth shadow, so the shadow
/// doesn't vanish at fit-zoom -- the same idea as the stroke's `max(1.25, inset)`
/// in the shader below. Both [`hud_bleed`]'s caller and the shader apply it, or
/// the padded quad and the drawn shadow disagree and the shadow clips.
///
/// `allow(dead_code)` here and on the two items below: `node_editor_harness`
/// pulls this module in by `#[path]` for its `script_mod` alone and never calls
/// `draw_hud_abs`, so the whole Rust-side seam is genuinely unreferenced in that
/// binary even though the editor uses all of it.
#[allow(dead_code)]
pub const HUD_SHADOW_ZOOM_FLOOR: f64 = 0.35;

/// Pixels of padding a HUD surface needs on every side for its shadow and bloom
/// to fall outside the frame without clipping.
///
/// `depth_blur + depth_y` covers the downward-offset shadow's far edge;
/// `bloom_px` is the un-offset halo radius; `+ 2.0` is antialias slack. `zoom` is
/// the already-floored effective zoom (see [`HUD_SHADOW_ZOOM_FLOOR`]) and must
/// match what the shader scales by.
#[allow(dead_code)]
pub fn hud_bleed(depth_y: f64, depth_blur: f64, bloom_px: f64, zoom: f64) -> f64 {
    ((depth_blur + depth_y).max(bloom_px) * zoom).max(0.0) + 2.0
}

/// The generic seam: draw an `AccentFrame`-derived pen at `rect`, padding the
/// drawn quad so the depth shadow and bloom have room to fall outside the
/// surface.
#[allow(dead_code)]
pub trait HudFrameExt {
    /// Reads the pen's own knob uniforms, computes the bleed, pushes it, and
    /// draws the inflated quad. `rect` stays the TRUE surface rect -- the frame
    /// lands exactly where `draw_abs(cx, rect)` would have put it.
    ///
    /// Only for pens derived from `mod.draw.AccentFrame`; a pen without the
    /// `bleed` uniform would take the padding without compensating for it.
    fn draw_hud_abs(&mut self, cx: &mut Cx2d, rect: Rect);
}

impl HudFrameExt for DrawColor {
    fn draw_hud_abs(&mut self, cx: &mut Cx2d, rect: Rect) {
        let read = |pen: &Self, cx: &mut Cx2d, id: LiveId| -> f64 {
            let mut slot = [0.0f32];
            pen.get_uniform(cx, id, &mut slot);
            slot[0] as f64
        };
        // `bloom` doesn't exist yet; get_uniform leaves the slot untouched, so an
        // absent knob reads as 0.0 and simply doesn't widen the bleed.
        let bleed = hud_bleed(
            read(self, cx, live_id!(depth_y)),
            read(self, cx, live_id!(depth_blur)),
            read(self, cx, live_id!(bloom)),
            read(self, cx, live_id!(zoom)).max(HUD_SHADOW_ZOOM_FLOOR),
        );
        self.set_uniform(cx, live_id!(bleed), &[bleed as f32]);
        self.draw_abs(
            cx,
            Rect {
                pos: rect.pos - dvec2(bleed, bleed),
                size: rect.size + dvec2(bleed * 2.0, bleed * 2.0),
            },
        );
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // The gradient stops default to the Atlas tokens; a consumer overrides only
    // the per-instance `color` fill. `zoom` scales the 1.5px border inset +
    // stroke width so a canvas node's frame thickens with its zoomed box instead
    // of staying a fixed screen-pixel hairline; the canvas pushes it per frame
    // via set_uniform. Panels leave it at the default 1.0 (screen-space, no zoom).
    // `selected` (0.0/1.0) widens the inset+stroke ~1.5x for the canvas's picked
    // node; the canvas pushes it per node before draw_abs, same as `zoom`.
    // Everyone else leaves it at 0.0 (the common, visually-unchanged path).
    //
    // Sharp corners use `sdf.rect`, NOT `sdf.box(..., 0.0)`: a zero corner radius
    // degenerates `box` and floods the fill (rounded variants get their own
    // primitive). The 150deg CSS gradient direction is precomputed:
    // (sin150, -cos150) = (0.5, 0.866), y-down; `span` = |x|+|y| normalizes the
    // stops to the box corners (CSS behavior); projection is longhand (no dot()).
    mod.draw.AccentFrame = mod.draw.DrawColor{
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        zoom: uniform(1.0)
        selected: uniform(0.0)
        // Padding the CALLER added on every side so the shadow has room to fall
        // outside the surface (`HudFrameExt::draw_hud_abs` pushes it). At the
        // default 0.0 the geometry below is byte-for-byte the pre-phase-C frame,
        // so a consumer still on plain `draw_abs` is visually unchanged.
        bleed: uniform(0.0)
        // Depth-shadow knobs, CSS box-shadow semantics: y offset, blur radius,
        // alpha. All 0.0 by default -- `depth_a` is the master gate and zeroes
        // the layer through arithmetic, NOT a branch (an `if` on a uniform
        // silently no-ops in this fork's shader VM; see `canvas.rs`'s EdgeMarker).
        depth_y: uniform(0.0)
        depth_blur: uniform(0.0)
        depth_a: uniform(0.0)
        shadow_col: uniform(atlas.shadow)
        pixel: fn() {
            // Selection widens the border ~1.5x: mix() lifts the 1.5px base to
            // 2.25px when selected == 1.0, leaving the unselected path untouched.
            let inset = 1.5 * self.zoom * mix(1.0, 1.5, self.selected)
            // Stroke width floors to a 1px screen-space hairline so the frame
            // never smears sub-pixel (and fades) when zoomed out, mirroring the
            // canvas EdgeLine pen. The rect inset stays proportional; only the
            // stroke is floored, so it centers on the box edge at low zoom.
            let sw = max(1.25, inset)
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)

            // --- depth shadow, under every other layer -------------------
            // The shadow belongs to the surface in WORLD space, exactly as the
            // border inset does, so offset and blur scale with `zoom` -- floored
            // so it doesn't evaporate at fit-zoom.
            let z = max(0.35, self.zoom)
            // Box distance to the true surface rect (the quad minus the bleed),
            // pushed down by `depth_y`. Longhand and component-wise: `sdf.box`
            // degenerates at radius 0 and floods the fill, and assigning
            // sdf.shape/dist from a pixel fn silently fails this fork's VM, so
            // this stays plain float math outside the Sdf2d entirely.
            let hw = (self.rect_size.x - self.bleed * 2.0) * 0.5
            let hh = (self.rect_size.y - self.bleed * 2.0) * 0.5
            let p = self.pos * self.rect_size
            let qx = abs(p.x - (self.bleed + hw)) - hw
            let qy = abs(p.y - (self.bleed + hh + self.depth_y * z)) - hh
            let ox = max(qx, 0.0)
            let oy = max(qy, 0.0)
            let sd = sqrt(ox * ox + oy * oy) + min(max(qx, qy), 0.0)
            // CSS blur radius B ramps the shadow across roughly [-B/2, +B/2] of
            // the edge. Floored at 1px so the smoothstep never has equal edges.
            let sblur = max(1.0, self.depth_blur * z)
            let salpha = self.depth_a * (1.0 - smoothstep(-sblur * 0.5, sblur * 0.5, sd))
            // `clear` writes premultiplied straight into a still-empty result;
            // the fill/stroke below then composite source-over on top, so the
            // shadow is correctly hidden under the opaque card.
            sdf.clear(vec4(self.shadow_col.rgb, salpha))

            // --- frame: stroke + flat fill, inset past the bleed ----------
            let x0 = self.bleed + inset
            let y0 = self.bleed + inset
            sdf.rect(x0, y0, self.rect_size.x - x0 * 2.0, self.rect_size.y - y0 * 2.0)
            sdf.fill_keep(self.color)
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            // `self.pos` normalizes over the PADDED quad, so renormalize onto the
            // true surface or the gradient's stops drift off the frame corners
            // once bleed is non-zero. At bleed = 0 this is `self.pos` exactly.
            let ux = (p.x - self.bleed) / max(1.0, self.rect_size.x - self.bleed * 2.0)
            let uy = (p.y - self.bleed) / max(1.0, self.rect_size.y - self.bleed * 2.0)
            let t = clamp((ux * dir.x + uy * dir.y) / span, 0.0, 1.0)
            // Zoomed out the 1px hairline of pale accent (frame_lo fades to 50%
            // alpha) washes into the near-white field. Lift the stroke alpha
            // toward opaque as zoom drops -- non-linearly, so the border stays
            // legible at fit-zoom. At zoom >= 1 (panels, near cards) k = 0, so
            // the common path is unchanged.
            let col = mix(self.border_hi, self.border_lo, t)
            let k = clamp((1.0 - self.zoom) * 2.0, 0.0, 0.85)
            sdf.stroke(vec4(col.rgb, mix(col.a, 1.0, k)), sw)
            return sdf.result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn zero_knobs_give_the_antialias_floor() {
        approx(hud_bleed(0.0, 0.0, 0.0, 1.0), 2.0);
    }

    #[test]
    fn offset_and_blur_add() {
        // The far edge of a shadow blurred by 22 and pushed down 8.
        approx(hud_bleed(8.0, 22.0, 0.0, 1.0), 32.0);
    }

    #[test]
    fn bloom_dominates_when_larger() {
        approx(hud_bleed(2.0, 4.0, 20.0, 1.0), 22.0);
        // ...and loses when it isn't.
        approx(hud_bleed(12.0, 30.0, 20.0, 1.0), 44.0);
    }

    #[test]
    fn scales_with_zoom() {
        approx(hud_bleed(8.0, 22.0, 0.0, 0.5), 17.0);
        approx(hud_bleed(8.0, 22.0, 0.0, 2.0), 62.0);
    }

    #[test]
    fn never_goes_below_the_floor() {
        // Nonsense knobs can't shrink the quad below the surface it frames.
        approx(hud_bleed(-100.0, 0.0, 0.0, 1.0), 2.0);
        approx(hud_bleed(8.0, 22.0, 0.0, 0.0), 2.0);
    }
}
