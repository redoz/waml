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
fn segment_blocked(inflated: &[Rect], a: P, b: P) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    inflated.iter().any(|r| {
        let ox0 = r.x.max(x0);
        let ox1 = (r.x + r.w).min(x1);
        let oy0 = r.y.max(y0);
        let oy1 = (r.y + r.h).min(y1);
        // Positive overlap on BOTH axes => the segment cuts the interior.
        (ox1 - ox0) > 1e-9 && (oy1 - oy0) > 1e-9
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
}
