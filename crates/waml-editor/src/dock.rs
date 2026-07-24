//! Pure state model for the docked/collapsible Model + Inspector panels. Holds
//! the `DockState` enum, its transition table (`next`), the peek auto-collapse
//! timer (`PeekTimer`, a pure dt function — same NextFrame dt pattern as
//! `panel_glass`, but testable without a live `Cx`), and the slot/center width
//! arithmetic that makes Pin shrink the center. No makepad types here.
//!
//! This module is pure and unit-tested standalone; it is wired into the app
//! body DSL and panel widgets across Tasks 3-5, so its public API is inert
//! (and thus dead-code-silenced) until then.
#![allow(dead_code)]

/// Which visual state a dock panel is in. Replaces the panels' old separate
/// `collapsed` / `pinned` / `folded` bools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DockState {
    /// Resting: a thin sideways-label strip at the body edge, no body drawn.
    #[default]
    Flag,
    /// Unpinned + expanded: body floats over the (frozen) center via
    /// `peek_layer`; auto-collapses back to `Flag` after `PEEK_COLLAPSE_SECS`.
    Peek,
    /// Docked column: consumes layout width, the center shrinks, sticky.
    Pinned,
}

/// A user/pointer event that may transition a `DockState`. See the plan's
/// authoritative transition table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockEvent {
    /// Hover or click the flag strip.
    FlagActivate,
    /// Pointer left the flag AND the body for `PEEK_COLLAPSE_SECS` (peek only).
    PointerLeft,
    /// Header pin button: Peek -> Pinned, or Pinned -> Flag (unpin).
    PinToggle,
    /// Header collapse button: Pinned -> Flag.
    Collapse,
}

/// The transition table. Any unlisted (state, event) pair is a no-op (returns
/// the state unchanged) — notably `PointerLeft` in `Pinned` (docked columns
/// never auto-collapse).
pub fn next(state: DockState, ev: DockEvent) -> DockState {
    use DockEvent::*;
    use DockState::*;
    match (state, ev) {
        (Flag, FlagActivate) => Peek,
        (Peek, PointerLeft) => Flag,
        (Peek, PinToggle) => Pinned,
        (Pinned, PinToggle) => Flag,
        (Pinned, Collapse) => Flag,
        (s, _) => s,
    }
}

/// Flag-strip width (px). The slot always reserves at least this much, so the
/// flag never occludes the canvas corner.
pub const FLAG_W: f64 = 28.0;

/// The layout width a panel's slot reserves in the `flow: Right` dock row.
/// Only `Pinned` reserves the full column and thereby shrinks the center;
/// `Flag`/`Peek` reserve just the flag spine.
pub fn slot_width(state: DockState, body_w: f64) -> f64 {
    match state {
        DockState::Flag | DockState::Peek => FLAG_W,
        DockState::Pinned => FLAG_W + body_w,
    }
}

/// Whether the panel body (frame + contents) draws at all. `Flag` draws only
/// the strip.
pub fn body_visible(state: DockState) -> bool {
    !matches!(state, DockState::Flag)
}

/// Seconds an unpinned peek lingers after the pointer leaves before collapsing.
pub const PEEK_COLLAPSE_SECS: f64 = 0.6;

/// Auto-collapse timer for `Peek`. Pure dt accumulator — the caller arms it
/// when the pointer leaves the flag+body, cancels it when the pointer returns
/// (or the panel pins), and calls `advance(dt)` each armed frame. Testable
/// without a `Cx`.
#[derive(Default)]
pub struct PeekTimer {
    armed: bool,
    elapsed: f64,
}

impl PeekTimer {
    /// Start (or restart) the countdown from zero.
    pub fn arm(&mut self) {
        self.armed = true;
        self.elapsed = 0.0;
    }

    /// Stop the countdown (pointer returned, or panel left Peek).
    pub fn cancel(&mut self) {
        self.armed = false;
        self.elapsed = 0.0;
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Accumulate `dt` seconds. Returns `true` exactly once, on the frame the
    /// elapsed time first reaches `PEEK_COLLAPSE_SECS`; the timer then
    /// auto-cancels so it won't fire again until re-armed. A no-op (returns
    /// `false`) while unarmed.
    pub fn advance(&mut self, dt: f64) -> bool {
        if !self.armed {
            return false;
        }
        self.elapsed += dt.max(0.0);
        if self.elapsed >= PEEK_COLLAPSE_SECS {
            self.cancel();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_table_matches_spec() {
        use DockEvent::*;
        use DockState::*;
        // Flag: only FlagActivate advances (to Peek).
        assert_eq!(next(Flag, FlagActivate), Peek);
        assert_eq!(next(Flag, PointerLeft), Flag);
        assert_eq!(next(Flag, PinToggle), Flag);
        assert_eq!(next(Flag, Collapse), Flag);
        // Peek: PointerLeft -> Flag, PinToggle -> Pinned.
        assert_eq!(next(Peek, PointerLeft), Flag);
        assert_eq!(next(Peek, PinToggle), Pinned);
        assert_eq!(next(Peek, FlagActivate), Peek);
        assert_eq!(next(Peek, Collapse), Peek);
        // Pinned: PinToggle (unpin) and Collapse both -> Flag; never auto-collapses.
        assert_eq!(next(Pinned, PinToggle), Flag);
        assert_eq!(next(Pinned, Collapse), Flag);
        assert_eq!(next(Pinned, PointerLeft), Pinned);
        assert_eq!(next(Pinned, FlagActivate), Pinned);
    }

    #[test]
    fn full_cycle_flag_peek_pinned_flag() {
        let mut s = DockState::default();
        assert_eq!(s, DockState::Flag);
        s = next(s, DockEvent::FlagActivate);
        assert_eq!(s, DockState::Peek);
        s = next(s, DockEvent::PinToggle);
        assert_eq!(s, DockState::Pinned);
        s = next(s, DockEvent::Collapse);
        assert_eq!(s, DockState::Flag);
    }

    #[test]
    fn slot_width_only_pinned_reserves_body() {
        assert_eq!(slot_width(DockState::Flag, 280.0), FLAG_W);
        assert_eq!(slot_width(DockState::Peek, 280.0), FLAG_W);
        assert_eq!(slot_width(DockState::Pinned, 280.0), FLAG_W + 280.0);
    }

    #[test]
    fn body_visible_only_when_expanded() {
        assert!(!body_visible(DockState::Flag));
        assert!(body_visible(DockState::Peek));
        assert!(body_visible(DockState::Pinned));
    }

    #[test]
    fn pinning_shrinks_center_by_exactly_slot_delta() {
        // The center is Fill = total - left_slot - right_slot. Pinning the left
        // Model panel must shrink the center by exactly its body width.
        let total = 1280.0;
        let right = slot_width(DockState::Flag, 320.0); // inspector at rest
        let center_flag = total - slot_width(DockState::Flag, 280.0) - right;
        let center_pinned = total - slot_width(DockState::Pinned, 280.0) - right;
        assert_eq!(center_flag - center_pinned, 280.0);
    }

    #[test]
    fn peek_timer_fires_once_after_threshold() {
        let mut t = PeekTimer::default();
        assert!(!t.advance(1.0)); // unarmed: no-op
        t.arm();
        assert!(!t.advance(0.3)); // 0.3 < 0.6
        assert!(!t.advance(0.2)); // 0.5 < 0.6
        assert!(t.advance(0.2)); // 0.7 >= 0.6 -> fire
        assert!(!t.is_armed()); // auto-cancelled
        assert!(!t.advance(1.0)); // stays fired-off until re-armed
    }

    #[test]
    fn peek_timer_cancel_prevents_fire() {
        let mut t = PeekTimer::default();
        t.arm();
        assert!(!t.advance(0.5));
        t.cancel();
        assert!(!t.is_armed());
        assert!(!t.advance(1.0));
    }

    #[test]
    fn peek_timer_rearm_restarts_countdown() {
        let mut t = PeekTimer::default();
        t.arm();
        assert!(!t.advance(0.5));
        t.arm(); // pointer left again -> restart
        assert!(!t.advance(0.3)); // only 0.3 since re-arm
        assert!(t.advance(0.4)); // 0.7 -> fire
    }
}
