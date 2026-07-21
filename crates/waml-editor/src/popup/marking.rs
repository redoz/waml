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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::Icon;

    fn item(id: LiveId, enabled: bool) -> PopupItem {
        PopupItem {
            id,
            label: "x".into(),
            icon: Icon::PackageOpen,
            danger: false,
            enabled,
        }
    }
    // menu: slot 0 enabled, 1 disabled, 2 enabled (matches both old `menu()` fixtures).
    fn menu() -> Vec<PopupItem> {
        vec![
            item(live_id!(a), true),
            item(live_id!(b), false),
            item(live_id!(c), true),
        ]
    }
    const P: DVec2 = DVec2 { x: 500.0, y: 400.0 }; // a press origin
    const T: f64 = 8.0; // a drag threshold

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
        assert_eq!(c.release(Some(0)), MarkOutcome::None); // tap wins, not a commit
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
