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
        // One pen so far: the spike migrated `EdgeLine` alone. Later tasks add
        // `EdgeElbow`, `EdgeMarker`, `GroupBorder` and `GroupDashed` here.
        let src = include_str!("class/widget.rs");
        assert_derives_from_cad_pen(src, "EdgeLine");
    }

    /// `ConstraintVeil` is a wash-and-hatch FILL, not a stroke: it has no
    /// coverage to correct and no width to quantise, and the spec keeps it on
    /// `DrawColor` deliberately. Pin that so a future sweep does not "fix" it.
    #[test]
    fn the_constraint_veil_stays_a_plain_fill() {
        let src = include_str!("class/widget.rs");
        assert!(src.contains("mod.draw.ConstraintVeil = mod.draw.DrawColor{"));
    }
}
