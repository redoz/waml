//! `FindStrip`: the `Ctrl+F` find-in-document overlay (Task 12, spec
//! `2026-08-09-bundle-search-design.md` §Find strip). A thin bar docked over
//! the open document (registered in `app.rs`'s live design, inside
//! `center_stack` so it overlays the active surface only), starting hidden.
//!
//! `FindModel` is the pure counter model (`"3 of 12"`), unit-tested without a
//! `Cx`. `FindStrip` is the widget: a query `TextInput`, a counter `Label`,
//! and next/previous/close `IconButton`s. It only reports what happened
//! (`FindStripAction`, read via [`FindStrip::action`]) -- `App` (Task 13,
//! `app/actions.rs`) re-queries `SearchState`, drives highlights/spotlight,
//! and wires Ctrl+F to `open`.

use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use makepad_widgets::*;

/// Pure counter model for the strip's `"3 of 12"` readout, unit-testable
/// without a `Cx`. `App` (Task 13) builds a fresh one every time the query or
/// its hit count changes -- `current` therefore starts `None` on every query
/// change by construction, never carried over from a previous query.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindModel {
    pub query: String,
    pub total: usize,
    /// 0-based; renders as `"3 of 12"`. `None` before the first `step`.
    pub current: Option<usize>,
}

impl FindModel {
    /// `""` for an empty (whitespace-only counts as empty) query, `"0
    /// results"` for a non-empty query with no hits, else `"{n} of
    /// {total}"` (1-based) -- `current.unwrap_or(0)` so the counter reads
    /// `"1 of N"` before the first explicit `step`.
    pub fn counter_text(&self) -> String {
        if self.query.trim().is_empty() {
            return String::new();
        }
        if self.total == 0 {
            return "0 results".to_string();
        }
        let current = self.current.unwrap_or(0);
        format!("{} of {}", current + 1, self.total)
    }

    /// Advance the cursor one step, wrapping at either end. A no-op (cursor
    /// stays `None`) when there is nothing to walk.
    ///
    /// No production caller: `App` (Task 13, `app/actions.rs`) rebuilds a
    /// fresh `FindModel` from `SearchSession`'s own cursor after every
    /// `advance` instead of stepping this model in place -- the session is
    /// the single ordering (spec: no second cursor to drift out of sync).
    #[allow(dead_code)]
    pub fn step(&mut self, forward: bool) {
        if self.total == 0 {
            self.current = None;
            return;
        }
        let len = self.total;
        let next = match self.current {
            None if forward => 0,
            None => len - 1,
            Some(index) if forward => (index + 1) % len,
            Some(index) => (index + len - 1) % len,
        };
        self.current = Some(next);
    }
}

/// What the strip reported this event pass, read via [`FindStrip::action`].
/// `Next`/`Previous` come from Enter/Shift+Enter in the query input as well
/// as the next/previous buttons; `Close` from Escape or the close button.
#[derive(Clone, Debug, PartialEq)]
pub enum FindStripAction {
    QueryChanged(String),
    Next,
    Previous,
    Close,
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.FindStripBase = #(FindStrip::register_widget(vm))

    mod.widgets.FindStrip = set_type_default() do mod.widgets.FindStripBase{
        width: Fit
        height: Fit
        visible: false
        flow: Right
        align: Align{y: 0.5}
        padding: Inset{left: 10.0, right: 6.0, top: 6.0, bottom: 6.0}
        spacing: 6.0
        show_bg: true
        draw_bg +: {
            color: atlas.surface
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 6.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        query_input := TextInput {
            width: 200.0
            height: Fit
            empty_text: "Find in document"
            draw_text +: {
                color: atlas.text
                text_style: fonts.text_menu
            }
        }
        counter_label := Label {
            width: 72.0
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: fonts.text_micro
            }
        }
        prev_button := mod.widgets.IconButton { }
        next_button := mod.widgets.IconButton { }
        close_button := mod.widgets.IconButton { }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct FindStrip {
    #[deref]
    view: View,
    /// Self-tracked open state, decoupled from the view's own `visible` so
    /// `is_open()` reads true the instant `open` sets it (before any redraw).
    #[rust]
    open: bool,
    /// Set by `open`, consumed by the next `draw_walk` (see its doc comment
    /// for why focusing can't happen inside `open` itself).
    #[rust]
    pending_focus: bool,
}

impl Widget for FindStrip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        self.widget_match_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Static glyphs; cheap and idempotent (`set_icon` only redraws on an
        // actual change), same idiom `tool_dock.rs` uses each `draw_walk`.
        self.view
            .widget(cx, ids!(prev_button))
            .as_icon_button()
            .set_icon(cx, Icon::ArrowUp);
        self.view
            .widget(cx, ids!(next_button))
            .as_icon_button()
            .set_icon(cx, Icon::ArrowDown);
        self.view
            .widget(cx, ids!(close_button))
            .as_icon_button()
            .set_icon(cx, Icon::CircleX);
        let step = self.view.draw_walk(cx, scope, walk);
        // `query_input`'s draw-quad area only exists once it has actually
        // drawn -- a `View` with `visible: false` skips `draw_walk`
        // entirely (never measures/draws its children), so `open`'s own
        // synchronous `.area()` read, right after flipping `visible` on the
        // strip's very first open, would always see `Area::Empty` and focus
        // nothing. Defer the focus grab to here, the first `draw_walk`
        // AFTER `open` -- by now `self.view.draw_walk` above has drawn
        // `query_input` for real, so its area is finally valid.
        if self.pending_focus {
            let area = self.view.widget(cx, ids!(query_input)).area();
            if !area.is_empty() {
                cx.set_key_focus(area);
                self.pending_focus = false;
            }
        }
        step
    }
}

impl WidgetMatchEvent for FindStrip {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions, _scope: &mut Scope) {
        let uid = self.widget_uid();
        let query_input = self.view.text_input(cx, ids!(query_input));
        if let Some(text) = query_input.changed(actions) {
            cx.widget_action(uid, FindStripAction::QueryChanged(text));
        }
        if let Some((_, modifiers)) = query_input.returned(actions) {
            let action = if modifiers.shift {
                FindStripAction::Previous
            } else {
                FindStripAction::Next
            };
            cx.widget_action(uid, action);
        }
        if query_input.escaped(actions) {
            cx.widget_action(uid, FindStripAction::Close);
        }
        if self
            .view
            .widget(cx, ids!(next_button))
            .as_icon_button()
            .clicked(actions)
        {
            cx.widget_action(uid, FindStripAction::Next);
        }
        if self
            .view
            .widget(cx, ids!(prev_button))
            .as_icon_button()
            .clicked(actions)
        {
            cx.widget_action(uid, FindStripAction::Previous);
        }
        if self
            .view
            .widget(cx, ids!(close_button))
            .as_icon_button()
            .clicked(actions)
        {
            cx.widget_action(uid, FindStripAction::Close);
        }
    }
}

impl FindStrip {
    /// Show the strip and arm the query input to grab keyboard focus on the
    /// next `draw_walk` (see that method's doc comment: `visible: false`
    /// skips drawing entirely, so `query_input` has no real area to focus
    /// yet on the very first open -- flipping `visible` here doesn't draw it
    /// synchronously).
    pub fn open(&mut self, cx: &mut Cx) {
        self.open = true;
        self.pending_focus = true;
        self.view.set_visible(cx, true);
        self.view.redraw(cx);
    }

    /// Hide the strip and release keyboard focus.
    pub fn close(&mut self, cx: &mut Cx) {
        self.open = false;
        self.pending_focus = false;
        self.view.set_visible(cx, false);
        cx.set_key_focus(Area::Empty);
        self.view.redraw(cx);
    }

    /// No production caller yet -- Task 14's Esc handling ("clear ...  the
    /// find strip if open") is the first one that needs to ask.
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Push a fresh counter reading. Does NOT touch the query input's own
    /// text -- that already echoes what the user typed (it is the source of
    /// `QueryChanged`, not a mirror of it), so writing it back here would
    /// fight live editing / move the cursor mid-type.
    pub fn set_model(&mut self, cx: &mut Cx, model: &FindModel) {
        self.view
            .label(cx, ids!(counter_label))
            .set_text(cx, &model.counter_text());
    }

    /// This strip's own action from `actions`, if any (see
    /// [`FindStripAction`]).
    pub fn action(&self, actions: &Actions) -> Option<FindStripAction> {
        let action = actions.find_widget_action(self.widget_uid())?;
        action.action.downcast_ref::<FindStripAction>().cloned()
    }
}

impl FindStripRef {
    /// See [`FindStrip::open`].
    pub fn open(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open(cx);
        }
    }

    /// See [`FindStrip::close`].
    pub fn close(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close(cx);
        }
    }

    /// See [`FindStrip::is_open`].
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_open())
    }

    /// See [`FindStrip::set_model`].
    pub fn set_model(&self, cx: &mut Cx, model: &FindModel) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_model(cx, model);
        }
    }

    /// See [`FindStrip::action`].
    pub fn action(&self, actions: &Actions) -> Option<FindStripAction> {
        self.borrow().and_then(|inner| inner.action(actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(query: &str, total: usize, current: Option<usize>) -> FindModel {
        FindModel {
            query: query.to_string(),
            total,
            current,
        }
    }

    #[test]
    fn counter_text_is_empty_for_an_empty_or_whitespace_query() {
        assert_eq!(model("", 5, Some(2)).counter_text(), "");
        assert_eq!(model("   ", 5, Some(2)).counter_text(), "");
    }

    #[test]
    fn counter_text_reports_zero_results_for_a_non_empty_query_with_no_hits() {
        assert_eq!(model("xyz", 0, None).counter_text(), "0 results");
    }

    #[test]
    fn counter_text_formats_the_current_position_one_based() {
        assert_eq!(model("term", 12, Some(2)).counter_text(), "3 of 12");
    }

    #[test]
    fn counter_text_reads_as_the_first_match_before_the_first_step() {
        assert_eq!(model("term", 12, None).counter_text(), "1 of 12");
    }

    #[test]
    fn step_walks_forward_and_wraps() {
        let mut model = model("q", 3, None);
        model.step(true);
        assert_eq!(model.current, Some(0));
        model.step(true);
        assert_eq!(model.current, Some(1));
        model.step(true);
        assert_eq!(model.current, Some(2));
        model.step(true);
        assert_eq!(model.current, Some(0));
    }

    #[test]
    fn step_walks_backward_and_wraps() {
        let mut model = model("q", 3, None);
        model.step(false);
        assert_eq!(model.current, Some(2));
        model.step(false);
        assert_eq!(model.current, Some(1));
        model.step(false);
        assert_eq!(model.current, Some(0));
        model.step(false);
        assert_eq!(model.current, Some(2));
    }

    #[test]
    fn step_over_no_results_leaves_the_cursor_at_none() {
        let mut model = model("q", 0, None);
        model.step(true);
        assert!(model.current.is_none());
        model.step(false);
        assert!(model.current.is_none());
    }

    #[test]
    fn a_fresh_query_carries_no_cursor_over_from_the_previous_one() {
        // `App` builds a new `FindModel` per query re-run (Task 13); a model
        // freshly constructed for a changed query therefore starts with
        // `current: None` regardless of where the previous query's cursor
        // landed -- there is no field that could carry it over.
        let fresh = model("new", 4, None);
        assert!(fresh.current.is_none());
    }
}
