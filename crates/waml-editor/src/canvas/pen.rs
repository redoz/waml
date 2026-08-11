//! One pen system for every diagram stroke.
//!
//! Two halves that never mix. The SHADER half (`mod.draw.CadPen`, below) owns
//! device quantisation, the half-pixel stroke bias, and the antialias
//! correction; every canvas pen derives from it. The RUST half (`Pen`) owns the
//! weight ladder and is pure -- no `Cx`, no dpi, no zoom.
//!
//! SPIKE VERDICT: Mode A -- the fork's shader VM resolves `self.pen_aa(...)`
//! through `mod.draw.EdgeLine = mod.draw.CadPen{...}`; the negative control
//! (`self.pen_does_not_exist(1.0)`) raised `shader call target is not a
//! function` on stderr during the headless UI suite, so the probe is not
//! blind and the real run passed clean. Inherited helper fns work.

use makepad_widgets::*;

/// A stroke weight, in logical pixels, on the one ladder both canvases share.
///
/// Pure: no `Cx`, no dpi, no zoom. A rung is what a line MEANS, not how many
/// device pixels it happens to occupy -- that is the shader's decision, made
/// from `width()`, which is the only number that crosses into a shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::canvas) struct Pen {
    lpx: f64,
}

impl Pen {
    const fn new(lpx: f64) -> Pen {
        Pen { lpx }
    }

    /// Card compartment dividers, group hulls, label leaders, behavior dividers.
    pub(in crate::canvas) const HAIRLINE: Pen = Pen::new(1.0);
    /// Card border, interaction frames, lifeline stems, origin overlay.
    pub(in crate::canvas) const LIGHT: Pen = Pen::new(1.5);
    /// Every connector: class edges, behavior routes, messages, ghost overlay.
    pub(in crate::canvas) const REGULAR: Pen = Pen::new(2.0);
    // The spec's fourth rung (HEAVY, 3.0) and an `emphasized(factor)`
    // multiplier are deliberately absent: nothing needs either yet, and the
    // only 1.5x that ships is `AccentFrame`'s in-shader
    // `mix(1.0, 1.5, self.selected)`. Add them in the change that first needs
    // one rather than shipping an API under `#[allow(dead_code)]`.

    /// The finished logical width a shader quantises. The ONLY number that
    /// crosses the CPU/GPU boundary.
    pub(in crate::canvas) fn width(self) -> f64 {
        self.lpx
    }
}

/// Whether a stroke from `a` to `b` runs along x. Routes, edges and messages
/// are orthogonal, so the longer delta is the run and the shorter is the
/// thickness axis. A degenerate pair reads as horizontal.
pub(in crate::canvas) fn band_is_horizontal(a: DVec2, b: DVec2) -> bool {
    (a.x - b.x).abs() >= (a.y - b.y).abs()
}

/// Whole device pixels a logical width resolves to, at `dpi`. The Rust mirror
/// of `CadPen::pen_dev`, and the ONE flat-fill quantiser: every flat-fill
/// snapper below ([`fill_band_at`], [`fill_at`]) goes through this rather than
/// restating the `+ 0.501` rounding term.
///
/// [`outline_at`] deliberately does not: an outline quad is a SHAPE, whose two
/// edges each belong on the device grid, not a rung whose thickness is the ink.
fn quantise_px(lpx: f64, dpi: f64) -> f64 {
    (lpx * dpi + 0.501).floor().max(1.0)
}

/// [`quantise_px`] with a rung's width in it -- the number a plain `DrawColor`
/// quad has to match to line up with a shader-inked stroke of the same pen.
pub(in crate::canvas) fn ink_px(pen: Pen, dpi: f64) -> f64 {
    quantise_px(pen.width(), dpi)
}

/// The quad a FLAT FILL inks a stroke from `a` to `b` inside -- this one IS the
/// ink. Whole device pixels on the thickness axis, starting on a device
/// boundary. A diagonal pair is treated as whichever axis it runs longest along
/// (routes and messages are orthogonal).
///
/// [`band`] is this same geometry grown for a `CadPen` to ink INSIDE; this is
/// the helper for a plain `DrawColor` that fills exactly what it is handed.
pub(in crate::canvas) fn fill_band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect {
    fill_band_at(a, b, pen, cx.current_dpi_factor())
}

/// The dpi-explicit core of [`fill_band`], so the geometry is testable without
/// a live `Cx2d`.
pub(in crate::canvas) fn fill_band_at(a: DVec2, b: DVec2, pen: Pen, dpi: f64) -> Rect {
    let thick_px = ink_px(pen, dpi);
    if band_is_horizontal(a, b) {
        let y0 = ((a.y + b.y) * 0.5 * dpi - thick_px * 0.5).round();
        let x0 = (a.x.min(b.x) * dpi).round();
        let x1 = (a.x.max(b.x) * dpi).round();
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2((x1 - x0).max(1.0) / dpi, thick_px / dpi),
        }
    } else {
        let x0 = ((a.x + b.x) * 0.5 * dpi - thick_px * 0.5).round();
        let y0 = (a.y.min(b.y) * dpi).round();
        let y1 = (a.y.max(b.y) * dpi).round();
        Rect {
            pos: dvec2(x0 / dpi, y0 / dpi),
            size: dvec2(thick_px / dpi, (y1 - y0).max(1.0) / dpi),
        }
    }
}

/// The quad a `CadPen` inks a stroke from `a` to `b` inside.
///
/// This is a CANVAS, not the stroke. It is [`fill_band`] grown one device pixel
/// past the widest band the pen can resolve to on each side of the centreline,
/// and the shader inks the quantised band within it -- the same arrangement
/// `AccentFrame` already uses to ink a border inside a card rect. Growing here
/// is what lets width stop being a CPU concern: the shader is free to disagree
/// with this rounding by a pixel without its stroke being clipped.
///
/// Only for quads a `CadPen`-derived pen inks inside. A quad that IS the ink
/// (a flat `DrawColor` fill) wants [`fill_band`] or [`fill`] instead.
pub(in crate::canvas) fn band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect {
    band_at(a, b, pen, cx.current_dpi_factor())
}

/// The dpi-explicit core of [`band`], split out so callers that need to unit
/// test the geometry they build ON TOP of a band (`labels.rs::leader_bars`)
/// can do so without a live `Cx2d`, the same way `band` itself is tested
/// here.
pub(in crate::canvas) fn band_at(a: DVec2, b: DVec2, pen: Pen, dpi: f64) -> Rect {
    // Growing the ink band by a whole device pixel per side is exact: shifting
    // the start by one pixel and the thickness by two is what rounding the
    // centreline against a two-pixel-wider band would have produced anyway, so
    // there is still only one snapping implementation here.
    let ink = fill_band_at(a, b, pen, dpi);
    let grow = 1.0 / dpi;
    if band_is_horizontal(a, b) {
        Rect {
            pos: dvec2(ink.pos.x, ink.pos.y - grow),
            size: dvec2(ink.size.x, ink.size.y + grow * 2.0),
        }
    } else {
        Rect {
            pos: dvec2(ink.pos.x - grow, ink.pos.y),
            size: dvec2(ink.size.x + grow * 2.0, ink.size.y),
        }
    }
}

/// The quad for a shape whose OUTLINE a pen strokes -- frames, group hulls,
/// fragment plates -- with its edges pulled onto the device grid, and
/// guaranteed wide enough that the shader's `pen_sw` inset cannot invert it.
///
/// NOT for a quad that IS the ink. The `2 * pen` floor below is the room a
/// `pen_sw` inset needs on BOTH sides of the shape; applied to a rect that is
/// itself a `pen.width()`-thick fill it doubles the rung -- [`fill`] is that
/// caller's helper.
pub(in crate::canvas) fn outline(cx: &Cx2d, rect: Rect, pen: Pen) -> Rect {
    outline_at(rect, pen, cx.current_dpi_factor())
}

fn outline_at(rect: Rect, pen: Pen, dpi: f64) -> Rect {
    snap_rect_at(rect, (pen.width() * 2.0 * dpi).ceil(), dpi)
}

/// The quad for a rect that IS the ink: a flat `DrawColor` fill already sized
/// from `pen.width()`. It is never widened to the `2 * pen` an inset stroke
/// needs, only kept off zero by the one device pixel a stroke may not round
/// away to. It takes no `Pen` for exactly that reason: the rung is already IN
/// the rect, and goes through [`quantise_px`] like every other flat fill.
pub(in crate::canvas) fn fill(cx: &Cx2d, rect: Rect) -> Rect {
    fill_at(rect, cx.current_dpi_factor())
}

fn fill_at(rect: Rect, dpi: f64) -> Rect {
    // The SIZE is quantised, not the far edge rounded: rounding both edges
    // independently makes a rung whose `w * dpi` is not an integer gain or
    // lose a device pixel with its subpixel phase, so two rails of one ghost
    // box, or two dividers in one card, could land at different weights. The
    // origin still snaps to the grid, so both edges do too.
    let x0 = (rect.pos.x * dpi).round();
    let y0 = (rect.pos.y * dpi).round();
    Rect {
        pos: dvec2(x0 / dpi, y0 / dpi),
        size: dvec2(
            quantise_px(rect.size.x, dpi) / dpi,
            quantise_px(rect.size.y, dpi) / dpi,
        ),
    }
}

/// Both edges of `rect` onto the device grid, keeping it at least `floor_px`
/// device pixels on each axis. An outline's own snapper: what matters for a
/// shape is where its edges land, so both are rounded independently.
fn snap_rect_at(rect: Rect, floor_px: f64, dpi: f64) -> Rect {
    let floor_px = floor_px.max(1.0);
    let x0 = (rect.pos.x * dpi).round();
    let y0 = (rect.pos.y * dpi).round();
    let x1 = ((rect.pos.x + rect.size.x) * dpi)
        .round()
        .max(x0 + floor_px);
    let y1 = ((rect.pos.y + rect.size.y) * dpi)
        .round()
        .max(y0 + floor_px);
    Rect {
        pos: dvec2(x0 / dpi, y0 / dpi),
        size: dvec2((x1 - x0) / dpi, (y1 - y0) / dpi),
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*

    // The one place stroke geometry is decided. A derived pen hands `pen_dev`
    // or `pen_sw` a LOGICAL width and gets device-grid geometry back; it never
    // does the arithmetic itself and never sees the camera.
    mod.draw.CadPen = mod.draw.DrawColor{
        // Logical stroke width, in lpx. The renderer pushes `Pen::width()`.
        pen_w: uniform(1.0)

        // Whole device pixels this pen resolves to. See PEN_ROUND_BIAS.
        pen_dev: fn(w: float) -> float {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            return max(1.0, floor(w * dpi + 0.501))
        }

        // Half-width plus the half device pixel that moves an SDF stroke's
        // antialias ramp off the crisp border samples. This is what `sdf.stroke`
        // wants.
        pen_sw: fn(w: float) -> float {
            let dpi = max(1.0, self.draw_pass.dpi_factor)
            return (max(1.0, floor(w * dpi + 0.501)) * 0.5 + 0.5) / dpi
        }

        // Sdf2d coverage is `clamp(-dist * aa)` with `aa = 1 /
        // length(vec2(|dFdx|, |dFdy|))` = 1/sqrt(2) for a pixel-unit quad, so
        // every stroke loses ~30% of its ink to the ramp. `gate` is 1.0 for a
        // canvas pen and `self.screen_space` for `AccentFrame`, whose non-canvas
        // consumers were tuned against the soft ramp.
        pen_aa: fn(gate: float) -> float {
            return mix(1.0, 1.4142136, gate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// The rounding term in the device quantiser. `0.501`, not `0.5`: `w`
    /// reaches the shader through an f32 round-trip, and a one-ULP shortfall on
    /// an expression meant to land exactly on an integer floors to the rung
    /// below. That is the bug that made the card border oscillate between 1 and
    /// 2 device pixels as the camera moved.
    const PEN_ROUND_BIAS: &str = "0.501";

    /// The factor that restores the `sqrt(2)` this fork's `antialias()` drops.
    const PEN_AA_FACTOR: &str = "1.4142136";

    /// The shader source of one `mod.draw.X = mod.draw.Y{ .. }` block from a
    /// file's `script_mod!`: comments stripped, and BOUNDED to that block's own
    /// braces.
    ///
    /// Both halves are load bearing. Returning the whole remainder of the file
    /// made every `contains` guard below match the Rust test module's own
    /// mirrors and literals, so the guard passed whatever the shader said;
    /// keeping the comments made it match CadPen's own prose, which names the
    /// very constants the guard greps for.
    fn pen_source(src: &str, name: &str) -> String {
        let head = format!("mod.draw.{name} = mod.draw.");
        let (_, rest) = src.split_once(&head).unwrap_or_else(|| {
            panic!("no `{head}` declaration found");
        });
        let code: String = rest
            .lines()
            .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
            .collect::<Vec<_>>()
            .join("\n");
        let open = code
            .find('{')
            .unwrap_or_else(|| panic!("`{head}` declares no block"));
        let mut depth = 0usize;
        for (offset, ch) in code[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return code[open..=open + offset].to_string();
                    }
                }
                _ => {}
            }
        }
        panic!("`{head}` block is never closed");
    }

    /// The setter each `live_id!(<field>)` in `src` is pushed through: the
    /// nearest `set_uniform`/`set_dyn_instance` at or above it.
    fn setters_for(src: &str, field: &str) -> Vec<&'static str> {
        let needle = format!("live_id!({field})");
        let lines: Vec<&str> = src.lines().collect();
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(&needle))
            .map(|(i, _)| {
                lines[..=i]
                    .iter()
                    .rev()
                    .find_map(|line| {
                        if line.contains("set_dyn_instance") {
                            Some("set_dyn_instance")
                        } else if line.contains("set_uniform") {
                            Some("set_uniform")
                        } else {
                            None
                        }
                    })
                    .unwrap_or("<none>")
            })
            .collect()
    }

    /// A shader field that changes PER QUAD must ride the instance buffer.
    /// makepad opens a new draw call whenever a draw's `dyn_uniforms` differ
    /// from the one it would append to, and a content lane may not cross that
    /// barrier -- so as a uniform, `thin_y` (which flips per routed segment and
    /// per leader leg) put every single bar in its own draw call on every pan
    /// and zoom frame. The buffer also decides the setter: `set_uniform`
    /// silently writes nothing to an instance.
    #[test]
    fn the_per_quad_edge_field_rides_the_instance_buffer() {
        let edge_line = pen_source(include_str!("class/widget.rs"), "EdgeLine");
        assert!(
            edge_line.contains("thin_y: instance("),
            "`thin_y` changes per quad, so it may not be a uniform"
        );
        for (label, src) in [
            (
                "class/render/edges.rs",
                include_str!("class/render/edges.rs"),
            ),
            (
                "class/render/labels.rs",
                include_str!("class/render/labels.rs"),
            ),
        ] {
            let setters = setters_for(src, "thin_y");
            assert!(!setters.is_empty(), "{label}: expected a `thin_y` push");
            for setter in setters {
                assert_eq!(
                    setter, "set_dyn_instance",
                    "{label}: `thin_y` is an instance, so `{setter}` writes nothing"
                );
            }
        }
    }

    /// Every canvas pen must derive from `CadPen`, not `DrawColor`. This is the
    /// regression guard: a new pen added on the old base renders soft, and
    /// nothing else in the suite would notice.
    fn assert_derives_from_cad_pen(src: &str, name: &str) {
        let head = format!("mod.draw.{name} = mod.draw.CadPen{{");
        assert!(
            src.contains(&head),
            "`{name}` must derive from CadPen, not DrawColor"
        );
    }

    #[test]
    fn cad_pen_carries_the_quantiser_and_the_coverage_correction() {
        let src = include_str!("pen.rs");
        let cad = pen_source(src, "CadPen");
        assert!(
            cad.contains(PEN_ROUND_BIAS),
            "CadPen must round with {PEN_ROUND_BIAS}, not a bare 0.5"
        );
        assert!(
            cad.contains(PEN_AA_FACTOR),
            "CadPen must restore the sqrt(2) the fork's antialias() drops"
        );
    }

    /// The guard above can only fail if it reads the shader and nothing else.
    #[test]
    fn the_pen_guard_reads_only_its_own_shader_block() {
        let cad = pen_source(include_str!("pen.rs"), "CadPen");
        // Bounded: the Rust mirror and the test literals below are not shader
        // source, and both carry the constants the guard greps for.
        assert!(!cad.contains("fn pen_dev(w: f32"));
        assert!(!cad.contains("mod tests"));
        // Comments stripped: CadPen's own prose names `PEN_ROUND_BIAS`, which
        // contains the string `0.501`... only as a spelling of a Rust const.
        assert!(!cad.contains("PEN_ROUND_BIAS"));
        // What survives is the arithmetic that actually decides a stroke width.
        assert!(cad.contains("floor(w * dpi + 0.501)"));
    }

    /// `pen_sw` already returns a HALF width (`pen_dev(w) * 0.5 + 0.5`), so a
    /// pen that pre-halves its rung strokes at half the weight it was given --
    /// and at dpi != 1 the two no longer quantise to matching device widths, so
    /// a marker stops matching the connector it terminates.
    #[test]
    fn no_pen_halves_its_rung_before_pen_sw() {
        for (label, src) in [
            ("class/widget.rs", include_str!("class/widget.rs")),
            ("behavior/mod.rs", include_str!("behavior/mod.rs")),
        ] {
            assert!(
                !src.contains("self.pen_sw(self.pen_w *"),
                "{label}: pen_sw already halves the rung -- pass the whole pen_w"
            );
        }
    }

    #[test]
    fn class_pens_derive_from_cad_pen() {
        let src = include_str!("class/widget.rs");
        for name in [
            "EdgeLine",
            "EdgeElbow",
            "EdgeMarker",
            "GroupBorder",
            "GroupDashed",
        ] {
            assert_derives_from_cad_pen(src, name);
        }
    }

    #[test]
    fn behavior_pens_derive_from_cad_pen() {
        let src = include_str!("behavior/mod.rs");
        for name in [
            "FlowBox",
            "FlowDiamond",
            "FlowCircle",
            "FlowTriangle",
            "InteractionOpenHead",
            "InteractionXMark",
            "InteractionFrameBorder",
            "InteractionTab",
        ] {
            assert_derives_from_cad_pen(src, name);
        }
    }

    /// `ConstraintVeil` is a wash-and-hatch FILL, not a stroke: it has no
    /// coverage to correct and no width to quantise, and the spec keeps it on
    /// `DrawColor` deliberately. Pin that so a future sweep does not "fix" it.
    #[test]
    fn the_constraint_veil_stays_a_plain_fill() {
        let src = include_str!("class/widget.rs");
        assert!(src.contains("mod.draw.ConstraintVeil = mod.draw.DrawColor{"));
    }

    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    /// Rust mirror of `CadPen::pen_dev`, so the rounding that decides a
    /// stroke's device width is testable without a GPU. `bias` is the shader's
    /// rounding term; the shader ships `0.501` and the test below shows why a
    /// bare `0.5` is not a rounding term at all here but a coin flip.
    fn pen_dev(w: f32, dpi: f32, bias: f32) -> f32 {
        (w * dpi + bias).floor().max(1.0)
    }

    /// Every zoom the camera can reach, paired with the inverse-zoom
    /// `stroke_scale` a frame cancels the camera with. `zoom * stroke_scale` is
    /// 1.0 only to within an f32 ULP, which is the whole hazard.
    fn zoom_sweep() -> impl Iterator<Item = (f32, f32)> {
        let named = [1.7749_f64, 5.0201, 0.2828];
        let mut zoom = MIN_ZOOM;
        let mut extra = named.into_iter();
        std::iter::from_fn(move || {
            if zoom <= MAX_ZOOM {
                let z = zoom;
                zoom *= 1.01;
                return Some((z as f32, (1.0 / z) as f32));
            }
            extra.next().map(|z| (z as f32, (1.0 / z) as f32))
        })
    }

    #[test]
    fn the_ladder_climbs() {
        let rungs = [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR];
        for rung in rungs {
            assert!(rung.width().is_finite() && rung.width() > 0.0);
        }
        for pair in rungs.windows(2) {
            assert!(
                pair[1].width() > pair[0].width(),
                "{} must be heavier than {}",
                pair[1].width(),
                pair[0].width()
            );
        }
        assert_eq!(Pen::HAIRLINE.width(), 1.0);
        assert_eq!(Pen::LIGHT.width(), 1.5);
        assert_eq!(Pen::REGULAR.width(), 2.0);
    }

    /// The CAD contract: one fixed device width at EVERY zoom, for every rung.
    #[test]
    fn a_rung_holds_one_device_width_across_the_whole_zoom_range() {
        for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
            let w = pen.width() as f32;
            for dpi in [1.0_f32, 1.25, 1.5, 2.0] {
                let expected = (w * dpi + 0.5).floor().max(1.0);
                for (zoom, stroke_scale) in zoom_sweep() {
                    assert_eq!(
                        pen_dev(w * zoom * stroke_scale, dpi, 0.501),
                        expected,
                        "pen {w} zoom {zoom} dpi {dpi}"
                    );
                }
            }
        }
    }

    /// The bug the epsilon fixes. At the rungs and dpis where `w * dpi` lands
    /// exactly on `integer - 0.5` (LIGHT at dpi 1, HAIRLINE at dpi 1.5,
    /// REGULAR at dpi 1.25) the unbiased expression evaluates to exactly
    /// `floor(n)`, so a one-ULP shortfall floors to `n - 1` and the stroke
    /// visibly changes weight as the camera moves. Prove the naive form really
    /// is unstable, so the epsilon can never be "simplified" away.
    #[test]
    fn the_unbiased_quantiser_flips_width_somewhere_in_that_range() {
        for (pen, dpi) in [
            (Pen::LIGHT, 1.0_f32),
            (Pen::HAIRLINE, 1.5),
            (Pen::REGULAR, 1.25),
        ] {
            let w = pen.width() as f32;
            let widths: std::collections::BTreeSet<u32> = zoom_sweep()
                .map(|(zoom, ss)| pen_dev(w * zoom * ss, dpi, 0.5) as u32)
                .collect();
            assert!(
                widths.len() > 1,
                "expected the naive `+ 0.5` form to be zoom-dependent at \
                 w {w} dpi {dpi}, got {widths:?}"
            );
        }
    }

    /// The rungs land on the device widths the spec intends.
    #[test]
    fn rungs_land_on_the_intended_device_widths() {
        let table = [
            (Pen::HAIRLINE, 1.0, 2.0),
            (Pen::LIGHT, 2.0, 3.0),
            (Pen::REGULAR, 2.0, 4.0),
        ];
        for (pen, at_dpi_1, at_dpi_2) in table {
            let w = pen.width() as f32;
            assert_eq!(pen_dev(w, 1.0, 0.501), at_dpi_1, "{w} at dpi 1");
            assert_eq!(pen_dev(w, 2.0, 0.501), at_dpi_2, "{w} at dpi 2");
        }
    }

    /// The card border's shader literal is a rung, spelled out. `frame.rs`
    /// cannot see `Pen` (it lives outside `crate::canvas`), so the tie is
    /// asserted from this side.
    #[test]
    fn the_card_border_is_the_light_rung() {
        let src = include_str!("../frame.rs");
        let expected = format!("self.pen_dev({} * self.zoom", Pen::LIGHT.width());
        assert!(
            src.contains(&expected),
            "AccentFrame's border literal must track Pen::LIGHT ({})",
            Pen::LIGHT.width()
        );
    }

    #[test]
    fn the_accent_frame_derives_from_cad_pen() {
        let src = include_str!("../frame.rs");
        assert_derives_from_cad_pen(src, "AccentFrame");
    }

    /// Nothing in either canvas may declare a pen on the old base. This catches
    /// the regression the per-name lists above cannot: a NEW pen, added later,
    /// on `DrawColor`, that this plan's per-name lists have no way to know
    /// about. `ConstraintVeil` is a sanctioned exception (a wash-and-hatch
    /// fill, not a stroke) and is pinned by its own test. `EdgeDashed`,
    /// `UseCaseEllipse` and `ActorLine` are the Use Case notation's pens,
    /// added on `origin/main` after this plan was authored: each computes its
    /// own CONTINUOUS distance-to-shape antialiasing (a fractional stroke_w
    /// blend, no `pen_dev` device-pixel quantisation), which is a different
    /// rendering policy than CadPen's, not a forgotten migration -- so they
    /// are out of this plan's scope, not a regression this guard should trip
    /// on.
    #[test]
    fn no_canvas_pen_is_left_on_draw_color() {
        const OUT_OF_SCOPE: [&str; 4] = [
            "ConstraintVeil ",
            "EdgeDashed ",
            "UseCaseEllipse ",
            "ActorLine ",
        ];
        for (label, src) in [
            ("class/widget.rs", include_str!("class/widget.rs")),
            ("behavior/mod.rs", include_str!("behavior/mod.rs")),
        ] {
            for line in src.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("mod.draw.") {
                    if rest.contains("= mod.draw.DrawColor{") {
                        assert!(
                            OUT_OF_SCOPE.iter().any(|name| rest.starts_with(name)),
                            "{label}: `{line}` must derive from CadPen"
                        );
                    }
                }
            }
        }
    }

    use makepad_widgets::dvec2;

    /// `band` is a CANVAS, never the ink: it must be at least one device pixel
    /// wider on each side than the widest band the shader can resolve the pen
    /// to, so a one-ULP disagreement between this f64 rounding and the shader's
    /// f32 one cannot clip the stroke.
    #[test]
    fn a_band_quad_clears_the_widest_stroke_its_pen_can_resolve_to() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                for center in [10.0, 10.2, 10.5, 10.7] {
                    let quad = band_at(dvec2(4.0, center), dvec2(64.0, center), pen, dpi);
                    let ink = pen_dev(pen.width() as f32, dpi as f32, 0.501) as f64;
                    let thick_px = quad.size.y * dpi;
                    assert!(
                        (thick_px - thick_px.round()).abs() <= 1e-9,
                        "quad must be a whole number of device pixels"
                    );
                    assert!(
                        thick_px >= ink + 2.0 - 1e-9,
                        "quad {thick_px} must clear ink {ink} by a device pixel each side"
                    );
                    let start_px = quad.pos.y * dpi;
                    assert!((start_px - start_px.round()).abs() <= 1e-9);
                    // Still centred on the band it was asked for, to within the
                    // half device pixel the snap is allowed to move it.
                    assert!((quad.pos.y + quad.size.y * 0.5 - center).abs() <= 0.5 / dpi + 1e-9);
                }
            }
        }
    }

    #[test]
    fn a_band_picks_the_axis_it_runs_longest_along() {
        assert!(band_is_horizontal(dvec2(0.0, 0.0), dvec2(40.0, 2.0)));
        assert!(!band_is_horizontal(dvec2(0.0, 0.0), dvec2(2.0, 40.0)));
        // A degenerate pair is horizontal, matching the old `stroke_quad`.
        assert!(band_is_horizontal(dvec2(5.0, 5.0), dvec2(5.0, 5.0)));
    }
    /// A flat fill IS the ink, so it renders at the rung it was sized at. The
    /// `2 * pen` floor an inset stroke needs would double every card divider,
    /// origin rail and ghost rail -- 1 device px to 2, 1.5 to 3, 2 to 4.
    #[test]
    fn a_flat_fill_keeps_the_rung_it_was_sized_at() {
        for dpi in [1.0, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                for y in [20.0, 20.3, 20.5, 20.8] {
                    let rule = fill_at(
                        Rect {
                            pos: dvec2(10.0, y),
                            size: dvec2(120.0, pen.width()),
                        },
                        dpi,
                    );
                    let thick_px = rule.size.y * dpi;
                    assert!(
                        (thick_px - thick_px.round()).abs() <= 1e-9,
                        "a fill must be a whole number of device pixels"
                    );
                    assert!(thick_px >= 1.0, "a rule may never round away entirely");
                    assert!(
                        thick_px <= (pen.width() * dpi).ceil(),
                        "pen {} at dpi {dpi} inked {thick_px} device px, \
                         which is fatter than its rung",
                        pen.width()
                    );
                }
            }
        }
    }

    /// A flat fill inks ONE width wherever it lands. The pre-plan snapper
    /// rounded both edges independently, so a rung whose `w * dpi` is not an
    /// integer picked up or lost a device pixel with its subpixel phase:
    /// `Pen::LIGHT` origin-overlay rails at dpi 1 rendered 2 px at y 100.0 and
    /// 1 px at y 100.8, so two rails of the same ghost box could differ in
    /// weight. That per-element lottery is the thing this module exists to
    /// remove, so a fill quantises its extent through `ink_px` like every
    /// other snapper here.
    #[test]
    fn a_flat_fill_inks_one_width_at_every_subpixel_phase() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                let ink = ink_px(pen, dpi);
                for phase in [100.0, 100.2, 100.5, 100.8, 100.9] {
                    let rail = fill_at(
                        Rect {
                            pos: dvec2(10.0, phase),
                            size: dvec2(240.0, pen.width()),
                        },
                        dpi,
                    );
                    assert!(
                        (rail.size.y * dpi - ink).abs() <= 1e-9,
                        "pen {} at dpi {dpi}, y {phase}: inked {} device px, not {ink}",
                        pen.width(),
                        rail.size.y * dpi
                    );
                    let stem = fill_at(
                        Rect {
                            pos: dvec2(phase, 10.0),
                            size: dvec2(pen.width(), 240.0),
                        },
                        dpi,
                    );
                    assert!(
                        (stem.size.x * dpi - ink).abs() <= 1e-9,
                        "pen {} at dpi {dpi}, x {phase}: inked {} device px, not {ink}",
                        pen.width(),
                        stem.size.x * dpi
                    );
                }
            }
        }
    }

    /// A fill and a filled band are the same quantiser seen from two sides: a
    /// rect sized at a rung must ink exactly what a band drawn along that rung
    /// inks, or a card divider stops matching the connector it meets.
    #[test]
    fn a_fill_and_a_filled_band_agree_on_the_rung() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                let band = fill_band_at(dvec2(4.0, 30.4), dvec2(64.0, 30.4), pen, dpi);
                let fill = fill_at(
                    Rect {
                        pos: dvec2(4.0, 30.4),
                        size: dvec2(60.0, pen.width()),
                    },
                    dpi,
                );
                assert!((band.size.y - fill.size.y).abs() <= 1e-9);
            }
        }
    }

    /// The flat-fill band snapper, exercised through the function the behavior
    /// canvas actually calls -- not a re-derivation of its arithmetic.
    #[test]
    fn a_filled_band_is_the_rung_wide_and_starts_on_a_device_pixel() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            for pen in [Pen::HAIRLINE, Pen::LIGHT, Pen::REGULAR] {
                for center in [10.0, 10.2, 10.5, 10.7] {
                    let ink = ink_px(pen, dpi);
                    let across = fill_band_at(dvec2(4.0, center), dvec2(64.0, center), pen, dpi);
                    assert_eq!(across.size.y * dpi, ink, "horizontal band is one rung");
                    assert!((across.pos.y * dpi - (across.pos.y * dpi).round()).abs() <= 1e-9);
                    assert!(
                        (across.pos.y + across.size.y * 0.5 - center).abs() <= 0.5 / dpi + 1e-9
                    );

                    let down = fill_band_at(dvec2(center, 4.0), dvec2(center, 64.0), pen, dpi);
                    assert_eq!(down.size.x * dpi, ink, "vertical band is one rung");
                    assert!((down.pos.x * dpi - (down.pos.x * dpi).round()).abs() <= 1e-9);
                    assert!((down.pos.x + down.size.x * 0.5 - center).abs() <= 0.5 / dpi + 1e-9);

                    // The shader's canvas is this same band, one device pixel
                    // wider per side -- the two never disagree on the axis.
                    let canvas = band_at(dvec2(4.0, center), dvec2(64.0, center), pen, dpi);
                    assert!((canvas.size.y * dpi - (ink + 2.0)).abs() <= 1e-9);
                    assert!((canvas.pos.y * dpi - (across.pos.y * dpi - 1.0)).abs() <= 1e-9);
                }
            }
        }
    }

    /// The argument list of every `callee(` call in `src`, bounded to that
    /// call's own parentheses so a later `.width()` on the following line
    /// cannot be read as one of its arguments.
    fn call_args(src: &str, callee: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut rest = src;
        while let Some(at) = rest.find(callee) {
            let after = &rest[at + callee.len()..];
            let mut depth = 1usize;
            let mut end = after.len();
            for (offset, ch) in after.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = offset;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            args.push(after[..end].to_string());
            rest = &after[end..];
        }
        args
    }

    /// Spellings that say "this rect IS the ink": a rect whose size comes from
    /// a stroke weight is a flat fill, and belongs to `fill`, never `outline`.
    const INK_SIZED: [&str; 2] = [".width()", "thickness"];

    fn outline_offenders(label: &str, src: &str) -> (usize, Vec<String>) {
        let mut calls = 0usize;
        let mut offenders = Vec::new();
        for callee in ["pen::outline(", "outline_at("] {
            for args in call_args(src, callee) {
                calls += 1;
                if let Some(ink) = INK_SIZED.iter().find(|ink| args.contains(**ink)) {
                    offenders.push(format!("{label}: `{callee}..{ink}..`"));
                }
            }
        }
        (calls, offenders)
    }

    fn canvas_sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                canvas_sources(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// `outline` snaps a SHAPE's edges and floors it at the `2 * pen` room an
    /// inset stroke needs on both sides; `fill` snaps a rect that IS the ink.
    /// Handing a flat fill to `outline` therefore doubles its rung -- card
    /// dividers 1 -> 2 device px, origin rails 1.5 -> 3, ghost rails 2 -> 4 --
    /// which is exactly the defect commit 87c81586 had to fix. The contract
    /// lives only in prose otherwise, so pin it at the CALL SITES: nothing may
    /// hand `outline` a rect sized from a stroke weight.
    #[test]
    fn no_flat_fill_is_routed_through_the_outline_snapper() {
        let canvas = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/canvas");
        let mut files = Vec::new();
        canvas_sources(&canvas, &mut files);
        assert!(
            !files.is_empty(),
            "expected canvas sources under {canvas:?}"
        );

        let mut calls = 0usize;
        let mut offenders = Vec::new();
        for path in &files {
            // The definition site: its own tests call `outline_at` on rects
            // built to prove the floor, which is the point there.
            if path.file_name().and_then(|n| n.to_str()) == Some("pen.rs") {
                continue;
            }
            let src = std::fs::read_to_string(path).expect("read canvas source");
            let (found, mut bad) = outline_offenders(&path.display().to_string(), &src);
            calls += found;
            offenders.append(&mut bad);
        }
        assert!(
            offenders.is_empty(),
            "a rect sized from a pen belongs to `pen::fill`, not `pen::outline`:\n{}",
            offenders.join("\n")
        );
        // A guard that finds no call sites at all guards nothing.
        assert!(calls > 0, "expected at least one `pen::outline` call site");
    }

    /// The guard above can only earn its keep if it would actually fire.
    #[test]
    fn the_outline_guard_sees_a_rect_sized_from_a_pen() {
        let flat_fill = "let bad = pen::outline(\n    cx,\n    Rect {\n        pos: p,\n        \
             size: dvec2(w, pen.width()),\n    },\n    pen,\n);\nlet w = pen.width();";
        let (calls, offenders) = outline_offenders("fake.rs", flat_fill);
        assert_eq!(calls, 1);
        assert_eq!(offenders.len(), 1, "the guard must flag a pen-sized rect");

        // ...and bounded to the call's own parens: the `pen.width()` on the
        // line AFTER the shape call below is not one of its arguments.
        let shape = "let ok = pen::outline(cx, world_rect_to_screen(v, r), pen);\n\
             draws.frame.set_uniform(cx, live_id!(pen_w), &[pen.width() as f32]);";
        let (calls, offenders) = outline_offenders("fake.rs", shape);
        assert_eq!(calls, 1);
        assert!(offenders.is_empty(), "{offenders:?}");
    }

    #[test]
    fn an_outline_lands_on_the_device_grid_and_keeps_room_for_its_stroke() {
        for dpi in [1.0, 1.5, 2.0] {
            let snapped = outline_at(
                Rect {
                    pos: dvec2(10.3, 20.7),
                    size: dvec2(40.4, 30.2),
                },
                Pen::LIGHT,
                dpi,
            );
            for v in [
                snapped.pos.x,
                snapped.pos.y,
                snapped.pos.x + snapped.size.x,
                snapped.pos.y + snapped.size.y,
            ] {
                assert!((v * dpi - (v * dpi).round()).abs() <= 1e-9);
            }
            // A degenerate rect cannot invert the shader's inset.
            let tiny = outline_at(
                Rect {
                    pos: dvec2(10.0, 20.0),
                    size: dvec2(0.0, 0.0),
                },
                Pen::LIGHT,
                dpi,
            );
            assert!(tiny.size.x >= Pen::LIGHT.width() * 2.0 - 1e-9);
            assert!(tiny.size.y >= Pen::LIGHT.width() * 2.0 - 1e-9);
        }
    }
}
