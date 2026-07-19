//! Dynamic 2--6 wedge radial (marking) menu. Immediate-mode component: the
//! parent owns placement and drives it via inherent methods; it does not
//! self-route tree events (same convention as `waml_button`/`tool_dock`).
//!
//! `RadialCore` is the pure, GPU-free geometry + state machine (fully unit
//! tested). The `Radial` widget (Task 3) wraps it with the wedge shader and a
//! `NextFrame` animation loop.
//!
//! Geometry (Layout A): N sectors of 360/N deg, first wedge CENTRED at 12
//! o'clock proceeding clockwise. Fixed disc radius; central hub dead-zone is
//! the cancel target. Hit-test is by angle from centre, so screen-edge
//! clipping of the drawn disc never affects which wedge is pickable.

use crate::icon::Icon;
use makepad_widgets::*;

/// Central cancel zone / neutral origin radius (screen px).
///
/// First landing unit: no Rust caller yet -- the `Radial` widget (a later
/// task) is the consumer. Allowed dead until then, same convention as
/// `icon::Icon`.
#[allow(dead_code)]
pub const HUB_RADIUS: f64 = 30.0;
/// Disc (rim) radius (screen px).
#[allow(dead_code)]
pub const DISC_RADIUS: f64 = 120.0;

/// One wedge. The radial owns no command semantics -- it reports `id` back on
/// commit and the parent maps it.
///
/// First landing unit: no Rust caller yet -- see `HUB_RADIUS`'s doc comment.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RadialItem {
    pub id: LiveId,
    pub label: String,
    pub icon: Icon,
    /// Danger-token hue across all wedge states.
    pub danger: bool,
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
}

/// What the radial reports to its parent.
///
/// First landing unit: no Rust caller yet -- see `HUB_RADIUS`'s doc comment.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum RadialOutcome {
    Committed(LiveId),
    Cancelled,
    None,
}

/// Wedge index under `cursor`, or `None` inside the hub dead-zone. Angle is
/// measured clockwise from 12 o'clock; the first wedge (index 0) is centred on
/// 12 o'clock. Pure geometry -- ignores enabled/disabled (see `resolve_target`).
///
/// First landing unit: no non-test Rust caller yet -- see `HUB_RADIUS`'s doc
/// comment.
#[allow(dead_code)]
pub fn wedge_index(center: DVec2, cursor: DVec2, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let dx = cursor.x - center.x;
    let dy = cursor.y - center.y;
    let r = (dx * dx + dy * dy).sqrt();
    if r < HUB_RADIUS {
        return None;
    }
    // atan2(dx, -dy): up=0, right=+90, down=+180, left=-90 -> clockwise from 12.
    let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    let sector = 360.0 / n as f64;
    // First wedge centred on 0 deg => its span is [-sector/2, +sector/2).
    let shifted = (deg + sector * 0.5).rem_euclid(360.0);
    let idx = (shifted / sector).floor() as usize;
    Some(idx.min(n - 1))
}

/// Wedge index under `cursor` that is actually actionable: `None` in the hub
/// dead-zone OR over a disabled wedge (spec: a disabled wedge is treated like
/// the dead-zone -- arms nothing).
///
/// First landing unit: no non-test Rust caller yet -- see `HUB_RADIUS`'s doc
/// comment.
#[allow(dead_code)]
pub fn resolve_target(items: &[RadialItem], center: DVec2, cursor: DVec2) -> Option<usize> {
    let idx = wedge_index(center, cursor, items.len())?;
    if items[idx].enabled {
        Some(idx)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::{Icon, IconShape};

    fn item(id: LiveId, enabled: bool) -> RadialItem {
        RadialItem {
            id,
            label: "x".into(),
            icon: Icon::Shape(IconShape::Open),
            danger: false,
            enabled,
        }
    }

    const C: DVec2 = DVec2 { x: 500.0, y: 400.0 };

    // Points at radius 100 (outside hub 30, inside disc 120) in the four
    // cardinal screen directions.
    fn up() -> DVec2 {
        dvec2(C.x, C.y - 100.0)
    }
    fn right() -> DVec2 {
        dvec2(C.x + 100.0, C.y)
    }
    fn down() -> DVec2 {
        dvec2(C.x, C.y + 100.0)
    }
    fn left() -> DVec2 {
        dvec2(C.x - 100.0, C.y)
    }

    #[test]
    fn n4_cardinal_directions_map_clockwise_from_twelve() {
        assert_eq!(wedge_index(C, up(), 4), Some(0));
        assert_eq!(wedge_index(C, right(), 4), Some(1));
        assert_eq!(wedge_index(C, down(), 4), Some(2));
        assert_eq!(wedge_index(C, left(), 4), Some(3));
    }

    #[test]
    fn n2_splits_top_and_bottom() {
        assert_eq!(wedge_index(C, up(), 2), Some(0));
        assert_eq!(wedge_index(C, down(), 2), Some(1));
    }

    #[test]
    fn n3_first_wedge_centred_on_twelve() {
        assert_eq!(wedge_index(C, up(), 3), Some(0));
        // 120 deg clockwise (down-right) -> wedge 1; 240 (down-left) -> wedge 2.
        let dr = dvec2(C.x + 86.6, C.y + 50.0);
        let dl = dvec2(C.x - 86.6, C.y + 50.0);
        assert_eq!(wedge_index(C, dr, 3), Some(1));
        assert_eq!(wedge_index(C, dl, 3), Some(2));
    }

    #[test]
    fn n5_and_n6_stay_in_range() {
        for p in [up(), right(), down(), left()] {
            assert!(wedge_index(C, p, 5).unwrap() < 5);
            assert!(wedge_index(C, p, 6).unwrap() < 6);
        }
        assert_eq!(wedge_index(C, up(), 6), Some(0));
    }

    #[test]
    fn hub_dead_zone_returns_none() {
        assert_eq!(wedge_index(C, C, 4), None);
        assert_eq!(wedge_index(C, dvec2(C.x + 10.0, C.y), 4), None); // r=10 < 30
    }

    #[test]
    fn wrap_around_at_twelve_oclock_stays_in_wedge_zero() {
        // Just clockwise of 12 (deg~5) and just anti-clockwise (deg~355) both
        // fall in wedge 0 for N=4 (span -45..45).
        let just_cw = dvec2(C.x + 8.7, C.y - 99.6); // ~5 deg
        let just_ccw = dvec2(C.x - 8.7, C.y - 99.6); // ~355 deg
        assert_eq!(wedge_index(C, just_cw, 4), Some(0));
        assert_eq!(wedge_index(C, just_ccw, 4), Some(0));
    }

    #[test]
    fn disabled_wedge_resolves_to_none() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), false)];
        // `right()` is wedge 1 for N=2? No -- N=2 top/bottom. Use down() = wedge 1.
        assert_eq!(resolve_target(&items, C, down()), None); // wedge 1 disabled
        assert_eq!(resolve_target(&items, C, up()), Some(0)); // wedge 0 enabled
    }

    #[test]
    fn resolve_target_none_in_hub() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), true)];
        assert_eq!(resolve_target(&items, C, C), None);
    }
}
