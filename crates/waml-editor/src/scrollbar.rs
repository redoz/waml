//! One scrollbar geometry, shared by every list that scrolls.
//!
//! The thumb rect and its inverse were copy-adapted three times — the tree
//! panel, the overlay shell's panel, and the book surface — each with its own
//! spelling of the same arithmetic. Copies of a formula drift silently: a
//! thumb that is painted from one derivation and hit-tested from another
//! lands under the pointer everywhere except the case nobody tried.
//!
//! Callers differ only in where the track sits and how far the thumb is
//! inset from the right edge, so those are the parameters.

use makepad_widgets::*;

/// The shortest a thumb may get before it stops being grabbable on a long
/// document. Shared: every surface agrees on this floor.
///
/// Thumb WIDTH is deliberately not shared — the tree panel draws 6px and the
/// overlay panel 4px, and that is a look decision per surface, not drift.
pub const SCROLLBAR_MIN_THUMB: f64 = 24.0;

/// Where a scrollbar's track is and what it is scrolling.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarGeometry {
    /// Top of the track, in the same absolute space as the returned rect.
    pub track_top: f64,
    /// Right edge of the region the thumb hangs off.
    pub track_right: f64,
    /// Track height — the visible viewport, not the content.
    pub track_h: f64,
    /// Total scrollable content height.
    pub content_h: f64,
    /// Current scroll offset.
    pub scroll: f64,
    /// Gap between the thumb's right edge and `track_right`.
    pub inset: f64,
    /// Thumb width, per surface.
    pub width: f64,
}

impl ScrollbarGeometry {
    /// The furthest this content can scroll. Never negative.
    pub fn max_scroll(&self) -> f64 {
        (self.content_h - self.track_h).max(0.0)
    }

    /// Height of the thumb, floored so it stays grabbable.
    fn thumb_h(&self) -> f64 {
        let visible = (self.track_h / self.content_h.max(1.0)).clamp(0.0, 1.0);
        (self.track_h * visible).max(SCROLLBAR_MIN_THUMB)
    }

    /// The thumb rect, or `None` when the content fits and nothing scrolls.
    ///
    /// The same rect must be painted and hit-tested — deriving one of them
    /// separately is how a thumb ends up unclickable.
    pub fn thumb_rect(&self) -> Option<Rect> {
        if self.content_h <= self.track_h || self.track_h <= 0.0 {
            return None;
        }
        let thumb_h = self.thumb_h();
        let travel = self.track_h - thumb_h;
        let max_scroll = self.max_scroll();
        let progress = if max_scroll > 0.0 {
            self.scroll / max_scroll
        } else {
            0.0
        };
        Some(Rect {
            pos: dvec2(
                self.track_right - self.width - self.inset,
                self.track_top + travel * progress,
            ),
            size: dvec2(self.width, thumb_h),
        })
    }

    /// Invert [`Self::thumb_rect`]: the scroll offset that puts the thumb's
    /// top at absolute `thumb_y`.
    ///
    /// Clamped, so a drag past either end pins at that end rather than
    /// running away.
    pub fn scroll_for_thumb_y(&self, thumb_y: f64) -> f64 {
        if self.content_h <= self.track_h {
            return 0.0;
        }
        let travel = (self.track_h - self.thumb_h()).max(1.0);
        let progress = ((thumb_y - self.track_top) / travel).clamp(0.0, 1.0);
        progress * self.max_scroll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(content_h: f64, scroll: f64) -> ScrollbarGeometry {
        ScrollbarGeometry {
            track_top: 100.0,
            track_right: 300.0,
            track_h: 200.0,
            content_h,
            scroll,
            inset: 0.0,
            width: 6.0,
        }
    }

    #[test]
    fn nothing_scrolls_means_no_thumb() {
        assert!(geom(200.0, 0.0).thumb_rect().is_none());
        assert!(geom(50.0, 0.0).thumb_rect().is_none());
    }

    #[test]
    fn the_thumb_spans_the_track_ends_exactly() {
        let top = geom(1000.0, 0.0).thumb_rect().unwrap();
        assert_eq!(top.pos.y, 100.0, "at rest the thumb sits at the track top");

        let g = geom(1000.0, 0.0);
        let bottom = geom(1000.0, g.max_scroll()).thumb_rect().unwrap();
        assert_eq!(
            bottom.pos.y + bottom.size.y,
            300.0,
            "fully scrolled, the thumb's bottom meets the track's"
        );
    }

    #[test]
    fn a_short_thumb_is_floored_so_it_stays_grabbable() {
        let thumb = geom(100_000.0, 0.0).thumb_rect().unwrap();
        assert_eq!(thumb.size.y, SCROLLBAR_MIN_THUMB);
    }

    /// The painted rect and the drag that chases it must agree, including
    /// where the floor has kicked in.
    #[test]
    fn dragging_to_a_thumb_position_reproduces_it() {
        for content_h in [400.0, 1000.0, 100_000.0] {
            let g = geom(content_h, 0.0);
            for scroll in [0.0, g.max_scroll() * 0.37, g.max_scroll()] {
                let g = geom(content_h, scroll);
                let thumb = g.thumb_rect().unwrap();
                let recovered = g.scroll_for_thumb_y(thumb.pos.y);
                assert!(
                    (recovered - scroll).abs() < 0.001,
                    "content {content_h} scroll {scroll}: recovered {recovered}"
                );
            }
        }
    }

    #[test]
    fn dragging_past_either_end_pins_rather_than_running_away() {
        let g = geom(1000.0, 0.0);
        assert_eq!(g.scroll_for_thumb_y(-10_000.0), 0.0);
        assert_eq!(g.scroll_for_thumb_y(10_000.0), g.max_scroll());
    }

    #[test]
    fn the_inset_moves_the_thumb_left_of_the_track_edge() {
        let mut g = geom(1000.0, 0.0);
        let flush = g.thumb_rect().unwrap();
        g.inset = 4.0;
        let inset = g.thumb_rect().unwrap();
        assert_eq!(flush.pos.x - inset.pos.x, 4.0);
        assert_eq!(flush.pos.x + 6.0, 300.0);
    }
}
