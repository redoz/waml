//! Start screen (launcher slice 1): shown when the app launches with no OKF
//! directory. Two panes -- a live, clickable recent-projects list (left) and
//! actions (right): New project, Open project (both stubs this slice). Same
//! hand-rolled immediate-mode convention as `tool_dock.rs`: manual rect layout
//! + hit-testing, no `script_mod!` sub-view tree, so click-testing and drawing
//! stay in one place.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.StartScreenBase = #(StartScreen::register_widget(vm))

    mod.widgets.StartScreen = set_type_default() do mod.widgets.StartScreenBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.ground }
        draw_pane +: { color: atlas.surface }
        draw_row_hover +: { color: atlas.selection }
        // Shared theme fonts (same as `shortcuts_overlay`'s title). Per-field
        // inline `latin := FontMember{...}` families left every DrawText but
        // the last one with an empty font family at runtime; the pre-loaded
        // `theme.font_*` members resolve reliably.
        draw_title +: {
            color: atlas.text
            text_style: theme.font_regular{font_size: 14 line_spacing: 1.2}
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: theme.font_regular{font_size: 11 line_spacing: 1.2}
        }
        draw_accent +: {
            color: atlas.accent
            text_style: theme.font_bold{font_size: 22 line_spacing: 1.2}
        }
    }
}

/// Flat render-copy of a `config::Recent`, so the widget never holds a live
/// config handle. `pub(crate)` so `App` can construct it for `set_recents`.
pub(crate) struct RecentRow {
    pub title: String,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub enum StartScreenAction {
    #[default]
    None,
    /// A recent row was clicked; indexes the rows last passed to `set_recents`.
    OpenRecent(usize),
    NewProject,
    OpenProject,
}

/// Identifies a clickable rect for hit-testing/hover.
#[derive(Clone, Copy, PartialEq)]
enum Hot {
    Recent(usize),
    New,
    Open,
}

const HEADER_H: f64 = 96.0;
const ROW_H: f64 = 52.0;
const BTN_H: f64 = 44.0;
const BTN_GAP: f64 = 10.0;
const PANE_PAD: f64 = 16.0;
const RIGHT_PANE_W: f64 = 260.0;

#[derive(Script, ScriptHook, Widget)]
pub struct StartScreen {
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
    draw_pane: DrawColor,
    #[redraw]
    #[live]
    draw_row_hover: DrawColor,
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,
    #[redraw]
    #[live]
    draw_accent: DrawText,

    #[rust]
    rows: Vec<RecentRow>,
    #[rust]
    hot_rects: Vec<(Hot, Rect)>,
    #[rust]
    hovered: Option<Hot>,
    // Self-managed like `ShortcutsOverlay`: the fork's `Widget::set_visible`
    // default is a no-op and custom widgets have no DSL `visible` property, so
    // hiding is a `#[rust]` flag gated in `handle_event`/`draw_walk`. Defaults
    // false -> the screen starts hidden; `App` reveals it via `set_visible`.
    #[rust]
    visible: bool,
}

impl Widget for StartScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if !self.visible {
            return;
        }
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                for (hot, rect) in self.hot_rects.clone() {
                    if rect.contains(fe.abs) {
                        let action = match hot {
                            Hot::Recent(i) => StartScreenAction::OpenRecent(i),
                            Hot::New => StartScreenAction::NewProject,
                            Hot::Open => StartScreenAction::OpenProject,
                        };
                        cx.widget_action(uid, action);
                        break;
                    }
                }
            }
            // Re-hit-test on every move: FingerHoverIn fires once on widget
            // entry and can't tell which row the pointer is now over.
            Hit::FingerHoverOver(fe) => {
                let now = self.hot_rects.iter().find(|(_, r)| r.contains(fe.abs)).map(|(h, _)| *h);
                cx.set_cursor(if now.is_some() { MouseCursor::Hand } else { MouseCursor::Default });
                if now != self.hovered {
                    self.hovered = now;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.hovered.is_some() {
                    self.hovered = None;
                    self.draw_bg.redraw(cx);
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if !self.visible {
            // Nothing drawn -- `main_column` (painted first) shows through.
            return DrawStep::done();
        }
        self.draw_bg.draw_abs(cx, rect);
        self.hot_rects.clear();

        // Header band.
        self.draw_accent.draw_abs(cx, dvec2(rect.pos.x + PANE_PAD, rect.pos.y + 28.0), "WAML");
        self.draw_dim.draw_abs(
            cx,
            dvec2(rect.pos.x + PANE_PAD, rect.pos.y + 60.0),
            "Open a project to get started",
        );

        let body_y = rect.pos.y + HEADER_H;
        let body_h = (rect.size.y - HEADER_H).max(0.0);

        // Right pane (actions) fill, then left pane (recents) fill.
        let right_x = rect.pos.x + rect.size.x - RIGHT_PANE_W;
        let left_rect = Rect { pos: dvec2(rect.pos.x, body_y), size: dvec2(right_x - rect.pos.x, body_h) };
        let right_rect = Rect { pos: dvec2(right_x, body_y), size: dvec2(RIGHT_PANE_W, body_h) };
        self.draw_pane.draw_abs(cx, right_rect);

        // --- Left: recents ---
        if self.rows.is_empty() {
            self.draw_dim.draw_abs(
                cx,
                dvec2(left_rect.pos.x + PANE_PAD, left_rect.pos.y + PANE_PAD),
                "No recent projects",
            );
        } else {
            let mut y = left_rect.pos.y;
            for (i, row) in self.rows.iter().enumerate() {
                let row_rect = Rect { pos: dvec2(left_rect.pos.x, y), size: dvec2(left_rect.size.x, ROW_H) };
                if self.hovered == Some(Hot::Recent(i)) {
                    self.draw_row_hover.draw_abs(cx, row_rect);
                }
                self.draw_title.draw_abs(cx, dvec2(row_rect.pos.x + PANE_PAD, y + 8.0), &row.title);
                self.draw_dim.draw_abs(cx, dvec2(row_rect.pos.x + PANE_PAD, y + 30.0), &row.path);
                self.hot_rects.push((Hot::Recent(i), row_rect));
                y += ROW_H;
            }
        }

        // --- Right: action buttons ---
        let btn_x = right_rect.pos.x + PANE_PAD;
        let btn_w = RIGHT_PANE_W - PANE_PAD * 2.0;
        let mut by = right_rect.pos.y + PANE_PAD;
        for (hot, label) in [(Hot::New, "New project"), (Hot::Open, "Open project...")] {
            let btn_rect = Rect { pos: dvec2(btn_x, by), size: dvec2(btn_w, BTN_H) };
            if self.hovered == Some(hot) {
                self.draw_row_hover.draw_abs(cx, btn_rect);
            }
            self.draw_title.draw_abs(cx, dvec2(btn_x + 12.0, by + 12.0), label);
            self.hot_rects.push((hot, btn_rect));
            by += BTN_H + BTN_GAP;
        }

        DrawStep::done()
    }
}

impl StartScreen {
    /// Replace the rendered recents. `App` calls this before showing the screen.
    pub fn set_recents(&mut self, cx: &mut Cx, rows: Vec<RecentRow>) {
        self.rows = rows;
        self.hovered = None;
        self.draw_bg.redraw(cx);
    }

    /// Show/hide the screen. Mirrors `ShortcutsOverlay::set_visible`: while
    /// hidden, `draw_walk` returns early so `draw_bg`'s `Area` is never
    /// assigned a draw-list id and `draw_bg.redraw` is a no-op -- so force a
    /// full repaint to flip state on the first toggle.
    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            self.hovered = None;
            cx.redraw_all();
        }
    }

    /// Convenience reader for `App`, mirroring `ToolDock::dock_action`.
    pub fn screen_action(&self, actions: &Actions) -> Option<StartScreenAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            StartScreenAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_action_is_none() {
        assert!(matches!(StartScreenAction::default(), StartScreenAction::None));
    }
}
