//! Toolbar segmented control for constraint-veil visibility (spec §1): a
//! three-cell None / Selected / All picker. Modeled on `ToolDock` — a
//! `#[deref] View` laying out three shared `IconButton` children; `draw_walk`
//! syncs each child's glyph + lit state from `active`, `handle_event` reads each
//! child's `clicked` and emits the picked `ConstraintVisibility`.

use makepad_widgets::*;

use crate::canvas::ConstraintVisibility;
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ConstraintToggleBase = #(ConstraintToggle::register_widget(vm))

    mod.widgets.ConstraintToggle = set_type_default() do mod.widgets.ConstraintToggleBase{
        width: 110.0
        height: 36.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 4.0, right: 4.0, top: 2.0, bottom: 2.0}
        spacing: 2.0
        show_bg: true
        // Same Atlas HUD frame as ToolDock.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }

        none_btn := IconButton {}
        selected_btn := IconButton {}
        all_btn := IconButton {}
    }
}

#[derive(Clone, Debug, Default)]
pub enum ConstraintToggleAction {
    #[default]
    None,
    /// A mode cell was clicked; carries the picked visibility.
    Picked(ConstraintVisibility),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ConstraintToggle {
    #[deref]
    view: View,
    #[rust]
    active: ConstraintVisibility,
}

impl Widget for ConstraintToggle {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        if let Event::Actions(actions) = event {
            for mode in ConstraintVisibility::ALL {
                if self.button(cx, mode).as_icon_button().clicked(actions) {
                    self.active = mode;
                    self.view.redraw(cx);
                    cx.widget_action(uid, ConstraintToggleAction::Picked(mode));
                    break;
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        for mode in ConstraintVisibility::ALL {
            let btn = self.button(cx, mode).as_icon_button();
            btn.set_icon(cx, Self::icon_for(mode));
            btn.set_active(cx, mode == self.active);
        }
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        DrawStep::done()
    }
}

impl ConstraintToggle {
    fn button(&mut self, cx: &mut Cx, mode: ConstraintVisibility) -> WidgetRef {
        match mode {
            ConstraintVisibility::None => self.view.widget(cx, ids!(none_btn)),
            ConstraintVisibility::Selected => self.view.widget(cx, ids!(selected_btn)),
            ConstraintVisibility::All => self.view.widget(cx, ids!(all_btn)),
        }
    }

    /// Catalog glyph per mode. None = eye-off, Selected = eye, All = bounding box.
    fn icon_for(mode: ConstraintVisibility) -> Icon {
        match mode {
            ConstraintVisibility::None => Icon::EyeOff,
            ConstraintVisibility::Selected => Icon::Eye,
            ConstraintVisibility::All => Icon::VectorSquare,
        }
    }

    /// Set the active mode directly (App-driven), bypassing the click round-trip.
    /// Unconsumed today (no hotkey drives the toggle yet); kept for parity with
    /// `ToolDock::set_active` and future callers (a visibility hotkey is a
    /// deferred follow-up per the plan's "Pending post-land" section).
    #[allow(dead_code)]
    pub fn set_active(&mut self, cx: &mut Cx, mode: ConstraintVisibility) {
        self.active = mode;
        self.view.redraw(cx);
    }

    /// Reader for `App`: the picked visibility this frame, if any.
    pub fn toggle_action(&self, actions: &Actions) -> Option<ConstraintVisibility> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ConstraintToggleAction::Picked(mode) => Some(mode),
            ConstraintToggleAction::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_active_is_selected() {
        assert_eq!(
            ConstraintVisibility::default(),
            ConstraintVisibility::Selected
        );
    }

    #[test]
    fn each_mode_maps_to_a_catalog_icon() {
        assert_eq!(
            ConstraintToggle::icon_for(ConstraintVisibility::None),
            Icon::EyeOff
        );
        assert_eq!(
            ConstraintToggle::icon_for(ConstraintVisibility::Selected),
            Icon::Eye
        );
        assert_eq!(
            ConstraintToggle::icon_for(ConstraintVisibility::All),
            Icon::VectorSquare
        );
    }
}
