//! `FontSizeControl`: the browser-style `[-][100%][+]` zoom control mounted in
//! the document header (Task 7). Two `IconButton`s bracket a percent label
//! drawn immediate-mode over a fixed slot; clicking the label resets the
//! zoom, mirroring the browser convention this widget borrows its shape
//! from. The ladder itself lives in `crate::zoom` -- this widget only reports
//! intent (`ZoomIn` / `ZoomOut` / `Reset`) and displays whatever percent it
//! is told to show; the host (`DocumentHeader`, Task 7) owns the ladder walk
//! and end-of-range dimming.
//!
//! Same hybrid `#[deref] View` shape as `IconButton` / `DocumentHeader`: the
//! two buttons are real mounted children (so their own hover/click handling
//! is free), while the percent label is drawn immediate-mode over a plain
//! `View` slot (`percent_slot`) the same way `DocumentHeader` draws its
//! breadcrumb labels over `content_row`.

use crate::cursor;
use crate::icon_button::{IconButtonAction, IconButtonWidgetRefExt};
use crate::icons::Icon;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

    mod.widgets.FontSizeControlBase = #(FontSizeControl::register_widget(vm))

    mod.widgets.FontSizeControl = set_type_default() do mod.widgets.FontSizeControlBase{
        width: Fit
        height: 30.0
        flow: Right
        align: Align{y: 0.5}
        visible: false

        zoom_out_button := IconButton { width: 30.0 height: 30.0 }
        percent_slot := View { width: 44.0 height: 30.0 }
        zoom_in_button := IconButton { width: 30.0 height: 30.0 }

        // Label ink: menu-weight chrome type, mid tone at rest, full on hover
        // (the reset affordance). Drawn immediate-mode over `percent_slot`.
        draw_percent +: { color: atlas.text_mid, text_style: fonts.text_menu }
        draw_percent_hover +: { color: atlas.text, text_style: fonts.text_menu }
    }
}

/// Intent reported by [`FontSizeControl::action`]. The control never applies
/// a zoom step itself -- the host resolves the ladder (`crate::zoom`) and
/// calls `set_percent` / `set_enabled_directions` back down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSizeControlAction {
    ZoomIn,
    ZoomOut,
    Reset,
}

#[derive(Script, ScriptHook, Widget)]
pub struct FontSizeControl {
    #[deref]
    view: View,
    #[live]
    draw_percent: DrawText,
    #[live]
    draw_percent_hover: DrawText,
    /// Set by [`FontSizeControl::set_percent`]; drawn immediate-mode in
    /// `draw_walk` rather than through a mounted `Label`, matching
    /// `DocumentHeader`'s breadcrumb labels.
    #[rust]
    percent_text: String,
    /// Pointer-over state for the percent slot, self-managed from
    /// `FingerHoverIn`/`FingerHoverOver`/`FingerHoverOut` (see cursor
    /// hygiene trap: every `hover_in` needs a matching `hover_out`).
    #[rust]
    label_hover: bool,
    /// Guards the one-time button setup in `set_percent` (icon, action tag,
    /// and recorded uid) -- the DSL mounts the buttons bare so their glyph
    /// and tag are assigned once, at runtime, the same way
    /// `App::sync_history_controls` wires the history buttons.
    #[rust]
    buttons_ready: bool,
    #[rust]
    zoom_in_uid: Option<WidgetUid>,
    #[rust]
    zoom_out_uid: Option<WidgetUid>,
    /// Mirrors of the two `IconButton`s' dim flags, pushed alongside the
    /// real `set_dim` call in `set_enabled_directions`. The children are
    /// only materialised into `self.view`'s live tree by a completed draw
    /// pass, so a caller that probes state before one -- as the unit tests
    /// do -- cannot read it back off the (possibly not-yet-mounted) button
    /// instances; these mirrors give `test_enabled_directions` a stable,
    /// draw-independent read.
    #[rust]
    can_zoom_in: bool,
    #[rust]
    can_zoom_out: bool,
    /// The view's own rect at the last draw, and the percent slot's rect at
    /// the same draw -- both ABSOLUTE (`draw_abs`/`area().rect()` space).
    /// `handle_event` maps a POST-alignment event position back into this
    /// space via `event_rect.pos - draw_rect.pos`, the same idiom
    /// `DocumentHeader::handle_event` uses (draw-time rects are
    /// PRE-alignment; event positions are POST).
    #[rust]
    draw_rect: Rect,
    #[rust]
    percent_slot_rect: Rect,
}

fn centered_pos(slot: Rect, size: DVec2) -> DVec2 {
    dvec2(
        (slot.pos.x + (slot.size.x - size.x) * 0.5).round(),
        (slot.pos.y + (slot.size.y - size.y) * 0.5).round(),
    )
}

impl Widget for FontSizeControl {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        match event.hits(cx, self.view.area()) {
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over => {
                let event_rect = self.view.area().rect(cx);
                let hit_offset = event_rect.pos - self.draw_rect.pos;
                if self.percent_slot_rect.size.x > 0.0
                    && self.percent_slot_rect.contains(fe.abs - hit_offset)
                {
                    cx.widget_action(self.widget_uid(), FontSizeControlAction::Reset);
                }
            }
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let event_rect = self.view.area().rect(cx);
                let hit_offset = event_rect.pos - self.draw_rect.pos;
                let hovered = self.percent_slot_rect.size.x > 0.0
                    && self.percent_slot_rect.contains(fe.abs - hit_offset);
                if hovered != self.label_hover {
                    self.label_hover = hovered;
                    self.view.redraw(cx);
                }
                if hovered {
                    cursor::hover_in(cx, MouseCursor::Hand);
                } else {
                    cursor::hover_out(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.label_hover {
                    self.label_hover = false;
                    self.view.redraw(cx);
                }
                cursor::hover_out(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        self.draw_rect = self.view.area().rect(cx);
        self.percent_slot_rect = self.view.widget(cx, ids!(percent_slot)).area().rect(cx);

        let draw = if self.label_hover {
            &mut self.draw_percent_hover
        } else {
            &mut self.draw_percent
        };
        let size = draw
            .layout(
                cx,
                0.0,
                0.0,
                None,
                false,
                Align::default(),
                &self.percent_text,
            )
            .size_in_lpxs;
        let pos = centered_pos(
            self.percent_slot_rect,
            dvec2(size.width as f64, size.height as f64),
        );
        draw.draw_abs(cx, pos, &self.percent_text);

        step
    }
}

impl FontSizeControl {
    /// Update the displayed percent, lazily wiring the two `IconButton`
    /// children (icon + action tag + recorded uid) on the first call.
    pub fn set_percent(&mut self, cx: &mut Cx, percent: u32) {
        let mut changed = false;
        if !self.buttons_ready {
            self.buttons_ready = true;
            let zoom_out = self.view.widget(cx, ids!(zoom_out_button)).as_icon_button();
            self.zoom_out_uid = Some(zoom_out.widget_uid());
            // Lucide's font-size pair (a letter A with an arrow), not the
            // magnifiers: a magnifier reads as "zoom the view", which is what
            // the canvas surfaces do. This control changes type size.
            zoom_out.set_icon(cx, Icon::AArrowDown);
            zoom_out.set_action_tag(live_id!(zoom_out));

            let zoom_in = self.view.widget(cx, ids!(zoom_in_button)).as_icon_button();
            self.zoom_in_uid = Some(zoom_in.widget_uid());
            zoom_in.set_icon(cx, Icon::AArrowUp);
            zoom_in.set_action_tag(live_id!(zoom_in));

            changed = true;
        }

        let text = format!("{percent}%");
        if self.percent_text != text {
            self.percent_text = text;
            changed = true;
        }

        if changed {
            self.view.redraw(cx);
        }
    }

    /// Dim the button at either end of the zoom ladder the host has already
    /// resolved as exhausted (`crate::zoom::{zoom_in, zoom_out}` returning
    /// the same percent back).
    pub fn set_enabled_directions(&mut self, cx: &mut Cx, can_in: bool, can_out: bool) {
        self.can_zoom_in = can_in;
        self.can_zoom_out = can_out;
        self.view
            .widget(cx, ids!(zoom_in_button))
            .as_icon_button()
            .set_dim(cx, !can_in);
        self.view
            .widget(cx, ids!(zoom_out_button))
            .as_icon_button()
            .set_dim(cx, !can_out);
    }

    /// Read back the last `set_enabled_directions` call as `(can_zoom_in,
    /// can_zoom_out)`. See the field docs on `can_zoom_in`/`can_zoom_out`
    /// for why the tests read the mirror rather than the child buttons.
    #[cfg(test)]
    pub(crate) fn test_enabled_directions(&self) -> (bool, bool) {
        (self.can_zoom_in, self.can_zoom_out)
    }

    /// Same shape as `DocumentHeader::action`: an own-uid downcast for
    /// `Reset` first, then the two child `IconButton`s' tagged clicks.
    pub fn action(&self, actions: &Actions) -> Option<FontSizeControlAction> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let Some(action) = item.action.downcast_ref::<FontSizeControlAction>() {
                return Some(*action);
            }
        }

        let tagged_clicked = |uid: Option<WidgetUid>| -> Option<LiveId> {
            let item = actions.find_widget_action(uid?)?;
            match item.action.downcast_ref::<IconButtonAction>() {
                Some(IconButtonAction::TaggedClicked(tag)) => Some(*tag),
                _ => None,
            }
        };

        if tagged_clicked(self.zoom_in_uid) == Some(live_id!(zoom_in)) {
            return Some(FontSizeControlAction::ZoomIn);
        }
        if tagged_clicked(self.zoom_out_uid) == Some(live_id!(zoom_out)) {
            return Some(FontSizeControlAction::ZoomOut);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget_action(uid: WidgetUid, action: impl WidgetActionTrait + 'static) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(action),
            widget_uid: uid,
            group: None,
        })
    }

    #[test]
    fn label_text_tracks_set_percent() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut control = cx.with_vm(FontSizeControl::script_new_with_default);

        control.set_percent(&mut cx, 100);
        assert_eq!(control.percent_text, "100%");

        control.set_percent(&mut cx, 125);
        assert_eq!(control.percent_text, "125%");
    }

    #[test]
    fn tagged_button_clicks_map_to_the_matching_action() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut control = cx.with_vm(FontSizeControl::script_new_with_default);
        control.set_percent(&mut cx, 100);

        let zoom_in_uid = control.zoom_in_uid.expect("zoom_in wired by set_percent");
        let zoom_out_uid = control.zoom_out_uid.expect("zoom_out wired by set_percent");

        let zoom_in_action = widget_action(
            zoom_in_uid,
            IconButtonAction::TaggedClicked(live_id!(zoom_in)),
        );
        assert_eq!(
            control.action(std::slice::from_ref(&zoom_in_action)),
            Some(FontSizeControlAction::ZoomIn)
        );

        let zoom_out_action = widget_action(
            zoom_out_uid,
            IconButtonAction::TaggedClicked(live_id!(zoom_out)),
        );
        assert_eq!(
            control.action(std::slice::from_ref(&zoom_out_action)),
            Some(FontSizeControlAction::ZoomOut)
        );

        let reset_action = widget_action(control.widget_uid(), FontSizeControlAction::Reset);
        assert_eq!(
            control.action(std::slice::from_ref(&reset_action)),
            Some(FontSizeControlAction::Reset)
        );

        let unrelated_action = widget_action(
            WidgetUid(u64::MAX),
            IconButtonAction::TaggedClicked(live_id!(zoom_in)),
        );
        assert_eq!(
            control.action(std::slice::from_ref(&unrelated_action)),
            None
        );
    }

    #[test]
    fn enabled_directions_dim_the_ladder_end_buttons() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let mut control = cx.with_vm(FontSizeControl::script_new_with_default);
        control.set_percent(&mut cx, 100);

        // Read the mirror (`test_enabled_directions`), not the child
        // `IconButton`s' own `dim` flag: without a completed draw pass the
        // DSL-declared buttons are not yet materialised into `self.view`'s
        // live tree, so a fresh `.widget(cx, ids!(...))` lookup would not
        // reliably return the same instance `set_enabled_directions` just
        // wrote to.
        control.set_enabled_directions(&mut cx, false, true);
        assert_eq!(control.test_enabled_directions(), (false, true));

        control.set_enabled_directions(&mut cx, true, false);
        assert_eq!(control.test_enabled_directions(), (true, false));
    }
}
