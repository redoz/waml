# Responsive Viewport Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `waml-editor` one hysteretic wide/narrow chrome mode so a ~390px viewport keeps its caption controls, documents, canvas, and dock panels usable without changing document or panel state.

**Architecture:** `App` owns the single `narrow: bool` and reconciles it in `sync_dock_slots`, while pure helpers determine the next breakpoint state and the slot/body widths derived from the existing panel `DockState`s. `DocTabs` remains the document renderer and emits a narrow-chip switcher request; the existing `PopupRoot`/`MenuPopup` presents the open documents. The panel widgets stay single-instanced in full-body overlay hosts: wide mode reserves matching side slots, while narrow mode zeros those reservations and caps the same hosts to the viewport.

**Tech Stack:** Rust 2024 workspace, Makepad widgets/live DSL, existing `PopupRoot`/`MenuPopup`, inline Rust unit tests, native Windows `PrintWindow`/pid-scoped input verification, and a headless Playwright probe of the Makepad wasm build.

**Source spec:** `docs/superpowers/specs/2026-07-25-narrow-viewport-chrome-design.md`

## Global Constraints

- There are exactly two width-driven modes. Enter narrow only when `viewport_width < 640.0`; leave narrow only when `viewport_width > 680.0`; preserve the current mode for every width in `[640.0, 680.0]`.
- `App` owns one `narrow: bool`; do not introduce separate caption-mode, tree-open, or inspector-open booleans.
- `ProjectTree::dock_state()` and `Inspector::dock_state()` remain the panel state authority. `DockState::Pinned` is open and `DockState::Flag` is closed for this feature.
- Wide mode reserves the normal 280px tree and 320px inspector widths and permits both panels. Narrow mode reserves `0.0` on both sides, caps each drawn panel to the viewport, and permits only one open panel.
- Narrowing with both panels open keeps the tree and closes the inspector. Widening preserves the surviving state.
- Resizing changes presentation only: it must not open, close, promote, remove, reorder, or otherwise mutate documents.
- Keep one `LogoMark`, sized `44.0 × 25.0`, inside `title_row`; the start screen retains it while existing editor-only controls remain hidden.
- The caption remains 66px and becomes one full-width two-row column. The tab-row top rule begins at `x = 0` in both modes and retains the current conditional `[I]` overshoot on the right.
- Narrow `DocTabs` shows only the active document, caps its chip at 320px, emits the existing `Close(active_id)` from `x`, and emits a switcher request from the chip body. No active document draws no chip and emits no request.
- The switcher uses the shared `PopupRoot` and existing `PopupItem` shape under `live_id!(doc_switcher)`; it preserves open-tab order, marks the active item, and selection follows the same `refresh_doc_tabs` + `sync_active_tab` path as a wide tab click.
- A mode change dismisses only an open `doc_switcher`; it must not dismiss unrelated popup surfaces.
- In narrow mode, opening either dock closes the other, including a view-side right-dock open request. A primary canvas press outside the visible panel closes it; an inside press must not reach the canvas.
- Do not change statusbar/tool-dock behavior, wide tab overflow behavior, document lifecycle/persistence semantics, touch gestures, desktop window buttons, or unrelated popup sizing.
- Add no dependencies. Keep tests inline under `#[cfg(test)]`; `waml-editor` is a binary crate and has no `--lib` test target.
- Every shell command in this plan is prefixed with `rtk`, per `RTK.md`. Use pid-scoped launch, capture, and shutdown; never stop every `waml-editor` process by name.
- Commit messages use the repository's conventional style and each implementation task ends in a reviewable commit.

---

## File Structure

| File | Responsibility in this change |
| --- | --- |
| `crates/waml-editor/src/popup/menu.rs` | Allow a latched menu to show an opening-row mark and opt into the already-modelled bounded scroll/thumb behavior. |
| `crates/waml-editor/src/popup/root.rs` | Carry the optional menu mark/max-height through `PopupSpec`, expose tag-specific open state, and keep one popup authority. |
| `crates/waml-editor/src/doc_tabs.rs` | Render wide tab runs versus the single narrow active chip and emit `OpenSwitcher { anchor }`. |
| `crates/waml-editor/src/dock.rs` | Pure responsive slot/body derivation, deterministic narrow-entry reconciliation, and narrow mutual-exclusion transitions. |
| `crates/waml-editor/src/tree_panel.rs` | Export the canonical 280px body width and add symmetric open/close/drawn-rect accessors. |
| `crates/waml-editor/src/inspector_panel.rs` | Export the canonical 320px body width and expose the panel's last-drawn rect. |
| `crates/waml-editor/src/app.rs` | Rebuild the caption/body hierarchy, own hysteresis, size reservations/overlay hosts, route dock interactions, and wire the document switcher. |

The panel widgets are deliberately not duplicated. `left_slot`/`right_slot` become reservation-only children of `dock_row`; `project_tree` and `inspector` each live once in later full-body overlay layers. In wide mode their host widths equal the matching reservations, so the overlay paints over reserved chrome. In narrow mode reservations become zero while the same hosts remain left/right aligned over the full-width center.

---

### Task 1: Marked, Bounded Latched Menus

**Files:**
- Modify: `crates/waml-editor/src/popup/menu.rs:38-205, 310-575`
- Modify: `crates/waml-editor/src/popup/root.rs:17-72, 188-310`
- Modify: `crates/waml-editor/src/app.rs:2110-2130, 2224-2244, 2360-2380, 2545-2565`

**Interfaces:**
- Consumes: `PopupItem`, `LinearGeom::set_max_height(Option<f64>)`, `LinearGeom::thumb_rect()`, `LinearGeom::scroll_for_thumb_y(f64)`, and `PopupRoot::show_at(&mut self, &mut Cx, PopupSpec)`.
- Produces: `MenuOpen::Popup { open_marking: Option<LiveId>, max_height: Option<f64> }`, `MenuPopup::open_popup(&mut self, &mut Cx, DVec2, Vec<PopupItem>, Option<LiveId>, Option<f64>)`, and `PopupRoot::is_open_for(&self, LiveId) -> bool`.
- Invariant: `open_marking` is a persistent visual mark only. A hovered/pressed row temporarily wins, and committing still comes exclusively from `MarkingCore`.

- [ ] **Step 1: Write the failing menu-mark and tag-specific-open tests**

Add these tests to `popup/menu.rs`'s existing `tests` module:

```rust
#[test]
fn opening_mark_shows_only_until_hover_selects_another_row() {
    let active = live_id!(active_doc);
    assert!(row_is_marked(None, Some(active), 1, active));
    assert!(!row_is_marked(None, Some(active), 0, live_id!(other_doc)));
    assert!(!row_is_marked(Some(0), Some(active), 1, active));
    assert!(row_is_marked(
        Some(0),
        Some(active),
        0,
        live_id!(other_doc)
    ));
}

#[test]
fn bounded_menu_geometry_exposes_scroll_and_a_thumb() {
    let mut g = LinearGeom::new(ANCHOR, 30);
    g.set_width(TEST_W);
    g.set_max_height(Some(320.0));
    assert_eq!(g.panel_height(), 320.0);
    assert!(g.max_scroll() > 0.0);
    assert!(g.thumb_rect().is_some());
}
```

Add this test to `popup/root.rs`'s existing test module:

```rust
#[test]
fn active_tag_query_distinguishes_the_open_surface_owner() {
    let active = Some((ActiveKind::Menu, live_id!(doc_switcher)));
    assert!(active_tag_is(active, live_id!(doc_switcher)));
    assert!(!active_tag_is(active, live_id!(logo)));
    assert!(!active_tag_is(None, live_id!(doc_switcher)));
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `rtk cargo test -p waml-editor opening_mark_shows_only -- --nocapture`

Expected: FAIL because `row_is_marked` does not exist.

Run: `rtk cargo test -p waml-editor active_tag_query_distinguishes -- --nocapture`

Expected: FAIL because `active_tag_is` does not exist.

- [ ] **Step 3: Add the optional opening mark and bounded menu state**

In `popup/menu.rs`, add:

```rust
fn row_is_marked(
    hovered: Option<usize>,
    open_marking: Option<LiveId>,
    index: usize,
    item_id: LiveId,
) -> bool {
    match hovered {
        Some(hovered) => hovered == index,
        None => open_marking == Some(item_id),
    }
}
```

Add these fields to `MenuPopup`:

```rust
#[redraw]
#[live]
draw_scrollbar: DrawColor,
#[rust]
open_marking: Option<LiveId>,
#[rust]
thumb_drag: Option<f64>,
```

Set `draw_scrollbar: mod.draw.DrawColor{ color: atlas.accent }` beside `draw_hover` in the DSL. Replace `open_marking` with:

```rust
pub fn open_marking(
    &mut self,
    cx: &mut Cx,
    anchor: DVec2,
    press: DVec2,
    items: Vec<PopupItem>,
) {
    self.geom = LinearGeom::new(anchor, items.len());
    self.open_marking = None;
    self.thumb_drag = None;
    self.mark.begin_marking(press, items, DRAG_THRESHOLD);
    self.draw_frame.redraw(cx);
}
```

Replace `open_popup` with:

```rust
pub fn open_popup(
    &mut self,
    cx: &mut Cx,
    anchor: DVec2,
    items: Vec<PopupItem>,
    open_marking: Option<LiveId>,
    max_height: Option<f64>,
) {
    self.geom = LinearGeom::new(anchor, items.len());
    self.geom.set_max_height(max_height);
    self.open_marking = open_marking;
    self.thumb_drag = None;
    self.mark.begin_popup(items, DRAG_THRESHOLD);
    self.draw_frame.redraw(cx);
}
```

In `draw`, compute one persistent-or-hovered verdict per row and clip the rows:

```rust
let clip = Rect {
    pos: dvec2(panel.pos.x, panel.pos.y + PAD_V),
    size: dvec2(panel.size.x, self.geom.viewport_height()),
};
cx.push_clip_rect(clip);
for (i, it) in items.iter().enumerate() {
    let row = self.geom.row_rect(i);
    let marked = row_is_marked(hovered, self.open_marking, i, it.id);
    // Retain the existing divider, fill, icon, and label statements here.
    // Replace both `hovered == Some(i)` visual checks with `marked`.
}
cx.pop_clip_rect();
if let Some(thumb) = self.geom.thumb_rect() {
    self.draw_scrollbar.draw_abs(cx, thumb);
}
```

Copy the bounded input mechanics from `SelectFlyout::handle` without changing unbounded menus:

```rust
Event::Scroll(e) if self.geom.panel_rect().contains(e.abs) => {
    let prev = self.geom.scroll();
    self.geom.set_scroll(prev + e.scroll.y);
    e.handled_x.set(true);
    e.handled_y.set(true);
    if self.geom.scroll() != prev {
        self.draw_frame.redraw(cx);
    }
    PopupVerdict::Consumed
}
Event::MouseMove(e) => {
    if let Some(grab) = self.thumb_drag {
        self.geom
            .set_scroll(self.geom.scroll_for_thumb_y(e.abs.y - grab));
    } else {
        self.mark.pointer_move(e.abs, self.geom.row_at(e.abs));
    }
    self.draw_frame.redraw(cx);
    PopupVerdict::Consumed
}
Event::MouseUp(e) if e.button.is_primary() => {
    if self.thumb_drag.take().is_some() {
        PopupVerdict::Consumed
    } else {
        map_outcome(self.mark.release(self.geom.row_at(e.abs)))
    }
}
```

In the popup-mode `MouseDown` branch, test the thumb before the card body:

```rust
if let Some(thumb) = self.geom.thumb_rect() {
    if thumb.contains(e.abs) {
        self.thumb_drag = Some(e.abs.y - thumb.pos.y);
        e.handled.set(self.draw_frame.area());
        return PopupVerdict::Consumed;
    }
}
```

Replace `reset` with:

```rust
fn reset(&mut self) {
    self.thumb_drag = None;
    self.open_marking = None;
    self.mark.close();
}
```

- [ ] **Step 4: Carry the options through `PopupRoot`**

Change the latched variant to:

```rust
pub enum MenuOpen {
    Press(DVec2),
    Popup {
        open_marking: Option<LiveId>,
        max_height: Option<f64>,
    },
}
```

Add the pure helper and public query:

```rust
fn active_tag_is(active: Option<(ActiveKind, LiveId)>, tag: LiveId) -> bool {
    active.is_some_and(|(_, active_tag)| active_tag == tag)
}

pub fn is_open_for(&self, tag: LiveId) -> bool {
    active_tag_is(self.active, tag)
}
```

In the `PopupSpec::Menu` arm, clamp the height before `Presenter::place` and pass both options:

```rust
let full_height = PAD_V * 2.0 + items.len() as f64 * ROW_H;
let menu_height = match &open {
    MenuOpen::Popup {
        max_height: Some(max),
        ..
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
    }
}
```

At the four existing `MenuOpen::Popup` call sites in `app.rs`, preserve current behavior with:

```rust
open: MenuOpen::Popup {
    open_marking: None,
    max_height: None,
},
```

- [ ] **Step 5: Run popup and editor tests**

Run: `rtk cargo test -p waml-editor popup::menu::tests -- --nocapture`

Expected: PASS, including the new opening-mark and bounded-geometry tests.

Run: `rtk cargo test -p waml-editor popup::root::tests -- --nocapture`

Expected: PASS, including tag-specific ownership and all existing routing tests.

Run: `rtk cargo test -p waml-editor`

Expected: PASS; every old menu remains unmarked and unbounded.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/waml-editor/src/popup/menu.rs crates/waml-editor/src/popup/root.rs crates/waml-editor/src/app.rs
rtk git commit -m "feat(popup): support marked bounded menus"
```

---

### Task 2: Narrow Active-Document Chip

**Files:**
- Modify: `crates/waml-editor/src/doc_tabs.rs:352-1010`

**Interfaces:**
- Consumes: `OpenTabs`, `DocTab`, the existing tab/close rect capture, and `DocTabsAction::{Activate, Promote, Close}`.
- Produces: `DocTabsAction::OpenSwitcher { anchor: DVec2 }` and `DocTabs::set_narrow(&mut self, &mut Cx, bool)`.
- Geometry: wide titles use 18 characters; narrow titles use 36; a narrow chip fills the remaining strip up to `NARROW_CHIP_MAX_W = 320.0`.

- [ ] **Step 1: Write failing pure interaction and geometry tests**

Derive `PartialEq` on `DocTabsAction`, then add:

```rust
#[test]
fn narrow_range_contains_only_the_active_tab_or_nothing() {
    let mut open = OpenTabs::diagram_base("d", "Diagram");
    let active = open.open_preview("customer", "Customer", TreeKind::Class);
    let range = visible_tab_range(&open.tabs, active, true);
    assert_eq!(&open.tabs[range], &open.tabs[1..2]);
    assert_eq!(
        visible_tab_range(&open.tabs, live_id!(missing), true),
        0..0
    );
    assert_eq!(visible_tab_range(&open.tabs, active, false), 0..2);
}

#[test]
fn narrow_body_requests_switcher_but_close_stays_close() {
    let tab = DocTab {
        id: live_id!(customer),
        key: "customer".into(),
        title: "Customer".into(),
        kind: TabKind::Classifier,
        node_kind: TreeKind::Class,
        preview: true,
    };
    let rect = Rect {
        pos: dvec2(32.0, 34.0),
        size: dvec2(250.0, 32.0),
    };
    assert_eq!(
        release_action(true, &tab, rect, false),
        DocTabsAction::OpenSwitcher {
            anchor: dvec2(32.0, 66.0)
        }
    );
    assert_eq!(
        release_action(true, &tab, rect, true),
        DocTabsAction::Close(tab.id)
    );
    assert_eq!(
        release_action(false, &tab, rect, false),
        DocTabsAction::Promote(tab.id)
    );
}

#[test]
fn narrow_chip_fills_available_width_up_to_320() {
    assert_eq!(tab_width(true, 140.0, 250.0), 250.0);
    assert_eq!(tab_width(true, 140.0, 500.0), 320.0);
    assert_eq!(tab_width(false, 140.0, 500.0), 140.0);
}

#[test]
fn top_rule_starts_at_zero_in_wide_and_narrow_geometry() {
    assert_eq!(rule_x_start(280.0, 280.0), 0.0);
    assert_eq!(rule_x_start(32.0, 32.0), 0.0);
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `rtk cargo test -p waml-editor narrow_range_contains_only -- --nocapture`

Expected: FAIL because `visible_tab_range` does not exist.

Run: `rtk cargo test -p waml-editor narrow_body_requests_switcher -- --nocapture`

Expected: FAIL because the new action/helper do not exist.

- [ ] **Step 3: Add the pure helpers and action**

Add:

```rust
use std::ops::Range;

const WIDE_MAX_TITLE_CHARS: usize = 18;
const NARROW_MAX_TITLE_CHARS: usize = 36;
const NARROW_CHIP_MAX_W: f64 = 320.0;

fn visible_tab_range(tabs: &[DocTab], active: LiveId, narrow: bool) -> Range<usize> {
    if !narrow {
        return 0..tabs.len();
    }
    tabs.iter()
        .position(|tab| tab.id == active)
        .map(|index| index..index + 1)
        .unwrap_or(0..0)
}

fn tab_width(narrow: bool, natural: f64, available: f64) -> f64 {
    if narrow {
        available.max(0.0).min(NARROW_CHIP_MAX_W)
    } else {
        natural
    }
}

fn rule_x_start(strip_left: f64, left_overshoot: f64) -> f64 {
    (strip_left - left_overshoot).round()
}
```

Change `truncate_title` to accept `max_chars: usize`. Extend the action:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub enum DocTabsAction {
    #[default]
    None,
    Activate(LiveId),
    Promote(LiveId),
    Close(LiveId),
    OpenSwitcher { anchor: DVec2 },
}

fn release_action(
    narrow: bool,
    tab: &DocTab,
    tab_rect: Rect,
    close_hit: bool,
) -> DocTabsAction {
    if close_hit {
        DocTabsAction::Close(tab.id)
    } else if narrow {
        DocTabsAction::OpenSwitcher {
            anchor: dvec2(tab_rect.pos.x, tab_rect.pos.y + tab_rect.size.y),
        }
    } else if tab.preview {
        DocTabsAction::Promote(tab.id)
    } else {
        DocTabsAction::Activate(tab.id)
    }
}
```

Add `#[rust] narrow: bool` to `DocTabs`.

- [ ] **Step 4: Fork drawing and release behavior without duplicating document state**

At the start of the tab draw loop:

```rust
let visible = visible_tab_range(&self.tabs, self.active, self.narrow);
let mut x = rect.pos.x + self.lead_inset;
for tab in &self.tabs[visible] {
    let max_chars = if self.narrow {
        NARROW_MAX_TITLE_CHARS
    } else {
        WIDE_MAX_TITLE_CHARS
    };
    let title = truncate_title(&tab.title, max_chars);
    let natural_w = lead + text_w + CLOSE_GAP + CLOSE_W;
    let available = rect.pos.x + rect.size.x - x;
    let w = tab_width(self.narrow, natural_w, available);
}
```

This is a local replacement inside the existing loop: retain the current `lead`, `text_w`, `text_h`, card/icon/title/preview/close drawing statements byte-for-byte, rename the current computed `w` to `natural_w`, and insert the `available`/`tab_width` assignments before `tab_rect` is built.

Replace the release loops with a close-first lookup followed by one `release_action` call. In narrow mode the visible range guarantees the only tab rect belongs to the active document; with no active tab, both rect vectors are empty:

```rust
let close_id = self.close_at(fe.abs);
if close_id != LiveId::default() {
    if let Some(tab) = self.tabs.iter().find(|tab| tab.id == close_id) {
        cx.widget_action(uid, release_action(self.narrow, tab, self.tab_rect(close_id), true));
    }
    return;
}
let id = self.tab_at(fe.abs);
if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) {
    let rect = self.tab_rect(id);
    cx.widget_action(uid, release_action(self.narrow, tab, rect, false));
}
```

Add private `tab_rect(&self, id: LiveId) -> Rect` alongside `tab_at`, returning the recorded rect or `Rect::default()`.

Use `rule_x_start(rect.pos.x, self.left_overshoot)` for the top-rule `x0`. Add:

```rust
pub fn set_narrow(&mut self, cx: &mut Cx, narrow: bool) {
    if self.narrow != narrow {
        self.narrow = narrow;
        self.hovered = LiveId::default();
        self.close_hovered = LiveId::default();
        self.pressed = LiveId::default();
        cx.redraw_all();
    }
}
```

- [ ] **Step 5: Run the focused and full tests**

Run: `rtk cargo test -p waml-editor doc_tabs::tests -- --nocapture`

Expected: PASS, including chip width, active-only range, switcher-vs-close action, existing preview promotion, and both top-rule endpoints.

Run: `rtk cargo test -p waml-editor`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/waml-editor/src/doc_tabs.rs
rtk git commit -m "feat(chrome): add narrow document chip"
```

---

### Task 3: Pure Responsive Dock Derivation and Panel APIs

**Files:**
- Modify: `crates/waml-editor/src/dock.rs:18-165, 167-310`
- Modify: `crates/waml-editor/src/tree_panel.rs:30-55, 919-960`
- Modify: `crates/waml-editor/src/inspector_panel.rs:389-397, 974-1020`

**Interfaces:**
- Consumes: `DockState`, `DockEvent::Toggle`, and `DockEdge`.
- Produces: `ResponsiveDockLayout`, `responsive_layout(bool, f64, DockState, DockState, f64, f64)`, `narrow_entry_states(DockState, DockState)`, and `narrow_toggle_states(DockState, DockState, DockEdge)`.
- Produces panel APIs: `PROJECT_TREE_W`, `INSPECTOR_W`, `open_dock`, `close_dock`, and `drawn_rect`.

- [ ] **Step 1: Write the failing responsive-dock tests**

Append to `dock.rs` tests:

```rust
#[test]
fn wide_and_narrow_layout_use_the_same_dock_states() {
    let wide = responsive_layout(false, 900.0, DockState::Pinned, DockState::Pinned, 280.0, 320.0);
    assert_eq!(
        wide,
        ResponsiveDockLayout {
            left_slot: 280.0,
            right_slot: 320.0,
            tree_body: 280.0,
            inspector_body: 320.0,
        }
    );
    let narrow = responsive_layout(true, 390.0, DockState::Pinned, DockState::Flag, 280.0, 320.0);
    assert_eq!(
        narrow,
        ResponsiveDockLayout {
            left_slot: 0.0,
            right_slot: 0.0,
            tree_body: 280.0,
            inspector_body: 0.0,
        }
    );
}

#[test]
fn narrow_body_width_is_capped_to_the_viewport() {
    let layout = responsive_layout(true, 240.0, DockState::Pinned, DockState::Flag, 280.0, 320.0);
    assert_eq!(layout.tree_body, 240.0);
}

#[test]
fn entering_narrow_with_two_open_docks_keeps_tree() {
    assert_eq!(
        narrow_entry_states(DockState::Pinned, DockState::Pinned),
        (DockState::Pinned, DockState::Flag)
    );
    assert_eq!(
        narrow_entry_states(DockState::Flag, DockState::Pinned),
        (DockState::Flag, DockState::Pinned)
    );
}

#[test]
fn narrow_toggles_are_mutually_exclusive() {
    assert_eq!(
        narrow_toggle_states(DockState::Flag, DockState::Pinned, DockEdge::Left),
        (DockState::Pinned, DockState::Flag)
    );
    assert_eq!(
        narrow_toggle_states(DockState::Pinned, DockState::Flag, DockEdge::Right),
        (DockState::Flag, DockState::Pinned)
    );
    assert_eq!(
        narrow_toggle_states(DockState::Pinned, DockState::Flag, DockEdge::Left),
        (DockState::Flag, DockState::Flag)
    );
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `rtk cargo test -p waml-editor dock::tests::wide_and_narrow_layout -- --nocapture`

Expected: FAIL because `responsive_layout` and `ResponsiveDockLayout` do not exist.

- [ ] **Step 3: Implement the pure derivations**

Add to `dock.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResponsiveDockLayout {
    pub left_slot: f64,
    pub right_slot: f64,
    pub tree_body: f64,
    pub inspector_body: f64,
}

pub fn responsive_layout(
    narrow: bool,
    viewport_w: f64,
    tree: DockState,
    inspector: DockState,
    tree_w: f64,
    inspector_w: f64,
) -> ResponsiveDockLayout {
    let cap = viewport_w.max(0.0);
    let tree_body = if tree == DockState::Pinned {
        if narrow { tree_w.min(cap) } else { tree_w }
    } else {
        0.0
    };
    let inspector_body = if inspector == DockState::Pinned {
        if narrow {
            inspector_w.min(cap)
        } else {
            inspector_w
        }
    } else {
        0.0
    };
    ResponsiveDockLayout {
        left_slot: if narrow { 0.0 } else { tree_body },
        right_slot: if narrow { 0.0 } else { inspector_body },
        tree_body,
        inspector_body,
    }
}

pub fn narrow_entry_states(
    tree: DockState,
    inspector: DockState,
) -> (DockState, DockState) {
    if tree == DockState::Pinned && inspector == DockState::Pinned {
        (tree, DockState::Flag)
    } else {
        (tree, inspector)
    }
}

pub fn narrow_toggle_states(
    tree: DockState,
    inspector: DockState,
    edge: DockEdge,
) -> (DockState, DockState) {
    match edge {
        DockEdge::Left => {
            let next_tree = next(tree, DockEvent::Toggle);
            if next_tree == DockState::Pinned {
                (next_tree, DockState::Flag)
            } else {
                (next_tree, inspector)
            }
        }
        DockEdge::Right => {
            let next_inspector = next(inspector, DockEvent::Toggle);
            if next_inspector == DockState::Pinned {
                (DockState::Flag, next_inspector)
            } else {
                (tree, next_inspector)
            }
        }
    }
}
```

- [ ] **Step 4: Expose symmetric panel state/geometry APIs**

In `tree_panel.rs`, define `pub(crate) const PROJECT_TREE_W: f64 = 280.0;`, use it in `slot_width`, and add:

```rust
pub fn open_dock(&mut self, cx: &mut Cx) {
    self.apply_dock(cx, DockEvent::Open);
}

pub fn close_dock(&mut self, cx: &mut Cx) {
    self.apply_dock(cx, DockEvent::Close);
}

pub fn drawn_rect(&self, cx: &Cx) -> Rect {
    self.view.area().rect(cx)
}
```

Remove the obsolete `#[allow(dead_code)]` from `dock_state`.

In `inspector_panel.rs`, change `INSPECTOR_W` to `pub(crate) const INSPECTOR_W: f64 = 320.0;` and add:

```rust
pub fn drawn_rect(&self, cx: &Cx) -> Rect {
    self.view.area().rect(cx)
}
```

Keep `dock_state`, `open_dock`, `close_dock`, and `slot_width` otherwise unchanged.

- [ ] **Step 5: Run dock and panel tests**

Run: `rtk cargo test -p waml-editor dock::tests -- --nocapture`

Expected: PASS, including breakpoint-independent width derivation, viewport clamping, tree-wins entry, and mutual exclusion.

Run: `rtk cargo test -p waml-editor tree_panel::tests -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor inspector_panel::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add crates/waml-editor/src/dock.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/inspector_panel.rs
rtk git commit -m "feat(chrome): derive responsive dock layout"
```

---

### Task 4: Full-Width Caption, Shared Breakpoint, and Overlay Hosts

**Files:**
- Modify: `crates/waml-editor/src/app.rs:42-514, 516-635, 852-1080, 1327-1450, 2338-2380, 2825-2905, 2908-2965`

**Interfaces:**
- Consumes: `DocTabs::set_narrow`, `ResponsiveDockLayout`, `responsive_layout`, `narrow_entry_states`, canonical panel widths, and the existing `sync_tree_gap`.
- Produces: `App::narrow: bool`, `next_narrow(bool, f64) -> bool`, and a single `sync_dock_slots` path that applies mode, reservations, overlay-host widths, button lights, tree gap, and one relayout.
- Layout ids produced by the DSL: `left_slot`, `right_slot`, `tree_host`, `inspector_host`, `project_tree`, and `inspector`.

- [ ] **Step 1: Write the failing breakpoint tests**

In `app.rs` tests, replace the existing `super` import with:

```rust
use super::{logo_command_for, next_narrow, place_rm_for, LogoCommand};
```

Then add:

```rust
#[test]
fn breakpoint_enters_below_640_and_leaves_above_680() {
    assert!(next_narrow(false, 639.9));
    assert!(next_narrow(true, 680.0));
    assert!(!next_narrow(true, 680.1));
}

#[test]
fn breakpoint_preserves_mode_through_the_hysteresis_band() {
    for width in [640.0, 650.0, 680.0] {
        assert!(!next_narrow(false, width));
        assert!(next_narrow(true, width));
    }
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `rtk cargo test -p waml-editor breakpoint_ -- --nocapture`

Expected: FAIL because `next_narrow` does not exist.

- [ ] **Step 3: Add the exact hysteresis helper and App fields**

Near `TREE_BTN_W`, add:

```rust
const NARROW_ENTER_W: f64 = 640.0;
const NARROW_EXIT_W: f64 = 680.0;

fn next_narrow(narrow: bool, viewport_w: f64) -> bool {
    if narrow {
        viewport_w <= NARROW_EXIT_W
    } else {
        viewport_w < NARROW_ENTER_W
    }
}
```

Replace `dock_slot_w` with:

```rust
#[rust]
narrow: bool,
#[rust]
dock_layout: crate::dock::ResponsiveDockLayout,
```

Keep `tree_gap_w` and `rule_overshoot`.

- [ ] **Step 4: Replace the caption hierarchy**

In the caption DSL:

1. Delete the complete `wordmark := View` subtree currently at `app.rs:78-88`.
2. Make `caption_bar` a one-child full-width container whose child is `caption_col`.
3. Keep `caption_col` `width: Fill`, `height: Fill`, `flow: Down`, and `clip_x: false`.
4. Keep `agent_mark` as the first, zero-width `title_row` child.
5. Insert exactly one `logo := LogoMark{ width: 44.0 height: 25.0 }` immediately after `agent_mark`, before `menu_btn`.
6. Preserve `menu_btn(30)`, `model_name(Fill)`, and `windows_buttons` after it.
7. Keep `tree_btn` first and `inspector_btn` last in `tab_row`.
8. Rewrite the surrounding comments to describe the actual nesting and `WindowDragQuery` client-area overrides; remove the obsolete direct-caption-child claim.

The resulting live-tree shape must be:

```text
caption_bar
└─ caption_col (Fill × Fill, Down)
   ├─ title_row (34)
   │  ├─ agent_mark (0w)
   │  ├─ logo (44 × 25)
   │  ├─ menu_btn (30)
   │  ├─ model_name (Fill)
   │  └─ windows_buttons
   └─ tab_row (Fill)
      ├─ tree_btn (30 + 2 left margin)
      ├─ tree_gap
      ├─ doc_tabs (Fill)
      └─ inspector_btn (30 + 2 right margin)
```

Change the logo menu anchor to match the burger's upper-row geometry, removing the `CAPTION_H` clamp:

```rust
let anchor = dvec2(
    logo_rect.pos.x,
    logo_rect.pos.y + logo_rect.size.y + crate::popup::menu::MENU_GAP,
);
```

- [ ] **Step 5: Separate panel hosts from reservation slots**

Keep `dock_body` as `flow: Overlay`. Its first child remains `dock_row` with three layout children, but make `left_slot` and `right_slot` empty reservation views. After `dock_row`, add the panel layers in paint order:

```text
dock_body (Overlay)
├─ dock_row (Right)
│  ├─ left_slot (runtime width)
│  ├─ center_stack (Fill; existing canvas/source/HUD children unchanged)
│  └─ right_slot (runtime width)
├─ tree_layer (Fill, align left)
│  └─ tree_host (runtime width)
│     └─ project_tree (Fill × Fill)
└─ inspector_layer (Fill, align right)
   └─ inspector_host (runtime width)
      └─ inspector (Fill × Fill)
```

Use this exact DSL shell:

```rust
tree_layer := View{
    width: Fill
    height: Fill
    align: Align{x: 0.0, y: 0.0}
    tree_host := View{
        width: 0.0
        height: Fill
        project_tree := ProjectTree{ width: Fill height: Fill }
    }
}
inspector_layer := View{
    width: Fill
    height: Fill
    align: Align{x: 1.0, y: 0.0}
    inspector_host := View{
        width: 0.0
        height: Fill
        inspector := Inspector{ width: Fill height: Fill }
    }
}
```

Do not move or reshape `center_stack`, statusbar, start screen, page overlays, or `popup_root`.

- [ ] **Step 6: Rebuild `sync_dock_slots` around mode and `DockState`**

At the start of `sync_dock_slots`, read the viewport width from `window_bounds(cx).size.x`, calculate `next_narrow`, and on an actual mode transition:

```rust
let viewport_w = self.window_bounds(cx).size.x;
let next = next_narrow(self.narrow, viewport_w);
if next != self.narrow {
    self.narrow = next;
    if self.narrow {
        let (tree, inspector) = self.dock_states(cx);
        let (tree, inspector) = crate::dock::narrow_entry_states(tree, inspector);
        self.apply_dock_states(cx, tree, inspector);
    }
    if let Some(mut tabs) = self
        .ui
        .widget(cx, ids!(doc_tabs))
        .borrow_mut::<crate::doc_tabs::DocTabs>()
    {
        tabs.set_narrow(cx, self.narrow);
    }
    cx.redraw_all();
}
```

Add these private App helpers:

```rust
fn dock_states(&mut self, cx: &mut Cx) -> (crate::dock::DockState, crate::dock::DockState) {
    let tree = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .map(|panel| panel.dock_state())
        .unwrap_or(crate::dock::DockState::Flag);
    let inspector = self
        .ui
        .widget(cx, ids!(inspector))
        .borrow::<crate::inspector_panel::Inspector>()
        .map(|panel| panel.dock_state())
        .unwrap_or(crate::dock::DockState::Flag);
    (tree, inspector)
}

fn apply_dock_states(
    &mut self,
    cx: &mut Cx,
    tree: crate::dock::DockState,
    inspector: crate::dock::DockState,
) {
    if let Some(mut panel) = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
    {
        if panel.dock_state() != tree {
            if tree == crate::dock::DockState::Pinned {
                panel.open_dock(cx);
            } else {
                panel.close_dock(cx);
            }
        }
    }
    if let Some(mut panel) = self
        .ui
        .widget(cx, ids!(inspector))
        .borrow_mut::<crate::inspector_panel::Inspector>()
    {
        if panel.dock_state() != inspector {
            if inspector == crate::dock::DockState::Pinned {
                panel.open_dock(cx);
            } else {
                panel.close_dock(cx);
            }
        }
    }
}
```

`apply_dock_states` never writes a parallel boolean.

Re-read states after entry reconciliation, derive:

```rust
let layout = crate::dock::responsive_layout(
    self.narrow,
    viewport_w,
    tree_state,
    inspector_state,
    crate::tree_panel::PROJECT_TREE_W,
    crate::inspector_panel::INSPECTOR_W,
);
```

When `layout != self.dock_layout`, assign it and mutate the four plain `View` widths explicitly:

```rust
if let Some(mut view) = self.ui.widget(cx, ids!(left_slot)).borrow_mut::<View>() {
    view.walk.width = Size::Fixed(layout.left_slot);
}
if let Some(mut view) = self.ui.widget(cx, ids!(right_slot)).borrow_mut::<View>() {
    view.walk.width = Size::Fixed(layout.right_slot);
}
if let Some(mut view) = self.ui.widget(cx, ids!(tree_host)).borrow_mut::<View>() {
    view.walk.width = Size::Fixed(layout.tree_body);
}
if let Some(mut view) = self
    .ui
    .widget(cx, ids!(inspector_host))
    .borrow_mut::<View>()
{
    view.walk.width = Size::Fixed(layout.inspector_body);
}
cx.redraw_all();
```

Set button lights directly from `tree_state == DockState::Pinned` and `inspector_state == DockState::Pinned`, not from slot width. Call `sync_tree_gap(cx, layout.left_slot)` so narrow always produces a zero gap. With `tab_row.pos.x == 0`, simplify its gap formula to:

```rust
let gap = (tree_w - TREE_BTN_W).max(0.0);
```

Continue measuring `tabs_x - row_x` for `left_overshoot`; after the caption restructure it lands the top rule at `x = 0` in either mode.

- [ ] **Step 7: Re-push responsive state after live reload and verify**

In `rehydrate`, after existing editor/start-screen content is restored, force the next slot sync to rewrite the live-reset `View` widths:

```rust
self.dock_layout = crate::dock::ResponsiveDockLayout::default();
self.tree_gap_w = -1.0;
self.rule_overshoot = -1.0;
self.sync_dock_slots(cx);
```

Run: `rtk cargo fmt -p waml-editor -- --check`

Expected: PASS.

Run: `rtk cargo test -p waml-editor breakpoint_ -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor doc_tabs::tests -- --nocapture`

Expected: PASS, including `x = 0` and chip behavior.

Run: `rtk cargo test -p waml-editor dock::tests -- --nocapture`

Expected: PASS, including `x = 0`, slot widths, clamping, and entry reconciliation.

- [ ] **Step 8: Commit**

```bash
rtk git add crates/waml-editor/src/app.rs
rtk git commit -m "feat(chrome): fork layout at viewport width"
```

---

### Task 5: Narrow Dock Mutual Exclusion and Outside Dismiss

**Files:**
- Modify: `crates/waml-editor/src/app.rs:1-10, 537-635, 1783-1875, 2495-2660, 2760-2905, 2908-2965`

**Interfaces:**
- Consumes: `narrow_toggle_states`, `DockEdge`, panel `drawn_rect`, `PopupRoot::is_open`, and the reverse child event order (`EventOrder::Up`) already used by Makepad `View`.
- Produces: `App::pointer_in_narrow_dock: bool`, `open_overlay_contains(DVec2, DockState, Rect, DockState, Rect) -> bool`, `should_dismiss_narrow_dock(DVec2, Rect, DockState, Rect, DockState, Rect) -> bool`, and narrow-aware caption/view-side dock transitions.
- Event order: `PopupRoot::route` remains first; dock outside-dismiss runs only if no popup owned the event at entry; then `ui.handle_event` lets the later overlay panel consume inside hits before the earlier canvas.

- [ ] **Step 1: Write the failing panel-containment test**

In `app.rs` tests, replace the `super` import with:

```rust
use super::{
    logo_command_for, next_narrow, open_overlay_contains, place_rm_for,
    should_dismiss_narrow_dock, LogoCommand,
};
use crate::dock::DockState;
```

Then add:

```rust
#[test]
fn only_the_open_narrow_panel_counts_as_inside() {
    let canvas = Rect {
        pos: dvec2(0.0, 66.0),
        size: dvec2(390.0, 700.0),
    };
    let tree = Rect {
        pos: dvec2(0.0, 66.0),
        size: dvec2(280.0, 700.0),
    };
    let inspector = Rect {
        pos: dvec2(70.0, 66.0),
        size: dvec2(320.0, 700.0),
    };
    assert!(open_overlay_contains(
        dvec2(100.0, 200.0),
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(!open_overlay_contains(
        dvec2(300.0, 200.0),
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(should_dismiss_narrow_dock(
        dvec2(300.0, 200.0),
        canvas,
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
    assert!(!should_dismiss_narrow_dock(
        dvec2(16.0, 50.0),
        canvas,
        DockState::Pinned,
        tree,
        DockState::Flag,
        inspector
    ));
}
```

The final assertion is the caption-toggle regression: `[T]` is outside the
panel but also outside the canvas, so the dismissal route must not close the
tree before the button action toggles it.

- [ ] **Step 2: Run the test and verify it fails**

Run: `rtk cargo test -p waml-editor only_the_open_narrow_panel -- --nocapture`

Expected: FAIL because `open_overlay_contains` and
`should_dismiss_narrow_dock` do not exist.

- [ ] **Step 3: Add raw-pointer containment and outside dismissal**

At the top of `app.rs`, add the production import:

```rust
use crate::dock::DockState;
```

Then add:

```rust
fn open_overlay_contains(
    point: DVec2,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    (tree_state == DockState::Pinned && tree_rect.contains(point))
        || (inspector_state == DockState::Pinned && inspector_rect.contains(point))
}

fn should_dismiss_narrow_dock(
    point: DVec2,
    canvas_rect: Rect,
    tree_state: DockState,
    tree_rect: Rect,
    inspector_state: DockState,
    inspector_rect: Rect,
) -> bool {
    canvas_rect.contains(point)
        && !open_overlay_contains(
            point,
            tree_state,
            tree_rect,
            inspector_state,
            inspector_rect,
        )
}
```

Add `#[rust] pointer_in_narrow_dock: bool` to `App`.

Add:

```rust
fn route_narrow_dock_pointer(
    &mut self,
    cx: &mut Cx,
    event: &Event,
    popup_was_open: bool,
) {
    if !self.narrow {
        return;
    }
    let (tree_state, inspector_state) = self.dock_states(cx);
    let tree_rect = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .map(|panel| panel.drawn_rect(cx))
        .unwrap_or_default();
    let inspector_rect = self
        .ui
        .widget(cx, ids!(inspector))
        .borrow::<crate::inspector_panel::Inspector>()
        .map(|panel| panel.drawn_rect(cx))
        .unwrap_or_default();
    let canvas_rect = self.ui.widget(cx, ids!(canvas)).area().rect(cx);
    let contains = |point| {
        open_overlay_contains(
            point,
            tree_state,
            tree_rect,
            inspector_state,
            inspector_rect,
        )
    };
    match event {
        Event::MouseMove(e) => {
            self.pointer_in_narrow_dock = contains(e.abs);
        }
        Event::MouseDown(e) if e.button.is_primary() => {
            let inside = contains(e.abs);
            self.pointer_in_narrow_dock = inside;
            if !popup_was_open
                && should_dismiss_narrow_dock(
                    e.abs,
                    canvas_rect,
                    tree_state,
                    tree_rect,
                    inspector_state,
                    inspector_rect,
                )
            {
                self.apply_dock_states(
                    cx,
                    crate::dock::DockState::Flag,
                    crate::dock::DockState::Flag,
                );
            }
        }
        _ => {}
    }
}
```

This deliberately does not stamp outside clicks handled; the canvas press is
the press that dismisses the overlay. Caption controls, the statusbar, and
other non-canvas chrome must not trigger this path. In particular, restricting
the close to `canvas_rect` prevents a lit `[T]` click from closing the tree
before its own toggle handler runs and immediately reopening it. Do not replace
the raw `MouseMove` branch with `Hit::FingerHover`.

Inside clicks are already consumed by the panel root hit tests. Because the overlay layers are declared after `dock_row`, Makepad's default `EventOrder::Up` dispatches them before the canvas. Keep this ordering explicit in the DSL comments; do not add a full-screen modal scrim.

In `handle_event`, capture popup ownership before routing, then invoke the dock route before the widget tree:

```rust
let popup_was_open = self
    .ui
    .widget(cx, ids!(popup_root))
    .borrow::<PopupRoot>()
    .map(|root| root.is_open())
    .unwrap_or(false);
if let Some(mut root) = self
    .ui
    .widget(cx, ids!(popup_root))
    .borrow_mut::<PopupRoot>()
{
    root.route(cx, event);
}
self.route_narrow_dock_pointer(cx, event, popup_was_open);
self.ui.handle_event(cx, event, &mut Scope::empty());
```

- [ ] **Step 4: Route caption toggles through the pure transition**

For each caption dock button:

- Wide mode keeps the current single-panel `toggle_dock`.
- Narrow mode reads both states, calls `narrow_toggle_states(tree, inspector, DockEdge::Left)` or `narrow_toggle_states(tree, inspector, DockEdge::Right)`, and applies the pair with `apply_dock_states`.

Use:

```rust
if self.narrow {
    let (tree, inspector) = self.dock_states(cx);
    let (tree, inspector) =
        crate::dock::narrow_toggle_states(tree, inspector, crate::dock::DockEdge::Left);
    self.apply_dock_states(cx, tree, inspector);
} else if let Some(mut panel) = self
    .ui
    .widget(cx, ids!(project_tree))
    .borrow_mut::<crate::tree_panel::ProjectTree>()
{
    panel.toggle_dock(cx);
}
```

Use the identical shape with `DockEdge::Right` and `Inspector`.

- [ ] **Step 5: Apply mutual exclusion to view-side inspector opens**

In `relay_outcome`, replace the direct `panel.open_dock(cx)` branch with:

```rust
if open_right_dock {
    if self.narrow {
        if let Some(mut tree) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            tree.close_dock(cx);
        }
    }
    if let Some(mut panel) = self
        .ui
        .widget(cx, ids!(inspector))
        .borrow_mut::<crate::inspector_panel::Inspector>()
    {
        panel.open_dock(cx);
    }
}
```

Do not alter `sync_right_dock_btn`'s unavailable-dock close rule.

- [ ] **Step 6: Run focused and full tests**

Run: `rtk cargo test -p waml-editor dock::tests -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor only_the_open_narrow_panel -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
rtk git add crates/waml-editor/src/app.rs
rtk git commit -m "feat(chrome): route narrow dock overlays"
```

---

### Task 6: Document Switcher Wiring and Transition Dismissal

**Files:**
- Modify: `crates/waml-editor/src/app.rs:1-10, 712-726, 852-925, 1476-1607, 1876-1980, 2467-2492, 2908-2965`

**Interfaces:**
- Consumes: `DocTabsAction::OpenSwitcher { anchor }`, `OpenTabs`, `PopupItem`, `MenuOpen::Popup { open_marking, max_height }`, `PopupRoot::{show_at, closed, is_open_for, close}`, `PopupResult::Invoked`, and `tree_panel::icon_for(TreeKind)`.
- Produces: `doc_switcher_items(&OpenTabs) -> Vec<PopupItem>`, `DOC_SWITCHER_MAX_H = 360.0`, and the `live_id!(doc_switcher)` popup flow.
- Selection path: `tabs.activate(id)` → `refresh_doc_tabs(cx)` → `sync_active_tab(cx)`.

- [ ] **Step 1: Write the failing switcher-item test**

In `app.rs` tests, replace the imports from Tasks 4–5 with:

```rust
use super::{
    doc_switcher_items, logo_command_for, next_narrow, open_overlay_contains, place_rm_for,
    should_dismiss_narrow_dock, LogoCommand,
};
use crate::doc_tabs::OpenTabs;
use crate::dock::DockState;
use crate::tree::TreeKind;
```

Then add:

```rust
#[test]
fn document_switcher_items_preserve_order_and_tab_identity() {
    let mut tabs = OpenTabs::diagram_base("d", "Diagram");
    let customer = tabs.open_preview("customer", "Customer", TreeKind::Class);
    tabs.promote(customer);
    let order = tabs.open_preview("order", "Order", TreeKind::Class);
    assert_eq!(tabs.active, order);

    let items = doc_switcher_items(&tabs);
    assert_eq!(
        items.iter().map(|item| item.id).collect::<Vec<_>>(),
        tabs.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>()
    );
    assert_eq!(
        items.iter().map(|item| item.label.as_str()).collect::<Vec<_>>(),
        vec!["Diagram", "Customer", "Order"]
    );
    assert!(items.iter().all(|item| item.enabled && !item.danger));
}
```

The active-row visual itself is already covered by Task 1's `opening_mark_shows_only_until_hover_selects_another_row`; this test proves `App` preserves the exact id that is passed as that opening mark.

- [ ] **Step 2: Run the test and verify it fails**

Run: `rtk cargo test -p waml-editor document_switcher_items_preserve -- --nocapture`

Expected: FAIL because `doc_switcher_items` does not exist.

- [ ] **Step 3: Build popup items from the authoritative open-tab order**

Near the existing menu-item builders in `app.rs`, add:

```rust
const DOC_SWITCHER_MAX_H: f64 = 360.0;

fn doc_switcher_items(open: &OpenTabs) -> Vec<crate::popup::base::PopupItem> {
    open.tabs
        .iter()
        .map(|tab| crate::popup::base::PopupItem {
            id: tab.id,
            label: tab.title.clone(),
            icon: crate::tree_panel::icon_for(tab.node_kind),
            danger: false,
            enabled: true,
        })
        .collect()
}
```

Do not add a close field or secondary close action to `PopupItem`.

- [ ] **Step 4: Open the switcher from the narrow chip**

Extend the `DocTabsAction` match:

```rust
Some(crate::doc_tabs::DocTabsAction::OpenSwitcher { anchor }) => {
    if self.tabs.active_tab().is_some() {
        let items = doc_switcher_items(&self.tabs);
        let bounds = self.window_bounds(cx);
        if let Some(mut root) = self
            .ui
            .widget(cx, ids!(popup_root))
            .borrow_mut::<PopupRoot>()
        {
            root.show_at(
                cx,
                PopupSpec::Menu {
                    tag: live_id!(doc_switcher),
                    anchor,
                    bounds,
                    items,
                    open: MenuOpen::Popup {
                        open_marking: Some(self.tabs.active),
                        max_height: Some(DOC_SWITCHER_MAX_H),
                    },
                },
            );
        }
    }
}
```

The action already carries the chip's bottom-left. Do not add `MENU_GAP` or a caption-height clamp.

- [ ] **Step 5: Commit a selected row through the normal activation path**

Inside the existing `PopupRoot` action read in `handle_actions`, add:

```rust
let doc_switcher_closed = pr.closed(actions, live_id!(doc_switcher));
```

After releasing the `PopupRoot` borrow:

```rust
if let Some(PopupResult::Invoked(id)) = doc_switcher_closed {
    self.tabs.activate(id);
    self.refresh_doc_tabs(cx);
    self.sync_active_tab(cx);
}
```

Dismissal does nothing. An unknown id remains a no-op through `OpenTabs::activate`.

- [ ] **Step 6: Dismiss only this popup on a mode transition**

In the `next != self.narrow` branch of `sync_dock_slots`, before `set_narrow`, add:

```rust
if let Some(mut root) = self
    .ui
    .widget(cx, ids!(popup_root))
    .borrow_mut::<PopupRoot>()
{
    if root.is_open_for(live_id!(doc_switcher)) {
        root.close(cx);
    }
}
```

Do not call `close` for a logo, burger, node, picker, or placement popup.

- [ ] **Step 7: Run switcher, tabs, popup, and full tests**

Run: `rtk cargo test -p waml-editor document_switcher_items_preserve -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor doc_tabs::tests -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor popup::menu::tests -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor popup::root::tests -- --nocapture`

Expected: PASS.

Run: `rtk cargo test -p waml-editor`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add crates/waml-editor/src/app.rs
rtk git commit -m "feat(chrome): open narrow document switcher"
```

---

### Task 7: Full Gates and Interactive Verification

**Files:**
- Verify: `crates/waml-editor/src/app.rs`
- Verify: `crates/waml-editor/src/doc_tabs.rs`
- Verify: `crates/waml-editor/src/dock.rs`
- Verify: `crates/waml-editor/src/tree_panel.rs`
- Verify: `crates/waml-editor/src/inspector_panel.rs`
- Verify: `crates/waml-editor/src/popup/menu.rs`
- Verify: `crates/waml-editor/src/popup/root.rs`
- Artifact: `C:\tmp\waml-responsive-390.png` (local verification artifact; do not commit)

**Interfaces:**
- Consumes: the finished responsive chrome and the repo scripts.
- Produces: passing Rust gates, a pid-scoped native screenshot at 390px, verified native interactions through both hysteresis thresholds, and a headless wasm console-panic verdict.

- [ ] **Step 1: Run the static and unit gates**

Run: `rtk cargo fmt --check`

Expected: PASS.

Run: `rtk cargo test -p waml-editor`

Expected: PASS, including all breakpoint, layout, transition, switcher, chip, and top-rule tests.

Run: `rtk cargo clippy -p waml-editor --all-targets -- -D warnings`

Expected: PASS with no warnings.

Run: `rtk cargo test --workspace`

Expected: PASS.

- [ ] **Step 2: Build and launch one pid-scoped native editor**

Run:

```powershell
rtk cargo build -p waml-editor --bin waml-editor
rtk pwsh -NoProfile
```

In that RTK-launched PowerShell session:

```powershell
$editor = Start-Process -FilePath "target/debug/waml-editor.exe" `
    -ArgumentList "crates/waml-editor/tests/fixtures/mini" -PassThru
while ($editor.MainWindowHandle -eq 0) {
    Start-Sleep -Milliseconds 200
    $editor.Refresh()
}
$editorPid = $editor.Id
$hwnd = $editor.MainWindowHandle
```

This is intentionally a visible interactive process.

- [ ] **Step 3: Install pid-scoped resize and client-click helpers in that session**

```powershell
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ResponsiveChromeWin32 {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  [DllImport("user32.dll")] public static extern bool MoveWindow(
      IntPtr hWnd, int X, int Y, int nWidth, int nHeight, bool repaint);
  [DllImport("user32.dll")] public static extern bool GetClientRect(
      IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(
      IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(
      IntPtr hWnd, ref POINT point);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(
      uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
"@

function Resize-Client([int]$width, [int]$height) {
    $client = New-Object ResponsiveChromeWin32+RECT
    $window = New-Object ResponsiveChromeWin32+RECT
    [ResponsiveChromeWin32]::GetClientRect($hwnd, [ref]$client) | Out-Null
    [ResponsiveChromeWin32]::GetWindowRect($hwnd, [ref]$window) | Out-Null
    $frameW = ($window.R - $window.L) - ($client.R - $client.L)
    $frameH = ($window.B - $window.T) - ($client.B - $client.T)
    [ResponsiveChromeWin32]::MoveWindow(
        $hwnd, 80, 80, $width + $frameW, $height + $frameH, $true
    ) | Out-Null
}

function Click-Client([int]$x, [int]$y) {
    $p = New-Object ResponsiveChromeWin32+POINT
    $p.X = $x
    $p.Y = $y
    [ResponsiveChromeWin32]::ClientToScreen($hwnd, [ref]$p) | Out-Null
    [ResponsiveChromeWin32]::SetCursorPos($p.X, $p.Y) | Out-Null
    [ResponsiveChromeWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [ResponsiveChromeWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
}
```

All input is targeted through this process's `MainWindowHandle`; do not use name-wide automation.

- [ ] **Step 4: Verify wide behavior, then cross the enter threshold**

In the same session:

```powershell
Resize-Client 900 840
Start-Sleep -Milliseconds 500
```

At 900px:

1. Confirm the tree is already open from `ProjectTree`'s seeded
   `DockState::Pinned`, then run `Click-Client 110 180` and
   `Click-Client 110 204` on the mini fixture's first two classifier rows to
   leave at least three documents open.
2. Confirm the wide strip displays all open tabs.
3. Run `Click-Client 884 50` to open `[I]` while the tree remains open; confirm both reserve columns and the center shrinks.

Resize in this exact order, pausing 500ms after each call:

```powershell
foreach ($width in 650, 640, 639, 500, 390) {
    Resize-Client $width 840
    Start-Sleep -Milliseconds 500
}
```

Confirm wide is preserved at 650 and 640, narrow begins at 639, the set/order of open documents is unchanged, the tree remains open, and the inspector closes exactly once.

- [ ] **Step 5: Verify every narrow interaction at 390px**

Use `Click-Client` with the centers visible in the 390px window and verify:

1. `Click-Client 22 17` opens the 44×25 logo menu below the upper-row mark; `Click-Client 380 220` dismisses it.
2. `Click-Client 61 17` opens the burger independently; `Click-Client 380 220` dismisses it.
3. The tree survived narrow entry. `Click-Client 16 50` closes it; a second
   `Click-Client 16 50` reopens it over the full-width canvas. This is the
   regression check that canvas-only dismissal does not pre-close the panel
   before the caption toggle runs.
4. `Click-Client 374 50` closes the tree before opening `[I]`.
5. `Click-Client 120 200` operates inside the inspector and never selects/pans the canvas underneath.
6. `Click-Client 20 400` lands on visible canvas outside the right panel and closes it.
7. `Click-Client 90 50` opens the bounded document switcher; inspect its icon/title order and active-row mark.
8. `Click-Client 90 123` commits the second switcher row and changes the active document without changing the open set.
9. `Click-Client 336 50` clicks the chip `x` and closes only the active document.
10. Repeat `Click-Client 336 50` once per remaining chip, confirming each press
    removes exactly one document. After the last closes, no chip or empty
    switcher target remains.
11. The top rule reaches `x = 0`; `[I]` remains flush right when available.
12. `Click-Client 16 50` reopens the tree over the zero-document canvas so an
    open narrow overlay exists for the exit-threshold check.

If a caption click drags the window instead of invoking its control, stop and fix `WindowDragQuery`; the logo, burger, tree button, chip/close rect, inspector button, and open-popup caption case must all answer `WindowDragQueryResponse::Client`.

- [ ] **Step 6: Cross the exit threshold and capture the narrow frame**

Before widening, capture:

```powershell
& rtk pwsh -File scripts/capture-window.ps1 `
    -Out C:\tmp\waml-responsive-390.png -ProcessId $editorPid
```

Inspect `C:\tmp\waml-responsive-390.png` at native pixels. Confirm there is no panel overflow, clipped close glyph, caption gap, or top-rule gap.
Also confirm the tree is visibly overlaid and the caption contains no document
chip.

Then resize:

```powershell
foreach ($width in 650, 680, 681, 900) {
    Resize-Client $width 840
    Start-Sleep -Milliseconds 500
}
```

Confirm narrow persists at 650 and 680, wide resumes at 681, the open tree
becomes a reserved column, and the zero-document state remains unchanged.
Close only this process:

```powershell
Stop-Process -Id $editorPid
exit
```

- [ ] **Step 7: Build the web editor and run the 390↔900 console-panic probe**

Run: `rtk cargo makepad wasm build -p waml-editor --release --no-threads`

Expected: PASS and create `target/makepad-wasm-app/release/waml-editor/index.html`.

Start a local static server and persist only its pid:

```powershell
rtk pwsh -NoProfile -Command `
  '$web = Start-Process -FilePath "python" -ArgumentList "-m","http.server","4173","--directory","target/makepad-wasm-app/release/waml-editor" -PassThru; $web.Id | Set-Content C:\tmp\waml-responsive-web.pid; Start-Sleep -Seconds 2'
```

Run:

```bash
rtk node --input-type=module -e "import { chromium } from 'playwright-core'; const browser=await chromium.launch({headless:true,args:['--use-gl=swiftshader','--enable-unsafe-swiftshader']}); const page=await browser.newPage({viewport:{width:390,height:844}}); const failures=[]; page.on('console',m=>{/panic|unreachable executed|RuntimeError/i.test(m.text())&&failures.push(m.text())}); page.on('pageerror',e=>failures.push(String(e))); await page.goto('http://127.0.0.1:4173/',{waitUntil:'load'}); await page.waitForTimeout(12000); for(const width of [900,650,639,390,680,681,900]){await page.setViewportSize({width,height:844}); await page.waitForTimeout(350);} console.log(JSON.stringify({failures},null,2)); await browser.close(); if(failures.length)process.exit(1);"
```

Expected: prints `{"failures":[]}` and exits 0. If `playwright-core` cannot find Chromium, pass the installed ms-playwright `chromium-1228/chrome-win64/chrome.exe` as `executablePath`; the Windows folder is `chrome-win64`, not `chrome-win`.

Stop only the recorded static-server process:

```powershell
rtk pwsh -NoProfile -Command `
  '$webPid = [int](Get-Content C:\tmp\waml-responsive-web.pid); Stop-Process -Id $webPid; Remove-Item -LiteralPath C:\tmp\waml-responsive-web.pid'
```

- [ ] **Step 8: Confirm the branch is clean and commits are reviewable**

Run: `rtk git status --short`

Expected: no tracked changes and no committed screenshot/build artifact.

Run: `rtk git log -6 --oneline`

Expected: the six conventional commits from Tasks 1–6, each scoped to one reviewable behavior.
