//! Toolbar conflict counter (spec §4): a red `! N` pill, shown only when the
//! solver dropped constraints. Clicking it opens the error-list popup (wired in
//! `app.rs`). A `#[deref] View` with a red `draw_bg` and a `Label`; a
//! `FingerDown` on its area emits `Clicked`.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.ConflictBadgeBase = #(ConflictBadge::register_widget(vm))

    mod.widgets.ConflictBadge = set_type_default() do mod.widgets.ConflictBadgeBase{
        width: Fit
        height: 28.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 10.0, right: 10.0}
        show_bg: true
        draw_bg +: {
            color: vec4(0.80, 0.22, 0.22, 0.95)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 6.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        label := Label{
            text: ""
            draw_text +: {
                color: #FFF
                text_style: TextStyle{
                    font_size: 12
                    font_family: FontFamily{
                        latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBold.ttf") asc: -0.1 desc: 0.0}
                    }
                    line_spacing: 1.2
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ConflictBadgeAction {
    #[default]
    None,
    Clicked,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ConflictBadge {
    #[deref]
    view: View,
}

impl Widget for ConflictBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        match event.hits(cx, self.view.area()) {
            Hit::FingerDown(_) => cx.widget_action(uid, ConflictBadgeAction::Clicked),
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl ConflictBadge {
    /// Set the counter text + show/hide by count (`0` hides the pill).
    pub fn set_count(&mut self, cx: &mut Cx, n: usize) {
        self.view
            .label(cx, ids!(label))
            .set_text(cx, &format!("! {n}"));
        self.view.set_visible(cx, n > 0);
        self.view.redraw(cx);
    }

    /// Reader for `App`: whether the badge was clicked this frame.
    pub fn clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .map(|a| matches!(a.cast(), ConflictBadgeAction::Clicked))
            .unwrap_or(false)
    }
}
