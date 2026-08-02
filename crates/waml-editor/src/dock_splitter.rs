//! `DockSplitter`: the draggable inner edge of a dock column.
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
//! The widget is pure input: it knows nothing about widths, limits or docks. It
//! reports the raw pointer x in WINDOW coordinates via
//! [`DockSplitterAction`], and `app/shell.rs` feeds that to
//! [`crate::splitter::drag`].

use makepad_widgets::*;

/// Width of the hit strip. The 1px rule lives inside it, at the inner edge.
pub const SPLITTER_W: f64 = 6.0;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.DockSplitterBase = #(DockSplitter::register_widget(vm))

    mod.widgets.DockSplitter = set_type_default() do mod.widgets.DockSplitterBase{
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
        // `sdf.rect` rather than `sdf.box(..., 0.0)`: a zero-radius box
        // degenerates and floods the strip.
        // `lit` lerps the rule from its resting theme colour to the accent; it
        // is the only value Rust pushes, so no RGBA crosses the seam.
        draw_bg +: {
            color: atlas.surface_border
            accent: atlas.accent
            edge: uniform(0.0)
            lit: uniform(0.0)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let x = mix(0.0, self.rect_size.x - 1.0, self.edge)
                sdf.rect(x, 0.0, 1.0, self.rect_size.y)
                sdf.fill(mix(self.color, self.accent, self.lit))
                return sdf.result
            }
        }
    }
}

/// Splitter input, read by the shell. Both carry the raw pointer x in WINDOW
/// coordinates -- the splitter deliberately does no width arithmetic itself.
#[derive(Clone, Debug, Default)]
pub enum DockSplitterAction {
    #[default]
    None,
    /// A drag frame (the initial press, and every move while captured).
    Dragged(f64),
    /// The drag ended. The shell persists on this, never per frame.
    Released,
}

#[derive(Script, ScriptHook, Widget)]
pub struct DockSplitter {
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

impl Widget for DockSplitter {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.dragging = true;
                cx.set_cursor(MouseCursor::ColResize);
                cx.widget_action(uid, DockSplitterAction::Dragged(fe.abs.x));
                self.view.redraw(cx);
            }
            Hit::FingerMove(fe) => {
                if self.dragging {
                    cx.widget_action(uid, DockSplitterAction::Dragged(fe.abs.x));
                }
            }
            Hit::FingerUp(_) => {
                if self.dragging {
                    self.dragging = false;
                    cx.widget_action(uid, DockSplitterAction::Released);
                    self.view.redraw(cx);
                }
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::ColResize);
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

impl DockSplitter {
    /// The pointer x (window coordinates) of the newest drag frame this
    /// splitter emitted in `actions`, else `None`.
    pub fn dragged(&self, actions: &Actions) -> Option<f64> {
        actions
            .find_widget_action(self.widget_uid())
            .and_then(|a| match a.cast() {
                DockSplitterAction::Dragged(x) => Some(x),
                _ => None,
            })
    }

    /// Whether this splitter's drag ended in `actions`.
    pub fn released(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .is_some_and(|a| matches!(a.cast(), DockSplitterAction::Released))
    }
}

impl DockSplitterRef {
    /// See [`DockSplitter::dragged`].
    pub fn dragged(&self, actions: &Actions) -> Option<f64> {
        self.borrow().and_then(|inner| inner.dragged(actions))
    }

    /// See [`DockSplitter::released`].
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
