### Task 2: `popup/marking.rs` — `MarkingCore`

The geometry-free tap/drag/popup/armed machine both surfaces embed. It is the union of `radial::RadialCore` and `app_menu::AppMenuCore` with all geometry removed: the surface resolves hits and feeds them in as `Option<usize>` (the item slot under the cursor, enabled-agnostic) plus booleans the surface computes from its own geometry. `MarkingCore` never sees coordinates except the raw `press_pos` origin it uses for the tap-vs-drag distance.

This is the crux task. Both old cores' state-machine tests are ported here verbatim-in-spirit (geometry replaced by fed hits), so behavior is pinned before anything draws.

**Files:**
- Create: `crates/waml-editor/src/popup/marking.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs` (uncomment `pub mod marking;`)

**Interfaces:**
- Consumes: `crate::popup::base::PopupItem`; `makepad_widgets::{DVec2, LiveId, dvec2, live_id}`.
- Produces (Tasks 3 & 4 rely on these EXACT signatures):
  - `pub enum MarkOutcome { Committed(LiveId), Cancelled, None }` — `#[derive(Clone, Debug, PartialEq)]`.
  - `pub struct MarkingCore { .. }` `#[derive(Default)]`, fields all private.
  - `pub fn is_open(&self) -> bool`
  - `pub fn is_popup(&self) -> bool`
  - `pub fn items(&self) -> &[PopupItem]`
  - `pub fn armed(&self) -> Option<usize>`
  - `pub fn begin_marking(&mut self, press_pos: DVec2, items: Vec<PopupItem>, drag_threshold: f64)`
  - `pub fn begin_popup(&mut self, items: Vec<PopupItem>, drag_threshold: f64)`
  - `pub fn pointer_move(&mut self, cursor: DVec2, hit: Option<usize>)`
  - `pub fn release(&mut self, hit: Option<usize>) -> MarkOutcome`
  - `pub fn press(&mut self, cursor: DVec2, hit: Option<usize>)`
  - `pub fn click(&mut self, hit: Option<usize>, outside: bool) -> MarkOutcome`
  - `pub fn close(&mut self)`

**Behavior map (how the two old cores fold in):**

| old `RadialCore` | old `AppMenuCore` | unified `MarkingCore` |
|---|---|---|
| `begin(center,items,bounds)` | `open(anchor,press,items)` | `begin_marking(press_pos, items, threshold)` — pressed, !popup |
| `begin_popup(...)` | `open_popup(...)` | `begin_popup(items, threshold)` — !pressed, popup |
| `pointer_move` (resolves geom) | `pointer_move` (row_at) | `pointer_move(cursor, hit)` — surface passes the hit |
| `release` (hub/geom) | `release` (row_at) | `release(hit)` — hit already resolved by surface |
| `click` (radial popup) | — | `click(hit, outside)` — radial popup only |
| — | `press` (menu popup arm) | `press(cursor, hit)` — menu popup only; caller checks outside first |
| `esc` | `esc` | *(removed — `PopupRoot` owns light-dismiss; surfaces call `close()`)* |
| `armed` (enabled-filtered) | `hovered` (raw row) | `armed()` = **raw** hit; surfaces filter `enabled` at draw |
| `flick` (draw-only) | — | *(NOT a field — the radial surface computes it from geometry for its own draw)* |

Two deliberate unifications to note:
1. `armed` is now the **raw** slot under the cursor (enabled-agnostic), matching `AppMenuCore.hovered`. The old `RadialCore.armed` was pre-filtered through `resolve_in`. Task 3's radial draw must therefore gate the arm-highlight on `items[i].enabled` itself (it already special-cases disabled → flat, so this is a one-line guard).
2. `esc` and outside-click leave the core. `MarkingCore.close()` just resets state; `PopupRoot` decides *when* to close on light-dismiss and does the emitting. `press`/`release`/`click` therefore never close on "outside" themselves — the surface returns `PopupVerdict::Ignored` for an unclaimed primary press and lets `PopupRoot` dismiss.

---

- [ ] **Step 1: Write the failing tests (ported from both old cores, geometry-free)**

Create `crates/waml-editor/src/popup/marking.rs` with the test module first. These are the union of `radial.rs:934-1033` and `app_menu.rs:545-714`, rewritten to feed hits instead of coordinates. `C`/`in_row` helpers collapse to plain `Some(i)` hits.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Icon;
    use makepad_widgets::*;

    fn item(id: LiveId, enabled: bool) -> PopupItem {
        PopupItem { id, label: "x".into(), icon: Icon::PackageOpen, danger: false, enabled }
    }
    // menu: slot 0 enabled, 1 disabled, 2 enabled (matches both old `menu()` fixtures).
    fn menu() -> Vec<PopupItem> {
        vec![item(live_id!(a), true), item(live_id!(b), false), item(live_id!(c), true)]
    }
    const P: DVec2 = DVec2 { x: 500.0, y: 400.0 }; // a press origin
    const T: f64 = 8.0;                             // a drag threshold

    // --- tap / marking-drag (was RadialCore + AppMenuCore begin/release) ---

    #[test]
    fn tap_latches_popup_then_click_commits() {
        let mut c = MarkingCore::default();
        c.begin_marking(P, menu(), T);
        // Release without moving = tap -> popup, stays open, no outcome.
        assert_eq!(c.release(None), MarkOutcome::None);
        assert!(c.is_open() && c.is_popup());
        // (click path is radial's; the menu equivalent uses press/release below)
    }

    #[test]
    fn drag_past_threshold_then_release_over_enabled_commits() {
        let mut c = MarkingCore::default();
        c.begin_marking(P, menu(), T);
        c.pointer_move(dvec2(P.x + 20.0, P.y), Some(0)); // drag clears threshold, arms slot 0
        assert_eq!(c.armed(), Some(0));
        assert_eq!(c.release(Some(0)), MarkOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn drag_release_off_items_cancels() {
        let mut c = MarkingCore::default();
        c.begin_marking(P, menu(), T);
        c.pointer_move(dvec2(P.x + 20.0, P.y), Some(0));
        c.pointer_move(dvec2(P.x + 200.0, P.y), None); // dragged clear of the surface
        assert_eq!(c.release(None), MarkOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn drag_release_over_disabled_cancels() {
        let mut c = MarkingCore::default();
        c.begin_marking(P, menu(), T);
        c.pointer_move(dvec2(P.x + 20.0, P.y), Some(1)); // disabled slot
        assert_eq!(c.release(Some(1)), MarkOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn tiny_move_under_threshold_is_still_a_tap() {
        let mut c = MarkingCore::default();
        c.begin_marking(P, menu(), T);
        c.pointer_move(dvec2(P.x + 2.0, P.y + 2.0), Some(0)); // < threshold
        assert_eq!(c.release(Some(0)), MarkOutcome::None);   // tap wins, not a commit
        assert!(c.is_popup());
    }

    #[test]
    fn release_without_a_held_press_is_noop() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T); // popup mode, nothing held
        assert_eq!(c.release(Some(0)), MarkOutcome::None);
        assert!(c.is_open());
    }

    // --- menu popup press-hold (was AppMenuCore press/release) ---

    #[test]
    fn popup_press_hold_release_enabled_commits() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        c.press(dvec2(P.x, P.y), Some(0)); // arm slot 0 (no commit yet)
        assert!(c.is_open());
        assert_eq!(c.release(Some(0)), MarkOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn popup_press_hold_release_off_items_cancels() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        c.press(dvec2(P.x, P.y), Some(0));
        c.pointer_move(dvec2(P.x + 200.0, P.y), None);
        assert_eq!(c.release(None), MarkOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn popup_press_hold_over_disabled_cancels() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        c.press(dvec2(P.x, P.y), Some(1));
        assert_eq!(c.release(Some(1)), MarkOutcome::Cancelled);
        assert!(!c.is_open());
    }

    // --- radial popup click (was RadialCore click) ---

    #[test]
    fn popup_click_enabled_commits() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        assert_eq!(c.click(Some(0), false), MarkOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn popup_click_outside_cancels() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        assert_eq!(c.click(None, true), MarkOutcome::Cancelled); // hub / beyond rim
        assert!(!c.is_open());
    }

    #[test]
    fn popup_click_disabled_is_noop_and_stays_open() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        assert_eq!(c.click(Some(1), false), MarkOutcome::None);
        assert!(c.is_open());
    }

    #[test]
    fn popup_click_blocked_region_cancels() {
        // hit None but NOT outside-disc: the blocked (empty) side of a partial
        // arc. Radial treats it like outside -> cancel (was
        // `click_into_blocked_region_cancels`).
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        assert_eq!(c.click(None, false), MarkOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn pointer_move_sets_armed_raw_even_for_disabled() {
        let mut c = MarkingCore::default();
        c.begin_popup(menu(), T);
        c.pointer_move(dvec2(P.x, P.y), Some(1)); // disabled slot still arms (raw)
        assert_eq!(c.armed(), Some(1));
        c.pointer_move(dvec2(P.x, P.y), None);
        assert_eq!(c.armed(), None);
    }
}
```

- [ ] **Step 2: Run to confirm the tests fail**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::marking`
Expected: FAIL — `MarkingCore` / `MarkOutcome` not found.

- [ ] **Step 3: Write `MarkingCore` above the test module**

Prepend to `crates/waml-editor/src/popup/marking.rs`:

```rust
//! `MarkingCore` — the geometry-free tap / drag / popup / armed state machine
//! both popup surfaces embed. Extracted from the near-identical `RadialCore`
//! and `AppMenuCore`. It never sees coordinates except the raw `press_pos`
//! origin used for the tap-vs-drag distance; the surface resolves the item slot
//! under the cursor (`hit`) and the geometry booleans (`outside`) and feeds
//! them in. `Default` = closed. Unit-tested directly.

use crate::popup::base::PopupItem;
use makepad_widgets::*;

/// What the marking machine reports on a resolving event. Marking-internal; the
/// surface maps this to a `PopupVerdict` and lets `PopupRoot` emit the action.
/// (Was `radial::RadialOutcome`.)
#[derive(Clone, Debug, PartialEq)]
pub enum MarkOutcome {
    Committed(LiveId),
    Cancelled,
    None,
}

#[allow(dead_code)]
#[derive(Default)]
pub struct MarkingCore {
    open: bool,
    items: Vec<PopupItem>,
    /// A button is held (marking candidate).
    pressed: bool,
    /// Passed the drag threshold -> committed to marking mode.
    dragged: bool,
    /// Latched click/press-to-pick mode (a tap, or a direct popup open).
    popup: bool,
    /// Raw slot under the cursor (enabled-agnostic); the surface filters enabled
    /// when drawing the arm/hover highlight.
    armed: Option<usize>,
    /// Drag-distance origin (the only coordinate the core stores).
    press_pos: DVec2,
    /// Cursor travel before a held press becomes a marking drag (surface-set).
    drag_threshold: f64,
}

#[allow(dead_code)]
impl MarkingCore {
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// True once latched into click/press-to-pick mode (direct popup open, or
    /// after a tap). Marking presses route release, not click.
    pub fn is_popup(&self) -> bool {
        self.popup
    }
    pub fn items(&self) -> &[PopupItem] {
        &self.items
    }
    pub fn armed(&self) -> Option<usize> {
        self.armed
    }

    /// Press-open: the press lands at `press_pos` (tap-vs-drag origin); enter
    /// marking mode — held-drag arms, release commits/cancels. (Burger / node
    /// right-press.)
    pub fn begin_marking(&mut self, press_pos: DVec2, items: Vec<PopupItem>, drag_threshold: f64) {
        self.open = true;
        self.items = items;
        self.pressed = true;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
        self.press_pos = press_pos;
        self.drag_threshold = drag_threshold;
    }

    /// Popup-open: open directly in click/press-to-pick mode, no button held.
    /// (Logo click / radial left-click.)
    pub fn begin_popup(&mut self, items: Vec<PopupItem>, drag_threshold: f64) {
        self.open = true;
        self.items = items;
        self.pressed = false;
        self.dragged = false;
        self.popup = true;
        self.armed = None;
        self.drag_threshold = drag_threshold;
    }

    /// Pointer moved to `cursor`; `hit` is the raw slot under it (or `None`).
    /// Promotes a held press to a drag past the threshold, then arms `hit`.
    pub fn pointer_move(&mut self, cursor: DVec2, hit: Option<usize>) {
        if self.pressed && !self.dragged && (cursor - self.press_pos).length() > self.drag_threshold
        {
            self.dragged = true;
        }
        self.armed = hit;
    }

    /// Button released over slot `hit`. A held press let up without dragging (and
    /// not already latched) becomes a tap → latch popup mode (no outcome).
    /// Otherwise resolve: an enabled slot commits, anything else cancels.
    pub fn release(&mut self, hit: Option<usize>) -> MarkOutcome {
        if !self.pressed {
            return MarkOutcome::None;
        }
        if !self.popup && !self.dragged {
            self.pressed = false;
            self.popup = true;
            return MarkOutcome::None;
        }
        self.pressed = false;
        match hit {
            Some(i) if self.items[i].enabled => {
                let id = self.items[i].id;
                self.close();
                MarkOutcome::Committed(id)
            }
            _ => {
                self.close();
                MarkOutcome::Cancelled
            }
        }
    }

    /// Primary press over slot `hit` in latched popup mode (menu press-hold):
    /// arm a held press so the matching `release` resolves it. The surface must
    /// only call this when the press IS on the surface — an off-surface press is
    /// the surface's `Ignored` verdict (outside-click), handled by `PopupRoot`.
    pub fn press(&mut self, cursor: DVec2, hit: Option<usize>) {
        self.pressed = true;
        self.dragged = false;
        self.press_pos = cursor;
        self.armed = hit;
    }

    /// A primary click in latched popup mode (radial immediate-commit): `outside`
    /// (hub / beyond rim / blocked arc) cancels; an enabled slot commits; a
    /// disabled slot is a no-op that stays open.
    pub fn click(&mut self, hit: Option<usize>, outside: bool) -> MarkOutcome {
        if outside {
            self.close();
            return MarkOutcome::Cancelled;
        }
        match hit {
            Some(i) if self.items[i].enabled => {
                let id = self.items[i].id;
                self.close();
                MarkOutcome::Committed(id)
            }
            Some(_) => MarkOutcome::None, // disabled: no-op, stay open
            None => {
                self.close();
                MarkOutcome::Cancelled // blocked region: like outside
            }
        }
    }

    /// Reset to closed WITHOUT any outcome (light-dismiss / supersede path;
    /// `PopupRoot` emits the `Closed` action).
    pub fn close(&mut self) {
        self.open = false;
        self.pressed = false;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
    }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor popup::marking`
Expected: PASS (all ~13 tests).

- [ ] **Step 5: Uncomment the module + build**

In `crates/waml-editor/src/popup/mod.rs` change `// pub mod marking;   // Task 2` to `pub mod marking;`.

Run: `cargo build -p waml-editor`
Expected: clean build (dead-code warnings suppressed by `#[allow(dead_code)]`).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/popup/marking.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): MarkingCore — unified tap/drag/popup state machine

Geometry-free union of RadialCore + AppMenuCore: surfaces resolve the hit
slot + outside flag and feed them in. All state-machine tests from both old
cores ported (tap-latches-popup, drag-commits, press-hold, click, disabled
no-op, blocked-region cancel). Light-dismiss/esc removed — PopupRoot owns it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
