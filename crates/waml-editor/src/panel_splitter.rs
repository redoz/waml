//! `PanelSplitter`: the draggable inner edge of a dock column.
//!
//! A [`SPLITTER_W`]-wide, `height: Fill` strip mounted as the LAST child of
//! `tree_host` and the FIRST child of `inspector_host` -- i.e. always on the
//! panel's inner edge, facing the center canvas. It paints a 1px theme rule at
//! that edge at all times, lerping to the accent while hovered or dragged, and
//! shows `MouseCursor::ColResize`.
//!
//! Mounted as a REAL child widget rather than a `draw_abs` overlay on purpose:
//! a child's hit rect comes from its own turtle, which sidesteps the
//! aligned-parent offset bug where a rect stored during `draw_walk` is
//! pre-alignment while events arrive post-alignment.
//!
//! Equally deliberate: the panel body beside it is given an explicit
//! runtime-computed `Size::Fixed` (host width minus [`SPLITTER_W`]) by the
//! shell, NOT `Size::Fill`. Makepad defers `Fill` sizing, so a widget drawn
//! after a `Fill` sibling caches a pre-shift rect and its hit test silently
//! misses.
//!
//! MOUNTING THIS (or any custom widget) IN `App`'s OWN DSL: registering the
//! module in `App::script_mod` is necessary but NOT sufficient. App's
//! `script_mod!` block imports widgets ONE BY ONE (`use mod.widgets.Foo`) --
//! it has no `use mod.widgets.*` glob. A widget missing from that import list
//! resolves to nothing and the mount is silently DROPPED: no draw, no hit
//! test, and `ids!()` cannot find the node. Nothing is logged once the type is
//! simply absent, and the whole test gate stays green, because the widget is
//! not there to fail. If a splitter ever goes missing again, check the `use
//! mod.widgets.PanelSplitter` line in `app.rs` before suspecting anything else.
//!
//! The widget is pure input: it knows nothing about widths, limits or docks. It
//! reports the raw pointer x in WINDOW coordinates via
//! [`PanelSplitterAction`], and `app/shell.rs` feeds that to
//! [`crate::splitter::drag`].

use crate::cursor;
use makepad_widgets::*;

/// Width of the hit strip. The 1px rule lives inside it, at the inner edge.
pub const SPLITTER_W: f64 = 6.0;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.PanelSplitterBase = #(PanelSplitter::register_widget(vm))

    mod.widgets.PanelSplitter = set_type_default() do mod.widgets.PanelSplitterBase{
        width: 6.0
        height: Fill
        show_bg: true
        // Which flank of the strip the 1px rule sits on: 0 = left (inspector),
        // 1 = right (tree). Declared here so the property EXISTS on the type --
        // a `#[live]` field only set at the mount site is rejected by the DSL
        // ("property not defined on type") and the instance silently dies.
        rule_edge: 0.0
        // A single 1px vertical rule inside an otherwise transparent strip.
        // `edge` picks which flank of the strip it sits on: 0 = left (the
        // inspector, whose splitter leads the panel), 1 = right (the tree,
        // whose splitter trails it). Pushed each `draw_walk` from `rule_edge`.
        //
        // The rule is ALWAYS drawn: it is the column's own edge, the vertical
        // twin of the horizontal chrome seam, and no panel paints that seam
        // itself (a border on the panel body would land `SPLITTER_W` inside the
        // column, not on its edge). `lit` only lerps it from `surface_border`
        // to the accent while hovered or dragged.
        //
        // `lit` and `edge` are the only values Rust pushes, so no RGBA crosses
        // the seam.
        //
        // The strip paints the PANEL's own background, not nothing. It lives
        // inside the panel host and the body beside it is sized `host -
        // SPLITTER_W`, so a transparent strip is a 6px hole in the column that
        // shows the dark window layer underneath -- which reads as a heavy
        // black bar, the opposite of an invisible resting splitter. `field_bg`
        // is exactly what `tree_panel` and the inspector fill themselves with,
        // so at rest the strip is indistinguishable from its column. Only
        // `lit` (hover/drag) reveals the 1px accent rule.
        draw_bg +: {
            color: atlas.field_bg
            accent: atlas.accent
            border: atlas.surface_border
            edge: uniform(0.0)
            lit: uniform(0.0)
            pixel: fn() {
                // NO Sdf2d here. The panel beside this strip fills itself with
                // a plain premultiplied `DrawColor` fill, and an SDF fill of
                // the very same colour does not land on the same pixels: its
                // coverage antialiases the strip's own outline, which is
                // precisely the faint box the handle must not have at rest.
                // Repeating the panel's fill verbatim makes the resting strip
                // pixel-identical to the column it sits in.
                let base = vec4(self.color.rgb * self.color.a, self.color.a)
                let px = self.pos.x * self.rect_size.x
                let x = mix(0.0, self.rect_size.x - 1.0, self.edge)
                let on = step(x, px) * step(px, x + 1.0)
                let hue = mix(self.border, self.accent, self.lit)
                // Source-over, NOT a mix onto the premultiplied rule colour:
                // `surface_border` is a translucent accent tint (alpha ~0.35),
                // so premultiplying it and lerping the opaque fill toward the
                // result just scales the fill down -- the accent rule reads as
                // a grey/black hairline. Composite it over the fill instead so
                // the seam is the same accent line the rest of the chrome uses.
                let rule = vec4(base.rgb * (1.0 - hue.a) + hue.rgb * hue.a, 1.0)
                return mix(base, rule, on)
            }
        }
    }
}

/// Splitter input, read by the shell. Both carry the raw pointer x in WINDOW
/// coordinates -- the splitter deliberately does no width arithmetic itself.
#[derive(Clone, Debug, Default)]
pub enum PanelSplitterAction {
    #[default]
    None,
    /// A drag frame (the initial press, and every move while captured).
    Dragged(f64),
    /// The drag ended. The shell persists on this, never per frame.
    Released,
}

#[derive(Script, ScriptHook, Widget)]
pub struct PanelSplitter {
    /// The hit strip; its `draw_bg` paints the rule.
    #[deref]
    view: View,

    /// Which flank of the strip the rule sits on: 0.0 = left, 1.0 = right.
    /// Set per instance in the App DSL.
    #[live]
    rule_edge: f64,

    /// Pointer-over state, self-managed from FingerHoverIn/Out.
    #[rust]
    hovered: bool,
    /// True between FingerDown and FingerUp, so the rule stays lit while the
    /// pointer travels outside the 6px strip mid-drag.
    #[rust]
    dragging: bool,
}

/// How lit the rule is: fully accent while hovered or dragging, fully the
/// resting rule colour otherwise. Pure so the rule is unit-testable without a
/// `Cx`.
fn rule_lit(hovered: bool, dragging: bool) -> f64 {
    if hovered || dragging {
        1.0
    } else {
        0.0
    }
}

impl Widget for PanelSplitter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.dragging = true;
                cursor::hover_in(cx, MouseCursor::ColResize);
                cx.widget_action(uid, PanelSplitterAction::Dragged(fe.abs.x));
                self.view.redraw(cx);
            }
            Hit::FingerMove(fe) => {
                if self.dragging {
                    cx.widget_action(uid, PanelSplitterAction::Dragged(fe.abs.x));
                }
            }
            Hit::FingerUp(_) => {
                if self.dragging {
                    self.dragging = false;
                    // A drag that ended away from the strip gets no HoverOut,
                    // so release the cursor here too.
                    cursor::drag_end(cx, self.hovered, MouseCursor::ColResize);
                    cx.widget_action(uid, PanelSplitterAction::Released);
                    self.view.redraw(cx);
                }
            }
            Hit::FingerHoverIn(_) => {
                cursor::hover_in(cx, MouseCursor::ColResize);
                self.hovered = true;
                self.view.redraw(cx);
            }
            Hit::FingerHoverOut(_) => {
                // Hand the cursor back explicitly. Makepad keeps the last
                // cursor any widget set until another one sets its own, and the
                // empty area below the tree's last row belongs to no widget --
                // so leaving the strip downward kept ColResize showing until
                // the pointer happened to cross a tree row.
                if !self.dragging {
                    cursor::hover_out(cx);
                }
                self.hovered = false;
                self.view.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let lit = rule_lit(self.hovered, self.dragging);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(lit), &[lit as f32]);
        self.view
            .draw_bg
            .set_uniform(cx, live_id!(edge), &[self.rule_edge as f32]);
        self.view.draw_walk(cx, scope, walk)
    }
}

impl PanelSplitter {
    /// The pointer x (window coordinates) of the newest drag frame this
    /// splitter emitted in `actions`, else `None`.
    pub fn dragged(&self, actions: &Actions) -> Option<f64> {
        actions
            .find_widget_action(self.widget_uid())
            .and_then(|a| match a.cast() {
                PanelSplitterAction::Dragged(x) => Some(x),
                _ => None,
            })
    }

    /// Whether this splitter's drag ended in `actions`.
    pub fn released(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), PanelSplitterAction::Released))
    }
}

impl PanelSplitterRef {
    /// See [`PanelSplitter::dragged`].
    pub fn dragged(&self, actions: &Actions) -> Option<f64> {
        self.borrow().and_then(|inner| inner.dragged(actions))
    }

    /// See [`PanelSplitter::released`].
    pub fn released(&self, actions: &Actions) -> bool {
        self.borrow().is_some_and(|inner| inner.released(actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_is_lit_by_hover_or_drag_only() {
        assert_eq!(rule_lit(false, false), 0.0);
        assert_eq!(rule_lit(true, false), 1.0);
        // A drag that leaves the 6px strip keeps the rule lit.
        assert_eq!(rule_lit(false, true), 1.0);
        assert_eq!(rule_lit(true, true), 1.0);
    }

    #[test]
    fn splitter_strip_is_six_pixels_wide() {
        assert_eq!(SPLITTER_W, 6.0);
    }
}
