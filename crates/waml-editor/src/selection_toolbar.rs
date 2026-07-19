//! Bottom selection toolbar (UX mock): a centered pill shown whenever a
//! classifier is focused (mirrors the doc-tab/inspector subject), with a
//! selection-count label and two actions. `Delete` is wired for real
//! (closes the focused classifier's doc tab -- in-memory only, the `Model`
//! is never touched); `New Diagram` is a mock no-op (creating a diagram is
//! out of scope for this pass). Hand-rolled immediate-mode widget, same
//! `draw_abs`/rect-hit-test convention as `doc_tabs.rs`/`tool_dock.rs`.
//! Reserves a fixed-height bottom strip (rather than a true floating
//! overlay) and simply draws nothing but background when hidden -- lower
//! risk than overlay compositing for a mock.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SelectionToolbarBase = #(SelectionToolbar::register_widget(vm))

    mod.widgets.SelectionToolbar = set_type_default() do mod.widgets.SelectionToolbarBase{
        width: Fill
        height: 44.0
        draw_bg: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_pill +: { color: atlas.surface }
        draw_label +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_action +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

/// "1 selected" / "N selected" -- pure so it's unit-tested without a `Cx`.
pub fn label_for_count(count: usize) -> String {
    if count == 1 {
        "1 selected".to_string()
    } else {
        format!("{count} selected")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    NewDiagram,
    Delete,
}

#[derive(Clone, Debug, Default)]
pub enum SelectionToolbarAction {
    #[default]
    None,
    NewDiagram,
    Delete,
}

const PILL_H: f64 = 32.0;
const PILL_PAD: f64 = 16.0;
const LABEL_W: f64 = 80.0;
const NEW_DIAGRAM_W: f64 = 116.0;
const DELETE_W: f64 = 56.0;
const GAP: f64 = 16.0;

#[derive(Script, ScriptHook, Widget)]
pub struct SelectionToolbar {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[redraw]
    #[live]
    draw_pill: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    #[redraw]
    #[live]
    draw_action: DrawText,

    /// `None` hides the toolbar entirely (no classifier focused).
    #[rust]
    count: Option<usize>,
    #[rust]
    item_rects: Vec<(Action, Rect)>,
}

impl Widget for SelectionToolbar {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                // `item_rects` holds positions *relative* to the container's
                // draw-time rect, because `cx.walk_turtle`'s returned rect is
                // stale (wrong `pos.y`) for a Fixed-height widget drawn after
                // a Fill sibling in the same `flow: Down` container -- the
                // turtle cursor hasn't yet accounted for the Fill sibling's
                // resolved height when this widget is walked. The *actual*
                // drawn position is only available after the fact via the
                // background area's own rect, which is correct because it's
                // resolved post-layout.
                let bg_rect = self.draw_bg.area().rect(cx);
                for (action, rel_rect) in self.item_rects.clone() {
                    let rect = Rect {
                        pos: bg_rect.pos + rel_rect.pos,
                        size: rel_rect.size,
                    };
                    if rect.contains(fe.abs) {
                        let action = match action {
                            Action::NewDiagram => SelectionToolbarAction::NewDiagram,
                            Action::Delete => SelectionToolbarAction::Delete,
                        };
                        cx.widget_action(uid, action);
                        break;
                    }
                }
            }
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        self.item_rects.clear();

        let Some(count) = self.count else {
            return DrawStep::done();
        };

        // All positions below are computed relative to `rect.pos` (itself
        // treated as the local origin) so `item_rects` can be re-anchored to
        // the widget's *actual* drawn position at hit-test time -- see the
        // comment in `handle_event`.
        let pill_w = PILL_PAD * 2.0 + LABEL_W + GAP + NEW_DIAGRAM_W + GAP + DELETE_W;
        let pill_x = (rect.size.x - pill_w) * 0.5;
        let pill_y = (rect.size.y - PILL_H) * 0.5;
        let pill_rect = Rect {
            pos: rect.pos + dvec2(pill_x, pill_y),
            size: dvec2(pill_w, PILL_H),
        };
        self.draw_pill.draw_abs(cx, pill_rect);

        let text_y = pill_y + PILL_H * 0.5 - 7.0;
        let mut x = pill_x + PILL_PAD;
        self.draw_label
            .draw_abs(cx, rect.pos + dvec2(x, text_y), &label_for_count(count));
        x += LABEL_W + GAP;

        let new_diagram_rect = Rect {
            pos: dvec2(x, pill_y),
            size: dvec2(NEW_DIAGRAM_W, PILL_H),
        };
        self.draw_action
            .draw_abs(cx, rect.pos + dvec2(x, text_y), "+ New Diagram");
        self.item_rects.push((Action::NewDiagram, new_diagram_rect));
        x += NEW_DIAGRAM_W + GAP;

        let delete_rect = Rect {
            pos: dvec2(x, pill_y),
            size: dvec2(DELETE_W, PILL_H),
        };
        self.draw_action
            .draw_abs(cx, rect.pos + dvec2(x, text_y), "Delete");
        self.item_rects.push((Action::Delete, delete_rect));

        DrawStep::done()
    }
}

impl SelectionToolbar {
    /// `Some(count)` shows the pill; `None` hides the toolbar.
    pub fn set_selection(&mut self, cx: &mut Cx, count: Option<usize>) {
        self.count = count;
        self.draw_bg.redraw(cx);
    }

    /// Convenience reader for `App`, mirroring `DocTabs::tab_action`.
    pub fn toolbar_action(&self, actions: &Actions) -> Option<SelectionToolbarAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            SelectionToolbarAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_singular_for_one() {
        assert_eq!(label_for_count(1), "1 selected");
    }

    #[test]
    fn label_plural_for_others() {
        assert_eq!(label_for_count(0), "0 selected");
        assert_eq!(label_for_count(3), "3 selected");
    }
}
