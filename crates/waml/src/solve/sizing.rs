//! Deterministic text measurement for layout sizing. Sums glyph advances from an
//! embedded IBM Plex Sans face via `ttf-parser`, so both frontends can size boxes
//! to real text metrics without a rendering backend. Pure and wasm-clean.

use std::sync::OnceLock;
use ttf_parser::Face;

/// IBM Plex Sans Regular, embedded so measurement is self-contained and matches
/// the family the frontends render with.
static FONT: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");

fn face() -> &'static Face<'static> {
    static FACE: OnceLock<Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| Face::parse(FONT, 0).expect("embedded IBM Plex Sans face parses"))
}

/// Advance width of `s` rendered at `font_size` pixels, in pixels.
pub fn text_width(s: &str, font_size: f64) -> f64 {
    let face = face();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_string_is_wider() {
        assert!(text_width("OrderId", 12.0) > text_width("id", 12.0));
    }

    #[test]
    fn width_scales_with_font_size() {
        let small = text_width("Order", 12.0);
        let big = text_width("Order", 24.0);
        assert!(big > small);
        // Advance is linear in font size.
        assert!((big - 2.0 * small).abs() < 1e-6);
    }

    #[test]
    fn deterministic() {
        assert_eq!(text_width("Customer", 15.0), text_width("Customer", 15.0));
    }

    #[test]
    fn empty_string_is_zero() {
        assert_eq!(text_width("", 12.0), 0.0);
    }
}
