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

/// The quad a `CadPen` inks a stroke from `a` to `b` inside.
///
/// This is a CANVAS, not the stroke. It is grown one device pixel past the
/// widest band the pen can resolve to on each side of the centreline, and the
/// shader inks the quantised band within it -- the same arrangement
/// `AccentFrame` already uses to ink a border inside a card rect. Growing here
/// is what lets width stop being a CPU concern: the shader is free to disagree
/// with this rounding by a pixel without its stroke being clipped.
///
/// Only for quads a `CadPen`-derived pen inks inside. A quad that IS the ink
/// (a flat `DrawColor` fill) wants [`outline`] instead.
pub(in crate::canvas) fn band(cx: &Cx2d, a: DVec2, b: DVec2, pen: Pen) -> Rect {
    band_at(a, b, pen, cx.current_dpi_factor())
}

/// The dpi-explicit core of [`band`], split out so callers that need to unit
/// test the geometry they build ON TOP of a band (`labels.rs::leader_bars`)
/// can do so without a live `Cx2d`, the same way `band` itself is tested
/// here.
pub(in crate::canvas) fn band_at(a: DVec2, b: DVec2, pen: Pen, dpi: f64) -> Rect {
    let thick_px = (pen.width() * dpi + 0.501).floor().max(1.0) + 2.0;
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

/// The quad for a shape whose OUTLINE a pen strokes -- frames, group hulls,
/// markers -- with its edges pulled onto the device grid, and guaranteed wide
/// enough that the shader's `pen_sw` inset cannot invert it.
///
/// Also the right helper for a quad that IS the ink: a flat `DrawColor` fill
/// sized from `pen.width()` needs its edges on the grid and nothing else.
pub(in crate::canvas) fn outline(cx: &Cx2d, rect: Rect, pen: Pen) -> Rect {
    outline_at(rect, pen, cx.current_dpi_factor())
}

fn outline_at(rect: Rect, pen: Pen, dpi: f64) -> Rect {
    let floor_px = (pen.width() * 2.0 * dpi).ceil().max(1.0);
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

    /// The rounding term in the device quantiser. `0.501`, not `0.5`: `w`
    /// reaches the shader through an f32 round-trip, and a one-ULP shortfall on
    /// an expression meant to land exactly on an integer floors to the rung
    /// below. That is the bug that made the card border oscillate between 1 and
    /// 2 device pixels as the camera moved.
    const PEN_ROUND_BIAS: &str = "0.501";

    /// The factor that restores the `sqrt(2)` this fork's `antialias()` drops.
    const PEN_AA_FACTOR: &str = "1.4142136";

    /// The shader source of one `mod.draw.X = mod.draw.Y{` block, comments
    /// stripped, from a file's `script_mod!`. Mirrors the extraction
    /// `frame.rs::shader_constants_match_the_padding_contract` uses.
    fn pen_source<'a>(src: &'a str, name: &str) -> &'a str {
        let head = format!("mod.draw.{name} = mod.draw.");
        let (_, rest) = src.split_once(&head).unwrap_or_else(|| {
            panic!("no `{head}` declaration found");
        });
        rest
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
        let expected = format!("floor({} * self.zoom", Pen::LIGHT.width());
        assert!(
            src.contains(&expected),
            "AccentFrame's border literal must track Pen::LIGHT ({})",
            Pen::LIGHT.width()
        );
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
