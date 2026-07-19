//! Start screen (launcher slice 1): shown when the app launches with no OKF
//! directory. Two panes -- a live, clickable recent-projects list (left) and
//! actions (right): New project, Open project (both stubs this slice). Same
//! hand-rolled immediate-mode convention as `tool_dock.rs`: manual rect layout
//! + hit-testing, no `script_mod!` sub-view tree, so click-testing and drawing
//! stay in one place.

use makepad_widgets::*;

use crate::waml_button::WamlButton;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // Soft ambient lift beneath the card: a rounded rectangular alpha falloff so
    // the panel floats over the blueprint grid instead of lying flat on it. The
    // quad is drawn `SHADOW_PAD` larger than the card on every side (see
    // `draw_walk`); `40.0` here must match that constant.
    mod.draw.CardShadow = mod.draw.DrawColor{
        tint: uniform(#x1a2c44)
        pixel: fn() {
            let p = self.pos * self.rect_size
            let c = self.rect_size * 0.5
            let half = c - vec2(56.0, 56.0)
            let d = length(max(abs(p - c) - half, vec2(0.0, 0.0)))
            // The draw pipeline blends premultiplied, so premultiply the tint by
            // the alpha -- otherwise a dim tint ADDS onto the bright backdrop and
            // reads as a white bloom instead of a shadow.
            let a = (1.0 - clamp(d / 56.0, 0.0, 1.0))
            let a2 = a * a * a * 0.20
            return vec4(self.tint.x * a2, self.tint.y * a2, self.tint.z * a2, a2)
        }
    }

    mod.widgets.StartScreenBase = #(StartScreen::register_widget(vm))

    mod.widgets.StartScreen = set_type_default() do mod.widgets.StartScreenBase{
        width: Fill
        height: Fill
        // Full-window backdrop: a plain radial bright-top gradient over the cool
        // ground. `color` is unused (the shader computes everything) but stays
        // set for the hit-test area.
        draw_bg: mod.draw.DrawColor{
            color: atlas.ground
            hi: uniform(#xeef4fb)
            lo: uniform(#xccd9e8)
            pixel: fn() {
                let d = length((self.pos - vec2(0.5, 0.0)) * vec2(1.0, 1.25))
                return mix(self.hi, self.lo, clamp(d, 0.0, 1.0))
            }
        }
        // Ambient lift drawn behind the card (see `CardShadow`).
        draw_shadow: mod.draw.CardShadow{ color: #x00000000 }
        // The centered dialog card uses our standard AccentFrame border, same as
        // every other panel (tool_dock/inspector/tree_panel).
        draw_frame: mod.draw.AccentFrame{ color: atlas.surface }
        // Thin structural rules (header divider, corner registration ticks +
        // row node-markers). `rule` is a soft hairline; `marker` is solid accent.
        draw_rule +: { color: atlas.accent_soft }
        draw_marker +: { color: atlas.accent }
        // The recents list gets its own AccentFrame, but a lighter accent than the
        // card's (the softer `surface_border`/`accent_soft` stops) and a
        // transparent fill so the card surface shows through -- border only.
        draw_list: mod.draw.AccentFrame{
            color: #x00000000
            border_hi: atlas.surface_border
            border_lo: atlas.accent_soft
        }
        draw_row_hover +: { color: atlas.selection }
        // The two right-pane action buttons, reusable `WamlButton` components:
        // each owns its shader, press ripple, and label. The screen positions +
        // hit-tests them (immediate mode); style/timing live in `waml_button`.
        btn_new: mod.widgets.WamlButton{}
        btn_open: mod.widgets.WamlButton{}
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
        // App wordmark logo (the 6-color "W"), drawn as an anti-aliased SDF (see
        // `logo.rs`) -- a DrawSvg stair-stepped at this size. Colon (not `+:`)
        // assignment so the custom `pixel: fn()` actually attaches.
        draw_logo: mod.draw.LogoMark{}
        // Header subtitle -- smaller than the row text, dim.
        draw_sub +: {
            color: atlas.text_dim
            text_style: theme.font_regular{font_size: 10 line_spacing: 1.2}
        }
        // Small uppercased section eyebrow (RECENT / START) -- structural label.
        draw_eyebrow +: {
            color: atlas.accent
            text_style: theme.font_bold{font_size: 10 line_spacing: 1.2}
        }
    }
}

/// Flat render-copy of a `config::Recent`, so the widget never holds a live
/// config handle. `pub(crate)` so `App` can construct it for `set_recents`.
pub(crate) struct RecentRow {
    pub title: String,
    pub path: String,
    /// Preformatted local "M/D/YYYY h:mm AM/PM" last-opened stamp.
    pub when: String,
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

// Dialog card: fixed width, centered in the window; height fits its content
// (`card_height`) so a short recents list never strands the card in dead space.
const CARD_W: f64 = 720.0;
const HEADER_H: f64 = 84.0;
// Recents-row height, and how many empty rows the card reserves height for so it
// stands tall with just one project.
const ROW_H: f64 = 64.0;
const VISIBLE_ROWS: usize = 6;
const BTN_H: f64 = 54.0;
const BTN_GAP: f64 = 12.0;
const PANE_PAD: f64 = 16.0;
const RIGHT_PANE_W: f64 = 260.0;
// Bottom padding below the taller pane, inside the card.
const BODY_PAD: f64 = 20.0;
// Left edge of the right-hand relative-time column in a recents row (padded in
// from the row's right edge). Humanized stamps are short, so a narrow column.
const WHEN_W: f64 = 110.0;
// Gap between the list frame and a row's hover fill; text padding inside the row.
const ROW_MARGIN: f64 = 10.0;
const ROW_PAD: f64 = 12.0;
// Height a section eyebrow (RECENT / START) reserves above its pane content.
const EYEBROW_H: f64 = 22.0;
// Ambient card shadow: spread (must match `CardShadow`'s `56.0`) + downward drop.
const SHADOW_PAD: f64 = 56.0;
const SHADOW_DROP: f64 = 6.0;
// Leading accent node-marker on a recents row (a small square canvas-entity dot).
const MARKER: f64 = 7.0;
// App wordmark logo (SDF, see `logo.rs`) drawn in the card header. Height sets
// the visual size; width holds the logo's tight content aspect (~1.749).
const LOGO_H: f64 = 30.0;
const LOGO_W: f64 = 52.5;

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
    draw_shadow: DrawColor,
    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_list: DrawColor,
    #[redraw]
    #[live]
    draw_rule: DrawColor,
    #[redraw]
    #[live]
    draw_marker: DrawColor,
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
    draw_logo: DrawQuad,
    #[redraw]
    #[live]
    draw_sub: DrawText,
    #[redraw]
    #[live]
    draw_eyebrow: DrawText,
    // The two right-pane action buttons. Each `WamlButton` owns its own shader,
    // press ripple, and label draw; the screen positions + hit-tests them and
    // drives the ripple via `press`/`tick`/`release`.
    #[live]
    btn_new: WamlButton,
    #[live]
    btn_open: WamlButton,

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
        // Drive either button's press ripple while held (each owns its own
        // next-frame loop; only the pressed one advances on a given frame).
        let ticked_new = self.btn_new.tick(cx, event);
        let ticked_open = self.btn_open.tick(cx, event);
        if ticked_new || ticked_open {
            self.draw_bg.redraw(cx);
            return;
        }
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                // Start the ripple on whichever button was pressed.
                for (h, r) in self.hot_rects.clone() {
                    if r.contains(fe.abs) {
                        match h {
                            Hot::New => self.btn_new.press(cx, r, fe.abs, fe.time),
                            Hot::Open => self.btn_open.press(cx, r, fe.abs, fe.time),
                            Hot::Recent(_) => {}
                        }
                        break;
                    }
                }
            }
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
                self.btn_new.release(cx);
                self.btn_open.release(cx);
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
        // Full-window backdrop over the hidden editor.
        self.draw_bg.draw_abs(cx, rect);
        self.hot_rects.clear();

        // The card height fits its content: whichever pane is taller (the recents
        // list or the two action buttons) sets the body, so a single recent row
        // never leaves the card stranded in a sea of empty surface.
        // Reserve room for several recents so the card stands tall even with one
        // project; a longer list grows it further (scrolling is a later slice).
        let list_h = ROW_MARGIN * 2.0 + self.rows.len().max(VISIBLE_ROWS) as f64 * ROW_H;
        let buttons_h = BTN_H * 2.0 + BTN_GAP;
        let pane_h = list_h.max(buttons_h);
        let card_h = HEADER_H + EYEBROW_H + pane_h + BODY_PAD;

        let card = Rect {
            pos: dvec2(
                rect.pos.x + (rect.size.x - CARD_W).max(0.0) * 0.5,
                rect.pos.y + (rect.size.y - card_h).max(0.0) * 0.5,
            ),
            size: dvec2(CARD_W, card_h),
        };
        let card_right = card.pos.x + card.size.x;

        // Ambient lift (drawn first, behind the card + nudged down).
        self.draw_shadow.draw_abs(
            cx,
            Rect {
                pos: dvec2(card.pos.x - SHADOW_PAD, card.pos.y - SHADOW_PAD + SHADOW_DROP),
                size: dvec2(card.size.x + SHADOW_PAD * 2.0, card.size.y + SHADOW_PAD * 2.0),
            },
        );
        self.draw_frame.draw_abs(cx, card);

        // Header band (relative to the card), closed by a hairline divider.
        self.draw_logo.draw_abs(
            cx,
            Rect {
                pos: dvec2(card.pos.x + PANE_PAD, card.pos.y + 20.0),
                size: dvec2(LOGO_W, LOGO_H),
            },
        );
        self.draw_sub.draw_abs(
            cx,
            dvec2(card.pos.x + PANE_PAD, card.pos.y + 54.0),
            "Create or open a project to get started",
        );
        self.draw_rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(card.pos.x + PANE_PAD, card.pos.y + HEADER_H),
                size: dvec2((card.size.x - PANE_PAD * 2.0).max(0.0), 1.5),
            },
        );

        let body_y = card.pos.y + HEADER_H + EYEBROW_H;

        // Right pane (actions), then left pane (recents), inside the card.
        let right_x = card_right - RIGHT_PANE_W;
        let left_w = right_x - card.pos.x;

        // --- Left: recents, inside a soft-accent framed list ---
        self.draw_eyebrow.draw_abs(
            cx,
            dvec2(card.pos.x + PANE_PAD, card.pos.y + HEADER_H + 6.0),
            "RECENT",
        );
        let list_rect = Rect {
            pos: dvec2(card.pos.x + PANE_PAD, body_y),
            size: dvec2((left_w - PANE_PAD).max(0.0), list_h),
        };
        self.draw_list.draw_abs(cx, list_rect);

        // Rows are inset from the list frame by ROW_MARGIN; a leading accent
        // node-marker (a small square, echoing a canvas entity) precedes the
        // title. Title + relative last-opened stamp share the top line; the path
        // gets its own full-width line below, so a long path never collides with
        // the time.
        let marker_x = list_rect.pos.x + ROW_MARGIN + ROW_PAD;
        let inner_x = marker_x + MARKER + 12.0;
        if self.rows.is_empty() {
            self.draw_dim.draw_abs(
                cx,
                dvec2(marker_x, list_rect.pos.y + ROW_MARGIN + 16.0),
                "No recent projects",
            );
        } else {
            let mut y = list_rect.pos.y + ROW_MARGIN;
            let when_x = list_rect.pos.x + list_rect.size.x - ROW_MARGIN - ROW_PAD - WHEN_W;
            for (i, row) in self.rows.iter().enumerate() {
                let row_rect = Rect {
                    pos: dvec2(list_rect.pos.x + ROW_MARGIN, y),
                    size: dvec2((list_rect.size.x - ROW_MARGIN * 2.0).max(0.0), ROW_H),
                };
                if self.hovered == Some(Hot::Recent(i)) {
                    self.draw_row_hover.draw_abs(cx, row_rect);
                }
                self.draw_marker.draw_abs(
                    cx,
                    Rect { pos: dvec2(marker_x, y + 17.0), size: dvec2(MARKER, MARKER) },
                );
                self.draw_title.draw_abs(cx, dvec2(inner_x, y + 12.0), &row.title);
                self.draw_dim.draw_abs(cx, dvec2(when_x, y + 14.0), &row.when);
                self.draw_dim.draw_abs(cx, dvec2(inner_x, y + 36.0), &row.path);
                self.hot_rects.push((Hot::Recent(i), row_rect));
                y += ROW_H;
            }
        }

        // --- Right: HUD action buttons (reusable `WamlButton` components) ---
        let btn_x = right_x + PANE_PAD;
        let btn_w = RIGHT_PANE_W - PANE_PAD * 2.0;
        self.draw_eyebrow.draw_abs(cx, dvec2(btn_x, card.pos.y + HEADER_H + 6.0), "START");

        let new_rect = Rect { pos: dvec2(btn_x, body_y), size: dvec2(btn_w, BTN_H) };
        self.btn_new.draw_at(cx, new_rect, "NEW PROJECT", self.hovered == Some(Hot::New));
        self.hot_rects.push((Hot::New, new_rect));

        let open_rect = Rect { pos: dvec2(btn_x, body_y + BTN_H + BTN_GAP), size: dvec2(btn_w, BTN_H) };
        self.btn_open.draw_at(cx, open_rect, "OPEN PROJECT", self.hovered == Some(Hot::Open));
        self.hot_rects.push((Hot::Open, open_rect));

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
