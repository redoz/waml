//! `RecentRowView`: one recent-project row on the start screen. Renders
//! `[accent marker] [title / path stacked] .......... [timestamp flush-right]`
//! purely from the makepad layout engine -- no text measurement, no y-offsets.
//! The timestamp is right-anchored by the `Fill` width on the middle text
//! column, which consumes all slack and shoves the `Fit`-width `when` label to
//! the right edge.
//!
//! Task 3 of the start-screen recents refactor adds interaction: the row now
//! hit-tests its own area, emits a `RecentRowViewAction::Clicked` widget-action
//! on `FingerUp` (read by `StartScreen` through `FlatList::items_with_actions`),
//! and self-manages a subtle hover wash driven by FingerHoverIn/Out -- the
//! `#[deref] View` hybrid pattern (same as `inspector_panel.rs`), with granular
//! per-line setters the parent calls per row.

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
        // `Icon::Package` is drawn immediate over this rect in `draw_walk`.
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
    /// row's hover). Fed to the `hover` uniform each `draw_walk`.
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
    /// Cached absolute rects from the last `draw_walk`, for containment hover.
    #[rust]
    row_rect: Rect,
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
            let over_row = self.row_rect.contains(e.abs);
            let over_pin = self.pin_rect.contains(e.abs);
            if over_row != self.hovered || over_pin != self.pin_hovered {
                self.hovered = over_row;
                self.pin_hovered = over_pin;
                self.view.redraw(cx);
            }
        }

        // Pin claims its FingerUp first (toggles without opening). The pin area
        // is topmost over its rect, so the row body's hit below bails there.
        if let Hit::FingerUp(fe) = event.hits(cx, self.pin_area) {
            if fe.is_primary_hit() && fe.is_over {
                cx.widget_action(uid, RecentRowViewAction::TogglePin);
                return;
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
        let step = self.view.draw_walk(cx, scope, walk);

        // Cache post-layout rects for containment hover + immediate glyphs.
        self.row_rect = self.view.area().rect(cx);
        self.glyph_rect = self.view.widget(cx, ids!(glyph)).area().rect(cx);
        self.pin_area = self.view.widget(cx, ids!(pin)).area();
        self.pin_rect = self.pin_area.rect(cx);

        // Left document glyph, filling its 16x16 anchor. Gated on `clickable`
        // like the pin below, so the empty-state row ("No recent models") shows
        // bare text with no package icon beside it.
        if self.clickable {
            self.icons
                .draw(cx, Icon::Package, self.glyph_rect, self.draw_pkg.color);
        }

        // Pin: VS on-hover reveal — draw only when the row is hovered or pinned.
        // Accent when pinned or pin-hovered, else dim.
        if self.clickable && (self.hovered || self.pinned) {
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

impl RecentRowView {
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
    pub fn set_clickable(&mut self, clickable: bool) {
        self.clickable = clickable;
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
    #[allow(clippy::assertions_on_constants)]
    fn row_height_is_positive() {
        // The list box height (5 * ROW_HEIGHT + padding) must be well-formed.
        // Clippy flags this as a constant-value assertion (ROW_HEIGHT is a
        // `const`), which is exactly the shape-gate this test locks in —
        // suppress the lint rather than drop the check.
        assert!(RecentRowView::ROW_HEIGHT > 0.0);
    }
}
