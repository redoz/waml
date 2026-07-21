### Task 4: `popup/menu.rs` — `LinearGeom` + `MenuPopup` widget + `impl Popup`

Port the linear card surface. The pure card geometry (`panel_rect` / `row_rect` / `row_at`, measured width) becomes a standalone `LinearGeom` struct the surface owns; the `AppMenu` widget becomes `MenuPopup` driving the shared `MarkingCore`, with `handle` returning `PopupVerdict`.

**Files:**
- Create: `crates/waml-editor/src/popup/menu.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (uncomment `pub mod menu;`)
- Reference (source of verbatim moves; deleted in Task 7): `crates/waml-editor/src/app_menu.rs`

**Interfaces:**
- Consumes: `crate::popup::base::{PopupItem, Popup, PopupVerdict, PopupResult}`; `crate::popup::marking::{MarkingCore, MarkOutcome}`; `crate::icons::IconSet`; `makepad_widgets::*`.
- Produces (Task 6 relies on these):
  - `pub struct LinearGeom { anchor: DVec2, width: f64, rows: usize }` with `pub fn new`, `pub fn set_width`, `pub fn width`, `pub fn panel_rect`, `pub fn row_rect`, `pub fn row_at`.
  - The layout consts moved from `app_menu.rs`: `MENU_MAX_W`, `LABEL_X`, `LABEL_PAD_R`, `ROW_H`, `PAD_V`, `PAD_H`, `MENU_GAP`, `MENU_INDENT_X`, `CAPTION_H`, `DRAG_THRESHOLD` (= 6.0, `pub`).
  - `pub struct MenuPopup` (a `#[derive(Script, ScriptHook, Widget)]` widget) with:
    - `pub fn open_marking(&mut self, cx: &mut Cx, anchor: DVec2, press: DVec2, items: Vec<PopupItem>)` — burger press-open (was `AppMenu::open`).
    - `pub fn open_popup(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<PopupItem>)` — logo click-open.
    - `pub fn is_open(&self) -> bool`.
    - `impl Popup for MenuPopup` (`handle`, `reset`).
  - DSL: `mod.widgets.MenuPopup` registered (was `mod.widgets.AppMenu`).

---

- [ ] **Step 1: Write the failing `LinearGeom` tests (ported from `app_menu.rs`)**

Create `crates/waml-editor/src/popup/menu.rs` with the test module first. Port `app_menu.rs`'s pure geometry tests — `row_at_maps_bands_and_rejects_outside` (`:545-561`) and `set_width_drives_panel_and_hit_edges` (`:693-705`) — rewritten against `LinearGeom` (no `MarkingCore` involved; these are the geometry cases only). The `AppMenuCore` state-machine tests (`popup_press_release_*`, `press_drag_release_*`, `tap_without_drag_*`, `esc_dismisses`, `pointer_move_sets_hovered_row`, `release_without_a_held_press_is_noop`) already moved to Task 2's `marking.rs`; do NOT duplicate them.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::*;

    const ANCHOR: DVec2 = DVec2 { x: 40.0, y: 60.0 };
    const TEST_W: f64 = 120.0;

    fn in_row(g: &LinearGeom, i: usize) -> DVec2 {
        dvec2(ANCHOR.x + 20.0, ANCHOR.y + PAD_V + i as f64 * ROW_H + ROW_H * 0.5)
    }

    #[test]
    fn row_at_maps_bands_and_rejects_outside() {
        let mut g = LinearGeom::new(ANCHOR, 3);
        g.set_width(TEST_W);
        assert_eq!(g.row_at(in_row(&g, 0)), Some(0));
        assert_eq!(g.row_at(in_row(&g, 1)), Some(1));
        assert_eq!(g.row_at(in_row(&g, 2)), Some(2));
        assert_eq!(g.row_at(dvec2(ANCHOR.x - 5.0, in_row(&g, 0).y)), None);
        assert_eq!(g.row_at(dvec2(ANCHOR.x + g.width() + 5.0, in_row(&g, 0).y)), None);
        assert_eq!(g.row_at(dvec2(in_row(&g, 0).x, ANCHOR.y - 5.0)), None);
        assert_eq!(g.row_at(dvec2(in_row(&g, 0).x, ANCHOR.y + PAD_V + 3.0 * ROW_H + 1.0)), None);
    }

    #[test]
    fn set_width_drives_panel_and_hit_edges() {
        let mut g = LinearGeom::new(ANCHOR, 3);
        g.set_width(140.0);
        assert_eq!(g.panel_rect().size.x, 140.0);
        assert_eq!(g.row_rect(0).size.x, 140.0);
        let y = in_row(&g, 0).y;
        assert_eq!(g.row_at(dvec2(ANCHOR.x + 139.0, y)), Some(0));
        assert_eq!(g.row_at(dvec2(ANCHOR.x + 141.0, y)), None);
    }
}
```

- [ ] **Step 2: Run to confirm the tests fail**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::menu`
Expected: FAIL — `LinearGeom` not found.

- [ ] **Step 3: Write the consts + `LinearGeom` above the test module**

Prepend to `crates/waml-editor/src/popup/menu.rs`. Move the const block from `app_menu.rs:29-58` verbatim (make `DRAG_THRESHOLD` `pub`), then the pure geometry extracted from `AppMenuCore` (`app_menu.rs:121-151`):

```rust
//! `MenuPopup` — the linear drop-down card surface. Ported from the old
//! `app_menu.rs`: same Atlas `AccentFrame` card + `IconSet` row glyphs, driven
//! by the shared `MarkingCore`. In-window overlay (the overlay `Presenter` in
//! plan 1); geometry lives in the pure `LinearGeom`, unit-tested directly.

use crate::icons::IconSet;
use crate::popup::base::{Popup, PopupItem, PopupResult, PopupVerdict};
use crate::popup::marking::{MarkOutcome, MarkingCore};
use makepad_widgets::*;

// (Move MENU_MAX_W, LABEL_X, LABEL_PAD_R, ROW_H, PAD_V, PAD_H, MENU_GAP,
//  MENU_INDENT_X, CAPTION_H, DRAG_THRESHOLD verbatim from app_menu.rs:29-58.
//  Keep every doc comment. Make DRAG_THRESHOLD `pub`.)

/// Pure card geometry (main-window coords). The surface sets `anchor` + `rows`
/// at open, and `width` each draw from makepad's measured widest label.
#[allow(dead_code)]
#[derive(Default)]
pub struct LinearGeom {
    anchor: DVec2,
    width: f64,
    rows: usize,
}

#[allow(dead_code)]
impl LinearGeom {
    pub fn new(anchor: DVec2, rows: usize) -> Self {
        Self { anchor, width: 0.0, rows }
    }
    pub fn set_width(&mut self, width: f64) {
        self.width = width;
    }
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn anchor(&self) -> DVec2 {
        self.anchor
    }
    /// The whole card rect.
    pub fn panel_rect(&self) -> Rect {
        Rect {
            pos: self.anchor,
            size: dvec2(self.width, PAD_V * 2.0 + self.rows as f64 * ROW_H),
        }
    }
    /// The rect of row `i`.
    pub fn row_rect(&self, i: usize) -> Rect {
        Rect {
            pos: dvec2(self.anchor.x, self.anchor.y + PAD_V + i as f64 * ROW_H),
            size: dvec2(self.width, ROW_H),
        }
    }
    /// Row index under `cursor`, or `None` off the rows.
    pub fn row_at(&self, cursor: DVec2) -> Option<usize> {
        if self.rows == 0 {
            return None;
        }
        if cursor.x < self.anchor.x || cursor.x > self.anchor.x + self.width {
            return None;
        }
        let rel = cursor.y - (self.anchor.y + PAD_V);
        if rel < 0.0 || rel >= self.rows as f64 * ROW_H {
            return None;
        }
        Some((rel / ROW_H).floor() as usize)
    }
}
```

- [ ] **Step 4: Run the `LinearGeom` tests to confirm they pass**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::menu`
Expected: PASS (2 tests).

- [ ] **Step 5: Move the shaders + widget struct verbatim, renamed**

Copy the `script_mod! { .. }` block from `app_menu.rs:258-294` into `popup/menu.rs`. Renames:
- `mod.widgets.AppMenuBase = #(AppMenu::register_widget(vm))` → `#(MenuPopup::register_widget(vm))`.
- `mod.widgets.AppMenu = set_type_default() do mod.widgets.AppMenuBase{ .. }` → `mod.widgets.MenuPopup = set_type_default() do mod.widgets.MenuPopupBase{ .. }`.
- Shader/holder bodies (`draw_frame`/`draw_hover`/`draw_icon_*`/`draw_divider*`/`draw_label`) unchanged.

Copy the widget struct `app_menu.rs:296-348` renamed `AppMenu` → `MenuPopup`, replacing `#[rust] core: AppMenuCore,` with:

```rust
    #[rust]
    mark: crate::popup::marking::MarkingCore,
    #[rust]
    geom: LinearGeom,
```

Keep `draw_list: DrawList2d` and all `#[live] draw_*` / `icons` fields verbatim.

Copy `impl Widget for MenuPopup` (`app_menu.rs:350-372`) verbatim (empty `handle_event`; `draw_walk` guards on `self.is_open()` then the `begin_overlay_reuse` + root-turtle + `self.draw(cx)` sequence). Change `self.core.is_open()` → `self.mark.is_open()`.

- [ ] **Step 6: Rewrite the inherent open/handle/draw methods against `MarkingCore` + `LinearGeom`**

```rust
#[allow(dead_code)]
impl MenuPopup {
    pub fn is_open(&self) -> bool {
        self.mark.is_open()
    }

    /// Burger press-open: marking mode dropping from `anchor`, press at `press`.
    pub fn open_marking(&mut self, cx: &mut Cx, anchor: DVec2, press: DVec2, items: Vec<PopupItem>) {
        self.geom = LinearGeom::new(anchor, items.len());
        self.mark.begin_marking(press, items, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }

    /// Logo click-open: latched popup mode dropping from `anchor`.
    pub fn open_popup(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<PopupItem>) {
        self.geom = LinearGeom::new(anchor, items.len());
        self.mark.begin_popup(items, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }
}

impl Popup for MenuPopup {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.mark.is_open() {
            return PopupVerdict::Consumed;
        }
        let verdict = match event {
            Event::MouseMove(e) => {
                self.mark.pointer_move(e.abs, self.geom.row_at(e.abs));
                self.draw_frame.redraw(cx);
                PopupVerdict::Consumed
            }
            // Marking release (primary let up after press-drag) OR a popup
            // press-hold release: MarkingCore distinguishes via its own state.
            Event::MouseUp(e) if e.button.is_primary() => {
                map_outcome(self.mark.release(self.geom.row_at(e.abs)))
            }
            // Popup mode: a primary press ON the card arms the row (press-hold);
            // a press OFF the card is the outside-click -> Ignored (PopupRoot
            // dismisses). This is the single outside-click seam; MarkingCore
            // never self-closes on outside.
            Event::MouseDown(e) if e.button.is_primary() && self.mark.is_popup() => {
                if self.geom.panel_rect().contains(e.abs) {
                    self.mark.press(e.abs, self.geom.row_at(e.abs));
                    self.draw_frame.redraw(cx);
                    PopupVerdict::Consumed
                } else {
                    PopupVerdict::Ignored
                }
            }
            _ => PopupVerdict::Consumed,
        };
        if let PopupVerdict::Closed(_) = verdict {
            self.draw_frame.redraw(cx);
        }
        verdict
    }

    fn reset(&mut self) {
        self.mark.close();
    }
}

fn map_outcome(o: MarkOutcome) -> PopupVerdict {
    match o {
        MarkOutcome::Committed(id) => PopupVerdict::Closed(PopupResult::Invoked(id)),
        MarkOutcome::Cancelled => PopupVerdict::Closed(PopupResult::Dismissed),
        MarkOutcome::None => PopupVerdict::Consumed,
    }
}
```

Move the old `AppMenu::draw` body (`app_menu.rs:432-509`) with these substitutions:
- `self.core.items()` → `self.mark.items()`; `self.core.hovered` → `self.mark.armed()`; `self.core.set_width(..)` → `self.geom.set_width(..)`; `self.core.panel_rect()` → `self.geom.panel_rect()`; `self.core.row_rect(i)` → `self.geom.row_rect(i)`.
- The measured-width loop (`app_menu.rs:443-450`) is unchanged except the `set_width` target (`self.geom`).
- The hover-highlight `if hovered == Some(i) && it.enabled` (`app_menu.rs:481`) already gates on `enabled`, so the raw `armed()` is correct (Task 2 note #1) with no change.

- [ ] **Step 7: Run tests + build**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::menu && cargo build -p waml-editor`
Expected: PASS + clean build (unused-until-Task-6 warnings covered by `#[allow(dead_code)]`).

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/popup/menu.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): MenuPopup surface (LinearGeom extracted, MarkingCore-driven)

Linear card ported off AppMenuCore onto the shared MarkingCore + an owned
LinearGeom; card shaders + measured-width draw moved verbatim from app_menu.rs.
impl Popup returns PopupVerdict; off-card popup press -> Ignored (outside-click
seam). LinearGeom row_at/panel_rect tests kept.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
