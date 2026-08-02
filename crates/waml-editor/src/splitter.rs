//! Pure geometry and drag arithmetic for the two dock splitters (the draggable
//! edges of the Model tree column and the Inspector column against the center
//! canvas). Holds the per-panel width limits (`DockLimits`), the dynamic
//! maximum a panel may grow to before the canvas would starve (`max_width`),
//! and the single pure decision function a drag frame runs (`drag`). No makepad
//! types here — this module is unit-tested standalone, exactly like `dock.rs`
//! beside it.
//!
//! The interesting behaviour is entirely in `drag`:
//!
//! * a **sticky band** between `collapse` and `min`, where the panel visually
//!   holds at `min` while the pointer keeps travelling — deliberate resistance
//!   so the collapse never happens by accident, and
//! * **hysteresis** across that snap: you push in past `collapse` to close, but
//!   you must pull back out past the strictly larger `reopen` to open again, so
//!   a pointer jittering on the threshold cannot flap the panel.
//!
//! Nothing in the crate consumes this module yet — the `DockSplitter` widget
//! and the shell wiring land separately — so every item here would trip rustc's
//! `dead_code` lint, which the project gate promotes to a hard error via clippy
//! `-D warnings`. Inert items therefore carry
//! `#[cfg_attr(not(test), allow(dead_code))]`, the same treatment `dock.rs`
//! gives its own currently-unsent API.

use crate::dock::DockEdge;

/// The narrowest the center canvas may ever become. Both splitters stop when
/// the canvas would shrink past this, so the diagram never reaches zero width
/// and both panels stay reachable.
#[cfg_attr(not(test), allow(dead_code))]
pub const MIN_CENTER_W: f64 = 320.0;

/// The three width thresholds that shape one dock column's drag.
///
/// The *default* width is deliberately not a field here: it already lives as
/// `crate::tree_panel::PROJECT_TREE_W` (280) and
/// `crate::inspector_panel::INSPECTOR_W` (320), and duplicating it would give
/// the editor two places to disagree about how wide a fresh project opens.
///
/// The invariant that gives the drag its feel is `collapse < min < reopen`:
/// `collapse` sits *below* `min` so there is a band of pointer travel where the
/// panel refuses to move (resistance), and `reopen` sits *above* `min` so
/// reopening costs strictly more travel than collapsing did (hysteresis).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct DockLimits {
    /// Narrowest width the panel will actually render at. Pointer positions
    /// between `collapse` and this clamp *up* to it.
    pub min: f64,
    /// Drag below this (while open) and the panel collapses mid-drag.
    pub collapse: f64,
    /// While collapsed, the panel only reopens once the pointer implies a width
    /// strictly greater than this. Must exceed `collapse` — that gap is the
    /// "and a bit" the user has to pull back out.
    pub reopen: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
impl DockLimits {
    /// Limits for the left-docked Model tree. Default width is
    /// `crate::tree_panel::PROJECT_TREE_W`.
    pub const TREE: Self = Self {
        min: 180.0,
        collapse: 140.0,
        reopen: 200.0,
    };

    /// Limits for the right-docked Inspector. Default width is
    /// `crate::inspector_panel::INSPECTOR_W`. Wider throughout than the tree's:
    /// the inspector's rows are label + field pairs, which go unreadable sooner
    /// than a tree of names does.
    pub const INSPECTOR: Self = Self {
        min: 220.0,
        collapse: 170.0,
        reopen: 240.0,
    };
}

/// The widest a panel may be dragged right now, given what the *other* panel is
/// currently consuming. There is no fixed maximum: the cap is whatever leaves
/// `MIN_CENTER_W` for the canvas, so it shrinks live as the other column opens
/// or is itself widened.
///
/// Floored at zero rather than at any panel's `min`, because this function has
/// no `DockLimits` and a tiny window genuinely has no room. `drag` is where the
/// result is lifted to at least `min`, so a cramped viewport yields a panel
/// pinned at `min` (overlapping the nominal center reservation) instead of a
/// nonsensical zero-width or inverted clamp range.
#[cfg_attr(not(test), allow(dead_code))]
pub fn max_width(viewport_w: f64, other_slot_w: f64) -> f64 {
    (viewport_w - other_slot_w - MIN_CENTER_W).max(0.0)
}

/// What one drag frame decided.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum DragOutcome {
    /// Stay open at this width (already clamped to `[min, max]`).
    Width(f64),
    /// Be collapsed. Returned both on the frame that crosses `collapse` and on
    /// every subsequent frame that stays inside the collapsed band, so the
    /// caller can treat it idempotently (`DockEvent::Close` is itself
    /// idempotent).
    Collapse,
    /// Come back open at this width (already clamped). Only ever returned from
    /// the collapsed state, and only past `reopen`.
    Reopen(f64),
}

/// Decide what a drag frame does, purely from the pointer position and whether
/// the panel is currently collapsed.
///
/// `edge` flips the sign: a left-docked panel's implied width is `pointer_x`
/// itself, a right-docked panel's is the distance from the pointer to the right
/// window edge. `collapsed` is the whole of the hysteresis — the same
/// `pointer_x` deliberately means different things depending on which side of
/// the snap the panel is already on.
#[cfg_attr(not(test), allow(dead_code))]
pub fn drag(
    edge: DockEdge,
    limits: DockLimits,
    pointer_x: f64,
    viewport_w: f64,
    other_slot_w: f64,
    collapsed: bool,
) -> DragOutcome {
    let raw = match edge {
        DockEdge::Left => pointer_x,
        DockEdge::Right => viewport_w - pointer_x,
    };
    // Lift the dynamic cap to `min` so the clamp range can never invert on a
    // window too small to host a panel plus `MIN_CENTER_W` (`f64::clamp` panics
    // when its bounds cross).
    let hi = max_width(viewport_w, other_slot_w).max(limits.min);
    let clamped = raw.clamp(limits.min, hi);

    if collapsed {
        if raw > limits.reopen {
            DragOutcome::Reopen(clamped)
        } else {
            DragOutcome::Collapse
        }
    } else if raw < limits.collapse {
        DragOutcome::Collapse
    } else {
        // Note the sticky band: `collapse <= raw < min` clamps UP to `min`, so
        // the panel holds still while the pointer keeps closing in. Deliberate
        // resistance before the snap, not a rounding artefact.
        DragOutcome::Width(clamped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A roomy viewport with the other panel shut, so the dynamic maximum is
    /// out of the way of the threshold tests.
    const WIDE: f64 = 1600.0;

    #[test]
    fn limits_keep_collapse_below_min_below_reopen() {
        for limits in [DockLimits::TREE, DockLimits::INSPECTOR] {
            assert!(
                limits.collapse < limits.min,
                "resistance band requires collapse < min"
            );
            assert!(
                limits.min < limits.reopen,
                "hysteresis requires reopen above min"
            );
        }
    }

    #[test]
    fn width_tracks_the_pointer_between_min_and_max() {
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 420.0, WIDE, 0.0, false),
            DragOutcome::Width(420.0)
        );
    }

    #[test]
    fn width_clamps_up_to_min_inside_the_sticky_band() {
        // Between collapse (140) and min (180) the panel sticks at min while
        // the pointer keeps travelling.
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 160.0, WIDE, 0.0, false),
            DragOutcome::Width(180.0)
        );
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 141.0, WIDE, 0.0, false),
            DragOutcome::Width(180.0)
        );
    }

    #[test]
    fn width_clamps_down_to_the_dynamic_maximum() {
        // 1000 - 0 - 320 = 680.
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 900.0, 1000.0, 0.0, false),
            DragOutcome::Width(680.0)
        );
    }

    #[test]
    fn crossing_collapse_yields_collapse() {
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 139.0, WIDE, 0.0, false),
            DragOutcome::Collapse
        );
        // The threshold itself is still open — only strictly below collapses.
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 140.0, WIDE, 0.0, false),
            DragOutcome::Width(180.0)
        );
    }

    #[test]
    fn hysteresis_needs_more_travel_out_than_in() {
        let l = DockLimits::TREE;
        // Open -> below collapse -> shut.
        assert_eq!(
            drag(DockEdge::Left, l, 100.0, WIDE, 0.0, false),
            DragOutcome::Collapse
        );
        // Collapsed: anything up to and including `reopen` stays shut, and that
        // deliberately includes widths the OPEN panel would have honoured
        // (above collapse, at or above min).
        for x in [100.0, 150.0, 180.0, 199.0, 200.0] {
            assert_eq!(
                drag(DockEdge::Left, l, x, WIDE, 0.0, true),
                DragOutcome::Collapse,
                "pointer_x {x} must not reopen"
            );
        }
        // Strictly past `reopen` it comes back.
        assert_eq!(
            drag(DockEdge::Left, l, 201.0, WIDE, 0.0, true),
            DragOutcome::Reopen(201.0)
        );
    }

    #[test]
    fn reopen_width_is_clamped_like_any_other() {
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 900.0, 1000.0, 0.0, true),
            DragOutcome::Reopen(680.0)
        );
    }

    #[test]
    fn max_width_shrinks_when_the_other_panel_is_pinned() {
        assert_eq!(max_width(1280.0, 0.0), 960.0);
        assert_eq!(max_width(1280.0, 320.0), 640.0);
        assert!(max_width(1280.0, 320.0) < max_width(1280.0, 0.0));
    }

    #[test]
    fn max_width_floors_at_zero_on_a_tiny_window() {
        assert_eq!(max_width(200.0, 0.0), 0.0);
        // ...and `drag` lifts that to `min` rather than inverting the clamp.
        assert_eq!(
            drag(DockEdge::Left, DockLimits::TREE, 400.0, 200.0, 0.0, false),
            DragOutcome::Width(180.0)
        );
    }

    #[test]
    fn other_pinned_panel_lowers_the_reachable_maximum() {
        // 1280 - 320 - 320 = 640.
        assert_eq!(
            drag(
                DockEdge::Left,
                DockLimits::TREE,
                900.0,
                1280.0,
                320.0,
                false
            ),
            DragOutcome::Width(640.0)
        );
    }

    #[test]
    fn left_and_right_edges_derive_width_with_opposite_signs() {
        // Same pointer_x, same viewport: the left panel reads 400 wide, the
        // right panel reads 1000 - 400 = 600 wide.
        assert_eq!(
            drag(
                DockEdge::Left,
                DockLimits::INSPECTOR,
                400.0,
                1000.0,
                0.0,
                false
            ),
            DragOutcome::Width(400.0)
        );
        assert_eq!(
            drag(
                DockEdge::Right,
                DockLimits::INSPECTOR,
                400.0,
                1000.0,
                0.0,
                false
            ),
            DragOutcome::Width(600.0)
        );
    }

    #[test]
    fn right_edge_collapses_near_the_right_window_edge() {
        // Pointer 100px from the right edge of a 1000px viewport => raw 100,
        // below the inspector's collapse (170).
        assert_eq!(
            drag(
                DockEdge::Right,
                DockLimits::INSPECTOR,
                900.0,
                1000.0,
                0.0,
                false
            ),
            DragOutcome::Collapse
        );
        // 250px in => raw 250, past reopen (240), so a collapsed inspector
        // comes back.
        assert_eq!(
            drag(
                DockEdge::Right,
                DockLimits::INSPECTOR,
                750.0,
                1000.0,
                0.0,
                true
            ),
            DragOutcome::Reopen(250.0)
        );
    }
}
