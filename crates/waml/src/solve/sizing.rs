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
    Mono,
}

/// makepad rasterizes a DSL `font_size` given in POINTS at `pts * 96/72` logical
/// px (`LPXS_PER_INCH / PTS_PER_INCH`). Measure at that lpx size, not at points,
/// or a box is measured ~25% too narrow and its text overflows.
pub const PT_TO_LPX: f64 = 96.0 / 72.0;

static SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

fn face(font: Font) -> &'static Face<'static> {
    static SANS_FACE: OnceLock<Face<'static>> = OnceLock::new();
    static MONO_FACE: OnceLock<Face<'static>> = OnceLock::new();
    match font {
        Font::Sans => SANS_FACE
            .get_or_init(|| Face::parse(SANS, 0).expect("embedded IBM Plex Sans face parses")),
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

/// Line height of `font` at `font_size` px: `(ascender - descender)` scaled from
/// font units to px. Used as the row height of a text leaf in the card box-tree.
pub fn line_height(font_size: f64, font: Font) -> f64 {
    let face = face(font);
    let units_per_em = face.units_per_em() as f64;
    let ascender = face.ascender() as f64;
    let descender = face.descender() as f64; // negative
    (ascender - descender) * font_size / units_per_em
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
        assert!(text_width("iWiWi", 12.0, Font::Sans) != five);
    }

    #[test]
    fn line_height_is_positive_and_scales() {
        let small = line_height(12.0, Font::Mono);
        let big = line_height(24.0, Font::Mono);
        assert!(small > 0.0);
        assert!((big - 2.0 * small).abs() < 1e-6);
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
