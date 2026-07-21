### Task 6: `popup/root.rs` — `PopupRoot` authority widget

The dismiss authority. One widget hosting the two surface child-widgets (`menu: MenuPopup`, `radial: RadialPopup`) plus a one-slot `active: Option<(ActiveKind, LiveId)>`. `App` calls `route(cx, event)` once per event; openers call `show_at(cx, spec)` and read `closed(actions, tag)`. Single-active + universal light-dismiss live here and nowhere else.

The pure routing decision (`decide`) is unit-tested against verdicts directly; single-active + emission are verified end-to-end by driving the app in Task 7 (they require a live `Cx` + widget tree, so a stub-widget unit test isn't practical in this fork).

**Files:**
- Create: `crates/waml-editor/src/popup/root.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (uncomment `pub mod root;`)

**Interfaces:**
- Consumes: `crate::popup::base::{Popup, PopupItem, PopupResult, PopupVerdict, is_light_dismiss, is_primary_press}`; `crate::popup::menu::{MenuPopup, MENU_MAX_W, PAD_V, ROW_H}`; `crate::popup::radial::RadialPopup`; `crate::popup::presenter::Presenter`; `makepad_widgets::*`.
- Produces (Task 7 & 8 rely on these):
  - `pub enum PopupSpec { Menu { tag: LiveId, anchor: DVec2, bounds: Rect, items: Vec<PopupItem>, open: MenuOpen }, Radial { tag: LiveId, center: DVec2, bounds: Rect, items: Vec<PopupItem>, open: RadialOpen } }`
  - `pub enum MenuOpen { Press(DVec2), Popup }` · `pub enum RadialOpen { Marking, Popup }`
  - `pub enum PopupRootAction { None, Closed { tag: LiveId, result: PopupResult } }` — `#[derive(Clone, Debug, DefaultNone)]`.
  - `pub struct PopupRoot` (a `#[derive(Script, ScriptHook, Widget)]` widget) with:
    - `pub fn show_at(&mut self, cx: &mut Cx, spec: PopupSpec)`
    - `pub fn route(&mut self, cx: &mut Cx, event: &Event)`
    - `pub fn is_open(&self) -> bool`
    - `pub fn closed(&self, actions: &Actions, tag: LiveId) -> Option<PopupResult>`
  - DSL: `mod.widgets.PopupRoot` registered, hosting `menu: MenuPopup{..}` + `radial: RadialPopup{..}`.

---

- [ ] **Step 1: Write the failing tests for the pure routing decision**

Create `crates/waml-editor/src/popup/root.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup::base::{PopupResult, PopupVerdict};
    use makepad_widgets::*;

    #[test]
    fn a_commit_closes_with_its_result() {
        let step = decide(PopupVerdict::Closed(PopupResult::Invoked(live_id!(x))), false);
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
```

- [ ] **Step 2: Run to confirm failure**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::root`
Expected: FAIL — `decide` / `RouteStep` not found.

- [ ] **Step 3: Write the pure decision + the action/spec types above the test module**

```rust
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
pub enum MenuOpen {
    /// Press-open (marking): the press landed at this point (tap-vs-drag origin).
    Press(DVec2),
    /// Direct latched popup open (click-to-pick).
    Popup,
}

/// How to open the wedge.
pub enum RadialOpen {
    /// Right-press marking open.
    Marking,
    /// Direct latched popup open.
    Popup,
}

/// One `show_at` request. Carries the opaque `tag`, the kind's geometry, its
/// items, and its open-mode. (The plan's realization of the spec's `show_at` —
/// the surfaces are widget-hosted, so the kind's data rides in this enum.)
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
#[derive(Clone, Debug, DefaultNone)]
pub enum PopupRootAction {
    None,
    Closed { tag: LiveId, result: PopupResult },
}

/// Which surface is active. Pairs with the active tag in the slot. (The spec's
/// `PopupKind`; an enum discriminant, not a `Box<dyn>` — the surfaces are
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
```

- [ ] **Step 4: Run the pure tests to confirm they pass**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::root`
Expected: PASS (5 tests). (`DefaultNone` is the fork's derive for widget actions — confirm the import path matches how `InspectorAction` derives it; `inspector_panel.rs:158` uses a plain `#[derive(..., Default)] enum` with `#[default] None`, so if `DefaultNone` isn't in scope, use `#[derive(Clone, Debug, Default)]` + `#[default] None` on the `None` variant instead, matching the inspector.)

- [ ] **Step 5: Write the `PopupRoot` widget (struct + DSL + Widget impl + inherent methods)**

Add the DSL registration in a `script_mod!` block:

```rust
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
```

The struct + Widget impl + methods:

```rust
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
    #[live]
    menu: MenuPopup,
    /// The wedge surface (Overlay-flow child, draws via draw_abs).
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
    /// first — the single-active guarantee.
    pub fn show_at(&mut self, cx: &mut Cx, spec: PopupSpec) {
        // Supersede: reset the prior surface and emit its Dismissed close.
        if let Some((kind, tag)) = self.active.take() {
            match kind {
                ActiveKind::Menu => self.menu.reset(),
                ActiveKind::Radial => self.radial.reset(),
            }
            cx.widget_action(
                self.uid,
                PopupRootAction::Closed { tag, result: PopupResult::Dismissed },
            );
        }
        match spec {
            PopupSpec::Menu { tag, anchor, bounds, items, open } => {
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
            PopupSpec::Radial { tag, center, bounds, items, open } => {
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
            cx.widget_action(self.uid, PopupRootAction::Closed { tag, result });
            self.active = None;
        }
    }

    /// Read a close for `tag` from the action queue (the opener's filter).
    pub fn closed(&self, actions: &Actions, tag: LiveId) -> Option<PopupResult> {
        let item = actions.find_widget_action(self.uid)?;
        match item.cast::<PopupRootAction>() {
            PopupRootAction::Closed { tag: t, result } if t == tag => Some(result),
            _ => None,
        }
    }
}
```

Confirm against the fork:
- `#[uid] uid: WidgetUid` + `self.uid` for `cx.widget_action` — mirror how `RadialPopup`/`MenuPopup` (and the old widgets) declare the uid; if the derive exposes `self.widget_uid()` instead of a field, use that (the inspector uses `self.widget_uid()` at `inspector_panel.rs:730`). Use whichever the sibling widgets use.
- `item.cast::<PopupRootAction>()` — mirror `inspector_panel.rs:853-862`'s `item.cast()` pattern exactly (it infers the type from the match arm; you may not need the turbofish).
- Child `draw_walk` return handling — the surfaces' `draw_walk` return `DrawStep::done()`, so a single call each is enough (no `while ... .step()` loop needed, unlike a `View`). If either surface needs re-driving, wrap in `while self.menu.draw_walk(...).step().is_some() {}` matching `app_menu.rs:355-371`'s own single-pass return.

- [ ] **Step 6: Uncomment the module, build, run all popup tests**

In `mod.rs` uncomment `pub mod root;`. Run:
`taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup:: && cargo build -p waml-editor`
Expected: PASS (all `popup::*` tests) + clean build. `PopupRoot` unused until Task 7 — `#[allow(dead_code)]` covers the inherent block; the widget type itself is referenced by the DSL registration so should not warn.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/popup/root.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): PopupRoot authority widget (single-active + light-dismiss)

One widget hosting MenuPopup + RadialPopup + a one-slot active surface.
show_at supersedes any open popup (single-active); route runs light-dismiss,
dispatches to the active surface, and maps the verdict via the pure decide()
(commit/self-dismiss/outside-click). Emits PopupRootAction::Closed{tag,result};
openers read closed(actions, tag). decide() unit-tested; end-to-end verified
when wired in the App collapse.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
