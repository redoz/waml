//! Crossing counters for diagram edges. See
//! `docs/superpowers/plans/2026-08-07-crossing-aware-placement.md`.
//!
//! Two counters, because the optimizer and the ground truth need different
//! things: [`route_crossings`] is exact over routed polylines (reporting,
//! tests, the number a human should trust); [`segment_crossings`] is a cheap
//! proxy over straight node-center segments (the hill-climb objective in
//! `stress::reduce_crossings`, which cannot afford to route every candidate).
//!
//! Both share one geometric primitive: a strict transversal-crossing test
//! that naturally excludes everything that is NOT a crossing --
//! shared-endpoint touches (a node's own fan-out), collinear overlap (edge
//! bundling, a separate defect), and orthogonal-route corner touches. All
//! three collapse to "some pair of the four orientation cross-products is
//! exactly zero", which the strict `>0`/`<0` test below simply never counts.

use super::Route;

const EPS: f64 = 1e-9;

/// Signed area of the triangle `(o, a, b)`, positive for a left turn.
fn cross(ox: f64, oy: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - ox) * (by - oy) - (ay - oy) * (bx - ox)
}

/// True iff segment `p1p2` properly crosses segment `p3p4`: their interiors
/// intersect at exactly one point that is an endpoint of neither segment.
/// Shared endpoints, collinear overlaps, and endpoint/corner touches all fail
/// this test (see module docs) -- exactly the non-crossing cases callers must
/// not count.
fn segments_cross(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)) -> bool {
    let d1 = cross(p3.0, p3.1, p4.0, p4.1, p1.0, p1.1);
    let d2 = cross(p3.0, p3.1, p4.0, p4.1, p2.0, p2.1);
    let d3 = cross(p1.0, p1.1, p2.0, p2.1, p3.0, p3.1);
    let d4 = cross(p1.0, p1.1, p2.0, p2.1, p4.0, p4.1);

    let straddles = |x: f64, y: f64| (x > EPS && y < -EPS) || (x < -EPS && y > EPS);
    straddles(d1, d2) && straddles(d3, d4)
}

/// Exact crossing count over routed polylines: for every pair of distinct
/// routes, every pair of their segments is tested with [`segments_cross`].
/// Ground truth -- use this for reporting and tests. `O(routes^2 *
/// avg_segments^2)`, fine for a post-hoc report but too slow to call per
/// candidate move in the optimizer (that is what [`segment_crossings`] is
/// for).
pub fn route_crossings(routes: &[Route]) -> usize {
    let mut count = 0;
    for i in 0..routes.len() {
        for j in (i + 1)..routes.len() {
            let a = &routes[i].points;
            let b = &routes[j].points;
            for wa in a.windows(2) {
                for wb in b.windows(2) {
                    if segments_cross(wa[0], wa[1], wb[0], wb[1]) {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

/// Cheap proxy crossing count over straight node-center segments: for every
/// pair of distinct edges (by node index, not sharing an endpoint node),
/// treat each as the segment between its two node centers and count proper
/// crossings. This is what `stress::reduce_crossings` hill-climbs on -- it is
/// only a legitimate objective insofar as it tracks [`route_crossings`]; the
/// two are compared and the correlation reported by the Task 1 harness.
pub fn segment_crossings(centers: &[(f64, f64)], edges: &[(usize, usize)]) -> usize {
    segment_crossings_inner(centers, edges, None)
}

/// Partial [`segment_crossings`]: counts only pairs where at least one edge is
/// incident to a node flagged in `moved`. Crossings between two edges with no
/// moved endpoint are invariant under a move that displaces only those nodes,
/// so the *difference* of this count before and after a candidate move equals
/// the difference of the full count -- at `O(moved_edges * E)` geometric
/// predicate evaluations instead of `O(E^2/2)`. That is what makes the
/// `stress::reduce_crossings` hill-climb affordable on a hot path. With every
/// node flagged this is exactly [`segment_crossings`].
pub fn segment_crossings_touching(
    centers: &[(f64, f64)],
    edges: &[(usize, usize)],
    moved: &[bool],
) -> usize {
    let touch: Vec<bool> = edges
        .iter()
        .map(|&(a, b)| {
            moved.get(a).copied().unwrap_or(false) || moved.get(b).copied().unwrap_or(false)
        })
        .collect();
    segment_crossings_inner(centers, edges, Some(&touch))
}

fn segment_crossings_inner(
    centers: &[(f64, f64)],
    edges: &[(usize, usize)],
    touch: Option<&[bool]>,
) -> usize {
    let mut count = 0;
    for i in 0..edges.len() {
        let ti = touch.map(|t| t[i]).unwrap_or(true);
        for j in (i + 1)..edges.len() {
            if !ti && !touch.map(|t| t[j]).unwrap_or(true) {
                continue;
            }
            let (a0, a1) = edges[i];
            let (b0, b1) = edges[j];
            // Edges sharing a node touch at that node's (identical) center
            // coordinate; `segments_cross` already excludes that case via the
            // zero cross-product it produces, but skip explicitly rather than
            // rely on float exactness at a coincident point.
            if a0 == b0 || a0 == b1 || a1 == b0 || a1 == b1 {
                continue;
            }
            if segments_cross(centers[a0], centers[a1], centers[b0], centers[b1]) {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(points: &[(f64, f64)], source: &str, target: &str) -> Route {
        Route {
            points: points.to_vec(),
            source: source.into(),
            target: target.into(),
            key: None,
        }
    }

    /// The hill-climb's delta objective is only valid if the partial count
    /// agrees with the full one on the flagged subset: with every node flagged
    /// it must equal `segment_crossings`, and the crossings it *omits* must be
    /// exactly those between two unflagged edges (which a move displacing only
    /// flagged nodes cannot change).
    #[test]
    fn touching_count_partitions_the_full_count() {
        // Two crossing pairs: (0-3 x 1-2) around x=0..100 and (4-7 x 5-6).
        let centers = [
            (0.0, 0.0),
            (0.0, 100.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (300.0, 0.0),
            (300.0, 100.0),
            (400.0, 0.0),
            (400.0, 100.0),
        ];
        let edges = [(0, 3), (1, 2), (4, 7), (5, 6)];
        assert_eq!(segment_crossings(&centers, &edges), 2);

        let all = vec![true; centers.len()];
        assert_eq!(
            segment_crossings_touching(&centers, &edges, &all),
            segment_crossings(&centers, &edges)
        );

        let none = vec![false; centers.len()];
        assert_eq!(segment_crossings_touching(&centers, &edges, &none), 0);

        // Flag only the first quad: its crossing is counted, the other is not.
        let mut first = vec![false; centers.len()];
        first[0] = true;
        assert_eq!(segment_crossings_touching(&centers, &edges, &first), 1);
    }

    #[test]
    fn plain_transversal_crossing_counts_once() {
        let a = route(&[(0.0, 0.0), (10.0, 10.0)], "a", "b");
        let b = route(&[(0.0, 10.0), (10.0, 0.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 1);
    }

    #[test]
    fn parallel_segments_never_cross() {
        let a = route(&[(0.0, 0.0), (10.0, 0.0)], "a", "b");
        let b = route(&[(0.0, 5.0), (10.0, 5.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 0);
    }

    #[test]
    fn edges_sharing_an_endpoint_are_not_a_crossing() {
        // Two routes fanning out of the same source point.
        let a = route(&[(0.0, 0.0), (10.0, 5.0)], "s", "a");
        let b = route(&[(0.0, 0.0), (10.0, -5.0)], "s", "b");
        assert_eq!(route_crossings(&[a, b]), 0);
    }

    #[test]
    fn collinear_overlap_is_not_a_crossing() {
        let a = route(&[(0.0, 0.0), (10.0, 0.0)], "a", "b");
        let b = route(&[(5.0, 0.0), (15.0, 0.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 0);
    }

    #[test]
    fn orthogonal_corner_touch_is_not_a_crossing() {
        // An L-shaped route whose corner sits exactly on another route's
        // straight segment -- a touch, not a transversal crossing.
        let a = route(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)], "a", "b");
        let b = route(&[(5.0, -5.0), (5.0, 0.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 0);
    }

    #[test]
    fn t_junction_touch_is_not_a_crossing() {
        // b's endpoint lands ON a's interior -- a touch, not a transversal
        // crossing (neither segment's interior straddles the other's line).
        let a = route(&[(0.0, 0.0), (10.0, 0.0)], "a", "b");
        let b = route(&[(5.0, -5.0), (5.0, 0.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 0);
    }

    #[test]
    fn multi_segment_polylines_can_cross_on_any_pair_of_segments() {
        let a = route(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], "a", "b");
        let b = route(&[(5.0, -5.0), (5.0, 5.0)], "c", "d");
        assert_eq!(route_crossings(&[a, b]), 1);
    }

    #[test]
    fn segment_crossings_matches_route_crossings_for_straight_edges() {
        let centers = [(0.0, 0.0), (10.0, 10.0), (0.0, 10.0), (10.0, 0.0)];
        let edges = [(0, 1), (2, 3)];
        assert_eq!(segment_crossings(&centers, &edges), 1);
    }

    #[test]
    fn segment_crossings_skips_edges_sharing_a_node() {
        let centers = [(0.0, 0.0), (10.0, 5.0), (10.0, -5.0)];
        let edges = [(0, 1), (0, 2)];
        assert_eq!(segment_crossings(&centers, &edges), 0);
    }

    #[test]
    fn no_crossings_among_zero_or_one_routes() {
        assert_eq!(route_crossings(&[]), 0);
        let a = route(&[(0.0, 0.0), (10.0, 10.0)], "a", "b");
        assert_eq!(route_crossings(&[a]), 0);
    }
}
