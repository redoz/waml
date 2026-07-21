//! `PopupRoot` — the dismiss authority. One widget, one active-surface slot,
//! universal light-dismiss. Hosts `MenuPopup` + `RadialPopup` as child widgets;
//! `App` calls `route` once per event and `show_at` to open. Single active
//! popup app-wide: `show_at` supersedes (dismisses) any open popup first.

use crate::popup::base::{
    is_light_dismiss, is_primary_press, Popup, PopupItem, PopupResult, PopupVerdict,
};
use crate::popup::menu::{MenuPopup, MENU_MAX_W, PAD_V, ROW_H};
use crate::popup::presenter::Presenter;
use crate::popup::radial::RadialPopup;
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
/// widget-hosted `#[live]` fields, so the slot only needs to know which one.)
#[derive(Clone, Copy, PartialEq)]
enum ActiveKind {
    Menu,
    Radial,
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
        // The two surface kinds, hosted as children so their #[live] shader
        // fields get configured from the DSL. Each paints nothing while closed.
        menu: MenuPopup{ width: Fill height: Fill }
        radial: RadialPopup{ width: Fill height: Fill }
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

    /// The linear card surface (in-window overlay via its own draw_walk).
    /// `#[redraw]` also gives the derive its required redraw/area source (no
    /// own `Draw*` holder here -- both children own their own).
    #[redraw]
    #[live]
    menu: MenuPopup,
    /// The wedge surface (Overlay-flow child, draws via draw_abs).
    #[redraw]
    #[live]
    radial: RadialPopup,

    /// The single active surface + its opaque tag, or none.
    #[rust]
    active: Option<(ActiveKind, LiveId)>,
}

impl Widget for PopupRoot {
    // Event-passive: `App` drives us via `route`, not tree routing.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Draw both surfaces; each paints nothing while closed (self-guarded).
        let _ = self.radial.draw_walk(cx, scope, walk);
        let _ = self.menu.draw_walk(cx, scope, walk);
        DrawStep::done()
    }
}

#[allow(dead_code)]
impl PopupRoot {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Open `spec`'s surface, superseding (dismissing) any currently-open popup
    /// first -- the single-active guarantee.
    pub fn show_at(&mut self, cx: &mut Cx, spec: PopupSpec) {
        // Supersede: reset the prior surface and emit its Dismissed close.
        if let Some((kind, tag)) = self.active.take() {
            match kind {
                ActiveKind::Menu => self.menu.reset(),
                ActiveKind::Radial => self.radial.reset(),
            }
            cx.widget_action(
                self.widget_uid(),
                PopupRootAction::Closed {
                    tag,
                    result: PopupResult::Dismissed,
                },
            );
        }
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
                match open {
                    MenuOpen::Press(press) => self.menu.open_marking(cx, placed, press, items),
                    MenuOpen::Popup => self.menu.open_popup(cx, placed, items),
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
                match open {
                    RadialOpen::Marking => self.radial.open_marking(cx, center, bounds, items, t),
                    RadialOpen::Popup => self.radial.open_popup(cx, center, bounds, items, t),
                }
                self.active = Some((ActiveKind::Radial, tag));
            }
        }
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
                ActiveKind::Menu => self.menu.handle(cx, ev),
                ActiveKind::Radial => self.radial.handle(cx, ev),
            };
            decide(verdict, is_primary_press(ev))
        };
        if let RouteStep::Close(result) = step {
            match kind {
                ActiveKind::Menu => self.menu.reset(),
                ActiveKind::Radial => self.radial.reset(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup::base::{PopupResult, PopupVerdict};

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
