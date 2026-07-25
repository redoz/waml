//! `IconButton`: a small square icon button -- a catalog glyph (`IconSet`) over a
//! rounded accent hover wash. The shared recipe already proven by the tool
//! dock, split across two channels (`wash_and_ink`): a hover lights the wash,
//! and hover OR an `active` flag tints the glyph `atlas.text` ->
//! `atlas.accent`. A resting-active toggle is therefore a bare accent glyph
//! with no wash, so "on" never reads as "hovered". The glyph is picked at
//! runtime (`set_icon`),
//! so one instance can flip between paired states (pin/pin-off,
//! collapse/expand).
//!
//! Hybrid `#[deref] View` widget, same shape as `ActionLink`:
//! the View's `draw_bg` paints the wash (a `lit` uniform fades it in) and the
//! glyph is drawn immediate-mode via `IconSet::draw` in `draw_walk`, centered on
//! the button rect -- both emitted in the one draw pass, so a parent's alignment
//! shifts wash and glyph together. `handle_event` hit-tests its own
//! `view.area()`, emitting `Clicked` on release and `Pressed` on the down edge
//! (the burger's down-menu model, kept for the later caption migration); hover
//! drives the wash and the Hand cursor.

use crate::icons::{Icon, IconSet};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.IconButtonBase = #(IconButton::register_widget(vm))

    mod.widgets.IconButton = set_type_default() do mod.widgets.IconButtonBase{
        // Square, sized to sit inline with a 32px field (e.g. the inspector's
        // element picker) so a `y:0.5` bar centres the row as one block.
        width: 32.0
        height: 32.0
        show_bg: true
        // Rounded accent wash behind the glyph, faded in by `lit` -- HOVER
        // only now; an `active` button tints its glyph and paints no wash (see
        // `wash_and_ink`). A centred 28px square, the SAME accent @16% the tool dock /
        // caption buttons paint, so every icon button reads identically. The
        // square is CAPPED at 28px (`min(w,h)-4`, clamped) so a host that
        // stretches the button -- e.g. the inspector's `Fill x Fill` flag button
        // collapsing to the whole panel -- still gets a small fixed wash, not one
        // that grows with the box. Inline `pixel:` on `draw_bg` renders here
        // (proven by the inspector/tree frame shaders); `lit` is pushed each
        // `draw_walk`. `sdf.fill` premultiplies the color it is handed, so pass a
        // STRAIGHT accent + alpha `a` -- premultiplying here too double-darkens
        // the wash to a grey.
        draw_bg +: {
            color: atlas.accent
            lit: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let a = 0.16 * self.lit
                let ws = min(min(self.rect_size.x, self.rect_size.y) - 4.0, 28.0)
                sdf.box(
                    (self.rect_size.x - ws) * 0.5,
                    (self.rect_size.y - ws) * 0.5,
                    ws,
                    ws,
                    2.0,
                )
                sdf.fill(vec4(self.color.x, self.color.y, self.color.z, a))
                return sdf.result
            }
        }
        // Colour-only holders (never drawn): the glyph's `color` is copied from
        // one of these per draw (lit => accent, idle => text), so no RGBA crosses
        // Rust.
        draw_icon_lit +: { color: atlas.accent }
        draw_icon_idle +: { color: atlas.text }
        // Disabled ink: the glyph greys out and the wash never lights, so a
        // no-op control reads inert without changing size or moving neighbours.
        draw_icon_dim +: { color: atlas.text_dim }
        icon_size: 16.0
    }
}

/// Button input, read by the host widget. `Clicked` fires on a primary release
/// over the button; `Pressed` fires on the primary press and carries the press
/// position (kept for a down-edge drop-down, the burger's model -- unread until
/// the caption buttons migrate onto this widget).
#[derive(Clone, Debug, Default)]
pub enum IconButtonAction {
    #[default]
    None,
    Clicked,
    Pressed(DVec2),
}

#[derive(Script, ScriptHook, Widget)]
pub struct IconButton {
    /// The button box: the wash `draw_bg` declared above.
    #[deref]
    view: View,

    /// Colour-only holders: the glyph's `color` is copied from one of these per
    /// draw (lit => accent, idle => text), so the tint RGBA stays in the DSL.
    #[live]
    draw_icon_lit: DrawColor,
    #[live]
    draw_icon_idle: DrawColor,
    #[live]
    draw_icon_dim: DrawColor,
    /// SDF icon set (the shared Atlas material), drawn via `IconSet::draw`,
    /// tinted per-draw from the holders above.
    #[live]
    icons: IconSet,
    /// Side of the glyph square, centred in the button rect.
    #[live]
    icon_size: f64,

    /// The glyph to draw, set at runtime via [`IconButton::set_icon`]. `None`
    /// until set -- nothing draws (an empty, unlit button).
    #[rust]
    icon: Option<Icon>,
    /// Persistent lit state (e.g. an active tool / a pinned panel). Tints the
    /// glyph `atlas.accent` on its own; it does NOT paint the hover wash (see
    /// `wash_and_ink`), so resting-on and hovered are distinguishable states.
    #[rust]
    active: bool,
    /// Disabled: the glyph greys and the wash never lights. The button keeps its
    /// size and still emits `Clicked` — the host decides what that means.
    #[rust]
    dim: bool,
    /// Pointer-over state, self-managed from FingerHoverIn/Out.
    #[rust]
    hovered: bool,
    /// Last-drawn absolute rect, cached in `draw_walk` (menu anchor / a later
    /// caption drag-query seam).
    #[rust]
    rect: Rect,
}

/// The two independent light channels of an icon button, split so a resting
/// `active` toggle does not read as a hovered one:
/// `.0` = the 16% accent **wash** behind the glyph (hover only);
/// `.1` = the accent glyph **tint** (hover OR active).
/// A `dim` (disabled) button lights neither, however it is hovered or flagged.
/// Pure so the rule is unit-testable without a `Cx`.
fn wash_and_ink(hovered: bool, active: bool, dim: bool) -> (bool, bool) {
    if dim {
        return (false, false);
    }
    (hovered, hovered || active)
}

impl Widget for IconButton {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                cx.widget_action(uid, IconButtonAction::Pressed(fe.abs));
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                cx.widget_action(uid, IconButtonAction::Clicked);
            }
            Hit::FingerHoverIn(_) => {
                if !self.dim {
                    cx.set_cursor(MouseCursor::Hand);
                }
                self.hovered = true;
                self.view.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                self.hovered = false;
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Two channels (see `wash_and_ink`): `hot` paints the wash, `ink` tints
        // the glyph. A dim button never lights either.
        let (hot, ink) = wash_and_ink(self.hovered, self.active, self.dim);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(lit), &[if hot { 1.0 } else { 0.0 }]);
        let step = self.view.draw_walk(cx, scope, walk);
        let rect = self.view.area().rect(cx);
        self.rect = rect;
        if let Some(icon) = self.icon {
            let tint = if ink {
                self.draw_icon_lit.color
            } else if self.dim {
                self.draw_icon_dim.color
            } else {
                self.draw_icon_idle.color
            };
            let sz = self.icon_size;
            let glyph = Rect {
                pos: dvec2(
                    (rect.pos.x + (rect.size.x - sz) * 0.5).round(),
                    (rect.pos.y + (rect.size.y - sz) * 0.5).round(),
                ),
                size: dvec2(sz, sz),
            };
            self.icons.draw(cx, icon, glyph, tint);
        }
        step
    }
}

impl IconButton {
    /// Set the glyph, redrawing only on a change.
    pub fn set_icon(&mut self, cx: &mut Cx, icon: Icon) {
        if self.icon != Some(icon) {
            self.icon = Some(icon);
            self.view.redraw(cx);
        }
    }

    /// Drive the persistent lit state (active tool / pinned panel), redrawing
    /// only on a change.
    pub fn set_active(&mut self, cx: &mut Cx, active: bool) {
        if self.active != active {
            self.active = active;
            self.view.redraw(cx);
        }
    }

    /// Drive the disabled (dim) state, redrawing only on a change.
    pub fn set_dim(&mut self, cx: &mut Cx, dim: bool) {
        if self.dim != dim {
            self.dim = dim;
            self.view.redraw(cx);
        }
    }

    /// Whether this button emitted a primary click in `actions`.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), IconButtonAction::Clicked))
    }

    /// The press position when this button emitted a primary press in `actions`,
    /// else `None` (open a menu on the DOWN edge). Unread until the caption
    /// buttons migrate onto this widget.
    pub fn pressed(&self, actions: &Actions) -> Option<DVec2> {
        actions
            .find_widget_action(self.widget_uid())
            .and_then(|a| match a.cast() {
                IconButtonAction::Pressed(p) => Some(p),
                _ => None,
            })
    }

    /// The button's last-drawn absolute rect (a menu anchor / caption
    /// drag-query seam). Unread until the caption buttons migrate here.
    pub fn rect(&self) -> Rect {
        self.rect
    }
}

impl IconButtonRef {
    /// See [`IconButton::set_icon`].
    pub fn set_icon(&self, cx: &mut Cx, icon: Icon) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_icon(cx, icon);
        }
    }

    /// See [`IconButton::set_active`].
    pub fn set_active(&self, cx: &mut Cx, active: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_active(cx, active);
        }
    }

    /// See [`IconButton::set_dim`].
    pub fn set_dim(&self, cx: &mut Cx, dim: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_dim(cx, dim);
        }
    }

    /// See [`IconButton::clicked`].
    pub fn clicked(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.clicked(actions))
    }

    /// See [`IconButton::pressed`].
    pub fn pressed(&self, actions: &Actions) -> Option<DVec2> {
        self.borrow().and_then(|inner| inner.pressed(actions))
    }

    /// See [`IconButton::rect`].
    pub fn rect(&self) -> Rect {
        self.borrow().map(|inner| inner.rect()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::wash_and_ink;

    // The whole point of the split: an active-but-unhovered toggle reads as a
    // bare accent glyph, so "on" no longer looks identical to "hovered".
    #[test]
    fn resting_active_tints_the_glyph_without_the_wash() {
        assert_eq!(wash_and_ink(false, true, false), (false, true));
    }

    // An active button that is also hovered looks like any other hovered
    // button -- both channels lit.
    #[test]
    fn hover_lights_both_channels() {
        assert_eq!(wash_and_ink(true, false, false), (true, true));
        assert_eq!(wash_and_ink(true, true, false), (true, true));
    }

    #[test]
    fn idle_lights_neither_channel() {
        assert_eq!(wash_and_ink(false, false, false), (false, false));
    }

    // Disabled never lights, however it is hovered or flagged active.
    #[test]
    fn dim_never_lights_either_channel() {
        assert_eq!(wash_and_ink(true, true, true), (false, false));
        assert_eq!(wash_and_ink(false, true, true), (false, false));
        assert_eq!(wash_and_ink(true, false, true), (false, false));
    }
}
