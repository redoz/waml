//! `RecentRowView`: one recent-project row on the start screen. Renders
//! `[Package glyph] [title / path stacked] [timestamp flush-right] [pin]`
//! purely from the makepad layout engine -- no text measurement, no y-offsets.
//! The timestamp is right-anchored by the `Fill` width on the middle text
//! column, which consumes all slack and shoves the `Fit`-width `when` label
//! right; the pin rides its own anchor past it, at the row's right edge. Both
//! icons are drawn immediate-mode (`IconSet::draw`) over their layout anchors.
//!
//! The row hit-tests its own area and emits widget-actions read by
//! `StartScreen` through `FlatList::items_with_actions`: `Clicked` on a
//! `FingerUp` over the row body, and `TogglePin` when the pin anchor claims
//! that `FingerUp` first. Its hover wash is self-managed from raw `MouseMove`
//! rect-containment (not `FingerHoverIn`/`Out`, which the pin's child area
//! would steal the moment the pointer crossed onto it) -- the `#[deref] View`
//! hybrid pattern (same as `inspector_panel.rs`), with granular per-line
//! setters the parent calls per row. Containment is tested against the row's
//! *live, clipped* area, and only for a row the current pass actually drew, so
//! a row `FlatList` scrolled out of the five-row box cannot latch hover off a
//! stale rect; hover is likewise dropped on window leave and whenever the row
//! is re-seated (`set_pinned`/`set_clickable`), since a `TogglePin` re-sort
//! moves the row out from under a pointer that never moves again.
//!
//! The empty-state row (`set_clickable(false)`) neither washes nor fires, and
//! hides the glyph anchor outright so its placeholder text starts flush at the
//! row padding instead of behind a phantom icon gap.

use makepad_widgets::*;

use crate::icons::{Icon, IconSet};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.RecentRowViewBase = #(RecentRowView::register_widget(vm))

    mod.widgets.RecentRowView = set_type_default() do mod.widgets.RecentRowViewBase{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        // Tighter than before (was top/bottom 5) to reach VS's compact pitch.
        padding: Inset{left: 12.0, right: 12.0, top: 3.0, bottom: 3.0}
        spacing: 10.0
        show_bg: true

        // Row hover wash (unchanged): a subtle premultiplied accent fill faded
        // by the `hover` uniform the widget pushes from its rect-containment
        // hover tracking each draw.
        draw_bg +: {
            color: atlas.accent
            hover: uniform(0.0)
            pixel: fn() {
                let a = 0.12 * self.hover
                return vec4(self.color.x * a, self.color.y * a, self.color.z * a, a)
            }
        }

        // Colour-only holders (never drawn): the immediate-mode glyphs copy
        // `color` from these per draw, so no RGBA crosses Rust (icon_button.rs
        // pattern). `draw_pkg` tints the left document glyph; the pin uses
        // `draw_pin_lit` when pinned or pin-hovered, else `draw_pin_idle`.
        draw_pkg +: { color: atlas.text_dim }
        draw_pin_idle +: { color: atlas.text_dim }
        draw_pin_lit +: { color: atlas.accent }

        // Left document glyph anchor: a 16x16 spacer reserving the flow slot;
        // `Icon::Package` is drawn immediate over this rect in `draw_walk`,
        // which also hides this whole anchor on the empty-state row.
        glyph := View {
            width: 16.0
            height: 16.0
        }

        // Title over path, stacked. The timestamp rides the title line; `title`
        // is `Fill` inside `titlerow`, shoving the `Fit` `when` flush right.
        textcol := View {
            width: Fill
            height: Fit
            flow: Down
            spacing: -2.0

            titlerow := View {
                width: Fill
                height: Fit
                flow: Right
                align: Align{y: 0.5}
                title := Label {
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: atlas.text
                        text_style: fonts.text_label
                    }
                }
                when := Label {
                    text: ""
                    draw_text +: {
                        color: atlas.text_dim
                        text_style: fonts.text_label
                    }
                }
            }

            path := Label {
                text: ""
                draw_text +: {
                    color: atlas.text_dim
                    text_style: fonts.text_menu
                }
            }
        }

        // Pin anchor: a 20x20 spacer at the row's right edge. `Icon::Pin` is
        // drawn immediate over this rect (centered 16px) only when the row is
        // hovered or the row is pinned (VS on-hover reveal). Its own FingerUp is
        // hit-tested first in `handle_event` so a pin click toggles without
        // opening the model.
        pin := View {
            width: 20.0
            height: 20.0
        }
    }
}

/// Emitted (grouped through the parent `FlatList`) when a row is clicked.
/// `StartScreen::handle_actions` reads it via `items_with_actions` +
/// `RecentRowViewRef::clicked` and maps the row back to a recent index.
#[derive(Clone, Debug, Default)]
pub enum RecentRowViewAction {
    #[default]
    None,
    /// The row body was clicked — open this recent.
    Clicked,
    /// The pin button was clicked — toggle this recent's pinned state.
    TogglePin,
}

#[derive(Script, ScriptHook, Widget)]
pub struct RecentRowView {
    /// The row container: glyph anchor + stacked text + pin anchor.
    #[deref]
    view: View,

    /// SDF icon set (shared Atlas material), drawn via `IconSet::draw`.
    #[live]
    icons: IconSet,
    /// Colour-only holders, copied into the glyph tint per draw.
    #[live]
    draw_pkg: DrawColor,
    #[live]
    draw_pin_idle: DrawColor,
    #[live]
    draw_pin_lit: DrawColor,

    /// Row pointer-over state, tracked by rect containment (a child area would
    /// steal `Hit::FingerHover`; the containment test keeps the pin inside the
    /// row's hover). Fed to the `hover` uniform each `draw_walk`. Only ever true
    /// while this row's area is live for the current draw pass — see
    /// [`hover_verdict`].
    #[rust]
    hovered: bool,
    /// Pointer-over the pin anchor specifically (lights the pin tint).
    #[rust]
    pin_hovered: bool,
    /// Whether this row responds to hover/click. The empty-state row leaves it
    /// false so it neither washes, fires, nor draws a pin.
    #[rust]
    clickable: bool,
    /// Whether this recent is pinned (drives the pin's always-visible + accent
    /// tint). Pushed per row from `StartScreen`.
    #[rust]
    pinned: bool,
    /// Cached absolute rects from the last `draw_walk`, for the immediate-mode
    /// glyphs drawn over their anchors. Hover does NOT read these — it re-reads
    /// the live areas each event, so a stale cache cannot latch hover.
    #[rust]
    glyph_rect: Rect,
    #[rust]
    pin_rect: Rect,
    /// The pin anchor's `Area`, for hit-testing its FingerUp before the row's.
    #[rust]
    pin_area: Area,
}

impl Widget for RecentRowView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.clickable {
            return;
        }
        let uid = self.widget_uid();

        // Hover by rect containment: a child area (the pin) would claim
        // Hit::FingerHover and drop the row's hover the moment the pointer
        // crosses onto the pin, so track both off raw MouseMove instead
        // (the scrim-hover fix). The row rect encloses the pin, so hovering the
        // pin keeps the row hovered -> the revealed pin never flickers away.
        if let Event::MouseMove(e) = event {
            let (over_row, over_pin) = self.hover_at(cx, e.abs);
            if over_row != self.hovered || over_pin != self.pin_hovered {
                self.hovered = over_row;
                self.pin_hovered = over_pin;
                self.view.redraw(cx);
            }
        }

        // MouseMove is the only thing that can *clear* containment hover, and
        // it never arrives when the pointer leaves the window outright -- which
        // would otherwise strand a washed, pin-revealed row behind the user's
        // back. Drop hover on the way out.
        if let Event::MouseLeave(_) = event {
            if self.hovered || self.pin_hovered {
                self.hovered = false;
                self.pin_hovered = false;
                self.view.redraw(cx);
            }
        }

        // Pin claims its FingerUp first (toggles without opening). The pin area
        // is topmost over its rect, so the row body's hit below bails there.
        //
        // Gated on the same verdict as the draw: the 20x20 anchor is laid out on
        // every row whether or not the pin is revealed, so an ungated hit lets an
        // *invisible* pin swallow the press. Two ways that bites: a `TogglePin`
        // re-sorts the list and clears hover, and no MouseMove follows a
        // stationary pointer -- so a second click in the same spot would pin
        // whichever recent slid underneath; and a wheel-scroll leaves hover
        // stale-false, so a click at a row's right edge would pin instead of
        // opening the model.
        if pin_visible(self.clickable, self.hovered, self.pinned) {
            if let Hit::FingerUp(fe) = event.hits(cx, self.pin_area) {
                if fe.is_primary_hit() && fe.is_over {
                    cx.widget_action(uid, RecentRowViewAction::TogglePin);
                    return;
                }
            }
        }

        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, RecentRowViewAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(hover), &[if self.hovered { 1.0 } else { 0.0 }]);
        // The empty-state row draws no `Icon::Package`, so drop its anchor
        // instead of merely skipping the glyph: an invisible child walks no
        // turtle, so the 16px slot *and* the 10px flow spacing beside it both
        // collapse and "No recent models" starts flush at the row padding.
        self.view
            .view(cx, ids!(glyph))
            .set_visible(cx, self.clickable);
        let step = self.view.draw_walk(cx, scope, walk);

        // Cache post-layout rects for the immediate-mode glyphs below.
        self.glyph_rect = self.view.widget(cx, ids!(glyph)).area().rect(cx);
        self.pin_area = self.view.widget(cx, ids!(pin)).area();
        self.pin_rect = self.pin_area.rect(cx);

        // Left document glyph, filling its 16x16 anchor. Gated on `clickable`
        // like the pin below, so the empty-state row ("No recent models") shows
        // bare text with no package icon beside it (its anchor is hidden above,
        // so `glyph_rect` is meaningless there anyway).
        if self.clickable {
            self.icons
                .draw(cx, Icon::Package, self.glyph_rect, self.draw_pkg.color);
        }

        // Pin: VS on-hover reveal — draw only when the row is hovered or pinned.
        // Accent when pinned or pin-hovered, else dim. Same verdict gates the
        // pin's hit test in `handle_event`, so the pin is never clickable while
        // it is invisible.
        if pin_visible(self.clickable, self.hovered, self.pinned) {
            let lit = self.pinned || self.pin_hovered;
            let tint = if lit {
                self.draw_pin_lit.color
            } else {
                self.draw_pin_idle.color
            };
            let sz = 16.0;
            let g = Rect {
                pos: dvec2(
                    (self.pin_rect.pos.x + (self.pin_rect.size.x - sz) * 0.5).round(),
                    (self.pin_rect.pos.y + (self.pin_rect.size.y - sz) * 0.5).round(),
                ),
                size: dvec2(sz, sz),
            };
            self.icons.draw(cx, Icon::Pin, g, tint);
        }
        step
    }
}

/// Containment-hover verdict for one row: `(over_row, over_pin)`.
///
/// `drawn` is false for a row this pass did not lay out (a `FlatList` only
/// draws its visible window, so a row scrolled out of the five-row box keeps
/// whatever rect it last had): such a row must never claim the pointer, or its
/// stale rect latches a hover wash + revealed pin on a row nobody is over.
/// `row`/`pin` are the *clipped* rects, so a row only partly inside the list
/// box is only hovered over its visible part. The pin only counts while the row
/// does, since the pin sits inside the row.
/// Whether the pin is revealed on this row — VS shows it on row hover, and
/// keeps it up permanently once pinned. The empty-state row (`clickable`
/// false) never shows one.
///
/// Single source of truth for both the draw and the pin's hit test: the pin
/// anchor is laid out on every row regardless, so a hit test that did not agree
/// with the draw would let an invisible pin eat clicks meant for the row body.
fn pin_visible(clickable: bool, hovered: bool, pinned: bool) -> bool {
    clickable && (hovered || pinned)
}

fn hover_verdict(drawn: bool, row: Rect, pin: Rect, p: DVec2) -> (bool, bool) {
    // `Rect::contains` is inclusive on both edges, so a collapsed rect (an area
    // clipped fully away, or one that was never drawn) still "contains" its own
    // corner -- require real extent before a rect may claim the pointer.
    fn covers(r: Rect, p: DVec2) -> bool {
        r.size.x > 0.0 && r.size.y > 0.0 && r.contains(p)
    }
    let over_row = drawn && covers(row, p);
    (over_row, over_row && covers(pin, p))
}

impl RecentRowView {
    /// Live containment hover for pointer `p`: re-reads this row's areas (never
    /// a cached rect), so a row the current pass did not draw reports no hover.
    fn hover_at(&self, cx: &Cx, p: DVec2) -> (bool, bool) {
        let row_area = self.view.area();
        let drawn = row_area.is_valid(cx);
        if !drawn {
            return hover_verdict(false, Rect::default(), Rect::default(), p);
        }
        let pin = if self.pin_area.is_valid(cx) {
            self.pin_area.clipped_rect(cx)
        } else {
            Rect::default()
        };
        hover_verdict(true, row_area.clipped_rect(cx), pin, p)
    }

    /// Set the bold project title (top line).
    pub fn set_title(&mut self, cx: &mut Cx, s: &str) {
        self.view
            .label(cx, ids!(textcol.titlerow.title))
            .set_text(cx, s);
    }
    /// Set the dim project path (second line).
    pub fn set_path(&mut self, cx: &mut Cx, s: &str) {
        self.view.label(cx, ids!(textcol.path)).set_text(cx, s);
    }
    /// Set the right-anchored last-opened stamp.
    pub fn set_when(&mut self, cx: &mut Cx, s: &str) {
        self.view
            .label(cx, ids!(textcol.titlerow.when))
            .set_text(cx, s);
    }
    /// Toggle whether the row hovers/clicks (false for the empty-state row).
    /// A change re-seats the row (`FlatList` recycles row widgets), so drop any
    /// hover it carried: the pointer never entered *this* content.
    pub fn set_clickable(&mut self, clickable: bool) {
        if self.clickable != clickable {
            self.clickable = clickable;
            self.clear_hover();
        }
    }

    /// Forget containment hover. Called whenever the row is re-seated onto
    /// different content, since hover is only ever *cleared* by a MouseMove and
    /// a re-sort (e.g. the pinned block jumping to the top after `TogglePin`)
    /// can move this row out from under a stationary pointer.
    fn clear_hover(&mut self) {
        self.hovered = false;
        self.pin_hovered = false;
    }
    /// True when this row emitted a click in `actions`.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RecentRowViewAction::Clicked))
    }

    /// Intrinsic drawn pitch of one row (the two-line title/path text stack
    /// plus the 2x3 vertical padding). `StartScreen` sizes its list box to
    /// `5 * ROW_HEIGHT + list padding` so exactly five rows fit; verify the fit
    /// by screenshot after any font/padding change and retune if a 6th peeks
    /// in or the 5th clips. Measured 61.0 by screenshot (2026-07-24): the
    /// original 30.0 undersized the box (only ~2.5 rows fit) because it did
    /// not account for the real two-line `text_label`/`text_menu` stack
    /// height, only the 16px glyph anchor.
    pub const ROW_HEIGHT: f64 = 61.0;

    /// Drive the pinned state (pin glyph visibility + accent tint), redrawing
    /// only on a change.
    pub fn set_pinned(&mut self, cx: &mut Cx, pinned: bool) {
        if self.pinned != pinned {
            self.pinned = pinned;
            // The toggle re-sorts the list (pinned block first), so the row very
            // likely moved out from under the pointer -- and no MouseMove
            // follows a stationary click. Drop hover; the next move re-acquires
            // it on whichever row is genuinely underneath.
            self.clear_hover();
            self.view.redraw(cx);
        }
    }

    /// True when this row's pin emitted a toggle in `actions`.
    pub fn pin_toggled(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), RecentRowViewAction::TogglePin))
    }
}

impl RecentRowViewRef {
    /// `WidgetRef`-side setters, so the FlatList draw loop can push per-row text
    /// through `row.as_recent_row_view()` without borrowing the inner widget by
    /// hand.
    pub fn set_title(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_title(cx, s);
        }
    }
    pub fn set_path(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_path(cx, s);
        }
    }
    pub fn set_when(&self, cx: &mut Cx, s: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_when(cx, s);
        }
    }
    pub fn set_clickable(&self, clickable: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_clickable(clickable);
        }
    }
    /// See [`RecentRowView::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }
    pub fn set_pinned(&self, cx: &mut Cx, pinned: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_pinned(cx, pinned);
        }
    }
    /// See [`RecentRowView::pin_toggled`].
    pub fn pin_toggled(&self, actions: &Actions) -> bool {
        self.borrow()
            .is_some_and(|inner| inner.pin_toggled(actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_default_is_none() {
        assert!(matches!(
            RecentRowViewAction::default(),
            RecentRowViewAction::None
        ));
    }

    #[test]
    fn pin_is_hittable_exactly_when_it_is_drawn() {
        // Hovered or pinned reveals it; the empty-state row never shows one.
        assert!(pin_visible(true, true, false));
        assert!(pin_visible(true, false, true));
        assert!(pin_visible(true, true, true));
        assert!(!pin_visible(false, true, true));
        // The unhovered, unpinned row: the 20x20 anchor is still laid out, so
        // this false is what keeps the invisible pin from swallowing a click
        // after a re-sort or a wheel-scroll left `hovered` stale-false.
        assert!(!pin_visible(true, false, false));
    }

    fn rects() -> (Rect, Rect) {
        // A row at y=100..161 with its 20x20 pin anchor at the right edge.
        let row = Rect {
            pos: dvec2(0.0, 100.0),
            size: dvec2(400.0, 61.0),
        };
        let pin = Rect {
            pos: dvec2(370.0, 120.0),
            size: dvec2(20.0, 20.0),
        };
        (row, pin)
    }

    #[test]
    fn hover_tracks_row_and_pin_when_drawn() {
        let (row, pin) = rects();
        assert_eq!(
            hover_verdict(true, row, pin, dvec2(50.0, 130.0)),
            (true, false)
        );
        // The pin sits inside the row, so hovering it keeps the row hovered.
        assert_eq!(
            hover_verdict(true, row, pin, dvec2(378.0, 130.0)),
            (true, true)
        );
        assert_eq!(
            hover_verdict(true, row, pin, dvec2(50.0, 300.0)),
            (false, false)
        );
    }

    #[test]
    fn undrawn_row_never_hovers_off_a_stale_rect() {
        // A row the pass did not lay out (scrolled out of the five-row box)
        // keeps its last rects; containment against them would latch a hover
        // wash + revealed pin on a row the pointer is nowhere near.
        let (row, pin) = rects();
        assert_eq!(
            hover_verdict(false, row, pin, dvec2(50.0, 130.0)),
            (false, false)
        );
        assert_eq!(
            hover_verdict(false, row, pin, dvec2(378.0, 130.0)),
            (false, false)
        );
    }

    #[test]
    fn clipped_away_row_never_hovers() {
        // Scrolled fully under the list box's clip: `clipped_rect` collapses to
        // an empty rect, which must claim nothing (not even at the origin).
        let empty = Rect::default();
        assert_eq!(
            hover_verdict(true, empty, empty, dvec2(0.0, 0.0)),
            (false, false)
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn row_height_is_positive() {
        // The list box height (5 * ROW_HEIGHT + padding) must be well-formed.
        // Clippy flags this as a constant-value assertion (ROW_HEIGHT is a
        // `const`), which is exactly the shape-gate this test locks in —
        // suppress the lint rather than drop the check.
        assert!(RecentRowView::ROW_HEIGHT > 0.0);
    }
}
