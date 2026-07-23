//! `SelectBox` — the reusable closed combo control. Renders the box you see when
//! nothing is open (Atlas `AccentFrame{field_bg}` + the selected row's lead +
//! label + a trailing caret). It CANNOT open the list itself (popup authority
//! lives in `PopupRoot`): a click emits `SelectBoxAction::OpenRequested`, `App`
//! relays it to `PopupRoot::show_at(PopupSpec::Select{…})`, and the close comes
//! back through the tag-filtered queue into `on_closed`. See
//! `docs/superpowers/specs/2026-07-22-select-box-flyout-design.md`.
#![allow(dead_code)]

use crate::icons::{Icon, IconSet};
use crate::popup::base::PopupResult;
use crate::popup::select::{SelectItem, SelectLead};
use makepad_widgets::*;

/// Emitted by the box. `App` reads `open_request` and relays to `PopupRoot`.
#[derive(Clone, Debug, Default)]
pub enum SelectBoxAction {
    #[default]
    None,
    OpenRequested {
        anchor: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SelectBoxBase = #(SelectBox::register_widget(vm))

    mod.widgets.SelectBox = set_type_default() do mod.widgets.SelectBoxBase{
        width: Fill
        height: 32.0
        // Active overlay RING drawn over the box while the list is open — a
        // source-bright accent border, the visual link to the open flyout.
        // Stroke-ONLY (no fill): a second `AccentFrame` would re-run
        // `sdf.fill_keep(self.color)` (see `frame.rs:50`) and re-paint the
        // interior, blanking the badge/label/caret drawn underneath (finding
        // H1). This variant strokes the accent edge and leaves the interior
        // untouched, so the open box keeps its content and gains an accent ring.
        draw_active: mod.draw.DrawColor{
            color: atlas.accent
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.stroke(self.color, inset)
                return sdf.result
            }
        }
        draw_badge: mod.draw.DrawColor{ color: atlas.bucket_slate }
        draw_badge_text +: {
            color: #xffffff
            text_style: theme.font_regular{ font_size: 10 }
        }
        draw_icon_idle +: { color: atlas.text }
        draw_caret +: { color: atlas.text_dim }
        // Web-header style: the selected subject's name reads as a bold title,
        // not a small combo-field label (mirrors the web inspector header).
        draw_label +: {
            color: atlas.text
            text_style: theme.font_bold{ font_size: 14 line_spacing: 1.2 }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SelectBox {
    #[deref]
    view: View,

    #[redraw]
    #[live]
    draw_active: DrawColor,
    #[redraw]
    #[live]
    draw_badge: DrawColor,
    #[redraw]
    #[live]
    draw_badge_text: DrawText,
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_caret: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[live]
    icons: IconSet,

    #[rust]
    items: Vec<SelectItem>,
    #[rust]
    selected: Option<usize>,
    #[rust]
    open: bool,
}

impl Widget for SelectBox {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let uid = self.widget_uid();
        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if fe.is_primary_hit() {
                // A hit on `view.area()` already means the press landed on the
                // box — NO `box_rect.contains` guard (that rect is pre-alignment;
                // the panel is right-aligned → event abs never matches → dead
                // click). See finding H2.
                self.open = true;
                self.view.redraw(cx);
                // Anchor from the EVENT-TIME area rect (post-alignment), never a
                // draw-captured rect (pre-alignment, x≈0 → mis-anchored far-left).
                let anchor = self.view.area().rect(cx);
                let min_width = anchor.size.x;
                let items = self.items.clone();
                cx.widget_action(
                    uid,
                    SelectBoxAction::OpenRequested {
                        anchor,
                        min_width,
                        items,
                    },
                );
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}
        let rect = self.view.area().rect(cx);
        let cy = rect.pos.y + rect.size.y * 0.5;

        // Flat web-header look: no boxed field frame -- the leading kind icon,
        // bold name, and trailing caret carry the affordance over the bare panel.
        // (The open-state accent ring below still draws when the list is open.)

        // Selected row's lead + label (or nothing selected → placeholder blank).
        let idle = self.draw_icon_idle.color;
        let mut label_x = rect.pos.x + 12.0;
        if let Some(sel) = self.selected.and_then(|i| self.items.get(i)).cloned() {
            match &sel.lead {
                SelectLead::None => {}
                SelectLead::Icon(icon) => {
                    let r = Rect {
                        pos: dvec2(rect.pos.x + 8.0, cy - 9.0),
                        size: dvec2(18.0, 18.0),
                    };
                    self.icons.draw(cx, *icon, r, idle);
                    label_x = rect.pos.x + 34.0;
                }
                SelectLead::Badge { color, letter } => {
                    let b = Rect {
                        pos: dvec2(rect.pos.x + 8.0, cy - 10.0),
                        size: dvec2(20.0, 20.0),
                    };
                    self.draw_badge.color = *color;
                    self.draw_badge.draw_abs(cx, b);
                    if !letter.is_empty() {
                        self.draw_badge_text.draw_abs(
                            cx,
                            dvec2(b.pos.x + 6.0, b.pos.y + 3.0),
                            letter,
                        );
                    }
                    label_x = rect.pos.x + 36.0;
                }
            }
            self.draw_label
                .draw_abs(cx, dvec2(label_x, cy - 8.0), &sel.label);
        }

        // Trailing caret (chevrons-up-down = the standard combo affordance).
        let caret = Rect {
            pos: dvec2(rect.pos.x + rect.size.x - 24.0, cy - 8.0),
            size: dvec2(16.0, 16.0),
        };
        let ct = self.draw_caret.color;
        self.icons.draw(cx, Icon::ChevronsUpDown, caret, ct);

        // Active accent RING over the box while the list is open — drawn LAST so
        // it sits atop the content. Stroke-only (no re-fill), so the badge/label/
        // caret stay visible; a full AccentFrame here would blank them (H1).
        if self.open {
            self.draw_active.draw_abs(cx, rect);
        }

        DrawStep::done()
    }
}

/// Pure `on_closed` decision over prior selection + result: returns the new
/// `(open, selected)` and the committed id. `find_index` maps an invoked id to a
/// row index (the widget passes a closure over its own `items`).
fn decide_closed(
    result: &PopupResult,
    prior_selected: Option<usize>,
    find_index: impl Fn(LiveId) -> Option<usize>,
) -> (bool, Option<usize>, Option<LiveId>) {
    match result {
        PopupResult::Invoked(id) => {
            let sel = find_index(*id).or(prior_selected);
            (false, sel, Some(*id))
        }
        PopupResult::Dismissed => (false, prior_selected, None),
    }
}

impl SelectBox {
    pub fn set_items(&mut self, cx: &mut Cx, items: Vec<SelectItem>) {
        self.items = items;
        self.view.redraw(cx);
    }

    pub fn set_selected(&mut self, cx: &mut Cx, selected: Option<usize>) {
        self.selected = selected;
        self.view.redraw(cx);
    }

    /// `App` reads this to relay the open. `None` unless the box asked to open.
    pub fn open_request(&self, actions: &Actions) -> Option<(Rect, f64, Vec<SelectItem>)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let SelectBoxAction::OpenRequested {
            anchor,
            min_width,
            items,
        } = item.cast()
        {
            Some((anchor, min_width, items))
        } else {
            None
        }
    }

    /// The list closed. Always clears `open`; on `Invoked(id)` updates
    /// `selected` to that row and returns the id (else `None`).
    pub fn on_closed(&mut self, cx: &mut Cx, result: PopupResult) -> Option<LiveId> {
        let idx_of = |id: LiveId| self.items.iter().position(|it| it.id == id);
        let (open, selected, picked) = decide_closed(&result, self.selected, idx_of);
        self.open = open;
        self.selected = selected;
        self.view.redraw(cx);
        picked
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismiss_clears_open_and_keeps_selection() {
        let (open, sel, picked) = decide_closed(&PopupResult::Dismissed, Some(2), |_| None);
        assert!(!open);
        assert_eq!(sel, Some(2));
        assert_eq!(picked, None);
    }

    #[test]
    fn invoke_clears_open_and_updates_selection() {
        let (open, sel, picked) =
            decide_closed(&PopupResult::Invoked(live_id!(row_b)), Some(0), |id| {
                if id == live_id!(row_b) {
                    Some(3)
                } else {
                    None
                }
            });
        assert!(!open);
        assert_eq!(sel, Some(3));
        assert_eq!(picked, Some(live_id!(row_b)));
    }

    #[test]
    fn invoke_of_unknown_id_keeps_prior_selection() {
        let (open, sel, picked) =
            decide_closed(&PopupResult::Invoked(live_id!(ghost)), Some(1), |_| None);
        assert!(!open);
        assert_eq!(sel, Some(1));
        assert_eq!(picked, Some(live_id!(ghost)));
    }
}
