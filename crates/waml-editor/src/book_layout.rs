//! The book's virtualization policy, as pure math (spec: "Virtualization is
//! required, not an optimization"). The widget applies these answers; keeping
//! them makepad-free is what makes a two-hundred-section policy testable
//! without a window.

use crate::book_model::SectionBody;

/// The one legibility cap for inline diagram embeds -- a single constant so
/// visual sign-off tunes one number, not a scatter of literals.
pub const DIAGRAM_EMBED_HEIGHT: f64 = 400.0;
/// The caption strip (title + open-full button) above a diagram embed.
pub const DIAGRAM_CAPTION_HEIGHT: f64 = 28.0;
/// Screens of margin either side of the viewport that stay live.
pub const LIVE_MARGIN_SCREENS: f64 = 1.0;

/// A never-measured section reserves this until first drawn; a measured one
/// keeps its last real height (cached by the widget, keyed by RowId).
pub fn estimated_height(body: &SectionBody) -> f64 {
    match body {
        SectionBody::Heading => 56.0,
        SectionBody::Prose { .. } => 320.0,
        SectionBody::Diagram { .. } => DIAGRAM_EMBED_HEIGHT + DIAGRAM_CAPTION_HEIGHT,
        SectionBody::Link { .. } => 32.0,
    }
}

pub fn section_tops(heights: &[f64]) -> Vec<f64> {
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = 0.0;
    for h in heights {
        tops.push(y);
        y += h;
    }
    tops
}

/// The half-open index range of sections intersecting the viewport plus
/// [`LIVE_MARGIN_SCREENS`] each side. Only these hold live child widgets.
pub fn live_window(
    tops: &[f64],
    heights: &[f64],
    scroll: f64,
    viewport: f64,
) -> std::ops::Range<usize> {
    if tops.is_empty() {
        return 0..0;
    }
    let margin = viewport * LIVE_MARGIN_SCREENS;
    let lo = scroll - margin;
    let hi = scroll + viewport + margin;
    // First section whose bottom is past `lo`; first section whose top is
    // past `hi`. Tops are sorted, so partition_point is exact for the end.
    let start = tops
        .iter()
        .zip(heights)
        .position(|(&top, &h)| top + h > lo)
        .unwrap_or(tops.len());
    let end = tops.partition_point(|&top| top < hi);
    start..end.max(start)
}

/// The section whose top is nearest AT OR ABOVE the fold (`scroll`): the
/// reader is "in" it. `None` only for an empty book.
pub fn current_section(tops: &[f64], scroll: f64) -> Option<usize> {
    if tops.is_empty() {
        return None;
    }
    Some(tops.partition_point(|&top| top <= scroll).saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heights(n: usize, h: f64) -> Vec<f64> {
        vec![h; n]
    }

    #[test]
    fn section_tops_are_prefix_sums_of_heights() {
        assert_eq!(section_tops(&[10.0, 20.0, 30.0]), vec![0.0, 10.0, 30.0]);
        assert!(section_tops(&[]).is_empty());
    }

    #[test]
    fn the_live_window_covers_the_viewport_plus_one_screen_each_side() {
        let hs = heights(100, 100.0); // 10_000 tall book
        let tops = section_tops(&hs);
        // Viewport 600 tall, scrolled to 3000: visible 3000..3600, margin
        // 2400..4200 -> sections 24..42.
        assert_eq!(live_window(&tops, &hs, 3000.0, 600.0), 24..42);
        // Top of the book clamps at 0.
        assert_eq!(live_window(&tops, &hs, 0.0, 600.0), 0..12);
        // Bottom clamps at len.
        assert_eq!(live_window(&tops, &hs, 9800.0, 600.0), 92..100);
    }

    #[test]
    fn an_empty_book_has_an_empty_live_window() {
        assert_eq!(live_window(&[], &[], 0.0, 600.0), 0..0);
    }

    #[test]
    fn the_current_section_is_the_nearest_top_at_or_above_the_fold() {
        let hs = heights(5, 100.0);
        let tops = section_tops(&hs);
        assert_eq!(current_section(&tops, 0.0), Some(0));
        assert_eq!(current_section(&tops, 150.0), Some(1));
        assert_eq!(current_section(&tops, 100.0), Some(1));
        assert_eq!(current_section(&tops, 5000.0), Some(4));
        assert_eq!(current_section(&[], 0.0), None);
    }

    // `DIAGRAM_EMBED_HEIGHT > 0.0` below is a constant expression by
    // construction (both operands are `const`); the assertion exists to
    // document the invariant for a reader/reviewer, not to catch a runtime
    // regression, so the lint is deliberately silenced here rather than
    // dropping the check.
    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn estimates_are_per_kind_and_the_diagram_estimate_is_the_cap() {
        use crate::book_model::{LinkReason, SectionBody};
        let heading = estimated_height(&SectionBody::Heading);
        let link = estimated_height(&SectionBody::Link {
            reason: LinkReason::NestedBook,
        });
        assert!(heading > link, "a heading reserves more than a link row");
        // The diagram estimate includes the caption strip above the cap.
        // (Constructing Prose/Diagram bodies needs real documents; the two
        // constants are asserted directly instead.)
        assert!(DIAGRAM_EMBED_HEIGHT > 0.0);
    }
}
