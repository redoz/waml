//! Start screen (launcher): shown when the app launches with no OKF directory.
//! A centered card with two panes -- a recent-projects list (left) and actions
//! (right): New project, Open project.
//!
//! Task 1 of the layout refactor: the hand-rolled immediate-mode card (absolute
//! rects + manual hit-testing) is rebuilt on the makepad layout engine. The card
//! shell is a `script_mod!` `View` tree and the recents list is a `FlatList`
//! whose rows are real flow-layout widgets. This slice proves the FlatList
//! draw-drive wiring (copied from the fork's `old/studio/src/run_list.rs`
//! consumer) with placeholder styling; the real row widget (Task 2) and click
//! routing / real buttons (Task 3) land later. The `StartScreen` widget now
//! derefs a `View` (the `inspector_panel.rs` hybrid pattern).

use crate::action_link::ActionLinkWidgetRefExt;
use crate::recent_row::RecentRowViewWidgetRefExt;
use makepad_widgets::*;
use waml::solve::sizing::{self, Font};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // Soft ambient lift beneath the card. NOT drawn this task (wiring it needs
    // the card's post-layout rect *and* correct z-order behind the card, which
    // is deferred to Task 3) -- but kept intact per the refactor plan. DO NOT
    // DELETE.
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
        show_bg: true
        // Full-window backdrop: a plain radial bright-top gradient over the cool
        // ground, ported verbatim from the previous immediate-mode `draw_bg`.
        // `color` is unused (the shader computes everything) but stays set for the
        // hit-test area.
        draw_bg +: {
            color: atlas.ground
            hi: uniform(atlas.ground)
            lo: uniform(atlas.canvas_ground)
            pixel: fn() {
                let d = length((self.pos - vec2(0.5, 0.0)) * vec2(1.0, 1.25))
                return mix(self.hi, self.lo, clamp(d, 0.0, 1.0))
            }
        }
        // Center the card in the window; the card's `Fit` height means a short
        // recents list never strands it in dead space (replaces the old
        // `(rect.size - CARD_W) * 0.5` centering + `card_h` math).
        align: Align{x: 0.5, y: 0.5}

        // The centered dialog card. Fixed width (was `CARD_W`), height fits its
        // content. Carries the standard Atlas AccentFrame material, inlined the same
        // way `inspector_panel.rs` inlines it onto a `View`'s `draw_bg`.
        card := View {
            width: 720.0
            height: Fit
            flow: Down
            show_bg: true
            padding: Inset{left: 20.0, right: 20.0, top: 20.0, bottom: 20.0}
            spacing: 14.0
            draw_bg +: {
                color: atlas.surface
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

            // Header: wordmark + subtitle, vertically centered by `align` (no
            // more `+ 20.0` / `+ 54.0` y-offsets).
            header := View {
                width: Fill
                height: Fit
                flow: Right
                // Bottom-align so the subtitle sits on the logo's baseline
                // rather than the logo's vertical center.
                align: Align{y: 1.0}
                spacing: 8.0
                // The splash wordmark, as the interactive `LogoMark` widget in
                // `auto` mode: it free-runs its `mode` colour pulse (no hover
                // gate, no click). `mode` picks the variant -- see logo.rs:
                //   1 accent · 2 Close Encounters · 3 bucket-palette
                //   4 molten · 5 neon · 6 electric
                logo := LogoMark {
                    width: 77.0
                    height: 44.0
                    auto: true
                    draw_bg: mod.draw.LogoMark{ mode: 2.0 }
                }
                sub := Label {
                    text: "Create or open a model to get started"
                    // Baseline-seating bottom margin is set from Rust in
                    // `draw_walk` (derived from this font's descent), so no magic
                    // pixel constant lives here and it retracks any font_size
                    // change automatically. See `seat_subtitle_baseline`.
                    draw_text +: {
                        color: atlas.text_dim
                        text_style: theme.font_regular{font_size: atlas.size_caption line_spacing: 1.0}
                    }
                }
            }

            // Header divider hairline. A thin solid-fill View (sharp `sdf.rect`
            // fill, per the fork's 0-radius `sdf.box` flood gotcha).
            rule := View {
                width: Fill
                height: 1.5
                show_bg: true
                draw_bg +: {
                    // Header hairline: the accent blue at full opacity.
                    // `frame_lo` (0x80/50%) / 0xBF read too faint; no exact
                    // token, so literal.
                    color: #x1496dcff
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
                        sdf.fill(self.color)
                        return sdf.result
                    }
                }
            }

            body := View {
                width: Fill
                height: Fit
                flow: Right
                spacing: 20.0

                // Left: eyebrow + the recents FlatList. `Fill` width so the
                // right actions column takes its fixed 260 and this consumes slack.
                recents_col := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    recent_eyebrow := Label {
                        text: "RECENT"
                        draw_text +: {
                            color: atlas.accent
                            text_style: theme.font_bold{font_size: atlas.size_eyebrow line_spacing: 1.2}
                        }
                    }
                    // Bordered frame around the recents list. Stroke-only (no
                    // fill) so the card surface shows through; sharp `sdf.rect`
                    // per the 0-radius `sdf.box` flood gotcha. The 1px padding
                    // keeps rows off the stroke. The draw loop finds the inner
                    // FlatList via `as_flat_list()` during traversal, so this
                    // extra nesting does not touch the id path.
                    list_frame := View {
                        width: Fill
                        // Fixed tall height so the recents box anchors the card
                        // (short lists still read as a real panel); the inner
                        // `Fill` FlatList scrolls when recents overflow.
                        height: 320.0
                        show_bg: true
                        // Inset the FlatList off the border so rows (and their
                        // hover wash) breathe inside the box.
                        padding: Inset{left: 5.0, right: 5.0, top: 5.0, bottom: 5.0}
                        draw_bg +: {
                            // List box border: accent blue at 50% alpha, softer
                            // than the 100% header divider so the inner frame
                            // recedes.
                            color: atlas.frame_lo
                            pixel: fn() {
                                let inset = 0.5
                                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                                sdf.stroke(self.color, 1.0)
                                return sdf.result
                            }
                        }

                        // The dynamic recents list. Rows are created from the `Row`
                        // template at draw time (see `draw_walk`). `Fit` height so it
                        // sizes to its rows inside the `Fit` card.
                        recents_list := FlatList {
                            width: Fill
                            height: Fill
                            flow: Down

                            // Task 2 row template: the real `RecentRowView` widget
                            // (marker + stacked title/path + right-anchored stamp).
                            Row := mod.widgets.RecentRowView { }
                        }
                    }
                }

                // Right: eyebrow + two action buttons. Fixed width (was
                // `RIGHT_PANE_W`).
                actions_col := View {
                    width: 260.0
                    height: Fit
                    flow: Down
                    spacing: 12.0
                    start_eyebrow := Label {
                        text: "START"
                        draw_text +: {
                            color: atlas.accent
                            text_style: theme.font_bold{font_size: atlas.size_eyebrow line_spacing: 1.2}
                        }
                    }
                    // VS-style borderless action links: an accent icon + prose
                    // label, hover wash, no button chrome. Each emits its own
                    // `ActionLinkAction::Clicked` that `handle_actions` maps to a
                    // `StartScreenAction`. `Fit` height (no fixed button rows).
                    link_new := mod.widgets.ActionLink { text: "Create a new model" kind: 0.0 }
                    link_open := mod.widgets.ActionLink { text: "Open a model" kind: 1.0 }
                }
            }
        }
    }
}

/// Flat render-copy of a `config::Recent`, so the widget never holds a live
/// config handle. `pub(crate)` so `App` can construct it for `set_recents`.
pub(crate) struct RecentRow {
    pub title: String,
    pub path: String,
    /// Preformatted local "M/D/YYYY h:mm AM/PM" last-opened stamp. Rendered
    /// right-anchored in the `RecentRowView` row.
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

#[derive(Script, ScriptHook, Widget)]
pub struct StartScreen {
    /// The container: the card shell + FlatList declared in the DSL tree above.
    #[deref]
    view: View,

    #[rust]
    rows: Vec<RecentRow>,
    // Self-managed like `ShortcutsOverlay`: the fork's `Widget::set_visible`
    // default is a no-op and custom widgets have no DSL `visible` property, so
    // hiding is a `#[rust]` flag gated in `handle_event`/`draw_walk`. Defaults
    // false -> the screen starts hidden; `App` reveals it via `set_visible`.
    #[rust]
    visible: bool,
}

impl StartScreen {
    /// Seat the header subtitle's baseline on the wordmark's. The header row
    /// bottom-aligns its boxes (`align: Align{y: 1.0}`), but the Label box extends
    /// its descent below the baseline while the wordmark is bottom-tight, so the
    /// subtitle floats a descent's-worth too high. Push the Label box down by
    /// exactly that descent, read from the font at the Label's own `font_size` --
    /// no pixel constant, and it retracks any font_size change in the DSL.
    fn seat_subtitle_baseline(&mut self, cx: &mut Cx2d) {
        if let Some(mut sub) = self.view.label(cx, ids!(sub)).borrow_mut() {
            let pt = sub.draw_text.text_style.font_size as f64;
            let descent = sizing::descent(pt * sizing::PT_TO_LPX, Font::Sans);
            sub.walk.margin.bottom = -descent;
        }
    }
}

impl Widget for StartScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.visible {
            return;
        }
        // Drive the container tree (list scrollbars, row + button events), then
        // route the grouped child actions into `StartScreenAction`s.
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            // Nothing drawn -- `main_column` (painted first) shows through.
            return DrawStep::done();
        }
        self.seat_subtitle_baseline(cx);
        // The run_list.rs interpose idiom: walk the tree, and when the FlatList
        // step surfaces, populate one child widget per recent row from the `Row`
        // template, push data in, and draw it.
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_flat_list().borrow_mut() {
                if self.rows.is_empty() {
                    // Empty state: one placeholder row (single code path -- no
                    // separate tree node to keep visible/hidden). Not clickable,
                    // so it neither washes on hover nor fires a click.
                    let item_id = LiveId::from_str("empty");
                    let row = list.item(cx, item_id, id!(Row)).unwrap();
                    let rv = row.as_recent_row_view();
                    rv.set_title(cx, "No recent models");
                    rv.set_path(cx, "");
                    rv.set_when(cx, "");
                    rv.set_clickable(false);
                    row.draw_all(cx, &mut Scope::empty());
                } else {
                    for row_data in self.rows.iter() {
                        // Stable per-recent id keeps a row's widget across redraws.
                        let item_id = LiveId::from_str(&row_data.path);
                        let row = list.item(cx, item_id, id!(Row)).unwrap();
                        let rv = row.as_recent_row_view();
                        rv.set_title(cx, &row_data.title);
                        rv.set_path(cx, &row_data.path);
                        rv.set_when(cx, &row_data.when);
                        rv.set_clickable(true);
                        row.draw_all(cx, &mut Scope::empty());
                    }
                }
            }
        }
        DrawStep::done()
    }
}

/// Map a `FlatList` row `item_id` back to its index in `rows`. Rows are keyed
/// `LiveId::from_str(&row.path)` in the draw loop, so re-hash each path and match.
/// Pure, so the round-trip is unit-tested without a `Cx`.
fn row_index_for(rows: &[RecentRow], item_id: LiveId) -> Option<usize> {
    rows.iter()
        .position(|r| LiveId::from_str(&r.path) == item_id)
}

impl WidgetMatchEvent for StartScreen {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let uid = self.widget_uid();

        // Recent rows: the clicked row's grouped action carries its `item_id`;
        // map it back to a recent index and emit `OpenRecent(i)`.
        let list = self.view.flat_list(cx, ids!(recents_list));
        for (item_id, item) in list.items_with_actions(actions) {
            if item.as_recent_row_view().clicked(actions) {
                if let Some(i) = row_index_for(&self.rows, item_id) {
                    cx.widget_action(uid, StartScreenAction::OpenRecent(i));
                }
            }
        }

        // Action links: read the standard clicked convention off each link.
        if self
            .view
            .widget(cx, ids!(link_new))
            .as_action_link()
            .clicked(actions)
        {
            cx.widget_action(uid, StartScreenAction::NewProject);
        }
        if self
            .view
            .widget(cx, ids!(link_open))
            .as_action_link()
            .clicked(actions)
        {
            cx.widget_action(uid, StartScreenAction::OpenProject);
        }
    }
}

impl StartScreen {
    /// Replace the rendered recents. `App` calls this before showing the screen.
    pub fn set_recents(&mut self, cx: &mut Cx, rows: Vec<RecentRow>) {
        self.rows = rows;
        self.view.redraw(cx);
    }

    /// Show/hide the screen. Mirrors `ShortcutsOverlay::set_visible`: while
    /// hidden, `draw_walk` returns early so the view's `Area` is never assigned a
    /// draw-list id and a scoped `redraw` is a no-op -- so force a full repaint to
    /// flip state on the first toggle.
    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
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

    fn row(path: &str) -> RecentRow {
        RecentRow {
            title: "t".into(),
            path: path.into(),
            when: "w".into(),
        }
    }

    #[test]
    fn row_index_round_trips_through_item_id() {
        let rows = vec![row("/a"), row("/b"), row("/c")];
        // The draw loop keys each row `LiveId::from_str(&path)`; routing must
        // recover the same index from that id.
        for (i, r) in rows.iter().enumerate() {
            let id = LiveId::from_str(&r.path);
            assert_eq!(row_index_for(&rows, id), Some(i));
        }
    }

    #[test]
    fn row_index_unknown_id_is_none() {
        let rows = vec![row("/a"), row("/b")];
        assert_eq!(row_index_for(&rows, LiveId::from_str("/nope")), None);
        // The empty-state placeholder id must never map to a real row.
        assert_eq!(row_index_for(&rows, LiveId::from_str("empty")), None);
    }
}
