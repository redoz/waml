//! The "STALE" projection badge shared by every canvas surface. One
//! implementation: `ClassDiagramSurface` and `BehaviorSurface` both pin it to
//! the canvas's top-right corner when their projection is stale.

use makepad_widgets::*;

const STALE_BADGE_LABEL: &str = "STALE";
const STALE_BADGE_WIDTH: f64 = 58.0;
const STALE_BADGE_HEIGHT: f64 = 24.0;
const STALE_BADGE_INSET: f64 = 12.0;

pub(super) fn stale_badge_rect(view_rect: Rect) -> Rect {
    Rect {
        pos: dvec2(
            view_rect.pos.x + view_rect.size.x - STALE_BADGE_WIDTH - STALE_BADGE_INSET,
            view_rect.pos.y + STALE_BADGE_INSET,
        ),
        size: dvec2(STALE_BADGE_WIDTH, STALE_BADGE_HEIGHT),
    }
}

pub(super) fn draw_stale_badge_overlay(
    cx: &mut Cx2d,
    view_rect: Rect,
    badge: &mut DrawColor,
    text: &mut DrawText,
) {
    let rect = stale_badge_rect(view_rect);
    badge.draw_abs(cx, rect);
    let size = text
        .layout(
            cx,
            0.0,
            0.0,
            None,
            false,
            Align::default(),
            STALE_BADGE_LABEL,
        )
        .size_in_lpxs;
    text.draw_abs(
        cx,
        dvec2(
            rect.pos.x + (rect.size.x - size.width as f64) * 0.5,
            rect.pos.y + (rect.size.y - size.height as f64) * 0.5,
        ),
        STALE_BADGE_LABEL,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_badge_is_fixed_to_the_canvas_top_right() {
        assert_eq!(
            stale_badge_rect(Rect {
                pos: dvec2(100.0, 50.0),
                size: dvec2(800.0, 600.0),
            }),
            Rect {
                pos: dvec2(830.0, 62.0),
                size: dvec2(58.0, 24.0),
            }
        );
    }
}
