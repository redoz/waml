//! Start screen shown when the app has no open model. A compact action/recent
//! column sits directly in the editor canvas over a responsive, subdued WAML
//! wordmark. Recents use a capped `FlatList` with real flow-layout row widgets.

use crate::action_link::ActionLinkWidgetRefExt;
use crate::recent_row::{RecentRowView, RecentRowViewWidgetRefExt};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

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
        flow: Overlay

        // Signature layer: the real six-segment WAML mark, kept deliberately
        // quiet so foreground actions remain legible. A plain View does not
        // run LogoMark's interactive Rust widget, so `fade` stays fixed.
        background_layer := View {
            width: Fill
            height: Fill
            align: Align{x: 0.5, y: 0.5}
            backdrop_logo := View {
                width: 0.0
                height: 0.0
                show_bg: true
                draw_bg: mod.draw.LogoMark {
                    fade: 0.07
                }
            }
        }

        // Compact in-canvas launcher. The background remains visible through
        // every child: there is no dialog surface, frame, or divider.
        foreground_host := ScrollYView {
            width: Fill
            height: Fill
            align: Align{x: 0.5, y: 0.5}

            content := View {
                width: 0.0
                height: Fit
                flow: Down
                spacing: 18.0

                actions := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 4.0
                    link_new := mod.widgets.ActionLink { text: "Create a new model" kind: 0.0 }
                    link_open := mod.widgets.ActionLink { text: "Open a model" kind: 1.0 }
                }

                recents := View {
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6.0
                    recent_eyebrow := Label {
                        text: "RECENT"
                        draw_text +: {
                            color: atlas.accent
                            text_style: fonts.text_eyebrow
                        }
                    }
                    list_host := View {
                        width: Fill
                        height: Fit
                        recents_list := FlatList {
                            width: Fill
                            height: Fill
                            flow: Down
                            Row := mod.widgets.RecentRowView { }
                        }
                    }
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
    /// Preformatted local "M/D/YYYY h:mm AM/PM" last-opened stamp.
    pub when: String,
    /// Whether this recent is pinned (drives the row's pin glyph).
    pub pinned: bool,
}

const MAX_RECENTS: usize = 5;
const LOGO_ASPECT: f64 = 1.749;
const LOGO_MARGIN: f64 = 48.0;
const FOREGROUND_WIDTH: f64 = 440.0;
const FOREGROUND_MARGIN: f64 = 24.0;

fn cap_recent_rows(mut rows: Vec<RecentRow>) -> Vec<RecentRow> {
    rows.truncate(MAX_RECENTS);
    rows
}

fn backdrop_logo_size(available: DVec2) -> DVec2 {
    let max_width = (available.x - LOGO_MARGIN * 2.0).max(0.0);
    let max_height = (available.y - LOGO_MARGIN * 2.0).max(0.0);
    let width = max_width.min(max_height * LOGO_ASPECT);
    dvec2(width, width / LOGO_ASPECT)
}

fn recent_list_height(row_count: usize) -> f64 {
    row_count.clamp(1, MAX_RECENTS) as f64 * RecentRowView::ROW_HEIGHT
}

fn foreground_width(available_width: f64) -> f64 {
    (available_width - FOREGROUND_MARGIN * 2.0).clamp(0.0, FOREGROUND_WIDTH)
}

#[derive(Clone, Debug, Default)]
pub enum StartScreenAction {
    #[default]
    None,
    /// A recent row was clicked; indexes the rows last passed to `set_recents`.
    OpenRecent(usize),
    /// A recent row's pin was toggled; indexes the rows passed to `set_recents`.
    TogglePin(usize),
    NewProject,
    OpenProject,
}

#[derive(Script, ScriptHook, Widget)]
pub struct StartScreen {
    /// The overlay composition + FlatList declared in the DSL tree above.
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
        let available = cx.peek_walk_turtle(walk).size;
        let logo_size = backdrop_logo_size(available);
        if let Some(mut logo) = self.view.view(cx, ids!(backdrop_logo)).borrow_mut() {
            logo.walk.width = Size::Fixed(logo_size.x);
            logo.walk.height = Size::Fixed(logo_size.y);
        }
        if let Some(mut content) = self.view.view(cx, ids!(content)).borrow_mut() {
            content.walk.width = Size::Fixed(foreground_width(available.x));
        }
        if let Some(mut host) = self.view.view(cx, ids!(list_host)).borrow_mut() {
            host.walk.height = Size::Fixed(recent_list_height(self.rows.len()));
        }
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
                        rv.set_pinned(cx, row_data.pinned);
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
            if item.as_recent_row_view().pin_toggled(actions) {
                if let Some(i) = row_index_for(&self.rows, item_id) {
                    cx.widget_action(uid, StartScreenAction::TogglePin(i));
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
        self.rows = cap_recent_rows(rows);
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
        assert!(matches!(
            StartScreenAction::default(),
            StartScreenAction::None
        ));
    }

    fn row(path: &str) -> RecentRow {
        RecentRow {
            title: "t".into(),
            path: path.into(),
            when: "w".into(),
            pinned: false,
        }
    }

    #[test]
    fn toggle_pin_indexes_a_row() {
        // A pinned row carries the flag, and TogglePin round-trips an index.
        let r = RecentRow {
            title: "t".into(),
            path: "/p".into(),
            when: "w".into(),
            pinned: true,
        };
        assert!(r.pinned);
        assert!(matches!(
            StartScreenAction::TogglePin(3),
            StartScreenAction::TogglePin(3)
        ));
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

    #[test]
    fn recent_rows_are_capped_at_five() {
        let rows = (0..7).map(|i| row(&format!("/{i}"))).collect();
        let capped = cap_recent_rows(rows);
        assert_eq!(capped.len(), 5);
        assert_eq!(capped[4].path, "/4");
    }

    #[test]
    fn backdrop_logo_is_nearly_full_width_and_preserves_aspect() {
        let size = backdrop_logo_size(dvec2(1536.0, 958.0));
        assert_eq!(size.x, 1440.0);
        assert!((size.x / size.y - LOGO_ASPECT).abs() < 0.0001);
    }

    #[test]
    fn backdrop_logo_shrinks_to_fit_short_viewports() {
        let size = backdrop_logo_size(dvec2(1200.0, 500.0));
        assert!(size.x <= 1104.0);
        assert!(size.y <= 404.0);
        assert!((size.x / size.y - LOGO_ASPECT).abs() < 0.0001);
    }

    #[test]
    fn empty_recent_list_reserves_one_placeholder_row() {
        assert_eq!(recent_list_height(0), RecentRowView::ROW_HEIGHT);
    }

    #[test]
    fn recent_list_reserves_one_height_per_row() {
        assert_eq!(recent_list_height(3), 3.0 * RecentRowView::ROW_HEIGHT);
    }

    #[test]
    fn recent_list_never_reserves_more_than_five_rows() {
        assert_eq!(recent_list_height(8), 5.0 * RecentRowView::ROW_HEIGHT);
    }

    #[test]
    fn foreground_uses_compact_width_when_space_allows() {
        assert_eq!(foreground_width(1280.0), 440.0);
    }

    #[test]
    fn foreground_keeps_safe_margins_in_narrow_viewports() {
        assert_eq!(foreground_width(400.0), 352.0);
        assert_eq!(foreground_width(20.0), 0.0);
    }
}
