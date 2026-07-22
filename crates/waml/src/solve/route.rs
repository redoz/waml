//! Orthogonal (Manhattan) edge router: OVG -> A* (bend penalty) -> nudge.
//! See docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md.

use super::{Box, BoxId, Rect, Route, SolveConfig};
use std::collections::{BTreeMap, BTreeSet};

fn key_of(id: &BoxId) -> Option<String> {
    match id {
        BoxId::Node(k) => Some(k.clone()),
        _ => None,
    }
}

fn fallback_l(src: Rect, tgt: Rect) -> Vec<P> {
    let s = (src.x + src.w / 2.0, src.y + src.h / 2.0);
    let t = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
    simplify(vec![s, (t.0, s.1), t])
}

/// Route every leaf-to-leaf edge as an orthogonal polyline avoiding obstacles.
pub(super) fn route(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    let membership = build_membership(boxes);
    let mut routes: Vec<Route> = Vec::new();
    for (s, t) in edges {
        if s == t {
            continue; // self-edge: out of scope
        }
        let (Some(source), Some(target)) = (key_of(s), key_of(t)) else {
            continue; // group-as-endpoint: out of scope
        };
        let (Some(&src), Some(&tgt)) = (rects.get(s), rects.get(t)) else {
            continue; // endpoint not in this diagram
        };
        // Leaf rects are always obstacles; a group is an obstacle for THIS edge
        // only when neither endpoint is one of its (transitive) members.
        let mut obstacles = leaf_obstacles(rects, &[s.clone(), t.clone()]);
        obstacles.extend(group_obstacles(rects, &membership, s, t));
        obstacles.sort_by(|a, b| a.id.cmp(&b.id)); // deterministic order
        let (ovg, srcv, tgtv) = build_ovg(&obstacles, src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let points = astar(&ovg, &srcv, &tgtv, goal).unwrap_or_else(|| fallback_l(src, tgt));
        routes.push(Route {
            points,
            source,
            target,
        });
    }
    hub_spread(&mut routes, rects);
    nudge(&mut routes);
    routes
}

/// Transitive leaf membership per group, taken from the `Box` forest child
/// lists — NEVER inferred from rect overlap.
struct Membership {
    members: BTreeMap<BoxId, BTreeSet<BoxId>>,
}

impl Membership {
    fn is_member(&self, group: &BoxId, leaf: &BoxId) -> bool {
        self.members.get(group).is_some_and(|s| s.contains(leaf))
    }
}

fn build_membership(boxes: &[Box]) -> Membership {
    let by_id: BTreeMap<BoxId, &Box> = boxes.iter().map(|b| (b.id.clone(), b)).collect();
    fn leaves(id: &BoxId, by_id: &BTreeMap<BoxId, &Box>, out: &mut BTreeSet<BoxId>) {
        let Some(b) = by_id.get(id) else { return };
        for c in &b.children {
            if matches!(c, BoxId::Node(_)) {
                out.insert(c.clone());
            }
            leaves(c, by_id, out);
        }
    }
    let mut members = BTreeMap::new();
    for b in boxes {
        if matches!(b.id, BoxId::Group(_)) {
            let mut set = BTreeSet::new();
            leaves(&b.id, &by_id, &mut set);
            members.insert(b.id.clone(), set);
        }
    }
    Membership { members }
}

/// Group rects that block THIS edge: a group is an obstacle only when neither
/// endpoint is one of its (transitive) members.
fn group_obstacles(
    rects: &BTreeMap<BoxId, Rect>,
    membership: &Membership,
    s: &BoxId,
    t: &BoxId,
) -> Vec<Obstacle> {
    let mut out: Vec<Obstacle> = rects
        .iter()
        .filter(|(id, _)| matches!(id, BoxId::Group(_)))
        .filter(|(id, _)| !membership.is_member(id, s) && !membership.is_member(id, t))
        .map(|(id, r)| Obstacle {
            id: id.clone(),
            rect: *r,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
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

/// A side of a box's border, used for hub-attachment grouping.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

fn side_of(bx: &Rect, p: P) -> Option<Side> {
    let e = 1e-6;
    if (p.0 - bx.x).abs() < e {
        Some(Side::Left)
    } else if (p.0 - (bx.x + bx.w)).abs() < e {
        Some(Side::Right)
    } else if (p.1 - bx.y).abs() < e {
        Some(Side::Top)
    } else if (p.1 - (bx.y + bx.h)).abs() < e {
        Some(Side::Bottom)
    } else {
        None
    }
}

/// A route endpoint (source or target attachment) landing on a box's border.
struct End {
    ri: usize,
    ep: usize,
    nb: usize,
    along: f64,
}

/// Spread route endpoints that land on the same side of the same box into
/// evenly-spaced, distinct attachment points along that side (no two edges
/// share an attachment point). Rewrites the endpoint and the adjacent
/// interior point that shares its coordinate so the first/last segment stays
/// perpendicular to the border.
fn hub_spread(routes: &mut [Route], rects: &BTreeMap<BoxId, Rect>) {
    let mut groups: BTreeMap<(String, u8), Vec<End>> = BTreeMap::new();
    let sd = |s: Side| match s {
        Side::Left => 0u8,
        Side::Right => 1,
        Side::Top => 2,
        Side::Bottom => 3,
    };

    for (ri, route) in routes.iter().enumerate() {
        if route.points.len() < 2 {
            continue;
        }
        let last = route.points.len() - 1;
        for (key, ep, nb) in [
            (route.source.clone(), 0usize, 1usize),
            (route.target.clone(), last, last - 1),
        ] {
            let Some(bx) = rects.get(&BoxId::Node(key.clone())) else {
                continue;
            };
            let p = route.points[ep];
            let Some(side) = side_of(bx, p) else {
                continue;
            };
            let neighbour = route.points[nb];
            let along = match side {
                Side::Left | Side::Right => neighbour.1,
                Side::Top | Side::Bottom => neighbour.0,
            };
            groups
                .entry((key, sd(side)))
                .or_default()
                .push(End { ri, ep, nb, along });
        }
    }

    for ((key, sdisc), mut ends) in groups {
        if ends.len() < 2 {
            continue;
        }
        let bx = rects[&BoxId::Node(key)];
        ends.sort_by(|a, b| a.along.total_cmp(&b.along).then(a.ri.cmp(&b.ri)));
        let m = ends.len();
        let horizontal_side = sdisc == 2 || sdisc == 3; // Top/Bottom spread along x
        let (span_lo, span_hi, fixed) = if horizontal_side {
            (
                bx.x,
                bx.x + bx.w,
                if sdisc == 2 { bx.y } else { bx.y + bx.h },
            )
        } else {
            (
                bx.y,
                bx.y + bx.h,
                if sdisc == 0 { bx.x } else { bx.x + bx.w },
            )
        };
        for (k, e) in ends.iter().enumerate() {
            let t = (k as f64 + 1.0) / (m as f64 + 1.0); // interior fraction, no corners
            let along = span_lo + t * (span_hi - span_lo);
            if horizontal_side {
                routes[e.ri].points[e.ep] = (along, fixed);
                routes[e.ri].points[e.nb].0 = along; // keep first/last segment perpendicular
            } else {
                routes[e.ri].points[e.ep] = (fixed, along);
                routes[e.ri].points[e.nb].1 = along;
            }
        }
    }
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

    #[test]
    fn hub_spread_gives_distinct_attachment_points() {
        // Hub `h`: three edges all attaching at the same right-side midpoint.
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("h".into()), r(0.0, 0.0, 100.0, 90.0));
        rects.insert(BoxId::Node("t1".into()), r(300.0, 0.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t2".into()), r(300.0, 40.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t3".into()), r(300.0, 80.0, 60.0, 30.0));
        let mk = |t: &str, ty: f64| Route {
            points: vec![(100.0, 45.0), (300.0, ty)],
            source: "h".into(),
            target: t.into(),
        };
        let mut routes = vec![mk("t1", 15.0), mk("t2", 55.0), mk("t3", 95.0)];
        hub_spread(&mut routes, &rects);
        let ys: Vec<f64> = routes.iter().map(|rt| rt.points[0].1).collect();
        for rt in &routes {
            assert!(
                (rt.points[0].0 - 100.0).abs() < 1e-6,
                "stay on right border"
            );
        }
        assert!(
            (ys[0] - ys[1]).abs() > 1e-6
                && (ys[1] - ys[2]).abs() > 1e-6
                && (ys[0] - ys[2]).abs() > 1e-6,
            "attachments must be distinct: {ys:?}"
        );
    }

    use crate::solve::{BoxKind, FlagSet, SolveConfig};
    use crate::syntax::{Axis, Margin, Shape};

    fn nrect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    fn leafbox(k: &str) -> Box {
        Box {
            id: BoxId::Node(k.into()),
            kind: BoxKind::Leaf,
            children: vec![],
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: 0,
        }
    }

    #[test]
    fn route_two_clear_boxes_is_straight_segment() {
        let boxes = vec![leafbox("a"), leafbox("b")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(300.0, 0.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "a");
        assert_eq!(out[0].target, "b");
        assert_eq!(
            out[0].points.len(),
            2,
            "clear LOS => straight: {:?}",
            out[0].points
        );
    }

    #[test]
    fn route_detours_around_third_box() {
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert!(out[0].points.len() >= 4, "detour: {:?}", out[0].points);
        let inf = inflate(nrect(150.0, -30.0, 80.0, 120.0), ROUTE_MARGIN);
        for &(x, y) in &out[0].points {
            assert!(!strictly_inside(&inf, x, y));
        }
    }

    #[test]
    fn route_skips_self_edges_and_unknown_endpoints() {
        let boxes = vec![leafbox("a")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        let edges = vec![
            (BoxId::Node("a".into()), BoxId::Node("a".into())), // self
            (BoxId::Node("a".into()), BoxId::Node("ghost".into())), // unknown target
        ];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert!(
            out.is_empty(),
            "self + unknown edges produce no routes: {out:?}"
        );
    }

    #[test]
    fn route_is_deterministic() {
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
        let edges = vec![
            (BoxId::Node("a".into()), BoxId::Node("b".into())),
            (BoxId::Node("a".into()), BoxId::Node("b".into())), // parallel
        ];
        let a = route(&boxes, &rects, &edges, &SolveConfig::default());
        let b = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(a, b, "identical input => identical routes");
        assert_ne!(a[0].points, a[1].points, "parallels separated");
        // silence unused import warning in this fixture-heavy module:
        let _ = Axis::Row;
    }

    fn groupbox(id: u32, children: Vec<BoxId>) -> Box {
        Box {
            id: BoxId::Group(id),
            kind: BoxKind::Group,
            children,
            axis: Some(Axis::Column),
            shape: Shape::Frame,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: Some("G".into()),
            depth: 0,
        }
    }

    #[test]
    fn membership_is_transitive_via_child_lists() {
        let boxes = vec![
            leafbox("a"),
            groupbox(1, vec![BoxId::Node("a".into())]),
            groupbox(0, vec![BoxId::Group(1)]),
        ];
        let m = build_membership(&boxes);
        assert!(m.is_member(&BoxId::Group(0), &BoxId::Node("a".into())));
        assert!(m.is_member(&BoxId::Group(1), &BoxId::Node("a".into())));
        assert!(!m.is_member(&BoxId::Group(0), &BoxId::Node("b".into())));
    }

    #[test]
    fn member_edge_crosses_group_frame_freely() {
        // "a" inside g0, "b" outside; the group is transparent to a->b.
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            groupbox(0, vec![BoxId::Node("a".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(20.0, 20.0, 100.0, 60.0));
        rects.insert(BoxId::Group(0), nrect(0.0, 0.0, 140.0, 100.0));
        rects.insert(BoxId::Node("b".into()), nrect(300.0, 20.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].points.len(),
            2,
            "member edge is straight: {:?}",
            out[0].points
        );
    }

    #[test]
    fn non_member_edge_detours_around_group() {
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("x"),
            groupbox(0, vec![BoxId::Node("x".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(400.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("x".into()), nrect(200.0, -10.0, 80.0, 40.0));
        rects.insert(BoxId::Group(0), nrect(180.0, -40.0, 120.0, 140.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        assert!(
            out[0].points.len() >= 4,
            "non-member edge detours: {:?}",
            out[0].points
        );
        let inf = inflate(nrect(180.0, -40.0, 120.0, 140.0), ROUTE_MARGIN);
        for &(px, py) in &out[0].points {
            assert!(!strictly_inside(&inf, px, py), "pierces group at ({px},{py})");
        }
    }

    #[test]
    fn membership_by_child_list_not_rect_overlap() {
        // "a"'s rect sits INSIDE g0's rect but "a" is NOT a child of g0, so
        // membership-by-child-list must keep g0 a solid obstacle for the a->b
        // edge. Asserted directly on membership + group_obstacles (NOT on a
        // route point count): a non-member endpoint whose rect is deep inside a
        // group's rect is geometrically landlocked, so route() correctly falls
        // back to a straight segment — the invariant under test is *containment
        // decided by child list, never rect overlap*, which is what we check.
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("x"),
            groupbox(0, vec![BoxId::Node("x".into())]),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Group(0), nrect(0.0, 0.0, 260.0, 200.0));
        rects.insert(BoxId::Node("x".into()), nrect(10.0, 10.0, 60.0, 40.0));
        rects.insert(BoxId::Node("a".into()), nrect(90.0, 80.0, 60.0, 40.0)); // rect inside g0
        rects.insert(BoxId::Node("b".into()), nrect(500.0, 80.0, 60.0, 40.0));
        let membership = build_membership(&boxes);
        // Rect overlap does NOT make "a" a member of g0.
        assert!(!membership.is_member(&BoxId::Group(0), &BoxId::Node("a".into())));
        // Therefore g0 stays an obstacle for the a->b edge (child list decides).
        let obs = group_obstacles(
            &rects,
            &membership,
            &BoxId::Node("a".into()),
            &BoxId::Node("b".into()),
        );
        assert!(
            obs.iter().any(|o| o.id == BoxId::Group(0)),
            "g0 must remain an obstacle: membership is by child list, not rect overlap"
        );
    }
}
