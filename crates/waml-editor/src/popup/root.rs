//! `PopupRoot` — the dismiss authority. One widget, one active-surface slot,
//! universal light-dismiss. Hosts `MenuPopup` + `RadialPopup` as child widgets;
//! `App` calls `route` once per event and `show_at` to open. Single active
//! popup app-wide: `show_at` supersedes (dismisses) any open popup first.

use crate::popup::base::{
    is_light_dismiss, is_primary_press, Popup, PopupItem, PopupResult, PopupVerdict,
};
use crate::popup::conflict_list::ConflictList;
use crate::popup::menu::{MenuPopup, MENU_MAX_W, PAD_V, ROW_H};
use crate::popup::presenter::Presenter;
use crate::popup::radial::RadialPopup;
use crate::popup::select::{
    SelectFlyout, SelectItem, SELECT_BOTTOM_MARGIN, SELECT_MAX_H, SELECT_MAX_W,
};
use crate::scene::SceneConflict;
use makepad_widgets::*;

/// How to open the linear card.
#[allow(dead_code)]
pub enum MenuOpen {
    /// Press-open (marking): the press landed at this point (tap-vs-drag origin).
    Press(DVec2),
    /// Direct latched popup open (click-to-pick).
    Popup,
}

/// How to open the wedge.
#[allow(dead_code)]
pub enum RadialOpen {
    /// Right-press marking open.
    Marking,
    /// Direct latched popup open.
    Popup,
}

/// One `show_at` request. Carries the opaque `tag`, the kind's geometry, its
/// items, and its open-mode. (The plan's realization of the spec's `show_at` --
/// the surfaces are widget-hosted, so the kind's data rides in this enum.)
#[allow(dead_code)]
pub enum PopupSpec {
    Menu {
        tag: LiveId,
        anchor: DVec2,
        bounds: Rect,
        items: Vec<PopupItem>,
        open: MenuOpen,
    },
    Radial {
        tag: LiveId,
        center: DVec2,
        bounds: Rect,
        items: Vec<PopupItem>,
        open: RadialOpen,
    },
    Select {
        tag: LiveId,
        anchor: DVec2,
        min_width: f64,
        bounds: Rect,
        items: Vec<SelectItem>,
    },
    Conflict {
        tag: LiveId,
        anchor: DVec2,
        bounds: Rect,
        conflicts: Vec<SceneConflict>,
    },
}

/// Emitted on every close. Openers filter for their own `tag`; `PopupRoot` never
/// inspects `tag` or `result` beyond routing.
#[derive(Clone, Debug, Default)]
pub enum PopupRootAction {
    #[default]
    None,
    Closed {
        tag: LiveId,
        result: PopupResult,
    },
}

/// Which surface is active. Pairs with the active tag in the slot. (The spec's
/// `PopupKind`; an enum discriminant, not a `Box<dyn>` -- the surfaces are
/// tree children reached by id-path, so the slot only needs to know which one.)
#[derive(Clone, Copy, PartialEq)]
enum ActiveKind {
    Menu,
    Radial,
    Select,
    Conflict,
}

/// The routing decision for one already-handled event.
#[derive(Clone, Debug, PartialEq)]
enum RouteStep {
    Keep,
    Close(PopupResult),
}

/// Pure post-`handle` decision: a commit/self-dismiss closes with its result; an
/// `Ignored` primary press is an outside-click (dismiss); everything else keeps
/// it open. (Light-dismiss is decided *before* this, in `route`.)
fn decide(verdict: PopupVerdict, primary_press: bool) -> RouteStep {
    match verdict {
        PopupVerdict::Closed(r) => RouteStep::Close(r),
        PopupVerdict::Ignored if primary_press => RouteStep::Close(PopupResult::Dismissed),
        _ => RouteStep::Keep,
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.PopupRootBase = #(PopupRoot::register_widget(vm))

    mod.widgets.PopupRoot = set_type_default() do mod.widgets.PopupRootBase{
        width: Fill
        height: Fill
        // The two surface kinds, hosted as genuine DSL tree children (reached
        // by id-path through `body`) rather than named `#[live]` struct
        // fields -- this fork's Widget-derive instantiation overflows the
        // stack when a struct carries two or more `#[live]` fields of full
        // nested-Widget type. Every other multi-child composite in this
        // codebase (App's own `ui`, and every panel it owns) goes through
        // exactly this `WidgetRef` + id-path lookup, so this mirrors the
        // codebase's one proven-working pattern. Each paints nothing while
        // closed.
        body: View{
            width: Fill
            height: Fill
            menu := MenuPopup{ width: Fill height: Fill }
            radial := RadialPopup{ width: Fill height: Fill }
            select := SelectFlyout{ width: Fill height: Fill }
            conflict := ConflictList{ width: Fill height: Fill }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct PopupRoot {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Hosts both surfaces as tree children (see `script_mod!` above for why
    /// this is a single `WidgetRef`, not two `#[live]` widget fields).
    #[redraw]
    #[live]
    body: WidgetRef,

    /// The single active surface + its opaque tag, or none.
    #[rust]
    active: Option<(ActiveKind, LiveId)>,
}

impl Widget for PopupRoot {
    // Event-passive: `App` drives us via `route`, not tree routing.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Draw both surfaces; each paints nothing while closed (self-guarded).
        self.body.draw_walk(cx, scope, walk)
    }
}

#[allow(dead_code)]
impl PopupRoot {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Open `spec`'s surface, superseding (dismissing) any currently-open popup
    /// first -- the single-active guarantee.
    /// Reset whichever surface is active (if any) and emit its Dismissed
    /// close. Shared by `show_at`'s supersede-before-open and `close`'s
    /// dismiss-without-opening.
    fn dismiss_active(&mut self, cx: &mut Cx) {
        if let Some((kind, tag)) = self.active.take() {
            match kind {
                ActiveKind::Menu => {
                    if let Some(mut m) = self.body.widget(cx, ids!(menu)).borrow_mut::<MenuPopup>()
                    {
                        m.reset();
                    }
                }
                ActiveKind::Radial => {
                    if let Some(mut r) = self
                        .body
                        .widget(cx, ids!(radial))
                        .borrow_mut::<RadialPopup>()
                    {
                        r.reset();
                    }
                }
                ActiveKind::Select => {
                    if let Some(mut s) = self
                        .body
                        .widget(cx, ids!(select))
                        .borrow_mut::<SelectFlyout>()
                    {
                        s.reset();
                    }
                }
                ActiveKind::Conflict => {
                    if let Some(mut c) = self
                        .body
                        .widget(cx, ids!(conflict))
                        .borrow_mut::<ConflictList>()
                    {
                        c.reset();
                    }
                }
            }
            cx.widget_action(
                self.widget_uid(),
                PopupRootAction::Closed {
                    tag,
                    result: PopupResult::Dismissed,
                },
            );
        }
    }

    /// Dismiss whatever is open, without opening a replacement (e.g. deleting
    /// the last conflict closes the list instead of re-anchoring it empty).
    /// A no-op (no action emitted) if nothing is active.
    pub fn close(&mut self, cx: &mut Cx) {
        if self.active.is_some() {
            self.dismiss_active(cx);
            self.body.redraw(cx);
        }
    }

    pub fn show_at(&mut self, cx: &mut Cx, spec: PopupSpec) {
        // Supersede: reset the prior surface and emit its Dismissed close.
        self.dismiss_active(cx);
        match spec {
            PopupSpec::Menu {
                tag,
                anchor,
                bounds,
                items,
                open,
            } => {
                // Overlay backing: clamp the card on-screen. Width is unknown
                // until draw measures the label, so clamp with the safety-cap
                // width; height is exact from the row count.
                let size = dvec2(MENU_MAX_W, PAD_V * 2.0 + items.len() as f64 * ROW_H);
                let placed = Presenter::place(anchor, size, bounds);
                if let Some(mut m) = self.body.widget(cx, ids!(menu)).borrow_mut::<MenuPopup>() {
                    match open {
                        MenuOpen::Press(press) => m.open_marking(cx, placed, press, items),
                        MenuOpen::Popup => m.open_popup(cx, placed, items),
                    }
                }
                self.active = Some((ActiveKind::Menu, tag));
            }
            PopupSpec::Radial {
                tag,
                center,
                bounds,
                items,
                open,
            } => {
                let t = cx.seconds_since_app_start();
                if let Some(mut r) = self
                    .body
                    .widget(cx, ids!(radial))
                    .borrow_mut::<RadialPopup>()
                {
                    match open {
                        RadialOpen::Marking => r.open_marking(cx, center, bounds, items, t),
                        RadialOpen::Popup => r.open_popup(cx, center, bounds, items, t),
                    }
                }
                self.active = Some((ActiveKind::Radial, tag));
            }
            PopupSpec::Select {
                tag,
                anchor,
                min_width,
                bounds,
                items,
            } => {
                // Clamp on-screen. Width is unknown until draw measures the
                // label, so clamp with the widest possible width — the cap, or
                // the control if it is wider (matches `select_width`'s ceiling).
                let cap = SELECT_MAX_W.max(min_width);
                // Clamp the card height to the space below the anchor (and the
                // hard cap) BEFORE placing. A long list would otherwise hand
                // `place` a taller-than-window size and get shoved up to the
                // window top; instead it stays anchored under the control and
                // scrolls (`SelectFlyout::draw` re-derives the same clamp for
                // the actual panel height). Mirrors `select::draw`'s formula.
                let full_h = PAD_V * 2.0 + items.len() as f64 * ROW_H;
                let space_below = (bounds.pos.y + bounds.size.y - anchor.y - SELECT_BOTTOM_MARGIN)
                    .max(ROW_H + PAD_V * 2.0);
                let clamped_h = full_h.min(SELECT_MAX_H).min(space_below);
                let size = dvec2(cap, clamped_h);
                let placed = Presenter::place(anchor, size, bounds);
                if let Some(mut s) = self
                    .body
                    .widget(cx, ids!(select))
                    .borrow_mut::<SelectFlyout>()
                {
                    // Keep the raw control-left x; the flyout centres itself on
                    // the control in `draw` (width isn't known until the labels
                    // are measured) and clamps into the window there. Only the
                    // vertical placement is taken from `place`.
                    s.open_select(cx, dvec2(anchor.x, placed.y), min_width, items);
                }
                self.active = Some((ActiveKind::Select, tag));
            }
            PopupSpec::Conflict {
                tag,
                anchor,
                bounds,
                conflicts,
            } => {
                let size = crate::popup::conflict_list::content_size(&conflicts);
                let placed = Presenter::place(anchor, size, bounds);
                if let Some(mut c) = self
                    .body
                    .widget(cx, ids!(conflict))
                    .borrow_mut::<ConflictList>()
                {
                    c.open(cx, Rect { pos: placed, size }, conflicts);
                }
                self.active = Some((ActiveKind::Conflict, tag));
            }
        }
        // A session-first open: `menu`/`radial`'s own draw components (`draw_frame`
        // / `draw_wedge`) are `#[redraw]` but have never executed `draw_abs`
        // (their `draw_walk` early-returns while closed), so their Area is not
        // yet established and `.redraw(cx)` on them alone can be a no-op. `body`
        // draws unconditionally every frame, so its Area IS always valid --
        // redraw through it (which recurses into its children) to guarantee the
        // newly-opened surface actually repaints, not just becomes logically open.
        self.body.redraw(cx);
    }

    /// The single per-event seam. Light-dismiss closes; otherwise the active
    /// surface handles it and `decide` maps the verdict.
    pub fn route(&mut self, cx: &mut Cx, event: &Event) {
        let Some((kind, tag)) = self.active else {
            return;
        };
        // Overlay backing: localize is identity (events already in main-window
        // space). A later plan's DComp backing translates here.
        let ev = Presenter.localize(event);
        let step = if is_light_dismiss(ev) {
            RouteStep::Close(PopupResult::Dismissed)
        } else {
            let verdict = match kind {
                ActiveKind::Menu => self
                    .body
                    .widget(cx, ids!(menu))
                    .borrow_mut::<MenuPopup>()
                    .map(|mut m| m.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
                ActiveKind::Radial => self
                    .body
                    .widget(cx, ids!(radial))
                    .borrow_mut::<RadialPopup>()
                    .map(|mut r| r.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
                ActiveKind::Select => self
                    .body
                    .widget(cx, ids!(select))
                    .borrow_mut::<SelectFlyout>()
                    .map(|mut s| s.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
                // Ridden on the same `decide` table as every other surface: a
                // `Consumed` verdict (row hover, trash/body press -- which
                // emit `ConflictListAction` but never self-close) keeps it
                // open, an `Ignored` primary press is an outside-click
                // dismiss. No new `RouteStep`/`decide` variant needed.
                ActiveKind::Conflict => self
                    .body
                    .widget(cx, ids!(conflict))
                    .borrow_mut::<ConflictList>()
                    .map(|mut c| c.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
            };
            decide(verdict, is_primary_press(ev))
        };
        if let RouteStep::Close(result) = step {
            match kind {
                ActiveKind::Menu => {
                    if let Some(mut m) = self.body.widget(cx, ids!(menu)).borrow_mut::<MenuPopup>()
                    {
                        m.reset();
                    }
                }
                ActiveKind::Radial => {
                    if let Some(mut r) = self
                        .body
                        .widget(cx, ids!(radial))
                        .borrow_mut::<RadialPopup>()
                    {
                        r.reset();
                    }
                }
                ActiveKind::Select => {
                    if let Some(mut s) = self
                        .body
                        .widget(cx, ids!(select))
                        .borrow_mut::<SelectFlyout>()
                    {
                        s.reset();
                    }
                }
                ActiveKind::Conflict => {
                    if let Some(mut c) = self
                        .body
                        .widget(cx, ids!(conflict))
                        .borrow_mut::<ConflictList>()
                    {
                        c.reset();
                    }
                }
            }
            cx.widget_action(self.widget_uid(), PopupRootAction::Closed { tag, result });
            self.active = None;
        }
    }

    /// Read a close for `tag` from the action queue (the opener's filter).
    pub fn closed(&self, actions: &Actions, tag: LiveId) -> Option<PopupResult> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            PopupRootAction::Closed { tag: t, result } if t == tag => Some(result),
            _ => None,
        }
    }

    /// Read the conflict-list surface's own action. A row body/trash press
    /// never closes the surface (unlike a menu commit), so it never shows up
    /// through `closed` -- `App` reads it directly off the `conflict` child.
    pub fn conflict_action(
        &self,
        cx: &mut Cx,
        actions: &Actions,
    ) -> Option<crate::popup::conflict_list::ConflictListAction> {
        self.body
            .widget(cx, ids!(conflict))
            .borrow::<ConflictList>()?
            .action(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup::base::{PopupResult, PopupVerdict};

    // `ActiveKind::Conflict` rides this same `decide` table as every other
    // surface (no new `RouteStep`/`decide` variant): `a_consumed_event_keeps_it_open`
    // covers a hover/press that emits `ConflictListAction` without closing, and
    // `an_ignored_primary_press_is_outside_click_dismiss` covers an outside click.

    #[test]
    fn a_commit_closes_with_its_result() {
        let step = decide(
            PopupVerdict::Closed(PopupResult::Invoked(live_id!(x))),
            false,
        );
        assert_eq!(step, RouteStep::Close(PopupResult::Invoked(live_id!(x))));
    }

    #[test]
    fn a_self_dismiss_closes_dismissed() {
        let step = decide(PopupVerdict::Closed(PopupResult::Dismissed), false);
        assert_eq!(step, RouteStep::Close(PopupResult::Dismissed));
    }

    #[test]
    fn an_ignored_primary_press_is_outside_click_dismiss() {
        let step = decide(PopupVerdict::Ignored, true);
        assert_eq!(step, RouteStep::Close(PopupResult::Dismissed));
    }

    #[test]
    fn an_ignored_non_press_keeps_it_open() {
        let step = decide(PopupVerdict::Ignored, false);
        assert_eq!(step, RouteStep::Keep);
    }

    #[test]
    fn a_consumed_event_keeps_it_open() {
        let step = decide(PopupVerdict::Consumed, true);
        assert_eq!(step, RouteStep::Keep);
    }
}
