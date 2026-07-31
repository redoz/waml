//! Deterministic text measurement for layout sizing. Sums glyph advances from
//! embedded IBM Plex Sans / Mono faces via `ttf-parser`, so both frontends can
//! size boxes to real text metrics without a rendering backend. Pure and
//! wasm-clean.

use std::sync::OnceLock;
use ttf_parser::Face;

/// Which embedded face to measure against. `Sans` is IBM Plex Sans (proportional),
/// `Mono` is IBM Plex Mono (monospace). Mono is weight-invariant in advance, so a
/// bold mono line measures exactly against the Regular face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Sans,
    /// IBM Plex Sans SemiBold: the cut the `text_heading` chrome role draws
    /// headings in. Its advances are WIDER than `Sans`, so a heading measured
    /// against `Sans` sizes its box too narrow.
    SansSemiBold,
    Mono,
}

/// makepad rasterizes a DSL `font_size` given in POINTS at `pts * 96/72` logical
/// px (`LPXS_PER_INCH / PTS_PER_INCH`). Measure at that lpx size, not at points,
/// or a box is measured ~25% too narrow and its text overflows.
pub const PT_TO_LPX: f64 = 96.0 / 72.0;

static SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
static SANS_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

fn face(font: Font) -> &'static Face<'static> {
    static SANS_FACE: OnceLock<Face<'static>> = OnceLock::new();
    static SEMIBOLD_FACE: OnceLock<Face<'static>> = OnceLock::new();
    static MONO_FACE: OnceLock<Face<'static>> = OnceLock::new();
    match font {
        Font::Sans => SANS_FACE
            .get_or_init(|| Face::parse(SANS, 0).expect("embedded IBM Plex Sans face parses")),
        Font::SansSemiBold => SEMIBOLD_FACE.get_or_init(|| {
            Face::parse(SANS_SEMIBOLD, 0).expect("embedded IBM Plex Sans SemiBold face parses")
        }),
        Font::Mono => MONO_FACE
            .get_or_init(|| Face::parse(MONO, 0).expect("embedded IBM Plex Mono face parses")),
    }
}

/// Advance width of `s` rendered at `font_size` pixels in `font`, in pixels.
pub fn text_width(s: &str, font_size: f64, font: Font) -> f64 {
    let face = face(font);
    let units_per_em = face.units_per_em() as f64;
    let scale = font_size / units_per_em;
    // Fallback advance for glyphs the face lacks (roughly a lowercase 'x' box).
    let fallback = units_per_em * 0.5;
    let units: f64 = s
        .chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map(|a| a as f64)
                .unwrap_or(fallback)
        })
        .sum();
    units * scale
}

/// Distance from the baseline up to the ascender of `font` at `font_size` px.
pub fn ascent(font_size: f64, font: Font) -> f64 {
    let face = face(font);
    face.ascender() as f64 * font_size / face.units_per_em() as f64
}

/// Distance from the baseline down to the descender of `font` at `font_size` px,
/// as a POSITIVE number. `face.descender()` is negative (below the baseline); this
/// returns its magnitude so callers can add it as a downward offset. Use this to
/// seat a label's baseline onto a neighbour instead of hardcoding a pixel nudge.
pub fn descent(font_size: f64, font: Font) -> f64 {
    let face = face(font);
    -(face.descender() as f64) * font_size / face.units_per_em() as f64
}

/// Line height of `font` at `font_size` px: `(ascender - descender)` scaled from
/// font units to px. Used as the row height of a text leaf in the card box-tree.
pub fn line_height(font_size: f64, font: Font) -> f64 {
    ascent(font_size, font) + descent(font_size, font)
}

/// The em fudges every behavior-canvas chrome text role carries
/// (`FontMember{asc: -0.1 desc: 0.0}` in `waml-editor`'s `mod.fonts`). makepad
/// ADDS these to the face's own ascender/descender in ems before scaling, so a
/// box sized from the raw face metrics does not match the glyphs drawn into it.
pub const CHROME_ASC_FUDGE_EM: f64 = -0.1;
/// See [`CHROME_ASC_FUDGE_EM`]. Note makepad adds this to a NEGATIVE descender,
/// so a positive value pulls the descender up.
pub const CHROME_DESC_FUDGE_EM: f64 = 0.0;
/// `line_spacing` on the behavior-canvas chrome text roles. makepad multiplies
/// the whole baseline-to-baseline distance by it, so it scales the STACKING
/// advance but not a single row's height.
pub const CHROME_LINE_SPACING: f64 = 1.2;

/// What a run of text actually occupies once makepad has drawn it. The two
/// heights differ and are not interchangeable: `row_height` is how tall ONE
/// drawn line is (use it to size a single-line label's rect), `line_advance` is
/// the baseline-to-baseline step between stacked lines (use it to size a
/// multi-line box, and to place each line inside one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnMetrics {
    /// Baseline to the top of the drawn line, fudges applied.
    pub ascent: f64,
    /// Baseline to the bottom of the drawn line, as a POSITIVE number.
    pub descent: f64,
    /// Height of a single drawn line: `ascent + descent`.
    pub row_height: f64,
    /// Distance from one line's top edge to the next line's top edge.
    pub line_advance: f64,
}

/// Metrics for `font` at `font_size` lpx as makepad DRAWS it, mirroring
/// `makepad_draw`'s layouter: the face's ascender/descender in ems are shifted
/// by the `FontMember` fudges, then the baseline-to-baseline step is
/// `(line_gap - descender + ascender) * line_spacing`.
///
/// This is the seam. `solve` sizes boxes with it and the editor places glyphs
/// with it, so the two cannot drift apart the way a hardcoded line height did.
pub fn drawn_metrics(
    font_size: f64,
    font: Font,
    asc_fudge_em: f64,
    desc_fudge_em: f64,
    line_spacing: f64,
) -> DrawnMetrics {
    let face = face(font);
    let upem = face.units_per_em() as f64;
    let ascent = (face.ascender() as f64 / upem + asc_fudge_em) * font_size;
    // `face.descender()` is negative; makepad adds the fudge in that signed
    // space and keeps it negative, so negate once at the end for a magnitude.
    let descent = -((face.descender() as f64 / upem + desc_fudge_em) * font_size);
    let line_gap = face.line_gap() as f64 / upem * font_size;
    DrawnMetrics {
        ascent,
        descent,
        row_height: ascent + descent,
        line_advance: (line_gap + descent + ascent) * line_spacing,
    }
}

/// [`drawn_metrics`] with the behavior-canvas chrome role's fudges and line
/// spacing applied. Every behavior solver and renderer measures through this.
pub fn chrome_metrics(font_size: f64, font: Font) -> DrawnMetrics {
    drawn_metrics(
        font_size,
        font,
        CHROME_ASC_FUDGE_EM,
        CHROME_DESC_FUDGE_EM,
        CHROME_LINE_SPACING,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_string_is_wider() {
        assert!(text_width("OrderId", 12.0, Font::Sans) > text_width("id", 12.0, Font::Sans));
    }

    #[test]
    fn width_scales_with_font_size() {
        let small = text_width("Order", 12.0, Font::Sans);
        let big = text_width("Order", 24.0, Font::Sans);
        assert!(big > small);
        // Advance is linear in font size.
        assert!((big - 2.0 * small).abs() < 1e-6);
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            text_width("Customer", 15.0, Font::Sans),
            text_width("Customer", 15.0, Font::Sans)
        );
    }

    #[test]
    fn empty_string_is_zero() {
        assert_eq!(text_width("", 12.0, Font::Sans), 0.0);
        assert_eq!(text_width("", 12.0, Font::Mono), 0.0);
    }

    #[test]
    fn mono_is_monospaced_and_differs_from_sans() {
        // Every mono glyph shares one advance, so an N-char string is N * one glyph.
        let one = text_width("i", 12.0, Font::Mono);
        let five = text_width("iWiWi", 12.0, Font::Mono);
        assert!((five - 5.0 * one).abs() < 1e-6);
        // Sans is proportional: "i" and "W" differ, so the same string measures
        // differently under Sans than under Mono.
        assert_ne!(text_width("iWiWi", 12.0, Font::Sans), five);
    }

    #[test]
    fn line_height_is_positive_and_scales() {
        let small = line_height(12.0, Font::Mono);
        let big = line_height(24.0, Font::Mono);
        assert!(small > 0.0);
        assert!((big - 2.0 * small).abs() < 1e-6);
    }

    #[test]
    fn descent_is_positive_and_line_height_is_ascent_plus_descent() {
        let a = ascent(12.0, Font::Sans);
        let d = descent(12.0, Font::Sans);
        assert!(a > 0.0 && d > 0.0);
        assert!((line_height(12.0, Font::Sans) - (a + d)).abs() < 1e-9);
    }

    #[test]
    fn sans_descent_at_11pt_matches_start_screen_baseline_nudge() {
        // The start-screen subtitle sits at DSL `font_size: 11` (points), which
        // makepad rasterizes at 11 * PT_TO_LPX lpx. Its baseline-seating margin in
        // start_screen.rs is derived from this descent; pin it so the DSL literal
        // can't silently drift from the font.
        let d = descent(11.0 * PT_TO_LPX, Font::Sans);
        assert!((d - 4.03).abs() < 0.05, "descent = {d} lpx");
    }

    #[test]
    fn semibold_is_wider_than_regular_at_the_same_size() {
        // The `text_heading` role draws SemiBold. Measuring a heading against
        // Regular sizes its box too narrow, which is why this face is embedded.
        let regular = text_width("Validate Order", 17.33, Font::Sans);
        let semi = text_width("Validate Order", 17.33, Font::SansSemiBold);
        assert!(semi > regular, "semi = {semi}, regular = {regular}");
    }

    #[test]
    fn chrome_metrics_match_the_drawn_behavior_canvas_role() {
        // The behavior canvases draw 13pt text. These are the numbers makepad
        // actually produces for `mod.fonts.text_body` (asc -0.1, desc 0.0,
        // line_spacing 1.2) at that size; the solvers size their boxes from
        // them. If a `mod.fonts` role changes, this fails rather than letting
        // text silently overflow its box again.
        let m = chrome_metrics(13.0 * PT_TO_LPX, Font::Sans);
        assert!((m.row_height - 20.8).abs() < 0.01, "row = {}", m.row_height);
        assert!(
            (m.line_advance - 24.96).abs() < 0.01,
            "advance = {}",
            m.line_advance
        );
        // The stacking step is NOT the row height -- the bug this replaced used
        // one number for both.
        assert!(m.line_advance > m.row_height);
    }

    #[test]
    fn chrome_ascent_fudge_shortens_the_row_against_the_raw_face() {
        // A negative `asc` fudge trims the drawn line; the raw face metric is
        // taller. Sizing from `line_height` would over-size, from the old 18.0
        // literal under-size.
        let fs = 13.0 * PT_TO_LPX;
        let m = chrome_metrics(fs, Font::Sans);
        assert!(m.row_height < line_height(fs, Font::Sans));
        assert!(m.row_height > 18.0);
    }

    #[test]
    fn drawn_metrics_scale_linearly_in_font_size() {
        let small = chrome_metrics(10.0, Font::Sans);
        let big = chrome_metrics(20.0, Font::Sans);
        assert!((big.row_height - 2.0 * small.row_height).abs() < 1e-9);
        assert!((big.line_advance - 2.0 * small.line_advance).abs() < 1e-9);
    }

    #[test]
    fn pt_to_lpx_is_the_makepad_rasterization_factor() {
        // makepad rasterizes DSL points at pts * 96/72 logical px. Measuring at
        // points instead of lpx makes boxes ~25% too narrow. Guard the factor.
        assert_eq!(PT_TO_LPX, 96.0 / 72.0);
        let at_pt = text_width("Order", 12.0, Font::Sans);
        let at_lpx = text_width("Order", 12.0 * PT_TO_LPX, Font::Sans);
        assert!(at_lpx > at_pt);
    }
}
