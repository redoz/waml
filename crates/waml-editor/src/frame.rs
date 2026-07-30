//! `AccentFrame`: the one reusable Atlas surface primitive, used app-wide.
//!
//! "Surface", not "HUD": the svelte class this ports is called `.hud-surface`,
//! but nothing here is a heads-up display -- it's the material worn by canvas
//! nodes, panels, popups and buttons alike. References to the CSS class keep
//! its real name; our own identifiers say `surface`.
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
//! Phase C adds the rest of the material: depth shadow, accent bloom,
//! theme-aware frost fill, and panel/node/button knob presets.
//!
//! Shadow and bloom paint OUTSIDE the surface, but the SDF is clamped to
//! `rect_size` -- so the drawn quad has to be bigger than the surface it frames.
//! `bleed` is that padding: the shader offsets its geometry inward by it so the
//! frame still lands on the true rect, and [`SurfaceExt::draw_surface_abs`]
//! does the padding for the caller. Consumers call `draw_surface_abs` instead
//! of `draw_abs` and get the shadow with no geometry math of their own.

use makepad_widgets::*;

/// Floor applied to `zoom` before it scales the depth shadow, so the shadow
/// doesn't vanish at fit-zoom -- the same idea as the stroke's `max(1.25, inset)`
/// in the shader below. Both [`surface_bleed`]'s caller and the shader apply
/// it, or the padded quad and the drawn shadow disagree and the shadow clips.
///
/// `allow(dead_code)` here and on the two items below: `node_editor_harness`
/// pulls this module in by `#[path]` for its `script_mod` alone and never calls
/// `draw_surface_abs`, so the whole Rust-side seam is genuinely unreferenced in
/// that binary even though the editor uses all of it.
#[allow(dead_code)]
pub const SURFACE_SHADOW_ZOOM_FLOOR: f64 = 0.35;

/// Master scale and CSS radius for the resting accent bloom. These values are
/// repeated in the shader because the script VM cannot read Rust constants.
#[allow(dead_code)]
pub const SURFACE_GLOW: f64 = 0.4;
#[allow(dead_code)]
pub const SURFACE_BLOOM_PX: f64 = 14.0;

/// Selected canvas-node depth and bloom values. Rust uses them to size the
/// padded quad; the shader uses the matching literals to draw the material.
#[allow(dead_code)]
pub const SURFACE_SEL_DEPTH_Y: f64 = 8.0;
#[allow(dead_code)]
pub const SURFACE_SEL_DEPTH_BLUR: f64 = 22.0;
#[allow(dead_code)]
pub const SURFACE_SEL_DEPTH_A: f64 = 0.14;
#[allow(dead_code)]
pub const SURFACE_SEL_BLOOM_PX: f64 = 26.0;
#[allow(dead_code)]
pub const SURFACE_SEL_BLOOM_A: f64 = 0.28;

/// Total CSS border thickness in logical pixels at zoom 1.0.
#[allow(dead_code)]
pub const SURFACE_BORDER_PX: f64 = 1.5;

/// Round a logical coordinate to the device-pixel grid.
#[allow(dead_code)]
pub fn surface_snap(value: f64, dpi: f64) -> f64 {
    (value * dpi).round() / dpi
}

/// Round padding up so snapping cannot clip an outside-the-box layer.
#[allow(dead_code)]
pub fn surface_snap_up(value: f64, dpi: f64) -> f64 {
    (value * dpi).ceil() / dpi
}

/// Mirror the shader's selected-value mix for padding calculations.
#[allow(dead_code)]
pub fn surface_lift(base: f64, selected_value: f64, selected: f64) -> f64 {
    base + (selected_value - base) * selected.clamp(0.0, 1.0)
}

/// Pixels of padding a surface needs on every side for its shadow and bloom to
/// fall outside the frame without clipping.
///
/// `depth_blur + depth_y` covers the downward-offset shadow's far edge;
/// `bloom_px` is the un-offset halo radius; `+ 2.0` is antialias slack. `zoom`
/// is the already-floored effective zoom (see [`SURFACE_SHADOW_ZOOM_FLOOR`])
/// and must match what the shader scales by.
#[allow(dead_code)]
pub fn surface_bleed(depth_y: f64, depth_blur: f64, bloom_px: f64, zoom: f64) -> f64 {
    ((depth_blur + depth_y).max(bloom_px) * zoom).max(0.0) + 2.0
}

/// The generic seam: draw an `AccentFrame`-derived pen at `rect`, padding the
/// drawn quad so the depth shadow and bloom have room to fall outside the
/// surface.
#[allow(dead_code)]
pub trait SurfaceExt {
    /// Reads the pen's own knob uniforms, computes the bleed, pushes it, and
    /// draws the inflated quad. `rect` stays the TRUE surface rect -- the frame
    /// lands exactly where `draw_abs(cx, rect)` would have put it.
    ///
    /// Only for pens derived from `mod.draw.AccentFrame`; a pen without the
    /// `bleed` uniform would take the padding without compensating for it.
    fn draw_surface_abs(&mut self, cx: &mut Cx2d, rect: Rect);
}

impl SurfaceExt for DrawColor {
    fn draw_surface_abs(&mut self, cx: &mut Cx2d, rect: Rect) {
        let read = |pen: &Self, cx: &mut Cx2d, id: LiveId| -> f64 {
            let mut slot = [0.0f32];
            pen.get_uniform(cx, id, &mut slot);
            slot[0] as f64
        };
        let bloom_px = if read(self, cx, live_id!(bloom)) > 0.0 {
            SURFACE_BLOOM_PX * SURFACE_GLOW
        } else {
            0.0
        };
        let selected = read(self, cx, live_id!(selected));
        let bleed = surface_bleed(
            surface_lift(
                read(self, cx, live_id!(depth_y)),
                SURFACE_SEL_DEPTH_Y,
                selected,
            ),
            surface_lift(
                read(self, cx, live_id!(depth_blur)),
                SURFACE_SEL_DEPTH_BLUR,
                selected,
            ),
            surface_lift(bloom_px, SURFACE_SEL_BLOOM_PX, selected),
            read(self, cx, live_id!(zoom)).max(SURFACE_SHADOW_ZOOM_FLOOR),
        );
        let dpi = cx.current_dpi_factor();
        let bleed = surface_snap_up(bleed, dpi);
        let pos = dvec2(
            surface_snap(rect.pos.x, dpi) - bleed,
            surface_snap(rect.pos.y, dpi) - bleed,
        );
        let size = dvec2(
            surface_snap(rect.size.x, dpi) + bleed * 2.0,
            surface_snap(rect.size.y, dpi) + bleed * 2.0,
        );
        self.set_uniform(cx, live_id!(bleed), &[bleed as f32]);
        self.draw_abs(cx, Rect { pos, size });
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas

    // The gradient stops default to the Atlas tokens; a consumer overrides only
    // the per-instance `color` fill. Frame geometry scales by `zoom *
    // stroke_scale`: the canvas pushes both per frame, and CAD supplies inverse
    // zoom for `stroke_scale` to hold linework fixed in screen space. Other
    // consumers leave `stroke_scale` at 1.0, preserving their raw-zoom behavior;
    // panels also leave `zoom` at its default 1.0.
    // `selected` (0.0/1.0) widens the inset+stroke ~1.5x for the canvas's picked
    // node; the canvas pushes it per node before draw_abs, same as `zoom`.
    // Everyone else leaves it at 0.0 (the common, visually-unchanged path).
    // `grey` (0.0/1.0) desaturates the accent stroke to its own luminance -- the
    // canvas mutes a card that is neither the picked node nor a constraint
    // neighbour of it under the Selected visibility POV (`FocusState`, canvas.rs).
    // The near-white fill has no chroma to lose, so the stroke is the whole tell.
    // Default 0.0: full-chroma stroke, the common unchanged path.
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
        stroke_scale: uniform(1.0)
        selected: uniform(0.0)
        grey: uniform(0.0)
        // Padding the CALLER added on every side so the shadow has room to fall
        // outside the surface (`SurfaceExt::draw_surface_abs` pushes it). At the
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
        bloom: uniform(0.0)
        accent_col: uniform(atlas.accent)
        frost_top: uniform(1.0)
        frost_bot: uniform(1.0)
        frost_tint: uniform(0.0)
        ground_col: uniform(atlas.ground)
        pixel: fn() {
            // Border size uses `zoom * stroke_scale`: CAD supplies inverse zoom
            // through stroke_scale for stable screen-space linework.
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            let bw_dev = max(1.0, floor(1.5 * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected) * dpi + 0.5))
            // `stroke` takes a half-width. The extra half device pixel moves
            // its antialias ramp away from the crisp border samples.
            let sw = (bw_dev * 0.5 + 0.5) / dpi
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)

            // Outside layers use raw zoom, never stroke_scale. Linework stays
            // screen-space while material remains attached to the surface.
            let z = max(0.35, self.zoom)
            let dy = mix(self.depth_y, 8.0, self.selected)
            let dblur = mix(self.depth_blur, 22.0, self.selected)
            let da = mix(self.depth_a, 0.14, self.selected)
            let hw = (self.rect_size.x - self.bleed * 2.0) * 0.5
            let hh = (self.rect_size.y - self.bleed * 2.0) * 0.5
            let p = self.pos * self.rect_size
            let qx = abs(p.x - (self.bleed + hw)) - hw
            let ox = max(qx, 0.0)
            let sqy = abs(p.y - (self.bleed + hh + dy * z)) - hh
            let soy = max(sqy, 0.0)
            let sd = sqrt(ox * ox + soy * soy) + min(max(qx, sqy), 0.0)
            let sblur = max(1.0, dblur * z)
            let salpha = da * (1.0 - smoothstep(-sblur, sblur, sd))

            // Bloom is un-displaced, so it rings the surface evenly.
            let glow = 0.4
            let bpx = mix(14.0 * glow, 26.0, self.selected)
            let ba = mix(self.bloom * glow, 0.28, self.selected)
            let bqy = abs(p.y - (self.bleed + hh)) - hh
            let boy = max(bqy, 0.0)
            let bd = sqrt(ox * ox + boy * boy) + min(max(qx, bqy), 0.0)
            let bblur = max(1.0, bpx * z)
            let balpha = ba * (1.0 - smoothstep(-bblur, bblur, bd))
            let orgb = self.accent_col.rgb * balpha + self.shadow_col.rgb * salpha * (1.0 - balpha)
            let oa = balpha + salpha * (1.0 - balpha)
            sdf.clear(vec4(orgb / max(oa, 0.0001), oa))

            // Draw from the snapped padding plus half the border, which puts
            // both edges on device-pixel boundaries.
            let ctr = (self.bleed * dpi + bw_dev * 0.5) / dpi
            sdf.rect(ctr, ctr, self.rect_size.x - ctr * 2.0, self.rect_size.y - ctr * 2.0)
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            let ux = (p.x - self.bleed) / max(1.0, self.rect_size.x - self.bleed * 2.0)
            let uy = (p.y - self.bleed) / max(1.0, self.rect_size.y - self.bleed * 2.0)
            let ghost = mix(self.ground_col, self.accent_col, self.frost_tint)
            let frost = mix(self.frost_top, self.frost_bot, uy)
            sdf.fill_keep(mix(ghost, self.color, frost))
            let t = clamp((ux * dir.x + uy * dir.y) / span, 0.0, 1.0)
            let col = mix(self.border_hi, self.border_lo, t)
            let k = clamp((1.0 - self.zoom) * 2.0, 0.0, 0.85)
            let g = col.rgb
            let lum = g.x * 0.299 + g.y * 0.587 + g.z * 0.114
            let scol = mix(col.rgb, vec3(lum, lum, lum), self.grey)
            sdf.stroke(vec4(scol, mix(col.a, 1.0, k)), sw)
            return sdf.result
        }
    }

    mod.draw.PanelSurface = mod.draw.AccentFrame{
        frost_top: 0.95 frost_bot: 0.82 frost_tint: 0.06
        depth_y: 12.0 depth_blur: 30.0 depth_a: 0.20 bloom: 0.16
    }
    mod.draw.NodeSurface = mod.draw.AccentFrame{
        frost_top: 0.94 frost_bot: 0.80 frost_tint: 0.06
        depth_y: 8.0 depth_blur: 22.0 depth_a: 0.14 bloom: 0.18
    }
    mod.draw.ButtonSurface = mod.draw.AccentFrame{
        frost_top: 0.92 frost_bot: 0.74 frost_tint: 0.10
        depth_y: 6.0 depth_blur: 18.0 depth_a: 0.14 bloom: 0.22
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
        approx(surface_bleed(0.0, 0.0, 0.0, 1.0), 2.0);
    }

    #[test]
    fn offset_and_blur_add() {
        // The far edge of a shadow blurred by 22 and pushed down 8.
        approx(surface_bleed(8.0, 22.0, 0.0, 1.0), 32.0);
    }

    #[test]
    fn bloom_dominates_when_larger() {
        approx(surface_bleed(2.0, 4.0, 20.0, 1.0), 22.0);
        // ...and loses when it isn't.
        approx(surface_bleed(12.0, 30.0, 20.0, 1.0), 44.0);
    }

    #[test]
    fn scales_with_zoom() {
        approx(surface_bleed(8.0, 22.0, 0.0, 0.5), 17.0);
        approx(surface_bleed(8.0, 22.0, 0.0, 2.0), 62.0);
    }

    #[test]
    fn snapping_lands_on_the_device_grid() {
        approx(surface_snap(10.4, 1.0), 10.0);
        approx(surface_snap(10.6, 1.0), 11.0);
        approx(surface_snap(10.5, 2.0), 10.5);
        approx(surface_snap(10.3, 2.0), 10.5);
        approx(surface_snap_up(10.1, 1.0), 11.0);
        approx(surface_snap_up(10.0, 1.0), 10.0);
        approx(surface_snap_up(10.1, 2.0), 10.5);
    }

    #[test]
    fn selection_lifts_each_knob_to_the_reference_value() {
        approx(surface_lift(12.0, SURFACE_SEL_DEPTH_Y, 0.0), 12.0);
        approx(surface_lift(12.0, SURFACE_SEL_DEPTH_Y, 1.0), 8.0);
        approx(surface_lift(0.0, SURFACE_SEL_BLOOM_PX, 1.0), 26.0);
        approx(surface_lift(0.0, SURFACE_SEL_BLOOM_PX, 2.0), 26.0);
        approx(surface_lift(0.0, SURFACE_SEL_BLOOM_PX, -1.0), 0.0);
    }

    #[test]
    fn selection_grows_the_bleed_for_a_shallow_surface() {
        let bleed_at = |depth_y: f64, depth_blur: f64, bloom: bool, selected: f64| {
            let bloom_px = if bloom {
                SURFACE_BLOOM_PX * SURFACE_GLOW
            } else {
                0.0
            };
            surface_bleed(
                surface_lift(depth_y, SURFACE_SEL_DEPTH_Y, selected),
                surface_lift(depth_blur, SURFACE_SEL_DEPTH_BLUR, selected),
                surface_lift(bloom_px, SURFACE_SEL_BLOOM_PX, selected),
                1.0,
            )
        };

        approx(bleed_at(6.0, 18.0, true, 0.0), 26.0);
        approx(bleed_at(6.0, 18.0, true, 1.0), 32.0);
    }

    #[test]
    fn shader_constants_match_the_padding_contract() {
        let src = include_str!("frame.rs");
        assert!(src.contains(&format!("let glow = {SURFACE_GLOW:.1}")));
        assert!(src.contains(&format!(
            "let bpx = mix({SURFACE_BLOOM_PX:.1} * glow, {SURFACE_SEL_BLOOM_PX:.1}, self.selected)"
        )));
        assert!(src.contains(&format!(
            "let dy = mix(self.depth_y, {SURFACE_SEL_DEPTH_Y:.1}, self.selected)"
        )));
        assert!(src.contains(&format!(
            "let dblur = mix(self.depth_blur, {SURFACE_SEL_DEPTH_BLUR:.1}, self.selected)"
        )));
        assert!(src.contains(&format!(
            "let da = mix(self.depth_a, {SURFACE_SEL_DEPTH_A:.2}, self.selected)"
        )));
        assert!(src.contains(&format!(
            "let ba = mix(self.bloom * glow, {SURFACE_SEL_BLOOM_A:.2}, self.selected)"
        )));
        assert!(src.contains(&format!(
            "let bw_dev = max(1.0, floor({SURFACE_BORDER_PX} * self.zoom * self.stroke_scale * mix(1.0, 1.5, self.selected) * dpi + 0.5))"
        )));
    }

    #[test]
    fn never_goes_below_the_floor() {
        // Nonsense knobs can't shrink the quad below the surface it frames.
        approx(surface_bleed(-100.0, 0.0, 0.0, 1.0), 2.0);
        approx(surface_bleed(8.0, 22.0, 0.0, 0.0), 2.0);
    }
}
