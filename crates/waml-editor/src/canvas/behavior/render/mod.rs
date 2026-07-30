//! `BehaviorSurface` render passes. Task 6 only draws the Atlas background and
//! the centered empty-state message -- `Flow`/`Interaction` passes land in
//! Tasks 7-8.

use crate::canvas::viewport::ViewportSnapshot;
use makepad_widgets::*;

pub(super) struct BehaviorDrawResources<'a> {
    pub(super) bg: &'a mut DrawColor,
    pub(super) text: &'a mut DrawText,
}

pub(super) fn draw(
    cx: &mut Cx2d,
    viewport: ViewportSnapshot,
    message: &str,
    draws: &mut BehaviorDrawResources<'_>,
) {
    let rect = viewport.view_rect;
    draws.bg.draw_abs(cx, rect);
    if message.is_empty() {
        return;
    }
    let size = draws
        .text
        .layout(cx, 0.0, 0.0, None, false, Align::default(), message)
        .size_in_lpxs;
    let pos = dvec2(
        rect.pos.x + (rect.size.x - size.width as f64) * 0.5,
        rect.pos.y + (rect.size.y - size.height as f64) * 0.5,
    );
    draws.text.draw_abs(cx, pos, message);
}
