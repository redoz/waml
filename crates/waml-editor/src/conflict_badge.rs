//! Toolbar conflict counter (spec §4): a red pill with a leading
//! `message-square-warning` glyph and a bare count, shown only when the
//! solver dropped constraints. Clicking it opens the error-list popup (wired in
//! `app.rs`). A `#[deref] View` with a red `draw_bg` and a `Label`; a
//! `FingerDown` on its area emits `Clicked`.

use crate::icons::{Icon, IconSet};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.ConflictBadgeBase = #(ConflictBadge::register_widget(vm))

    mod.widgets.ConflictBadge = set_type_default() do mod.widgets.ConflictBadgeBase{
        width: Fit
        height: 28.0
        flow: Right
        align: Align{x: 0.5, y: 0.5}
        padding: Inset{left: 30.0, right: 10.0}
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
        draw_icon +: { color: #FFF }
        label := Label{
            text: ""
            draw_text +: {
                color: #FFF
                text_style: fonts.text_body
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
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    #[live]
    icons: IconSet,
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
        let step = self.view.draw_walk(cx, scope, walk);
        let r = self.view.area().rect(cx);
        if r.size.x > 0.0 {
            let d = 16.0;
            let icon = Rect {
                pos: dvec2(r.pos.x + 10.0, r.pos.y + (r.size.y - d) * 0.5),
                size: dvec2(d, d),
            };
            self.icons
                .draw(cx, Icon::MessageSquareWarning, icon, self.draw_icon.color);
        }
        step
    }
}

impl ConflictBadge {
    /// Set the counter text + show/hide by count (`0` hides the pill).
    pub fn set_count(&mut self, cx: &mut Cx, n: usize) {
        self.view
            .label(cx, ids!(label))
            .set_text(cx, &format!("{n}"));
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
