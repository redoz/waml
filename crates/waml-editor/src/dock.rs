//! Pure state model for the docked/collapsible Model + Inspector panels. Holds
//! the `DockState` enum, its transition table (`next`), the peek auto-collapse
//! timer (`PeekTimer`, a pure dt function testable without a live `Cx`), and
//! the slot/center width arithmetic that makes Pin shrink the center. No
//! makepad types here.
//!
//! This module is pure and unit-tested standalone. Parts of its public API have
//! no production sender any more -- `Peek` and everything that serves it
//! (`PeekTimer`, `peek_hover_span`, `FlagActivate`, `PointerLeft`, `PinToggle`,
//! `FLAG_W`, `DockEdge`, `header_controls_visible`) went inert when both panels
//! became binary `Flag` <-> `Pinned` columns. The states stay modelled and
//! tested rather than deleted; `allow(dead_code)` is what keeps that green.
#![allow(dead_code)]

/// Which visual state a dock panel is in. Replaces the panels' old separate
/// `collapsed` / `pinned` / `folded` bools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DockState {
    /// Resting: a thin sideways-label strip at the body edge, no body drawn.
    #[default]
    Flag,
    /// Unpinned + expanded: body floats over the (frozen) center, auto-collapsing
    /// back to `Flag` after `PEEK_COLLAPSE_SECS`. No panel enters this state any
    /// more -- the `peek_layer` that hosted the floating body was deleted when the
    /// inspector became a binary column, and both panels now toggle Flag <-> Pinned.
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
    /// A caption bar dock toggle (`[T]` for the tree, `[I]` for the right
    /// dock). Moves Flag <-> Pinned directly: a column summoned from the
    /// window chrome is a deliberate act, so it must not land in the
    /// self-collapsing `Peek` state the flag strip opens into.
    Toggle,
    /// The responsive shell forced a dock open, idempotently. Drives any state to
    /// `Pinned` and never collapses.
    Open,
    /// The shell forced the panel shut because the active view declares no such
    /// dock (`BodyChrome.right_dock == None` -- the last tab was closed, or a
    /// model with no diagrams was opened). The mirror of `Open`: it drives ANY
    /// state to `Flag` and never expands. Not a user gesture -- with the flag
    /// spine and pin button gone, the caption toggle is the only affordance,
    /// and it is hidden in exactly this case, so a still-`Pinned` column would
    /// strand on screen with no way to close it.
    Close,
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
        (Flag, Toggle) => Pinned,
        (Pinned, Toggle) => Flag,
        // Neither `Toggle` sender ever enters Peek — this arm exists so the
        // pair is total rather than silently falling through the catch-all,
        // and collapsing is the safer reading of a toggle-off.
        (Peek, Toggle) => Flag,
        (_, Open) => Pinned,
        (_, Close) => Flag,
        (s, _) => s,
    }
}

/// Flag-strip width (px). The slot always reserves at least this much, so the
/// flag never occludes the canvas corner.
pub const FLAG_W: f64 = 28.0;

/// The layout width a panel's slot reserves in the `flow: Right` dock row.
/// Only `Pinned` reserves a column and thereby shrinks the center; `Flag`/`Peek`
/// reserve nothing (a collapsed panel draws zero pixels and must not carve a
/// window-bg gutter down the edge).
pub fn slot_width(state: DockState, body_w: f64) -> f64 {
    match state {
        DockState::Flag | DockState::Peek => 0.0,
        DockState::Pinned => body_w,
    }
}

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
        if narrow {
            tree_w.min(cap)
        } else {
            tree_w
        }
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

pub fn narrow_entry_states(tree: DockState, inspector: DockState) -> (DockState, DockState) {
    if tree == DockState::Pinned && inspector == DockState::Pinned {
        (tree, DockState::Flag)
    } else {
        (tree, inspector)
    }
}

/// Whether the panel body (frame + contents) draws at all. `Flag` draws only
/// the strip.
pub fn body_visible(state: DockState) -> bool {
    !matches!(state, DockState::Flag)
}

/// Whether the header pin/collapse controls should be shown. They must stay
/// reachable whenever the panel is expanded (`Peek`/`Pinned`) so a `Pinned`
/// panel is never stranded with no unpin/collapse affordance — independent of
/// whether the element-picker bar is showing. (A classifier/package preview
/// hides the picker, but a `Pinned` inspector switched onto such a tab must
/// still expose its controls, since `Pinned` never auto-collapses.)
pub fn header_controls_visible(state: DockState) -> bool {
    body_visible(state)
}

/// In narrow mode the inspector floats above the center column, so its host
/// starts below a visible document header. Wide mode remains a full-height
/// reserved column beside the center.
pub fn narrow_inspector_top(narrow: bool, header_height: f64) -> f64 {
    if narrow {
        header_height.max(0.0)
    } else {
        0.0
    }
}

/// Which window edge a dock panel is anchored to. The Model tree docks left,
/// the Inspector docks right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockEdge {
    Left,
    Right,
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

/// The `[lo, hi)` x-interval to treat as "inside the panel" for the `Peek`
/// auto-collapse timer, given the body's on-screen x-span (`body_x`, `body_w`)
/// and the docked edge. The body starts one `FLAG_W` *inside* the window edge
/// (the reserved flag gutter it does not itself span); this widens the span by
/// `FLAG_W` toward that edge so the pointer that opened the peek from the flag
/// strip still reads as "inside" and keeps the timer cancelled.
pub fn peek_hover_span(body_x: f64, body_w: f64, edge: DockEdge) -> (f64, f64) {
    match edge {
        DockEdge::Left => (body_x - FLAG_W, body_x + body_w),
        DockEdge::Right => (body_x, body_x + body_w + FLAG_W),
    }
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
    fn narrow_inspector_starts_below_only_a_visible_header() {
        assert_eq!(narrow_inspector_top(true, 30.0), 30.0);
        assert_eq!(narrow_inspector_top(true, 0.0), 0.0);
        assert_eq!(narrow_inspector_top(false, 30.0), 0.0);
    }

    #[test]
    fn wide_and_narrow_layout_use_the_same_dock_states() {
        let wide = responsive_layout(
            false,
            900.0,
            DockState::Pinned,
            DockState::Pinned,
            280.0,
            320.0,
        );
        assert_eq!(
            wide,
            ResponsiveDockLayout {
                left_slot: 280.0,
                right_slot: 320.0,
                tree_body: 280.0,
                inspector_body: 320.0,
            }
        );
        let narrow = responsive_layout(
            true,
            390.0,
            DockState::Pinned,
            DockState::Flag,
            280.0,
            320.0,
        );
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
    fn document_header_and_inspector_rects_follow_wide_and_narrow_reservations() {
        let body = makepad_widgets::Rect {
            pos: makepad_widgets::dvec2(12.0, 66.0),
            size: makepad_widgets::dvec2(900.0, 600.0),
        };
        let wide = responsive_layout(
            false,
            body.size.x,
            DockState::Pinned,
            DockState::Pinned,
            280.0,
            320.0,
        );
        assert_eq!(wide.left_slot, 280.0);
        assert_eq!(wide.right_slot, 320.0);
        let left_slot = makepad_widgets::Rect {
            pos: body.pos,
            size: makepad_widgets::dvec2(wide.left_slot, body.size.y),
        };
        let right_slot = makepad_widgets::Rect {
            pos: makepad_widgets::dvec2(body.pos.x + body.size.x - wide.right_slot, body.pos.y),
            size: makepad_widgets::dvec2(wide.right_slot, body.size.y),
        };
        let header = makepad_widgets::Rect {
            pos: makepad_widgets::dvec2(left_slot.pos.x + left_slot.size.x, body.pos.y),
            size: makepad_widgets::dvec2(body.size.x - wide.left_slot - wide.right_slot, 30.0),
        };
        assert_eq!(header.pos.x, left_slot.pos.x + left_slot.size.x);
        assert_eq!(header.pos.x + header.size.x, right_slot.pos.x);

        let narrow = responsive_layout(
            true,
            390.0,
            DockState::Flag,
            DockState::Pinned,
            280.0,
            320.0,
        );
        assert_eq!(narrow.left_slot, 0.0);
        assert_eq!(narrow.right_slot, 0.0);
        let visible_header = makepad_widgets::Rect {
            pos: body.pos,
            size: makepad_widgets::dvec2(390.0, 30.0),
        };
        let inspector_with_header = makepad_widgets::Rect {
            pos: makepad_widgets::dvec2(
                body.pos.x,
                body.pos.y + narrow_inspector_top(true, visible_header.size.y),
            ),
            size: makepad_widgets::dvec2(narrow.inspector_body, 570.0),
        };
        assert!(inspector_with_header.pos.y >= visible_header.pos.y + visible_header.size.y);
        let inspector_without_header = makepad_widgets::Rect {
            pos: makepad_widgets::dvec2(body.pos.x, body.pos.y + narrow_inspector_top(true, 0.0)),
            size: makepad_widgets::dvec2(narrow.inspector_body, body.size.y),
        };
        assert_eq!(inspector_without_header.pos.y, body.pos.y);
    }

    #[test]
    fn narrow_body_width_is_capped_to_the_viewport() {
        let layout = responsive_layout(
            true,
            240.0,
            DockState::Pinned,
            DockState::Flag,
            280.0,
            320.0,
        );
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

    #[test]
    fn transition_table_matches_spec() {
        use DockEvent::*;
        use DockState::*;
        // Flag: only FlagActivate advances (to Peek).
        assert_eq!(next(Flag, FlagActivate), Peek);
        assert_eq!(next(Flag, PointerLeft), Flag);
        assert_eq!(next(Flag, PinToggle), Flag);
        // Peek: PointerLeft -> Flag, PinToggle -> Pinned.
        assert_eq!(next(Peek, PointerLeft), Flag);
        assert_eq!(next(Peek, PinToggle), Pinned);
        assert_eq!(next(Peek, FlagActivate), Peek);
        // Pinned: PinToggle unpins -> Flag; never auto-collapses.
        assert_eq!(next(Pinned, PinToggle), Flag);
        assert_eq!(next(Pinned, PointerLeft), Pinned);
        assert_eq!(next(Pinned, FlagActivate), Pinned);
    }

    #[test]
    fn toggle_skips_peek_in_both_directions() {
        use DockEvent::*;
        use DockState::*;
        // The caption tree toggle jumps straight to the docked column and back;
        // it must never park the tree in the self-collapsing Peek state.
        assert_eq!(next(Flag, Toggle), Pinned);
        assert_eq!(next(Pinned, Toggle), Flag);
        // Unreachable for the tree (it never peeks), but defined for totality.
        assert_eq!(next(Peek, Toggle), Flag);
    }

    #[test]
    fn open_is_idempotent_and_never_collapses() {
        use DockEvent::*;
        use DockState::*;
        // Request-only: a view can ask for its panel, never for its collapse,
        // so a user who closed it isn't fought by the next click. Every state
        // lands on Pinned, including Pinned itself (a no-op, so no redraw).
        assert_eq!(next(Flag, Open), Pinned);
        assert_eq!(next(Peek, Open), Pinned);
        assert_eq!(next(Pinned, Open), Pinned);
    }

    #[test]
    fn close_is_idempotent_and_never_opens() {
        use DockEvent::*;
        use DockState::*;
        // The mirror of `Open`: the shell forces a panel shut when the active
        // view declares no such dock (the last tab closed, or a model with no
        // diagrams opened). Every state lands on Flag, including Flag itself (a
        // no-op, so no redraw).
        assert_eq!(next(Pinned, Close), Flag);
        assert_eq!(next(Peek, Close), Flag);
        assert_eq!(next(Flag, Close), Flag);
    }

    #[test]
    fn full_cycle_flag_peek_pinned_flag() {
        let mut s = DockState::default();
        assert_eq!(s, DockState::Flag);
        s = next(s, DockEvent::FlagActivate);
        assert_eq!(s, DockState::Peek);
        s = next(s, DockEvent::PinToggle);
        assert_eq!(s, DockState::Pinned);
        s = next(s, DockEvent::PinToggle);
        assert_eq!(s, DockState::Flag);
    }

    #[test]
    fn slot_width_only_pinned_reserves_body() {
        assert_eq!(slot_width(DockState::Flag, 280.0), 0.0);
        assert_eq!(slot_width(DockState::Peek, 280.0), 0.0);
        assert_eq!(slot_width(DockState::Pinned, 280.0), 280.0);
    }

    #[test]
    fn body_visible_only_when_expanded() {
        assert!(!body_visible(DockState::Flag));
        assert!(body_visible(DockState::Peek));
        assert!(body_visible(DockState::Pinned));
    }

    #[test]
    fn header_controls_visible_whenever_expanded() {
        // Hidden only in Flag (no body drawn). A Pinned panel MUST keep its
        // controls — even on a classifier/package preview tab (picker hidden) —
        // or it strands with no unpin/collapse affordance (Pinned never
        // auto-collapses).
        assert!(!header_controls_visible(DockState::Flag));
        assert!(header_controls_visible(DockState::Peek));
        assert!(header_controls_visible(DockState::Pinned));
    }

    #[test]
    fn peek_hover_span_widens_toward_docked_edge() {
        // Left panel: body at [28, 308) (one FLAG_W inside the left edge).
        // The span must reach the window edge (0) to cover the flag gutter.
        assert_eq!(peek_hover_span(28.0, 280.0, DockEdge::Left), (0.0, 308.0));
        // Right panel: body at [W-348, W-28). The span must reach the right
        // window edge (W) to cover the flag gutter, without moving its left end.
        let w = 1280.0;
        let body_x = w - 348.0;
        assert_eq!(peek_hover_span(body_x, 320.0, DockEdge::Right), (body_x, w));
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
