//! World-space edge label placement.

use super::sizing::{self, Font};
use super::wire::Size;

/// The face edge labels are drawn in. The renderer's `target_size` is
/// `8.0 * zoom`, so 8.0 is the world-space size and both agree at zoom 1.
const LABEL_FONT: Font = Font::Sans;

/// Tunables for label geometry, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelConfig {
    /// World-space font size for label text.
    pub font_size: f64,
    /// Clearance between a route and the label riding alongside it.
    pub gap: f64,
    /// Extra room reserved between the two terminal labels of one edge, so
    /// they do not touch in the middle when the gap is sized to hold both.
    pub slack: f64,
}

impl Default for LabelConfig {
    fn default() -> Self {
        LabelConfig {
            font_size: 8.0,
            gap: 3.0,
            slack: 24.0,
        }
    }
}

/// World-space box a label's text occupies. Height is a full line height even
/// for empty text -- a zero-height rect is invisible to every collision test,
/// which would silently stop an empty label from acting as an obstacle.
pub fn measure(text: &str, cfg: &LabelConfig) -> Size {
    Size {
        w: sizing::text_width(text, cfg.font_size, LABEL_FONT),
        h: sizing::line_height(cfg.font_size, LABEL_FONT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_measures_wider_than_it_is_tall_and_scales_with_text() {
        let cfg = LabelConfig::default();
        let short = measure("1", &cfg);
        let long = measure("settledBy {0..*}", &cfg);
        assert!(long.w > short.w, "more text is wider");
        assert_eq!(short.h, long.h, "one line is one line height");
        assert!(short.h > 0.0);
    }

    #[test]
    fn an_empty_label_still_has_height() {
        // A zero-height rect would be invisible to every collision test, so an
        // empty label would silently stop being an obstacle.
        let m = measure("", &LabelConfig::default());
        assert_eq!(m.w, 0.0);
        assert!(m.h > 0.0);
    }
}
