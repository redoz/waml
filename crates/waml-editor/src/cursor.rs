//! Hover cursor helpers.
//!
//! Makepad keeps whatever cursor a widget last set until some *other* widget
//! sets its own. There is no per-widget scope and no automatic reset, so a
//! widget that sets `Hand` on `FingerHoverIn` and stays silent on
//! `FingerHoverOut` leaks that cursor over every pixel that belongs to no
//! widget -- empty chrome, the gap under the tree's last row, panel padding.
//! `PanelSplitter` hit this first and reset by hand; these two wrappers exist
//! so the pairing is obvious at every other call site.
//!
//! Rule: every [`hover_in`] needs a matching [`hover_out`], and any widget
//! that sets a cursor on `FingerDown` must release it on `FingerUp` when the
//! release landed off the widget (see [`drag_end`]) -- a drag that ends
//! outside gets no `FingerHoverOut`.

use makepad_widgets::*;

/// Claim `cursor` while the pointer is over this widget.
pub fn hover_in(cx: &mut Cx, cursor: MouseCursor) {
    cx.set_cursor(cursor);
}

/// Hand the cursor back to the platform default as the pointer leaves.
pub fn hover_out(cx: &mut Cx) {
    cx.set_cursor(MouseCursor::Default);
}

/// Settle the cursor after a press ends: back to `hovered` if the release
/// landed on the widget, otherwise released outright. `is_over` on the
/// `FingerUp` event is the value to pass for `still_over`.
pub fn drag_end(cx: &mut Cx, still_over: bool, hovered: MouseCursor) {
    if still_over {
        cx.set_cursor(hovered);
    } else {
        hover_out(cx);
    }
}
