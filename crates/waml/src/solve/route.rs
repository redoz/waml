//! Orthogonal (Manhattan) edge router: OVG -> A* (bend penalty) -> nudge.
//! See docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md.
//!
//! Built incrementally, task by task: the OVG, A*, nudge, and hub-spreading
//! pieces below are standalone and unit-tested before `route()` wires them
//! together, so the plain (non-test) lib target sees them as unused in the
//! interim. Remove this allow once the pipeline is fully wired.
#![allow(dead_code)]

use super::{Box, BoxId, Rect, Route, SolveConfig};
use std::collections::BTreeMap;

/// Route every leaf-to-leaf edge as an orthogonal polyline avoiding obstacles.
pub(super) fn route(
    _boxes: &[Box],
    _rects: &BTreeMap<BoxId, Rect>,
    _edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    Vec::new()
}

const ROUTE_MARGIN: f64 = 12.0;

type P = (f64, f64);

#[derive(Debug, Clone, PartialEq)]
struct Obstacle {
    id: BoxId,
    rect: Rect,
}

#[derive(Debug, Clone)]
struct Ovg {
    verts: Vec<P>,
    adj: Vec<Vec<(usize, f64)>>,
}

fn inflate(r: Rect, m: f64) -> Rect {
    Rect {
        x: r.x - m,
        y: r.y - m,
        w: r.w + 2.0 * m,
        h: r.h + 2.0 * m,
    }
}

/// Strictly inside (edges are allowed — a vertex may sit on an inflated border).
fn strictly_inside(r: &Rect, x: f64, y: f64) -> bool {
    x > r.x + 1e-9 && x < r.x + r.w - 1e-9 && y > r.y + 1e-9 && y < r.y + r.h - 1e-9
}

/// True if the axis-aligned segment (a..b) passes through any inflated obstacle interior.
///
/// Segments here are always axis-aligned (horizontal or vertical, never diagonal),
/// so one axis is always degenerate (a single coordinate, not a range). A degenerate
/// axis needs a "strictly between the rect's bounds" test, not an interval-overlap
/// test — an interval-overlap of a single point against a range always has zero
/// width, so it would never report a crossing even when the point sits deep inside
/// the rect's interior on that axis.
fn segment_blocked(inflated: &[Rect], a: P, b: P) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let degenerate_x = (x1 - x0).abs() < 1e-9;
    let degenerate_y = (y1 - y0).abs() < 1e-9;
    inflated.iter().any(|r| {
        let x_overlap = if degenerate_x {
            x0 > r.x + 1e-9 && x0 < r.x + r.w - 1e-9
        } else {
            let ox0 = r.x.max(x0);
            let ox1 = (r.x + r.w).min(x1);
            (ox1 - ox0) > 1e-9
        };
        let y_overlap = if degenerate_y {
            y0 > r.y + 1e-9 && y0 < r.y + r.h - 1e-9
        } else {
            let oy0 = r.y.max(y0);
            let oy1 = (r.y + r.h).min(y1);
            (oy1 - oy0) > 1e-9
        };
        // Positive overlap on BOTH axes => the segment cuts the interior.
        x_overlap && y_overlap
    })
}

fn leaf_obstacles(rects: &BTreeMap<BoxId, Rect>, exclude: &[BoxId]) -> Vec<Obstacle> {
    let mut out: Vec<Obstacle> = rects
        .iter()
        .filter(|(id, _)| matches!(id, BoxId::Node(_)) && !exclude.contains(id))
        .map(|(id, r)| Obstacle {
            id: id.clone(),
            rect: *r,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Deterministic sorted-unique coordinate list.
fn axis_coords(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

fn build_ovg(obstacles: &[Obstacle], src: Rect, tgt: Rect) -> (Ovg, Vec<usize>, Vec<usize>) {
    let inflated: Vec<Rect> = obstacles
        .iter()
        .map(|o| inflate(o.rect, ROUTE_MARGIN))
        .collect();

    // Interesting coordinates: inflated obstacle borders + endpoint box borders.
    let mut xs = vec![src.x, src.x + src.w, tgt.x, tgt.x + tgt.w];
    let mut ys = vec![src.y, src.y + src.h, tgt.y, tgt.y + tgt.h];
    for r in &inflated {
        xs.push(r.x);
        xs.push(r.x + r.w);
        ys.push(r.y);
        ys.push(r.y + r.h);
    }
    let xs = axis_coords(xs);
    let ys = axis_coords(ys);

    // Grid intersections that are not strictly inside any inflated obstacle.
    let mut verts: Vec<P> = Vec::new();
    let mut at: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (xi, &x) in xs.iter().enumerate() {
        for (yi, &y) in ys.iter().enumerate() {
            if inflated.iter().any(|r| strictly_inside(r, x, y)) {
                continue;
            }
            at.insert((xi, yi), verts.len());
            verts.push((x, y));
        }
    }

    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); verts.len()];
    let connect = |verts: &Vec<P>, adj: &mut Vec<Vec<(usize, f64)>>, i: usize, j: usize| {
        let (a, b) = (verts[i], verts[j]);
        if segment_blocked(&inflated, a, b) {
            return;
        }
        let len = (a.0 - b.0).abs() + (a.1 - b.1).abs();
        adj[i].push((j, len));
        adj[j].push((i, len));
    };
    // Horizontal neighbours: same yi, next present xi.
    for yi in 0..ys.len() {
        let mut prev: Option<usize> = None;
        for xi in 0..xs.len() {
            if let Some(&idx) = at.get(&(xi, yi)) {
                if let Some(p) = prev {
                    connect(&verts, &mut adj, p, idx);
                }
                prev = Some(idx);
            }
        }
    }
    // Vertical neighbours: same xi, next present yi.
    for xi in 0..xs.len() {
        let mut prev: Option<usize> = None;
        for yi in 0..ys.len() {
            if let Some(&idx) = at.get(&(xi, yi)) {
                if let Some(p) = prev {
                    connect(&verts, &mut adj, p, idx);
                }
                prev = Some(idx);
            }
        }
    }

    // Free-perimeter attachment candidates for one endpoint box: a vertex at
    // every interesting coordinate on its four sides (plus side midpoints),
    // joined perpendicular into any aligned, unblocked grid vertex.
    let attach = |verts: &mut Vec<P>, adj: &mut Vec<Vec<(usize, f64)>>, bx: Rect| -> Vec<usize> {
        let mut points: Vec<P> = Vec::new();
        for &y in &ys {
            if y >= bx.y - 1e-9 && y <= bx.y + bx.h + 1e-9 {
                points.push((bx.x, y));
                points.push((bx.x + bx.w, y));
            }
        }
        for &x in &xs {
            if x >= bx.x - 1e-9 && x <= bx.x + bx.w + 1e-9 {
                points.push((x, bx.y));
                points.push((x, bx.y + bx.h));
            }
        }
        // Side midpoints guarantee at least one candidate per side.
        points.push((bx.x, bx.y + bx.h / 2.0));
        points.push((bx.x + bx.w, bx.y + bx.h / 2.0));
        points.push((bx.x + bx.w / 2.0, bx.y));
        points.push((bx.x + bx.w / 2.0, bx.y + bx.h));
        points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);

        let mut idxs = Vec::new();
        for pt in points {
            let ai = verts.len();
            verts.push(pt);
            adj.push(Vec::new());
            idxs.push(ai);
            for gi in 0..ai {
                let g = verts[gi];
                let aligned = (g.0 - pt.0).abs() < 1e-9 || (g.1 - pt.1).abs() < 1e-9;
                if aligned && !segment_blocked(&inflated, pt, g) {
                    let len = (g.0 - pt.0).abs() + (g.1 - pt.1).abs();
                    adj[ai].push((gi, len));
                    adj[gi].push((ai, len));
                }
            }
        }
        idxs
    };

    let srcv = attach(&mut verts, &mut adj, src);
    let tgtv = attach(&mut verts, &mut adj, tgt);
    (Ovg { verts, adj }, srcv, tgtv)
}

const BEND_PENALTY: f64 = 20.0;

#[derive(Clone, Copy, PartialEq)]
struct Ord64(f64);
impl Eq for Ord64 {}
impl PartialOrd for Ord64 {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ord64 {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

fn dir_of(a: P, b: P) -> u8 {
    if (a.1 - b.1).abs() < 1e-9 {
        1
    } else {
        2
    } // horizontal else vertical
}

fn simplify(pts: Vec<P>) -> Vec<P> {
    let mut out: Vec<P> = Vec::new();
    for p in pts {
        if out
            .last()
            .is_some_and(|&l| (l.0 - p.0).abs() < 1e-9 && (l.1 - p.1).abs() < 1e-9)
        {
            continue; // duplicate
        }
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let colinear_x = (a.0 - b.0).abs() < 1e-9 && (b.0 - p.0).abs() < 1e-9;
            let colinear_y = (a.1 - b.1).abs() < 1e-9 && (b.1 - p.1).abs() < 1e-9;
            if colinear_x || colinear_y {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

fn astar(ovg: &Ovg, sources: &[usize], targets: &[usize], goal: P) -> Option<Vec<P>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    let n = ovg.verts.len();
    let state = |v: usize, d: u8| v * 3 + d as usize;
    let mut dist = vec![f64::INFINITY; n * 3];
    let mut prev: Vec<Option<usize>> = vec![None; n * 3]; // predecessor STATE
    let is_target = |v: usize| targets.contains(&v);
    let h = |v: usize| {
        let (x, y) = ovg.verts[v];
        (x - goal.0).abs() + (y - goal.1).abs()
    };

    let mut srt = sources.to_vec();
    srt.sort_unstable();
    let mut heap: BinaryHeap<Reverse<(Ord64, usize)>> = BinaryHeap::new();
    for &s in &srt {
        let st = state(s, 0);
        if dist[st] > 0.0 {
            dist[st] = 0.0;
            heap.push(Reverse((Ord64(h(s)), st)));
        }
    }

    let mut goal_state: Option<usize> = None;
    while let Some(Reverse((_f, st))) = heap.pop() {
        let v = st / 3;
        let d = (st % 3) as u8;
        let g = dist[st];
        if is_target(v) {
            goal_state = Some(st);
            break;
        }
        for &(w, len) in &ovg.adj[v] {
            let nd = dir_of(ovg.verts[v], ovg.verts[w]);
            let bend = if d != 0 && d != nd { BEND_PENALTY } else { 0.0 };
            let ng = g + len + bend;
            let ns = state(w, nd);
            if ng + 1e-9 < dist[ns] {
                dist[ns] = ng;
                prev[ns] = Some(st);
                heap.push(Reverse((Ord64(ng + h(w)), ns)));
            }
        }
    }

    let mut cur = goal_state?;
    let mut rev: Vec<P> = Vec::new();
    loop {
        rev.push(ovg.verts[cur / 3]);
        match prev[cur] {
            Some(p) => cur = p,
            None => break,
        }
    }
    rev.reverse();
    Some(simplify(rev))
}

/// Minimum channel gap between coincident parallel route segments.
const NUDGE_GAP: f64 = 8.0;

/// A single interior segment of a route, keyed by its channel coordinate.
#[derive(Clone)]
struct Seg {
    ri: usize,
    a: usize,
    b: usize,
    other_mid: f64,
    src: String,
    tgt: String,
}

/// Split parallel segments that share a routing channel (same axis + coincident
/// coordinate) into distinct parallel lines via an order-then-push sweep.
/// Endpoints (first/last point of each route) are never moved.
fn nudge(routes: &mut [Route]) {
    let mut chan_h: BTreeMap<i64, Vec<Seg>> = BTreeMap::new(); // key = quantized y
    let mut chan_v: BTreeMap<i64, Vec<Seg>> = BTreeMap::new(); // key = quantized x
    let q = |c: f64| (c * 1e6).round() as i64;

    for (ri, route) in routes.iter().enumerate() {
        let n = route.points.len();
        for i in 0..n.saturating_sub(1) {
            // Skip first/last segment: keep route endpoints anchored to their box.
            if i == 0 || i + 1 == n - 1 {
                continue;
            }
            let a = route.points[i];
            let b = route.points[i + 1];
            if (a.1 - b.1).abs() < 1e-9 {
                chan_h.entry(q(a.1)).or_default().push(Seg {
                    ri,
                    a: i,
                    b: i + 1,
                    other_mid: (a.0 + b.0) / 2.0,
                    src: route.source.clone(),
                    tgt: route.target.clone(),
                });
            } else if (a.0 - b.0).abs() < 1e-9 {
                chan_v.entry(q(a.0)).or_default().push(Seg {
                    ri,
                    a: i,
                    b: i + 1,
                    other_mid: (a.1 + b.1) / 2.0,
                    src: route.source.clone(),
                    tgt: route.target.clone(),
                });
            }
        }
    }

    fn sweep(chan: BTreeMap<i64, Vec<Seg>>, routes: &mut [Route], horizontal: bool) {
        for (key, mut segs) in chan {
            if segs.len() < 2 {
                continue;
            }
            segs.sort_by(|p, r| {
                p.other_mid
                    .total_cmp(&r.other_mid)
                    .then(p.src.cmp(&r.src))
                    .then(p.tgt.cmp(&r.tgt))
            });
            let base = key as f64 / 1e6;
            let m = segs.len();
            let start = base - (m as f64 - 1.0) * NUDGE_GAP / 2.0;
            for (k, s) in segs.iter().enumerate() {
                let coord = start + k as f64 * NUDGE_GAP;
                if horizontal {
                    routes[s.ri].points[s.a].1 = coord;
                    routes[s.ri].points[s.b].1 = coord;
                } else {
                    routes[s.ri].points[s.a].0 = coord;
                    routes[s.ri].points[s.b].0 = coord;
                }
            }
        }
    }
    sweep(chan_h, routes, true);
    sweep(chan_v, routes, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::BoxId;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn ovg_has_attachments_on_all_four_sides_and_is_obstacle_free() {
        // Two boxes, clear gap; no third obstacle.
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(300.0, 0.0, 100.0, 60.0);
        let (ovg, srcv, tgtv) = build_ovg(&[], src, tgt);
        assert!(!srcv.is_empty(), "source has attachment candidates");
        assert!(!tgtv.is_empty(), "target has attachment candidates");
        // Every adjacency segment is axis-aligned (orthogonal).
        for (i, nbrs) in ovg.adj.iter().enumerate() {
            for &(j, _len) in nbrs {
                let (ax, ay) = ovg.verts[i];
                let (bx, by) = ovg.verts[j];
                assert!(
                    (ax - bx).abs() < 1e-9 || (ay - by).abs() < 1e-9,
                    "segment {i}->{j} must be orthogonal"
                );
            }
        }
    }

    #[test]
    fn ovg_vertices_avoid_inflated_obstacle_interior() {
        // An obstacle sitting between src and tgt.
        let mid = Obstacle {
            id: BoxId::Node("m".into()),
            rect: r(150.0, -20.0, 80.0, 100.0),
        };
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(350.0, 0.0, 100.0, 60.0);
        let (ovg, _s, _t) = build_ovg(std::slice::from_ref(&mid), src, tgt);
        let inflated = inflate(mid.rect, ROUTE_MARGIN);
        for &(x, y) in &ovg.verts {
            assert!(
                !strictly_inside(&inflated, x, y),
                "vertex ({x},{y}) must not be strictly inside the inflated obstacle"
            );
        }
    }

    #[test]
    fn segment_blocked_detects_degenerate_horizontal_crossing() {
        // Regression: a horizontal segment (y0 == y1) passing straight through an
        // obstacle's interior must be detected, even though the segment's y-range
        // is a single point (zero width), not an interval.
        let obstacle = r(150.0, -30.0, 80.0, 120.0); // y spans [-30, 90]
        let inflated = [inflate(obstacle, ROUTE_MARGIN)]; // x:[138,242] y:[-42,102]
        assert!(
            segment_blocked(&inflated, (100.0, 30.0), (350.0, 30.0)),
            "horizontal segment at y=30 crosses the obstacle's x-span [138,242]"
        );
        // Regression: same for a vertical segment (x0 == x1).
        assert!(
            segment_blocked(&inflated, (190.0, -60.0), (190.0, 120.0)),
            "vertical segment at x=190 crosses the obstacle's y-span [-42,102]"
        );
        // Sanity: a horizontal segment entirely above the obstacle is NOT blocked.
        assert!(!segment_blocked(
            &inflated,
            (100.0, -100.0),
            (350.0, -100.0)
        ));
    }

    #[test]
    fn leaf_obstacles_excludes_endpoints_and_sorts_by_boxid() {
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("b".into()), r(0.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Node("a".into()), r(20.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Node("c".into()), r(40.0, 0.0, 10.0, 10.0));
        rects.insert(BoxId::Group(0), r(0.0, 0.0, 60.0, 20.0)); // groups excluded here
        let obs = leaf_obstacles(&rects, &[BoxId::Node("a".into())]);
        let ids: Vec<_> = obs.iter().map(|o| o.id.clone()).collect();
        assert_eq!(ids, vec![BoxId::Node("b".into()), BoxId::Node("c".into())]);
    }

    #[test]
    fn astar_clear_line_of_sight_is_two_point_straight() {
        // Boxes sharing a y-band with a clear horizontal gap.
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(300.0, 0.0, 100.0, 60.0);
        let (ovg, srcv, tgtv) = build_ovg(&[], src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let path = astar(&ovg, &srcv, &tgtv, goal).expect("path exists");
        // Straight degenerate: a single horizontal segment => two points, equal y.
        assert_eq!(path.len(), 2, "straight route is two points, got {path:?}");
        assert!((path[0].1 - path[1].1).abs() < 1e-6, "same y => horizontal");
    }

    #[test]
    fn astar_detours_around_blocking_obstacle_orthogonally() {
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(350.0, 0.0, 100.0, 60.0);
        let mid = Obstacle {
            id: BoxId::Node("m".into()),
            rect: r(150.0, -30.0, 80.0, 120.0),
        };
        let (ovg, srcv, tgtv) = build_ovg(std::slice::from_ref(&mid), src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let path = astar(&ovg, &srcv, &tgtv, goal).expect("path exists");
        assert!(path.len() >= 4, "a detour has >= 4 points, got {path:?}");
        for w in path.windows(2) {
            assert!(
                (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                "segment {:?}->{:?} not orthogonal",
                w[0],
                w[1]
            );
        }
        let inf = inflate(mid.rect, ROUTE_MARGIN);
        for &(x, y) in &path {
            assert!(
                !strictly_inside(&inf, x, y),
                "path pierces obstacle at ({x},{y})"
            );
        }
    }

    #[test]
    fn simplify_collapses_collinear_and_duplicates() {
        let pts = vec![
            (0.0, 0.0),
            (0.0, 0.0),
            (10.0, 0.0),
            (20.0, 0.0),
            (20.0, 10.0),
        ];
        assert_eq!(simplify(pts), vec![(0.0, 0.0), (20.0, 0.0), (20.0, 10.0)]);
    }

    #[test]
    fn nudge_separates_coincident_parallel_segments() {
        // Two routes both running horizontally along y = 50 via an INTERIOR
        // segment (first/last segments are anchored and excluded from nudging).
        let mk = |src: &str| Route {
            points: vec![(0.0, 0.0), (0.0, 50.0), (100.0, 50.0), (100.0, 0.0)],
            source: src.into(),
            target: "t".into(),
        };
        let mut routes = vec![mk("a"), mk("b")];
        nudge(&mut routes);
        let y0 = routes[0].points[1].1;
        let y1 = routes[1].points[1].1;
        assert!(
            (y0 - y1).abs() >= NUDGE_GAP - 1e-6,
            "runs must separate: {y0} vs {y1}"
        );
        // Endpoints untouched.
        assert_eq!(routes[0].points[0], (0.0, 0.0));
        assert_eq!(routes[0].points[3], (100.0, 0.0));
    }
}
