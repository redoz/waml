//! Shared overlay chrome: a window-centered scrim + panel that scrolls its
//! content under a clip. `PanelGeom` is the pure geometry (this task); the
//! `OverlayShell` embed (Task 2) wraps it with draw + event plumbing. Mirrors
//! the `LinearGeom` (popup/menu.rs) split: geometry is `Cx`-free and unit-tested.

use makepad_widgets::*;

// Not yet consumed outside this file's own tests until Task 2 (`OverlayShell`)
// wires it up — `#[allow(dead_code)]` follows the `LinearGeom` precedent
// (popup/menu.rs) so the workspace clippy gate stays green in the meantime.
#[allow(dead_code)]
/// Vertical padding between the panel edge and the content column (lpx).
pub const PANEL_PAD_V: f64 = 20.0;
#[allow(dead_code)]
/// Horizontal padding between the panel edge and the content column (lpx).
pub const PANEL_PAD_H: f64 = 24.0;
#[allow(dead_code)]
/// Minimum margin the panel keeps off the top/bottom window edges (lpx); the
/// panel never grows taller than `window.y - 2*WINDOW_MARGIN`.
pub const WINDOW_MARGIN: f64 = 56.0;
#[allow(dead_code)]
/// Scrollbar thumb width (lpx).
pub const SCROLLBAR_W: f64 = 4.0;
#[allow(dead_code)]
/// Inset of the thumb from the panel's right edge (lpx).
pub const SCROLLBAR_INSET: f64 = 3.0;
#[allow(dead_code)]
/// Shortest the thumb ever gets so it stays grabbable (lpx).
pub const SCROLLBAR_MIN_THUMB: f64 = 24.0;
#[allow(dead_code)]
/// Height of the source-bright top edge hairline (lpx).
pub const EDGE_H: f64 = 1.5;

/// Pure geometry for a window-centered overlay panel that scrolls its content
/// under a clip. `content_h` is the consumer's measured content height; the
/// panel clamps to `max_panel_h`, the overflow scrolls, and `scroll` is always
/// kept in `[0, max_scroll]` by `set_scroll`. Mirrors `LinearGeom`.
#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
pub struct PanelGeom {
    window: DVec2,
    panel_w: f64,
    content_h: f64,
    scroll: f64,
}

#[allow(dead_code)]
impl PanelGeom {
    pub fn new(window: DVec2, panel_w: f64, content_h: f64) -> Self {
        Self {
            window,
            panel_w,
            content_h,
            scroll: 0.0,
        }
    }
    /// Tallest the panel may be (window minus top+bottom margin).
    pub fn max_panel_h(&self) -> f64 {
        (self.window.y - WINDOW_MARGIN * 2.0).max(PANEL_PAD_V * 2.0)
    }
    /// Visible panel height: content + vertical pad, clamped to the window.
    pub fn panel_height(&self) -> f64 {
        (self.content_h + PANEL_PAD_V * 2.0).min(self.max_panel_h())
    }
    /// The whole card rect, centered in the window.
    pub fn panel_rect(&self) -> Rect {
        let h = self.panel_height();
        Rect {
            pos: dvec2(
                (self.window.x - self.panel_w) * 0.5,
                (self.window.y - h) * 0.5,
            ),
            size: dvec2(self.panel_w, h),
        }
    }
    /// Height of the content viewport (panel minus top/bottom pad).
    pub fn viewport_height(&self) -> f64 {
        (self.panel_height() - PANEL_PAD_V * 2.0).max(0.0)
    }
    /// Content column width (panel minus left/right pad).
    pub fn content_width(&self) -> f64 {
        (self.panel_w - PANEL_PAD_H * 2.0).max(0.0)
    }
    /// Largest valid scroll offset; `0` when the whole content fits.
    pub fn max_scroll(&self) -> f64 {
        (self.content_h - self.viewport_height()).max(0.0)
    }
    pub fn scroll(&self) -> f64 {
        self.scroll
    }
    /// Set the scroll offset, clamped into `[0, max_scroll]`.
    pub fn set_scroll(&mut self, scroll: f64) {
        self.scroll = scroll.clamp(0.0, self.max_scroll());
    }
    /// Top-left of the (scroll-shifted) content column — what `begin` hands the
    /// consumer to place its first row at.
    pub fn content_origin(&self) -> DVec2 {
        let p = self.panel_rect();
        dvec2(p.pos.x + PANEL_PAD_H, p.pos.y + PANEL_PAD_V - self.scroll)
    }
    /// Interior viewport rect the content is clipped to (full panel width so a
    /// row's own inset decides its left edge; height = viewport).
    pub fn clip_rect(&self) -> Rect {
        let p = self.panel_rect();
        Rect {
            pos: dvec2(p.pos.x, p.pos.y + PANEL_PAD_V),
            size: dvec2(self.panel_w, self.viewport_height()),
        }
    }
    /// Source-bright top-edge hairline rect.
    pub fn edge_rect(&self) -> Rect {
        let p = self.panel_rect();
        Rect {
            pos: p.pos,
            size: dvec2(self.panel_w, EDGE_H),
        }
    }
    /// The scrollbar thumb rect, or `None` when nothing scrolls.
    pub fn thumb_rect(&self) -> Option<Rect> {
        let max = self.max_scroll();
        if max <= 0.0 {
            return None;
        }
        let track_h = self.viewport_height();
        let p = self.panel_rect();
        let track_top = p.pos.y + PANEL_PAD_V;
        let thumb_h = (track_h * track_h / self.content_h.max(1.0)).max(SCROLLBAR_MIN_THUMB);
        let t = self.scroll / max;
        let x = p.pos.x + self.panel_w - SCROLLBAR_W - SCROLLBAR_INSET;
        Some(Rect {
            pos: dvec2(x, track_top + t * (track_h - thumb_h)),
            size: dvec2(SCROLLBAR_W, thumb_h),
        })
    }
    /// Invert `thumb_rect`: the scroll offset that puts the thumb top at `y`.
    pub fn scroll_for_thumb_y(&self, thumb_y: f64) -> f64 {
        let track_h = self.viewport_height();
        let track_top = self.panel_rect().pos.y + PANEL_PAD_V;
        let thumb_h = (track_h * track_h / self.content_h.max(1.0)).max(SCROLLBAR_MIN_THUMB);
        let span = (track_h - thumb_h).max(1.0);
        let t = ((thumb_y - track_top) / span).clamp(0.0, 1.0);
        t * self.max_scroll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: DVec2 = DVec2 {
        x: 1200.0,
        y: 800.0,
    };
    const PANEL_W: f64 = 460.0;

    #[test]
    fn short_content_hugs_and_never_scrolls() {
        // Content shorter than the window budget: panel = content + vertical pad,
        // nothing scrolls, no thumb.
        let g = PanelGeom::new(WIN, PANEL_W, 120.0);
        assert_eq!(g.panel_height(), 120.0 + PANEL_PAD_V * 2.0);
        assert_eq!(g.max_scroll(), 0.0);
        assert!(g.thumb_rect().is_none());
        // Centered.
        let r = g.panel_rect();
        assert!((r.pos.x - (WIN.x - PANEL_W) * 0.5).abs() < 0.001);
        assert_eq!(r.size.x, PANEL_W);
    }

    #[test]
    fn tall_content_clamps_to_window_and_scrolls() {
        let g = PanelGeom::new(WIN, PANEL_W, 4000.0);
        assert_eq!(g.panel_height(), g.max_panel_h());
        assert_eq!(g.max_panel_h(), WIN.y - WINDOW_MARGIN * 2.0);
        assert!(g.max_scroll() > 0.0);
        assert!(g.thumb_rect().is_some());
    }

    #[test]
    fn set_scroll_clamps_to_range() {
        let mut g = PanelGeom::new(WIN, PANEL_W, 4000.0);
        g.set_scroll(-100.0);
        assert_eq!(g.scroll(), 0.0);
        g.set_scroll(1_000_000.0);
        assert_eq!(g.scroll(), g.max_scroll());
    }

    #[test]
    fn content_origin_shifts_up_with_scroll() {
        let mut g = PanelGeom::new(WIN, PANEL_W, 4000.0);
        let base = g.content_origin().y;
        g.set_scroll(200.0);
        assert!((g.content_origin().y - (base - 200.0)).abs() < 0.001);
        // x is the left inset regardless of scroll.
        assert!((g.content_origin().x - (g.panel_rect().pos.x + PANEL_PAD_H)).abs() < 0.001);
        assert!((g.content_width() - (PANEL_W - PANEL_PAD_H * 2.0)).abs() < 0.001);
    }

    #[test]
    fn thumb_y_round_trips_through_scroll() {
        let mut g = PanelGeom::new(WIN, PANEL_W, 4000.0);
        g.set_scroll(g.max_scroll() * 0.4);
        let thumb = g.thumb_rect().unwrap();
        assert!((g.scroll_for_thumb_y(thumb.pos.y) - g.scroll()).abs() < 0.5);
        // Thumb sits inside the panel's right edge.
        let p = g.panel_rect();
        assert!((thumb.pos.x - (p.pos.x + PANEL_W - SCROLLBAR_W - SCROLLBAR_INSET)).abs() < 0.001);
    }

    #[test]
    fn clip_rect_is_the_panel_interior_viewport() {
        let g = PanelGeom::new(WIN, PANEL_W, 4000.0);
        let p = g.panel_rect();
        let clip = g.clip_rect();
        assert!((clip.pos.y - (p.pos.y + PANEL_PAD_V)).abs() < 0.001);
        assert!((clip.size.y - g.viewport_height()).abs() < 0.001);
        assert!((g.viewport_height() - (g.panel_height() - PANEL_PAD_V * 2.0)).abs() < 0.001);
    }
}
