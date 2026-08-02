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
//! The consumer is `App::apply_splitter_drag` (`app/shell.rs`), which feeds it
//! the raw pointer x reported by the `DockSplitter` widget and routes the
//! outcome into `dock_widths` / `DockEvent::{Close, Open}`.

use crate::dock::DockEdge;

/// The narrowest the center canvas may ever become. Both splitters stop when
/// the canvas would shrink past this, so the diagram never reaches zero width
/// and both panels stay reachable.
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

impl DockLimits {
    /// Limits for the left-docked Model tree. Default width is
    /// `crate::tree_panel::PROJECT_TREE_W`.
    pub const TREE: Self = Self {
        min: 108.0,
        collapse: 84.0,
        reopen: 120.0,
    };

    /// Limits for the right-docked Inspector. Default width is
    /// `crate::inspector_panel::INSPECTOR_W`. Wider throughout than the tree's:
    /// the inspector's rows are label + field pairs, which go unreadable sooner
    /// than a tree of names does.
    pub const INSPECTOR: Self = Self {
        min: 132.0,
        collapse: 102.0,
        reopen: 144.0,
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
pub fn max_width(viewport_w: f64, other_slot_w: f64) -> f64 {
    (viewport_w - other_slot_w - MIN_CENTER_W).max(0.0)
}

/// Widest sliver a collapsed panel shows while the pointer is still down. Small
/// on purpose: this reads as the panel straining against a magnet, not as a
/// panel that is partly open.
pub const RUBBER_MAX_W: f64 = 9.0;

/// The springy give a collapsed-but-still-held panel shows for a pointer
/// implying width `raw`, where `reopen` is the width that would let it go.
///
/// Proportional to how far the pointer has travelled back toward `reopen`, so
/// the sliver grows under the finger and is widest just before the snap.
/// Deliberately tied to `reopen` rather than linear in `raw` alone: the give
/// always maps the *whole* remaining pull, whatever a panel's thresholds are.
///
/// The progress is CUBED, not linear. A linear ramp reads as elastic — the
/// panel starts sliding open the instant you move. Cubing keeps the sliver
/// near zero through most of the pull and lets it open up only close to the
/// threshold, which reads as a magnet you have to break rather than a rubber
/// band you are stretching.
///
/// Guards a non-positive `reopen` (no sensible progress to measure) and clamps
/// both ends, so this is total for any pointer position, including one dragged
/// past the far side of the window.
fn rubber_give(raw: f64, reopen: f64) -> f64 {
    if reopen <= 0.0 {
        return 0.0;
    }
    let t = (raw.max(0.0) / reopen).clamp(0.0, 1.0);
    t * t * t * RUBBER_MAX_W
}

/// Fold a panel's springy give into the width it will actually draw at.
///
/// `max`, not `body + rubber`: a panel that has just snapped shut is still
/// animating its `DockMotion` toward zero for 180ms, and ADDING the sliver
/// during that window would make the column bulge outward at the exact moment
/// it is supposed to be shutting. Taking the larger instead lets the animation
/// run and the sliver simply becomes the floor it settles onto.
pub fn with_rubber(body_w: f64, rubber: f64) -> f64 {
    body_w.max(rubber.max(0.0))
}

/// What one drag frame decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragOutcome {
    /// Stay open at this width (already clamped to `[min, max]`).
    Width(f64),
    /// Be collapsed, showing `rubber` px of springy give at the window edge.
    ///
    /// Returned on the frame that crosses `collapse` and on every subsequent
    /// frame still inside the collapsed band, so the caller can treat the
    /// collapse itself idempotently (`DockEvent::Close` is idempotent). The
    /// payload is the iOS-overscroll part: while the finger is still down the
    /// panel is magnetically held shut, but a fraction of the pointer's travel
    /// leaks through as a sliver, so pulling outward visibly loads the spring
    /// before it lets go at `reopen`. Zero once the pointer is at or past the
    /// window edge.
    Collapse { rubber: f64 },
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
            DragOutcome::Collapse {
                rubber: rubber_give(raw, limits.reopen),
            }
        }
    } else if raw < limits.collapse {
        // Crossing inward: the panel shuts NOW, and the sliver is measured from
        // the same pointer position, so the snap does not also jump the edge.
        DragOutcome::Collapse {
            rubber: rubber_give(raw, limits.reopen),
        }
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
        // Anywhere inside [collapse, min) the panel sticks at `min` while the
        // pointer keeps travelling. Written against the constants rather than
        // literals so retuning the feel cannot silently invalidate the test.
        let l = DockLimits::TREE;
        for x in [l.collapse, l.collapse + 1.0, l.min - 1.0] {
            assert_eq!(
                drag(DockEdge::Left, l, x, WIDE, 0.0, false),
                DragOutcome::Width(l.min),
                "pointer_x {x} must stick at min"
            );
        }
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
        let l = DockLimits::TREE;
        assert!(matches!(
            drag(DockEdge::Left, l, l.collapse - 1.0, WIDE, 0.0, false),
            DragOutcome::Collapse { .. }
        ));
        // The threshold itself is still open — only strictly below collapses.
        assert_eq!(
            drag(DockEdge::Left, l, l.collapse, WIDE, 0.0, false),
            DragOutcome::Width(l.min)
        );
    }

    #[test]
    fn hysteresis_needs_more_travel_out_than_in() {
        let l = DockLimits::TREE;
        // Open -> below collapse -> shut.
        assert!(matches!(
            drag(DockEdge::Left, l, l.collapse - 40.0, WIDE, 0.0, false),
            DragOutcome::Collapse { .. }
        ));
        // Collapsed: anything up to and including `reopen` stays shut, and that
        // deliberately includes widths the OPEN panel would have honoured
        // (above collapse, at or above min).
        for x in [20.0, l.collapse, l.min, l.reopen - 1.0, l.reopen] {
            assert!(
                matches!(
                    drag(DockEdge::Left, l, x, WIDE, 0.0, true),
                    DragOutcome::Collapse { .. }
                ),
                "pointer_x {x} must not reopen"
            );
        }
        // Strictly past `reopen` it comes back.
        assert_eq!(
            drag(DockEdge::Left, l, l.reopen + 1.0, WIDE, 0.0, true),
            DragOutcome::Reopen(l.reopen + 1.0)
        );
    }

    #[test]
    fn rubber_give_grows_toward_the_reopen_threshold() {
        let l = DockLimits::TREE;
        let give = |x: f64| match drag(DockEdge::Left, l, x, WIDE, 0.0, true) {
            DragOutcome::Collapse { rubber } => rubber,
            other => panic!("expected Collapse, got {other:?}"),
        };
        // Flush at the window edge, widest just before the snap, monotonic
        // between: the spring loads smoothly under the finger.
        assert_eq!(give(0.0), 0.0);
        assert_eq!(give(l.reopen), RUBBER_MAX_W);
        let samples = [0.0, l.reopen * 0.25, l.reopen * 0.5, l.reopen * 0.75];
        assert!(samples
            .windows(2)
            .all(|pair| give(pair[0]) <= give(pair[1])));
        // A pointer dragged off the far side of the panel cannot exceed the cap.
        assert_eq!(give(-500.0), 0.0);
    }

    #[test]
    fn rubber_never_exceeds_its_cap_for_any_pointer() {
        for l in [DockLimits::TREE, DockLimits::INSPECTOR] {
            for x in [-1000.0, 0.0, 50.0, 1000.0] {
                if let DragOutcome::Collapse { rubber } =
                    drag(DockEdge::Left, l, x, WIDE, 0.0, true)
                {
                    assert!(
                        (0.0..=RUBBER_MAX_W).contains(&rubber),
                        "rubber {rubber} out of range at {x}"
                    );
                }
            }
        }
    }

    #[test]
    fn with_rubber_takes_the_larger_so_a_closing_panel_cannot_bulge() {
        // Mid-close: the body is animating down through 60 and the sliver is
        // 22. Adding them would widen the column while it is shutting.
        assert_eq!(with_rubber(60.0, 22.0), 60.0);
        // Settled shut: the sliver becomes the floor.
        assert_eq!(with_rubber(0.0, 22.0), 22.0);
        assert_eq!(with_rubber(0.0, 0.0), 0.0);
        assert_eq!(with_rubber(280.0, -5.0), 280.0);
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
            DragOutcome::Width(DockLimits::TREE.min)
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
        // Pointer 40px from the right edge of a 1000px viewport => raw 40,
        // below the inspector's collapse.
        assert!(matches!(
            drag(
                DockEdge::Right,
                DockLimits::INSPECTOR,
                960.0,
                1000.0,
                0.0,
                false
            ),
            DragOutcome::Collapse { .. }
        ));
        // 250px in => raw 250, past the inspector's reopen, so a collapsed
        // inspector comes back.
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
