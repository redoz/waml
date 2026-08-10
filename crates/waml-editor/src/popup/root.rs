//! `PopupRoot` — the dismiss authority. One widget, one active-surface slot,
//! universal light-dismiss. Hosts `MenuPopup` + `RadialPopup` as child widgets;
//! `App` calls `route` once per event and `show_at` to open. Single active
//! popup app-wide: `show_at` supersedes (dismisses) any open popup first.

use crate::cursor;
use crate::popup::base::{
    is_light_dismiss, is_primary_press, swallows_underlay, Popup, PopupItem, PopupResult,
    PopupVerdict,
};
use crate::popup::conflict_list::ConflictList;
use crate::popup::menu::{MenuPopup, MENU_MAX_W, PAD_V, ROW_H};
use crate::popup::palette::{PalettePopup, PaletteSectionModel};
use crate::popup::presenter::Presenter;
use crate::popup::radial::RadialPopup;
use crate::popup::select::{
    SelectFlyout, SelectItem, SELECT_BOTTOM_MARGIN, SELECT_MAX_H, SELECT_MAX_W,
};
use crate::scene::SceneConflict;
use makepad_widgets::*;

/// Palette card width cap (lpx) and its top offset from the popup bounds
/// (decision: "PopupSpec::Palette ... opens it centred-top like a command
/// palette", final look owed as a visual sign-off item).
const PALETTE_MAX_W: f64 = 560.0;
const PALETTE_TOP: f64 = 80.0;
const PALETTE_SIDE_MARGIN: f64 = 16.0;
const PALETTE_BOTTOM_MARGIN: f64 = 40.0;

/// How to open the linear card.
#[allow(dead_code)]
pub enum MenuOpen {
    /// Press-open (marking): the press landed at this point (tap-vs-drag origin).
    Press(DVec2),
    /// Direct latched popup open (click-to-pick).
    Popup {
        open_marking: Option<LiveId>,
        max_height: Option<f64>,
    },
    /// Checklist open: rows toggle and the card stays up until light-dismiss.
    Sticky { max_height: Option<f64> },
}

/// How to open the wedge.
#[allow(dead_code)]
pub enum RadialOpen {
    /// Right-press marking open.
    Marking,
    /// Drag-dwell open: marking driven by the PRIMARY button (the left-drag
    /// that opened it is still in flight), on an unsnapped full fan.
    Dial,
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
        compact_frame: bool,
    },
    Conflict {
        tag: LiveId,
        anchor: DVec2,
        bounds: Rect,
        conflicts: Vec<SceneConflict>,
    },
    /// The Ctrl+K command palette (decision 1). Opens centred-top in
    /// `bounds` -- unlike the other surfaces there is no button/press
    /// anchor, the palette is a global overlay.
    Palette {
        tag: LiveId,
        bounds: Rect,
        sections: Vec<PaletteSectionModel>,
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
    /// The armed (hovered) item changed while the surface is still open, so an
    /// opener can preview the choice before it commits. `id: None` = nothing
    /// armed. Only emitted on a CHANGE, never per pointer move.
    Armed {
        tag: LiveId,
        id: Option<LiveId>,
    },
    /// A sticky surface's row was toggled. The surface is STILL OPEN; the
    /// opener updates its own state and pushes fresh rows back via
    /// `MenuPopup::set_items`.
    Toggled {
        tag: LiveId,
        id: LiveId,
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
    Palette,
}

fn active_tag_is(active: Option<(ActiveKind, LiveId)>, tag: LiveId) -> bool {
    active.is_some_and(|(_, active_tag)| active_tag == tag)
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
            palette := PalettePopup{ width: Fill height: Fill }
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

    /// Last armed item reported for the active surface, so `route` emits
    /// `Armed` only on a change.
    #[rust]
    armed_slot: Option<LiveId>,
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

    /// Re-seed an open sticky menu's rows in place. A no-op unless `tag` is the
    /// currently-open menu surface -- an opener whose toggle raced a dismiss
    /// must not resurrect a closed card.
    pub fn set_menu_items(&mut self, cx: &mut Cx, tag: LiveId, items: Vec<PopupItem>) {
        if !active_tag_is(self.active, tag) {
            return;
        }
        if let Some(mut m) = self.body.widget(cx, ids!(menu)).borrow_mut::<MenuPopup>() {
            m.set_items(cx, items);
        }
    }

    /// Open `spec`'s surface, superseding (dismissing) any currently-open popup
    /// first -- the single-active guarantee.
    /// Reset whichever surface is active (if any) and emit its Dismissed
    /// close. Shared by `show_at`'s supersede-before-open and `close`'s
    /// dismiss-without-opening.
    fn dismiss_active(&mut self, cx: &mut Cx) {
        self.armed_slot = None;
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
                ActiveKind::Palette => {
                    if let Some(mut p) = self
                        .body
                        .widget(cx, ids!(palette))
                        .borrow_mut::<PalettePopup>()
                    {
                        p.reset();
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

    pub fn is_open_for(&self, tag: LiveId) -> bool {
        active_tag_is(self.active, tag)
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
                let full_height = PAD_V * 2.0 + items.len() as f64 * ROW_H;
                let menu_height = match &open {
                    MenuOpen::Popup {
                        max_height: Some(max),
                        ..
                    }
                    | MenuOpen::Sticky {
                        max_height: Some(max),
                    } => full_height.min(*max),
                    _ => full_height,
                };
                let size = dvec2(MENU_MAX_W, menu_height);
                let placed = Presenter::place(anchor, size, bounds);
                if let Some(mut m) = self.body.widget(cx, ids!(menu)).borrow_mut::<MenuPopup>() {
                    match open {
                        MenuOpen::Press(press) => m.open_marking(cx, placed, press, items),
                        MenuOpen::Popup {
                            open_marking,
                            max_height,
                        } => m.open_popup(cx, placed, items, open_marking, max_height),
                        MenuOpen::Sticky { max_height } => {
                            m.open_sticky(cx, placed, items, max_height)
                        }
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
                        RadialOpen::Dial => r.open_dial(cx, center, items, t),
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
                compact_frame,
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
                    s.open_select(
                        cx,
                        dvec2(anchor.x, placed.y),
                        min_width,
                        items,
                        compact_frame,
                    );
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
            PopupSpec::Palette {
                tag,
                bounds,
                sections,
            } => {
                let width = PALETTE_MAX_W
                    .min((bounds.size.x - PALETTE_SIDE_MARGIN * 2.0).max(0.0))
                    .max(0.0);
                let available_h = (bounds.size.y - PALETTE_TOP - PALETTE_BOTTOM_MARGIN).max(0.0);
                let height = crate::popup::palette::content_height(&sections).min(available_h);
                let left = bounds.pos.x + (bounds.size.x - width) * 0.5;
                let top = bounds.pos.y + PALETTE_TOP;
                let rect = Rect {
                    pos: dvec2(left, top),
                    size: dvec2(width, height),
                };
                if let Some(mut p) = self
                    .body
                    .widget(cx, ids!(palette))
                    .borrow_mut::<PalettePopup>()
                {
                    p.open_palette(cx, rect, available_h, sections);
                }
                self.active = Some((ActiveKind::Palette, tag));
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
        // Modality: while a popup owns the screen, swallow pointer events from
        // the rest of the widget tree beneath the overlay. `App` runs `route`
        // before `ui.handle_event`, so without this a row click -- or an
        // outside-click that dismisses -- would *also* hit-test the canvas node
        // under the card (its `hits()` skips any event whose `handled` is set).
        // `body` draws every frame, so its area is a valid non-empty capture
        // target by the time any surface is active. (See `swallows_underlay`
        // for why `MouseUp` is excluded.)
        if swallows_underlay(event) {
            let area = self.body.area();
            match event {
                Event::MouseDown(e) => e.handled.set(area),
                Event::MouseMove(e) => e.handled.set(area),
                _ => {}
            }
        }
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
                ActiveKind::Palette => self
                    .body
                    .widget(cx, ids!(palette))
                    .borrow_mut::<PalettePopup>()
                    .map(|mut p| p.handle(cx, ev))
                    .unwrap_or(PopupVerdict::Ignored),
            };
            // Cursor while a surface owns the screen. The widget the pointer
            // came from set the standing cursor (a canvas `Grab`, a button's
            // `Hand`) and gets no hover-out under the overlay, so the authority
            // has to state it every pointer move: `Hand` over an armed item,
            // released everywhere else on the surface.
            if matches!(ev, Event::MouseMove(_)) {
                let hovers = match kind {
                    ActiveKind::Menu => self
                        .body
                        .widget(cx, ids!(menu))
                        .borrow::<MenuPopup>()
                        .is_some_and(|m| m.hovers_item()),
                    ActiveKind::Radial => self
                        .body
                        .widget(cx, ids!(radial))
                        .borrow::<RadialPopup>()
                        .is_some_and(|r| r.hovers_item()),
                    ActiveKind::Select => self
                        .body
                        .widget(cx, ids!(select))
                        .borrow::<SelectFlyout>()
                        .is_some_and(|s| s.hovers_item()),
                    ActiveKind::Conflict => self
                        .body
                        .widget(cx, ids!(conflict))
                        .borrow::<ConflictList>()
                        .is_some_and(|c| c.hovers_item()),
                    ActiveKind::Palette => self
                        .body
                        .widget(cx, ids!(palette))
                        .borrow::<PalettePopup>()
                        .is_some_and(|p| p.hovers_item()),
                };
                if hovers {
                    cursor::hover_in(cx, MouseCursor::Hand);
                } else {
                    cursor::hover_out(cx);
                }
            }
            // Arm-change notification, emitted BEFORE any close so a preview
            // opener sees the last hover even on the committing event. Only the
            // radial reports an armed item today.
            if kind == ActiveKind::Radial {
                let armed = self
                    .body
                    .widget(cx, ids!(radial))
                    .borrow::<RadialPopup>()
                    .and_then(|r| r.armed_id());
                if armed != self.armed_slot {
                    self.armed_slot = armed;
                    cx.widget_action(self.widget_uid(), PopupRootAction::Armed { tag, id: armed });
                }
            }
            // A sticky menu commit toggles and stays open: `MenuPopup` cannot
            // emit its own tagged action (it doesn't know the opener's opaque
            // `tag`, only `PopupRoot` does), so it stashes the id and this
            // drains it right after `handle`.
            if kind == ActiveKind::Menu {
                let toggled = self
                    .body
                    .widget(cx, ids!(menu))
                    .borrow_mut::<MenuPopup>()
                    .and_then(|mut m| m.take_toggled());
                if let Some(id) = toggled {
                    cx.widget_action(self.widget_uid(), PopupRootAction::Toggled { tag, id });
                }
            }
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
                ActiveKind::Palette => {
                    if let Some(mut p) = self
                        .body
                        .widget(cx, ids!(palette))
                        .borrow_mut::<PalettePopup>()
                    {
                        p.reset();
                    }
                }
            }
            // The surface's `Hand` dies with it: the widget underneath gets no
            // hover-in from a pointer that never moved, so nothing else would
            // take the cursor back.
            cursor::hover_out(cx);
            cx.widget_action(self.widget_uid(), PopupRootAction::Closed { tag, result });
            self.armed_slot = None;
            self.active = None;
        }
    }

    /// Read a close from the action queue. The shell handles its own tags and
    /// relays the same tagged result to the active document.
    /// Scans ALL of our actions, not just the first: one event can emit an
    /// `Armed` and a `Closed` together (a commit reports its last hover first).
    pub fn closed_event(&self, actions: &Actions) -> Option<(LiveId, PopupResult)> {
        actions
            .filter_widget_actions(self.widget_uid())
            .find_map(|item| match item.cast() {
                PopupRootAction::Closed { tag, result } => Some((tag, result)),
                _ => None,
            })
    }

    /// Read an arm-change from the action queue. The outer `Option` is "was
    /// there a change"; the inner one is the armed id (`None` = the pointer
    /// left every item).
    pub fn armed_event(&self, actions: &Actions) -> Option<(LiveId, Option<LiveId>)> {
        actions
            .filter_widget_actions(self.widget_uid())
            .find_map(|item| match item.cast() {
                PopupRootAction::Armed { tag, id } => Some((tag, id)),
                _ => None,
            })
    }

    /// Read a sticky toggle from the action queue. The surface is still open;
    /// the opener updates its own state and re-seeds rows via
    /// `MenuPopup::set_items`.
    pub fn toggled_event(&self, actions: &Actions) -> Option<(LiveId, LiveId)> {
        actions
            .filter_widget_actions(self.widget_uid())
            .find_map(|item| match item.cast() {
                PopupRootAction::Toggled { tag, id } => Some((tag, id)),
                _ => None,
            })
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

    /// Read the palette's live query-text change, if `tag` matches the open
    /// palette. The opener re-queries `SearchState` and pushes fresh rows
    /// back via `set_palette_sections` (Task 11).
    pub fn palette_query_changed(
        &self,
        cx: &mut Cx,
        tag: LiveId,
        actions: &Actions,
    ) -> Option<String> {
        if !active_tag_is(self.active, tag) {
            return None;
        }
        self.body
            .widget(cx, ids!(palette))
            .borrow::<PalettePopup>()?
            .query_changed(actions)
    }

    /// Re-seed an open palette's rows in place (a live-query re-run). A
    /// no-op unless `tag` is the currently-open palette -- mirrors
    /// `set_menu_items`.
    pub fn set_palette_sections(
        &mut self,
        cx: &mut Cx,
        tag: LiveId,
        sections: Vec<PaletteSectionModel>,
    ) {
        if !active_tag_is(self.active, tag) {
            return;
        }
        if let Some(mut p) = self
            .body
            .widget(cx, ids!(palette))
            .borrow_mut::<PalettePopup>()
        {
            p.set_sections(cx, sections);
        }
    }

    /// Every row title the live palette widget is currently holding, via the
    /// SAME `self.body.widget(cx, ids!(palette))` lookup every other
    /// `PopupRoot` method uses -- for App-level tests asserting a re-query
    /// reached the actual widget, not just `App`'s own mirror (Task 11).
    #[cfg(test)]
    pub(crate) fn test_palette_row_titles(&self, cx: &mut Cx) -> Vec<String> {
        self.body
            .widget(cx, ids!(palette))
            .borrow::<PalettePopup>()
            .map(|p| p.test_row_titles())
            .unwrap_or_default()
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

    #[test]
    fn active_tag_query_distinguishes_the_open_surface_owner() {
        let active = Some((ActiveKind::Menu, live_id!(doc_switcher)));
        assert!(active_tag_is(active, live_id!(doc_switcher)));
        assert!(!active_tag_is(active, live_id!(logo)));
        assert!(!active_tag_is(None, live_id!(doc_switcher)));
    }
}
