//! Pure keep-out geometry for the drag-place constraint veil (spec §2). A
//! placement carves out the region its subject may NOT occupy relative to its
//! reference; the renderer (canvas.rs) hatches + scrims that region and
//! desaturates the non-participant cards inside it. All functions here are pure
//! (world rects + `Direction`), unit-tested without a GPU like `node_at`.
//!
//! Not yet wired into canvas.rs (Task 4 consumes this module to replace the
//! Stage-4 connector overlay) — `waml-editor` is a bin-only crate, so these
//! pub items are otherwise flagged dead_code under `-D warnings` until then.
#![allow(dead_code)]

use waml::solve::Rect;
use waml::syntax::Direction;

/// The anchored keep-out region for one placement. Per axis it locks, carries
/// `(anchor_edge_coord, extend_sign)`: the world coordinate of the reference edge
/// the veil starts on, and the sign (+1/-1) it extends toward. A cardinal locks
/// one axis; a diagonal locks both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeepOut {
    pub x: Option<(f64, f64)>,
    pub y: Option<(f64, f64)>,
}

/// The keep-out region a placement direction carves out of the plane, anchored to
/// the reference rect's near edge/corner (spec §2 mapping). Pure.
pub fn keep_out(reference: Rect, dir: Direction) -> KeepOut {
    use Direction::*;
    let left = reference.x;
    let right = reference.x + reference.w;
    let top = reference.y;
    let bottom = reference.y + reference.h;
    match dir {
        LeftOf => KeepOut {
            x: Some((left, 1.0)),
            y: None,
        },
        RightOf => KeepOut {
            x: Some((right, -1.0)),
            y: None,
        },
        Above => KeepOut {
            x: None,
            y: Some((top, 1.0)),
        },
        Below => KeepOut {
            x: None,
            y: Some((bottom, -1.0)),
        },
        AboveLeft => KeepOut {
            x: Some((left, 1.0)),
            y: Some((top, 1.0)),
        },
        AboveRight => KeepOut {
            x: Some((right, -1.0)),
            y: Some((top, 1.0)),
        },
        BelowLeft => KeepOut {
            x: Some((left, 1.0)),
            y: Some((bottom, -1.0)),
        },
        BelowRight => KeepOut {
            x: Some((right, -1.0)),
            y: Some((bottom, -1.0)),
        },
    }
}

/// Whether a card lies inside the keep-out. On each locked axis the card must
/// overlap the anchored half-plane (its far edge on the extend side is past the
/// anchor). A diagonal requires both axes; a cardinal only its one axis. Pure.
pub fn in_keep_out(k: &KeepOut, card: Rect) -> bool {
    let axis_hit = |anchor_sign: Option<(f64, f64)>, lo: f64, hi: f64| -> bool {
        match anchor_sign {
            None => true, // axis not locked ⇒ no constraint on this axis
            Some((anchor, sign)) => {
                if sign > 0.0 {
                    hi > anchor // card extends to the right/below the anchor
                } else {
                    lo < anchor // card extends to the left/above the anchor
                }
            }
        }
    };
    axis_hit(k.x, card.x, card.x + card.w) && axis_hit(k.y, card.y, card.y + card.h)
}

/// Keys of the cards that should be drawn desaturated for this placement: every
/// card inside the keep-out EXCEPT the two participants (subject + reference),
/// which keep full colour (spec §2 — colour, not a cutout, marks the
/// participant). Pure.
pub fn desaturated_cards<'a>(
    reference: Rect,
    dir: Direction,
    cards: &'a [(String, Rect)],
    subject_key: &str,
    reference_key: &str,
) -> Vec<&'a str> {
    let k = keep_out(reference, dir);
    cards
        .iter()
        .filter(|(key, _)| key != subject_key && key != reference_key)
        .filter(|(_, rect)| in_keep_out(&k, *rect))
        .map(|(key, _)| key.as_str())
        .collect()
}

/// Monotonic distance fade for the hatch: opaque (`1.0`) at the anchor edge,
/// linearly decaying to `0.0` at `reach`, clamped outside `[0, reach]`. Keeps a
/// half-plane veil from flooding the whole canvas (spec §2 — distance fade). Pure.
pub fn distance_fade(dist_from_anchor: f64, reach: f64) -> f64 {
    if reach <= 0.0 || dist_from_anchor <= 0.0 {
        return if dist_from_anchor <= 0.0 { 1.0 } else { 0.0 };
    }
    if dist_from_anchor >= reach {
        return 0.0;
    }
    1.0 - dist_from_anchor / reach
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn cardinal_anchors_on_the_correct_edge_and_side() {
        let r = rect(100.0, 50.0, 200.0, 90.0); // left=100 right=300 top=50 bottom=140
        assert_eq!(
            keep_out(r, Direction::LeftOf),
            KeepOut {
                x: Some((100.0, 1.0)),
                y: None
            }
        );
        assert_eq!(
            keep_out(r, Direction::RightOf),
            KeepOut {
                x: Some((300.0, -1.0)),
                y: None
            }
        );
        assert_eq!(
            keep_out(r, Direction::Above),
            KeepOut {
                x: None,
                y: Some((50.0, 1.0))
            }
        );
        assert_eq!(
            keep_out(r, Direction::Below),
            KeepOut {
                x: None,
                y: Some((140.0, -1.0))
            }
        );
    }

    #[test]
    fn diagonals_lock_both_axes_on_the_matching_corner() {
        let r = rect(100.0, 50.0, 200.0, 90.0);
        assert_eq!(
            keep_out(r, Direction::AboveLeft),
            KeepOut {
                x: Some((100.0, 1.0)),
                y: Some((50.0, 1.0))
            }
        );
        assert_eq!(
            keep_out(r, Direction::BelowRight),
            KeepOut {
                x: Some((300.0, -1.0)),
                y: Some((140.0, -1.0))
            }
        );
    }

    #[test]
    fn card_membership_respects_extend_side() {
        let r = rect(100.0, 50.0, 200.0, 90.0);
        let k = keep_out(r, Direction::LeftOf); // x >= 100, extends right
                                                // A card fully right of the left edge is inside; one fully left is out.
        assert!(in_keep_out(&k, rect(120.0, 0.0, 40.0, 40.0)));
        assert!(!in_keep_out(&k, rect(0.0, 0.0, 40.0, 40.0)));
    }

    #[test]
    fn diagonal_membership_needs_both_axes() {
        let r = rect(100.0, 50.0, 200.0, 90.0);
        let k = keep_out(r, Direction::AboveLeft); // x>=100 AND y>=50
        assert!(in_keep_out(&k, rect(150.0, 100.0, 30.0, 30.0))); // right & below anchors
        assert!(!in_keep_out(&k, rect(150.0, 0.0, 30.0, 30.0))); // right but above the top anchor
    }

    #[test]
    fn participants_are_exempt_from_desaturation() {
        let r = rect(100.0, 50.0, 200.0, 90.0);
        let cards = vec![
            ("subj".to_string(), rect(400.0, 60.0, 50.0, 50.0)), // inside (right of left edge), participant
            ("ref".to_string(), r),                              // reference, participant
            ("other".to_string(), rect(500.0, 60.0, 50.0, 50.0)), // inside, NOT a participant
            ("faraway".to_string(), rect(0.0, 60.0, 50.0, 50.0)), // left of the edge, outside
        ];
        let got = desaturated_cards(r, Direction::LeftOf, &cards, "subj", "ref");
        assert_eq!(
            got,
            vec!["other"],
            "only the non-participant inside the keep-out desaturates"
        );
    }

    #[test]
    fn distance_fade_is_monotonic_non_increasing() {
        let reach = 300.0;
        let samples: Vec<f64> = [-10.0, 0.0, 50.0, 150.0, 299.0, 300.0, 400.0]
            .iter()
            .map(|&d| distance_fade(d, reach))
            .collect();
        assert_eq!(samples[0], 1.0); // before the edge: full
        assert_eq!(samples[1], 1.0); // at the edge: full
        assert_eq!(*samples.last().unwrap(), 0.0); // beyond reach: zero
        for w in samples.windows(2) {
            assert!(
                w[0] >= w[1],
                "fade must never increase with distance: {samples:?}"
            );
        }
    }
}
