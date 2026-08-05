//! Orthogonal (Manhattan) edge router: OVG -> A* (bend penalty) -> nudge.
//! See docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md.

use super::{Box, BoxId, Rect, Route, SolveConfig};
use std::collections::{BTreeMap, BTreeSet};

/// One edge as `route_keyed_with` takes it: endpoints, the authored key the
/// produced `Route` is tagged with, and the `(width, height)` of the label band
/// the router should try to keep clear beside it (`None` when unlabelled).
pub type KeyedEdge = (BoxId, BoxId, Option<String>, Option<(f64, f64)>);

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
pub fn route(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId)],
    cfg: &SolveConfig,
) -> Vec<Route> {
    let keyed: Vec<(BoxId, BoxId, Option<String>)> = edges
        .iter()
        .map(|(s, t)| (s.clone(), t.clone(), None))
        .collect();
    route_keyed(boxes, rects, &keyed, cfg)
}

/// `route`, but each edge carries the authored key the produced `Route` should
/// be tagged with (`Route::key`). Callers with two edges between the same pair
/// of boxes need this to map routes back to edges.
pub fn route_keyed(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId, Option<String>)],
    cfg: &SolveConfig,
) -> Vec<Route> {
    let keyed: Vec<KeyedEdge> = edges
        .iter()
        .map(|(s, t, key)| (s.clone(), t.clone(), key.clone(), None))
        .collect();
    route_keyed_with(boxes, rects, &keyed, cfg, &RouteCost::default())
}

/// Weights for the router's A* cost function.
///
/// Lifted out of `astar`'s hardcoded constants so layout tuning has one place
/// to live instead of each change inventing its own mechanism. `Default`
/// reproduces the legacy constants exactly: adding a weight must never move a
/// route until that weight is deliberately changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteCost {
    /// Cost per unit of path length.
    pub length: f64,
    /// Cost per direction change.
    pub bend: f64,
    /// Cost per unit of label band that collides with a hard obstacle. Zero in
    /// `Default`, and applied only to edges that actually carry a label, so
    /// unlabelled edges route byte-identically to before this existed.
    pub label_pressure: f64,
}

impl Default for RouteCost {
    fn default() -> Self {
        RouteCost {
            length: 1.0,
            bend: BEND_PENALTY,
            label_pressure: 0.0,
        }
    }
}

/// The routing inputs an edge yields, or `None` when `route_keyed_with` emits
/// no `Route` for it at all: a self-edge, a group-as-endpoint, or an endpoint
/// missing from this diagram's rects. THE one place that skip rule lives, so
/// `routable_edge_indices` can never drift out of step with the router.
fn routable(
    rects: &BTreeMap<BoxId, Rect>,
    s: &BoxId,
    t: &BoxId,
) -> Option<(String, String, Rect, Rect)> {
    if s == t {
        return None; // self-edge: out of scope
    }
    let (source, target) = (key_of(s)?, key_of(t)?); // group-as-endpoint: out of scope
    let (&src, &tgt) = (rects.get(s)?, rects.get(t)?); // endpoint not in this diagram
    Some((source, target, src, tgt))
}

/// Indices into `edges` that `route_keyed_with` actually emits a `Route` for,
/// in order: the route at position `i` came from `edges[indices[i]]`.
///
/// Callers that map a ROUTE position back to the edge it was built from need
/// this: the router silently skips edges, so a route position is not an edge
/// index and using one as the other reroutes (and overwrites) the wrong edge.
pub fn routable_edge_indices(
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[(BoxId, BoxId, Option<String>)],
) -> Vec<usize> {
    edges
        .iter()
        .enumerate()
        .filter(|(_, (s, t, _))| routable(rects, s, t).is_some())
        .map(|(i, _)| i)
        .collect()
}

/// `route_keyed`, but with an explicit `RouteCost` rather than the legacy
/// defaults. The seam `label_pressure` and future layout tuning hang off.
///
/// Each edge's fourth field is the size of the label band the router should try
/// to keep clear beside it (`None` for an unlabelled edge). It is a full
/// `(width, height)`: a label needs its HEIGHT of clearance beside a horizontal
/// run but its WIDTH beside a vertical one.
pub fn route_keyed_with(
    boxes: &[Box],
    rects: &BTreeMap<BoxId, Rect>,
    edges: &[KeyedEdge],
    _cfg: &SolveConfig,
    cost: &RouteCost,
) -> Vec<Route> {
    let membership = build_membership(boxes);
    // P-3: the full obstacle candidate list (every leaf + every group, sorted
    // by id) is invariant across edges, so build it ONCE per solve and mask it
    // per edge instead of walking `rects` and re-sorting for every edge. A
    // filtered subsequence of a sorted list is sorted, so the per-edge result
    // is byte-identical to the old build-then-sort.
    let mut all_obstacles: Vec<Obstacle> = rects
        .iter()
        .map(|(id, r)| Obstacle {
            id: id.clone(),
            rect: *r,
        })
        .collect();
    all_obstacles.sort_by(|a, b| a.id.cmp(&b.id)); // deterministic order
    let mut routes: Vec<Route> = Vec::new();
    for (s, t, key, label_size) in edges {
        let Some((source, target, src, tgt)) = routable(rects, s, t) else {
            continue;
        };
        // Leaf rects are always obstacles (except this edge's endpoints); a
        // group is an obstacle for THIS edge only when neither endpoint is one
        // of its (transitive) members.
        let obstacles: Vec<Obstacle> = all_obstacles
            .iter()
            .filter(|o| match &o.id {
                BoxId::Node(_) => o.id != *s && o.id != *t,
                BoxId::Group(_) => {
                    !membership.is_member(&o.id, s) && !membership.is_member(&o.id, t)
                }
                BoxId::Inline(_) => false, // never an obstacle (matches leaf/group_obstacles)
            })
            .cloned()
            .collect();
        let (ovg, srcv, tgtv) = build_ovg(&obstacles, src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let inflated: Vec<Rect> = obstacles
            .iter()
            .map(|o| inflate(o.rect, ROUTE_MARGIN))
            .collect();
        let points = astar(&ovg, &srcv, &tgtv, goal, cost, &inflated, *label_size)
            .unwrap_or_else(|| fallback_l(src, tgt));
        routes.push(Route {
            points,
            source,
            target,
            key: key.clone(),
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
/// endpoint is one of its (transitive) members. (Production now applies this
/// rule as a mask over the per-solve `all_obstacles` list — see
/// `route_keyed_with`; kept as the executable spec the tests assert against.)
#[cfg(test)]
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

/// Perpendicular exit distance a connector travels straight off a node border
/// before it may bend, and (same value) the keep-out each node is inflated by
/// for the A* grid -- the two are one constant because the stub tip lands on the
/// inflated ring, which is where it joins the grid.
///
/// Sized to seat the largest terminal adornment on the straight stub: a
/// composition/aggregation diamond reaches back `2 * marker_size` from the
/// border (see `canvas.rs` `marker_geometry`; `marker_size` is ~10 world units
/// at 1:1 zoom, so ~20), plus a little slack so the line still shows past the
/// glyph. If the stub were shorter, the diamond's tail would overshoot the bend
/// and stick out perpendicular to the routed line. Keep >= that reach; the
/// connected-pair gutter (`MIN_ASSOC`) already clears two facing stubs.
const ROUTE_MARGIN: f64 = 24.0;

/// Keep border attachment points at least this far from a box corner: a
/// connector meeting a node right on its corner reads as ambiguous/ugly, so
/// grid-derived attach candidates in the corner band are dropped. Side
/// midpoints are always kept, so every side still offers a candidate even when
/// the box is shorter than `2 * CORNER_INSET`.
const CORNER_INSET: f64 = 12.0;

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
    inflated.iter().any(|r| segment_cuts(r, a, b))
}

/// The single-rect body of `segment_blocked`: does the axis-aligned segment
/// (a..b) pass through THIS inflated rect's interior? Split out so the slab
/// index can prune to candidate rects and still apply the exact same predicate.
fn segment_cuts(r: &Rect, a: P, b: P) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let degenerate_x = (x1 - x0).abs() < 1e-9;
    let degenerate_y = (y1 - y0).abs() < 1e-9;
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
}

/// One-axis slab decomposition over the inflated obstacle spans (P-3).
///
/// `bounds` is the sorted, exactly-deduped list of every span endpoint; gap `g`
/// is the half-open interval `[bounds[g], bounds[g+1])`, and `active[g]` lists
/// the spans covering that whole gap. `candidates(x)` returns a SUPERSET of
/// the spans strictly containing `x` (proof: a span with `lo + 1e-9 < x` and
/// `hi - 1e-9 > x` has `lo <= bounds[pos-1]` and `hi >= bounds[pos]`, because
/// `lo`/`hi` are themselves bounds, so it covers `x`'s gap). Callers MUST
/// re-apply the exact per-rect predicate to each candidate — the index only
/// prunes, it never decides, so results stay byte-identical to a linear scan.
struct SlabIndex {
    bounds: Vec<f64>,
    active: Vec<Vec<usize>>,
}

impl SlabIndex {
    fn new(spans: &[(f64, f64)]) -> Self {
        let mut bounds: Vec<f64> = spans.iter().flat_map(|&(a, b)| [a, b]).collect();
        bounds.sort_by(f64::total_cmp);
        bounds.dedup(); // exact dedup: the superset proof needs lo/hi ∈ bounds verbatim
        let gaps = bounds.len().saturating_sub(1);
        let mut active: Vec<Vec<usize>> = vec![Vec::new(); gaps];
        for (i, &(lo, hi)) in spans.iter().enumerate() {
            let g0 = bounds.partition_point(|b| *b < lo);
            let g1 = bounds.partition_point(|b| *b < hi);
            for slot in &mut active[g0..g1] {
                slot.push(i);
            }
        }
        SlabIndex { bounds, active }
    }

    /// Indices of spans that could strictly contain `x` (superset; see above).
    fn candidates(&self, x: f64) -> &[usize] {
        let pos = self.bounds.partition_point(|b| *b <= x);
        if pos == 0 || pos > self.active.len() {
            return &[];
        }
        &self.active[pos - 1]
    }
}

/// (Production now applies this rule as a mask over the per-solve
/// `all_obstacles` list — see `route_keyed_with`; kept as the executable spec
/// the tests assert against.)
#[cfg(test)]
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

    // Interesting coordinates: inflated obstacle borders + endpoint box borders
    // + endpoint box centre lines. The centre lines give the side-midpoint
    // attach points a grid line to join, so the router can leave a border away
    // from its corners (attach candidates in the corner band are dropped -- see
    // CORNER_INSET); without them the only grid-aligned attach on an axis-aligned
    // box would be its corners.
    let mut xs = vec![
        src.x,
        src.x + src.w,
        src.x + src.w / 2.0,
        tgt.x,
        tgt.x + tgt.w,
        tgt.x + tgt.w / 2.0,
    ];
    let mut ys = vec![
        src.y,
        src.y + src.h,
        src.y + src.h / 2.0,
        tgt.y,
        tgt.y + tgt.h,
        tgt.y + tgt.h / 2.0,
    ];
    for r in &inflated {
        xs.push(r.x);
        xs.push(r.x + r.w);
        ys.push(r.y);
        ys.push(r.y + r.h);
    }
    let xs = axis_coords(xs);
    let ys = axis_coords(ys);

    // P-3: slab indexes over the inflated obstacle spans. Every containment /
    // blocking query below prunes through these to a candidate superset and
    // then re-applies the EXACT original per-rect predicate, so the graph is
    // byte-identical to the old all-rects linear scans — just without the
    // O(N) rect walk per grid point / per segment.
    let x_spans: Vec<(f64, f64)> = inflated.iter().map(|r| (r.x, r.x + r.w)).collect();
    let y_spans: Vec<(f64, f64)> = inflated.iter().map(|r| (r.y, r.y + r.h)).collect();
    let x_slab = SlabIndex::new(&x_spans);
    let y_slab = SlabIndex::new(&y_spans);
    // Exact-equivalent fast `segment_blocked` for the axis-aligned segments the
    // OVG uses. A degenerate axis means a fixed coordinate: prune by that
    // coordinate's slab, then run the full per-rect test on the candidates.
    let seg_blocked = |a: P, b: P| -> bool {
        let degenerate_x = (a.0 - b.0).abs() < 1e-9;
        let degenerate_y = (a.1 - b.1).abs() < 1e-9;
        let idxs = if degenerate_x {
            x_slab.candidates(a.0.min(b.0))
        } else if degenerate_y {
            y_slab.candidates(a.1.min(b.1))
        } else {
            return segment_blocked(&inflated, a, b); // never hit: OVG is orthogonal
        };
        idxs.iter().any(|&i| segment_cuts(&inflated[i], a, b))
    };

    // Grid intersections that are not strictly inside any inflated obstacle.
    let mut verts: Vec<P> = Vec::new();
    let mut at: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (xi, &x) in xs.iter().enumerate() {
        let col = x_slab.candidates(x);
        for (yi, &y) in ys.iter().enumerate() {
            if col.iter().any(|&i| strictly_inside(&inflated[i], x, y)) {
                continue;
            }
            at.insert((xi, yi), verts.len());
            verts.push((x, y));
        }
    }

    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); verts.len()];
    let connect = |verts: &Vec<P>, adj: &mut Vec<Vec<(usize, f64)>>, i: usize, j: usize| {
        let (a, b) = (verts[i], verts[j]);
        if seg_blocked(a, b) {
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

    // P-3: grid vertices bucketed by row / column so `attach` can wire a stub
    // by looking only at the vertices that share its axis line, instead of
    // scanning every vertex per candidate. Alignment uses a < 1e-9 tolerance,
    // so `near` collects every axis line within that window (superset) and the
    // exact per-vertex aligned test still decides.
    let grid_len = verts.len();
    let mut col_verts: Vec<Vec<usize>> = vec![Vec::new(); xs.len()];
    let mut row_verts: Vec<Vec<usize>> = vec![Vec::new(); ys.len()];
    for (&(xi, yi), &vi) in &at {
        col_verts[xi].push(vi);
        row_verts[yi].push(vi);
    }
    let near = |coords: &[f64], v: f64| -> std::ops::Range<usize> {
        let lo = coords.partition_point(|c| *c < v - 1e-9);
        let hi = coords.partition_point(|c| *c <= v + 1e-9);
        lo..hi
    };

    // Free-perimeter attachment candidates for one endpoint box. Each candidate
    // is an on-border point `p` on side S paired with a mandatory perpendicular
    // STUB vertex `p' = p + ROUTE_MARGIN * outward_normal(S)`. The stub segment
    // `p <-> p'` is the ONLY edge on the on-border vertex, so every A* path is
    // forced to leave (or enter) perpendicular to the border for at least
    // ROUTE_MARGIN before any grid movement -- parallel/hugging exits cannot
    // exist in the adjacency. Only the stub `p'` joins the grid (aligned,
    // unblocked). A stub that reaches no grid vertex (e.g. it pokes into a
    // neighbour's inflated zone) simply contributes an unusable candidate; if
    // every stub is blocked A* finds no path and `route()` falls back to
    // `fallback_l`. `on_border` never receives a second edge, keeping the
    // one-neighbour invariant across BOTH attach calls (src stub may still wire
    // into tgt stubs and vice versa).
    let attach = |verts: &mut Vec<P>,
                  adj: &mut Vec<Vec<(usize, f64)>>,
                  on_border: &mut BTreeSet<usize>,
                  bx: Rect|
     -> Vec<usize> {
        let mut cands: Vec<(P, Side)> = Vec::new();
        // Corner band is excluded (see CORNER_INSET): a grid line that only
        // meets the side within CORNER_INSET of a corner yields no candidate.
        for &y in &ys {
            if y >= bx.y + CORNER_INSET - 1e-9 && y <= bx.y + bx.h - CORNER_INSET + 1e-9 {
                cands.push(((bx.x, y), Side::Left));
                cands.push(((bx.x + bx.w, y), Side::Right));
            }
        }
        for &x in &xs {
            if x >= bx.x + CORNER_INSET - 1e-9 && x <= bx.x + bx.w - CORNER_INSET + 1e-9 {
                cands.push(((x, bx.y), Side::Top));
                cands.push(((x, bx.y + bx.h), Side::Bottom));
            }
        }
        // Side midpoints guarantee at least one candidate per side.
        cands.push(((bx.x, bx.y + bx.h / 2.0), Side::Left));
        cands.push(((bx.x + bx.w, bx.y + bx.h / 2.0), Side::Right));
        cands.push(((bx.x + bx.w / 2.0, bx.y), Side::Top));
        cands.push(((bx.x + bx.w / 2.0, bx.y + bx.h), Side::Bottom));
        cands.sort_by(|(pa, sa), (pb, sb)| {
            pa.0.total_cmp(&pb.0)
                .then(pa.1.total_cmp(&pb.1))
                .then(sa.cmp(sb))
        });
        // Dedup by point AND side: a corner keeps both of its sides (each with
        // its own perpendicular stub direction).
        cands.dedup_by(|(pa, sa), (pb, sb)| {
            (pa.0 - pb.0).abs() < 1e-9 && (pa.1 - pb.1).abs() < 1e-9 && sa == sb
        });

        let mut idxs = Vec::new();
        for (pt, side) in cands {
            // On-border vertex: its sole neighbour is the stub below.
            let bi = verts.len();
            verts.push(pt);
            adj.push(Vec::new());
            on_border.insert(bi);
            // Stub vertex, ROUTE_MARGIN out along the side's outward normal.
            let nrm = outward_normal(side);
            let stub = (pt.0 + ROUTE_MARGIN * nrm.0, pt.1 + ROUTE_MARGIN * nrm.1);
            let si = verts.len();
            verts.push(stub);
            adj.push(Vec::new());
            // Mandatory perpendicular stub segment p <-> p'.
            adj[bi].push((si, ROUTE_MARGIN));
            adj[si].push((bi, ROUTE_MARGIN));
            // Only the stub joins the grid; never wire into an on-border vertex.
            // P-3: instead of scanning every vertex, collect the SUPERSET of
            // possibly-aligned ones — grid vertices on an axis line within the
            // alignment tolerance of the stub, plus every attach-added vertex
            // so far — sorted ascending so the adjacency push order (and thus
            // A* tie-breaking) is identical to the old `0..si` scan. The exact
            // aligned test below still decides.
            let mut cand: Vec<usize> = Vec::new();
            for xi in near(&xs, stub.0) {
                cand.extend_from_slice(&col_verts[xi]);
            }
            for yi in near(&ys, stub.1) {
                cand.extend_from_slice(&row_verts[yi]);
            }
            cand.extend(grid_len..si);
            cand.sort_unstable();
            cand.dedup();
            for gi in cand {
                if on_border.contains(&gi) {
                    continue;
                }
                let g = verts[gi];
                let aligned = (g.0 - stub.0).abs() < 1e-9 || (g.1 - stub.1).abs() < 1e-9;
                if aligned && !seg_blocked(stub, g) {
                    let len = (g.0 - stub.0).abs() + (g.1 - stub.1).abs();
                    adj[si].push((gi, len));
                    adj[gi].push((si, len));
                }
            }
            idxs.push(bi);
        }
        idxs
    };

    let mut on_border: BTreeSet<usize> = BTreeSet::new();
    let srcv = attach(&mut verts, &mut adj, &mut on_border, src);
    let tgtv = attach(&mut verts, &mut adj, &mut on_border, tgt);
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

/// Sum, over a set of intervals clipped to `[s0, s1]`, of the length they
/// cover -- a union, so overlapping obstacle intervals are not double-counted.
fn interval_union_len(mut ivs: Vec<(f64, f64)>) -> f64 {
    ivs.retain(|&(a, b)| b > a);
    ivs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    let mut cur: Option<(f64, f64)> = None;
    for (a, b) in ivs {
        match cur {
            None => cur = Some((a, b)),
            Some((c0, c1)) => {
                if a <= c1 {
                    cur = Some((c0, c1.max(b)));
                } else {
                    total += c1 - c0;
                    cur = Some((a, b));
                }
            }
        }
    }
    if let Some((c0, c1)) = cur {
        total += c1 - c0;
    }
    total
}

/// Fraction (0..=1) of segment `a..b` whose label band collides with a hard
/// obstacle, taking the BETTER of the two sides a label could sit on -- a
/// label only needs one clear side. `inflated` is the same inflated obstacle
/// list A* routes against, so "clear" here means the same margin the route
/// itself keeps.
///
/// `size` is the label's `(width, height)`, and the band's THICKNESS is picked
/// per segment orientation: a label beside a horizontal run needs its height of
/// clearance, but beside a vertical one it needs its (typically much larger)
/// width. Using the height for both under-penalised every vertical run.
fn band_blocked_fraction(inflated: &[Rect], a: P, b: P, size: (f64, f64)) -> f64 {
    let len = (b.0 - a.0).abs() + (b.1 - a.1).abs();
    if len < 1e-9 {
        return 0.0;
    }
    let horizontal = (a.1 - b.1).abs() < 1e-9;
    let height = if horizontal { size.1 } else { size.0 };
    if height <= 0.0 || inflated.is_empty() {
        return 0.0;
    }
    let (s0, s1) = if horizontal {
        (a.0.min(b.0), a.0.max(b.0))
    } else {
        (a.1.min(b.1), a.1.max(b.1))
    };
    let y0 = a.1; // horizontal case: shared y; vertical case: shared x is a.0
    let x0 = a.0;
    let mut best = f64::INFINITY;
    for sign in [-1.0f64, 1.0f64] {
        let (band_lo, band_hi) = if horizontal {
            if sign < 0.0 {
                (y0 - height, y0)
            } else {
                (y0, y0 + height)
            }
        } else if sign < 0.0 {
            (x0 - height, x0)
        } else {
            (x0, x0 + height)
        };
        let ivs: Vec<(f64, f64)> = inflated
            .iter()
            .filter(|r| {
                if horizontal {
                    r.y < band_hi && r.y + r.h > band_lo
                } else {
                    r.x < band_hi && r.x + r.w > band_lo
                }
            })
            .map(|r| {
                if horizontal {
                    (r.x.max(s0), (r.x + r.w).min(s1))
                } else {
                    (r.y.max(s0), (r.y + r.h).min(s1))
                }
            })
            .collect();
        let blocked = interval_union_len(ivs);
        let frac = (blocked / len).min(1.0);
        if frac < best {
            best = frac;
        }
    }
    best
}

fn astar(
    ovg: &Ovg,
    sources: &[usize],
    targets: &[usize],
    goal: P,
    cost: &RouteCost,
    inflated: &[Rect],
    label_size: Option<(f64, f64)>,
) -> Option<Vec<P>> {
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
    // Blocked-fraction is expensive (rect queries) and the same OVG edge is
    // relaxed many times during the search, so cache it per (v, w) pair.
    let pressured =
        cost.label_pressure > 0.0 && label_size.is_some_and(|(w, h)| w > 0.0 || h > 0.0);
    let mut band_cache: BTreeMap<(usize, usize), f64> = BTreeMap::new();

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
            let bend = if d != 0 && d != nd { cost.bend } else { 0.0 };
            let pressure = if pressured {
                let key = (v.min(w), v.max(w));
                let frac = *band_cache.entry(key).or_insert_with(|| {
                    band_blocked_fraction(
                        inflated,
                        ovg.verts[v],
                        ovg.verts[w],
                        label_size.unwrap_or((0.0, 0.0)),
                    )
                });
                cost.label_pressure * frac * len
            } else {
                0.0
            };
            let ng = g + len * cost.length + bend + pressure;
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Which side of `bx` a route endpoint attaches to, disambiguated by the
/// direction of its perpendicular stub (the `ep -> nb` first/last segment).
/// `side_of` alone is ambiguous at a corner (a corner lies on two sides and
/// picks the first by fixed priority), but the stub direction reveals the real
/// side: a horizontal stub means a vertical (Left/Right) border, a vertical stub
/// a horizontal (Top/Bottom) border. Falls back to `side_of` only when the stub
/// is degenerate (coincident points).
fn attach_side(bx: &Rect, ep: P, nb: P) -> Option<Side> {
    let e = 1e-6;
    let horizontal = (ep.1 - nb.1).abs() < e && (ep.0 - nb.0).abs() > e;
    let vertical = (ep.0 - nb.0).abs() < e && (ep.1 - nb.1).abs() > e;
    if horizontal {
        if (ep.0 - bx.x).abs() < e {
            return Some(Side::Left);
        }
        if (ep.0 - (bx.x + bx.w)).abs() < e {
            return Some(Side::Right);
        }
    } else if vertical {
        if (ep.1 - bx.y).abs() < e {
            return Some(Side::Top);
        }
        if (ep.1 - (bx.y + bx.h)).abs() < e {
            return Some(Side::Bottom);
        }
    }
    side_of(bx, ep)
}

/// A route endpoint (source or target attachment) landing on a box's border.
struct End {
    ri: usize,
    ep: usize,
    nb: usize,
    along: f64,
}

/// Unit outward normal of a border side (points away from the box interior).
fn outward_normal(side: Side) -> P {
    match side {
        Side::Left => (-1.0, 0.0),
        Side::Right => (1.0, 0.0),
        Side::Top => (0.0, -1.0),
        Side::Bottom => (0.0, 1.0),
    }
}

/// True when leaving `side` perpendicularly means moving along the x-axis
/// (i.e. the border is vertical — Left/Right).
fn perp_is_horizontal(side: Side) -> bool {
    matches!(side, Side::Left | Side::Right)
}

/// Keep the first/last segment perpendicular to the border after moving an
/// endpoint along it: pull the adjacent INTERIOR bend onto the endpoint's
/// along-coordinate so the stub stays perpendicular. Perpendicular stubs are now
/// structural (see `attach`), so the first hop is always perpendicular and the
/// old parallel-hug guard (only realign when the bend was off-axis) is gone --
/// the rewrite always applies. Only valid when `nb` is a true interior bend,
/// never the opposite (border-attached) endpoint of a 2-point route -- dragging
/// that would slide it off its own box's border (handled by `connect_ends`).
fn realign_interior(points: &mut [P], ep: usize, nb: usize, side: Side) {
    let e = points[ep];
    if perp_is_horizontal(side) {
        points[nb].1 = e.1;
    } else {
        points[nb].0 = e.0;
    }
}

/// Orthogonally connect two border attachment points, each leaving its border
/// perpendicular. A single segment (2-point route) whose endpoints were BOTH
/// spread cannot stay straight without dragging one endpoint off its border, so
/// insert bends: an S when both exits are parallel, an L when perpendicular.
/// `simplify` collapses the polyline back to a straight segment when the two
/// endpoints happen to already line up.
fn connect_ends(s: P, s_side: Option<Side>, t: P, t_side: Option<Side>) -> Vec<P> {
    let s_horiz = s_side.map(perp_is_horizontal);
    let t_horiz = t_side.map(perp_is_horizontal);
    let raw = match (s_horiz, t_horiz) {
        (Some(true), Some(true)) => {
            let mx = (s.0 + t.0) / 2.0;
            vec![s, (mx, s.1), (mx, t.1), t]
        }
        (Some(false), Some(false)) => {
            let my = (s.1 + t.1) / 2.0;
            vec![s, (s.0, my), (t.0, my), t]
        }
        (Some(false), _) => vec![s, (s.0, t.1), t],
        // Source exits horizontally (or its side is unknown -> assume so):
        // bend to the target's x, then run into it.
        _ => vec![s, (t.0, s.1), t],
    };
    simplify(raw)
}

/// Spread route endpoints that land on the same side of the same box into
/// evenly-spaced, distinct attachment points along that side (no two edges
/// share an attachment point). Runs in two passes: first compute every
/// endpoint's spread coordinate, then apply. A multi-point route's endpoint is
/// moved in place and its adjacent interior bend realigned to keep the exit
/// perpendicular. A 2-point route is rebuilt whole via `connect_ends`, because
/// BOTH of its endpoints may be spread (on different boxes) and each must stay
/// on its own border -- the old single-pass code dragged the opposite endpoint
/// off the target, deleting the connecting segment.
fn hub_spread(routes: &mut [Route], rects: &BTreeMap<BoxId, Rect>) {
    let mut groups: BTreeMap<(String, Side), Vec<End>> = BTreeMap::new();

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
            let neighbour = route.points[nb];
            let Some(side) = attach_side(bx, p, neighbour) else {
                continue;
            };
            let along = match side {
                Side::Left | Side::Right => neighbour.1,
                Side::Top | Side::Bottom => neighbour.0,
            };
            groups
                .entry((key, side))
                .or_default()
                .push(End { ri, ep, nb, along });
        }
    }

    // Pass 1: spread. Multi-point routes move in place; 2-point routes only
    // record their new endpoint (both ends may still be spread by another group,
    // and each must stay on its own border) for the rebuild pass.
    let mut moved: BTreeMap<(usize, usize), (P, Side)> = BTreeMap::new();
    for ((key, side), mut ends) in groups {
        if ends.len() < 2 {
            continue;
        }
        let bx = rects[&BoxId::Node(key)];
        ends.sort_by(|a, b| a.along.total_cmp(&b.along).then(a.ri.cmp(&b.ri)));
        let m = ends.len();
        let horizontal_side = matches!(side, Side::Top | Side::Bottom); // Top/Bottom spread along x
        let (span_lo, span_hi, fixed) = if horizontal_side {
            (
                bx.x,
                bx.x + bx.w,
                if side == Side::Top { bx.y } else { bx.y + bx.h },
            )
        } else {
            (
                bx.y,
                bx.y + bx.h,
                if side == Side::Left {
                    bx.x
                } else {
                    bx.x + bx.w
                },
            )
        };
        for (k, e) in ends.iter().enumerate() {
            let t = (k as f64 + 1.0) / (m as f64 + 1.0); // interior fraction, no corners
            let along = span_lo + t * (span_hi - span_lo);
            let new = if horizontal_side {
                (along, fixed)
            } else {
                (fixed, along)
            };
            if routes[e.ri].points.len() == 2 {
                moved.insert((e.ri, e.ep), (new, side));
            } else {
                routes[e.ri].points[e.ep] = new;
                realign_interior(&mut routes[e.ri].points, e.ep, e.nb, side);
            }
        }
    }

    // Pass 2: rebuild each 2-point route whose endpoint(s) were spread, so both
    // endpoints stay on their borders and the connecting segment is preserved.
    let touched: BTreeSet<usize> = moved.keys().map(|(ri, _)| *ri).collect();
    for ri in touched {
        let last = routes[ri].points.len() - 1;
        let s = moved
            .get(&(ri, 0))
            .map_or(routes[ri].points[0], |(p, _)| *p);
        let t = moved
            .get(&(ri, last))
            .map_or(routes[ri].points[last], |(p, _)| *p);
        let s_side = moved.get(&(ri, 0)).map(|(_, sd)| *sd).or_else(|| {
            rects
                .get(&BoxId::Node(routes[ri].source.clone()))
                .and_then(|bx| attach_side(bx, routes[ri].points[0], routes[ri].points[1]))
        });
        let t_side = moved.get(&(ri, last)).map(|(_, sd)| *sd).or_else(|| {
            rects
                .get(&BoxId::Node(routes[ri].target.clone()))
                .and_then(|bx| {
                    attach_side(bx, routes[ri].points[last], routes[ri].points[last - 1])
                })
        });
        routes[ri].points = connect_ends(s, s_side, t, t_side);
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
        // Boxes sharing a y-band with a clear horizontal gap. With perpendicular
        // stubs the path is stub-out + straight run + stub-in; when all three are
        // collinear `simplify` collapses them, but the invariants that matter hold
        // regardless of the exact point count, so assert THOSE rather than a
        // brittle length: the ends are perpendicular to their borders and every
        // segment is orthogonal.
        let src = r(0.0, 0.0, 100.0, 60.0);
        let tgt = r(300.0, 0.0, 100.0, 60.0);
        let (ovg, srcv, tgtv) = build_ovg(&[], src, tgt);
        let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
        let path =
            astar(&ovg, &srcv, &tgtv, goal, &RouteCost::default(), &[], None).expect("path exists");
        assert!(
            path.len() >= 2,
            "path has at least two points, got {path:?}"
        );
        // Source leaves perpendicular to its border for >= ROUTE_MARGIN.
        assert!(
            perp_to_border(&src, path[0], path[1]),
            "source exit not perpendicular: {path:?}"
        );
        assert!(seg_len(path[0], path[1]) >= ROUTE_MARGIN - 1e-6);
        // Target enters perpendicular to its border for >= ROUTE_MARGIN.
        let n = path.len();
        assert!(
            perp_to_border(&tgt, path[n - 1], path[n - 2]),
            "target entry not perpendicular: {path:?}"
        );
        assert!(seg_len(path[n - 1], path[n - 2]) >= ROUTE_MARGIN - 1e-6);
        // Every segment is orthogonal.
        for w in path.windows(2) {
            assert!(
                (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                "diagonal segment {:?}->{:?}",
                w[0],
                w[1]
            );
        }
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
        let path =
            astar(&ovg, &srcv, &tgtv, goal, &RouteCost::default(), &[], None).expect("path exists");
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
            key: None,
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
            key: None,
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

    /// Every route endpoint must stay ON the border of its own box after a
    /// spread. Regression: spreading the hub side dragged the OTHER (opposite)
    /// endpoint off its target's border, so the last segment no longer reached
    /// the target -- the on-screen "vertical part is just gone" bug.
    #[test]
    fn hub_spread_keeps_opposite_endpoint_on_its_target_border() {
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("h".into()), r(0.0, 0.0, 100.0, 90.0));
        rects.insert(BoxId::Node("t1".into()), r(300.0, 0.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t2".into()), r(300.0, 40.0, 60.0, 30.0));
        rects.insert(BoxId::Node("t3".into()), r(300.0, 80.0, 60.0, 30.0));
        let mk = |t: &str, ty: f64| Route {
            points: vec![(100.0, 45.0), (300.0, ty)],
            source: "h".into(),
            target: t.into(),
            key: None,
        };
        let mut routes = vec![mk("t1", 15.0), mk("t2", 55.0), mk("t3", 95.0)];
        hub_spread(&mut routes, &rects);
        for rt in &routes {
            let tgt = rects[&BoxId::Node(rt.target.clone())];
            let last = *rt.points.last().unwrap();
            // Endpoint lands on the target's LEFT border, within its y-extent.
            assert!(
                (last.0 - tgt.x).abs() < 1e-6,
                "{} target endpoint off left border: {last:?}",
                rt.target
            );
            assert!(
                last.1 >= tgt.y - 1e-6 && last.1 <= tgt.y + tgt.h + 1e-6,
                "{} target endpoint {last:?} outside border y-extent [{}, {}]",
                rt.target,
                tgt.y,
                tgt.y + tgt.h
            );
            // Source endpoint stays on the hub's right border.
            assert!(
                (rt.points[0].0 - 100.0).abs() < 1e-6,
                "{} source off hub border: {:?}",
                rt.target,
                rt.points[0]
            );
            // Whole polyline is orthogonal (no diagonal segments).
            for pair in rt.points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert!(
                    (a.0 - b.0).abs() < 1e-6 || (a.1 - b.1).abs() < 1e-6,
                    "{} has a diagonal segment: {a:?}->{b:?}",
                    rt.target
                );
            }
        }
    }

    use crate::layout::{Axis, Margin, Shape};
    use crate::solve::{BoxKind, FlagSet, SolveConfig};

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

    type BentCase = (Vec<Box>, BTreeMap<BoxId, Rect>, Vec<(BoxId, BoxId)>);

    /// A box + rects + edges triple whose route actually bends around an
    /// obstacle, so it exercises the bend penalty (a straight route would not).
    fn three_box_bent_case() -> BentCase {
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        (boxes, rects, edges)
    }

    #[test]
    fn route_cost_default_reproduces_the_legacy_router() {
        let (boxes, rects, edges) = three_box_bent_case();
        let legacy = route(&boxes, &rects, &edges, &SolveConfig::default());
        let via_cost = route_keyed_with(
            &boxes,
            &rects,
            &edges
                .iter()
                .map(|(s, t)| (s.clone(), t.clone(), None, None))
                .collect::<Vec<_>>(),
            &SolveConfig::default(),
            &RouteCost::default(),
        );
        assert_eq!(
            legacy, via_cost,
            "default weights must not move a single point"
        );
    }

    /// A box + rects + edges triple with two viable detours around a central
    /// obstacle: one ("below") is shorter but runs beside a second obstacle
    /// that leaves no room for a label; the other ("above") is a little
    /// longer but has open space on both sides.
    fn corridor_with_tight_and_roomy_paths() -> BentCase {
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("wall"),
            leafbox("squeeze"),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 40.0, 40.0));
        rects.insert(BoxId::Node("b".into()), nrect(400.0, 0.0, 40.0, 40.0));
        // Forces a detour above y=-84 or below y=94 (both inflated boundaries
        // equidistant in principle, but the below route is shorter here).
        rects.insert(BoxId::Node("wall".into()), nrect(180.0, -60.0, 40.0, 130.0));
        // Sits just past the below route's clearance, so a wide label band on
        // that side collides with it while the above route stays open.
        rects.insert(
            BoxId::Node("squeeze".into()),
            nrect(180.0, 120.0, 40.0, 100.0),
        );
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        (boxes, rects, edges)
    }

    /// Tag every edge with a square `height` x `height` label band.
    fn labelled(edges: &[(BoxId, BoxId)], height: f64) -> Vec<KeyedEdge> {
        edges
            .iter()
            .map(|(s, t)| (s.clone(), t.clone(), None, Some((height, height))))
            .collect()
    }

    #[test]
    fn a_vertical_run_is_measured_against_the_label_width() {
        // A label beside a vertical run needs its WIDTH of clearance, not its
        // height. Measuring the band with the height under-penalised every
        // vertical run, so pressure only ever steered horizontal ones.
        let wide_and_short = (80.0, 10.0);
        let vertical = ((0.0, 0.0), (0.0, 100.0));
        // Both sit inside the label's width but well outside its height, so
        // NEITHER side of the run has room -- a label only needs one.
        let obstacle = [
            nrect(20.0, 0.0, 40.0, 100.0),
            nrect(-60.0, 0.0, 40.0, 100.0),
        ];
        assert_eq!(
            band_blocked_fraction(&obstacle, vertical.0, vertical.1, (10.0, 10.0)),
            0.0,
            "a band narrower than the gap is clear"
        );
        assert!(
            band_blocked_fraction(&obstacle, vertical.0, vertical.1, wide_and_short) > 0.9,
            "the obstacle sits inside the label's width, so the band is blocked"
        );
    }

    /// Minimum distance from any point of `points` to the nearer of the two
    /// obstacles that box in `corridor_with_tight_and_roomy_paths`'s corridor.
    fn clearance_beside(points: &[(f64, f64)]) -> f64 {
        let obstacles = [
            nrect(180.0, -60.0, 40.0, 130.0),
            nrect(180.0, 120.0, 40.0, 100.0),
        ];
        let dist_to_rect = |p: (f64, f64), r: &Rect| -> f64 {
            let dx = (r.x - p.0).max(0.0).max(p.0 - (r.x + r.w));
            let dy = (r.y - p.1).max(0.0).max(p.1 - (r.y + r.h));
            (dx * dx + dy * dy).sqrt()
        };
        points
            .iter()
            .flat_map(|&p| obstacles.iter().map(move |r| dist_to_rect(p, r)))
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn label_pressure_steers_a_route_toward_room_for_its_label() {
        let (boxes, rects, edges) = corridor_with_tight_and_roomy_paths();
        let roomy = route_keyed_with(
            &boxes,
            &rects,
            &labelled(&edges, 40.0),
            &SolveConfig::default(),
            &RouteCost {
                label_pressure: 50.0,
                ..RouteCost::default()
            },
        );
        assert_eq!(roomy.len(), 1);
        assert!(
            clearance_beside(&roomy[0].points) >= 40.0,
            "route should leave a label band's worth of room: {:?}",
            roomy[0].points
        );
    }

    #[test]
    fn an_unlabelled_edge_is_untouched_by_label_pressure() {
        let (boxes, rects, edges) = corridor_with_tight_and_roomy_paths();
        let keyed: Vec<KeyedEdge> = edges
            .iter()
            .map(|(s, t)| (s.clone(), t.clone(), None, None))
            .collect();
        let baseline = route_keyed_with(
            &boxes,
            &rects,
            &keyed,
            &SolveConfig::default(),
            &RouteCost::default(),
        );
        let pressured = route_keyed_with(
            &boxes,
            &rects,
            &keyed,
            &SolveConfig::default(),
            &RouteCost {
                label_pressure: 50.0,
                ..RouteCost::default()
            },
        );
        assert_eq!(baseline, pressured, "no label means no pressure");
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
            assert!(
                !strictly_inside(&inf, px, py),
                "pierces group at ({px},{py})"
            );
        }
    }

    #[test]
    fn hub_spread_keeps_every_segment_orthogonal() {
        // Regression: when astar's first segment runs PARALLEL to the hub border
        // (the route leaves by hugging the border, then turns), hub_spread must
        // not rewrite the neighbour's perpendicular coordinate — doing so
        // collapses the first segment and tilts the second into a diagonal, which
        // the corner-to-corner edge shader then draws as a broken connection.
        let boxes = vec![
            leafbox("h"),
            leafbox("t1"),
            leafbox("t2"),
            leafbox("t3"),
            leafbox("t4"),
            leafbox("t5"),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("h".into()), nrect(0.0, 200.0, 120.0, 120.0));
        rects.insert(BoxId::Node("t1".into()), nrect(400.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("t2".into()), nrect(400.0, 120.0, 100.0, 60.0));
        rects.insert(BoxId::Node("t3".into()), nrect(400.0, 240.0, 100.0, 60.0));
        rects.insert(BoxId::Node("t4".into()), nrect(400.0, 360.0, 100.0, 60.0));
        rects.insert(BoxId::Node("t5".into()), nrect(400.0, 480.0, 100.0, 60.0));
        let edges: Vec<(BoxId, BoxId)> = ["t1", "t2", "t3", "t4", "t5"]
            .iter()
            .map(|t| (BoxId::Node("h".into()), BoxId::Node((*t).into())))
            .collect();
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        for r in &out {
            for w in r.points.windows(2) {
                assert!(
                    (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                    "non-orthogonal segment {:?} -> {:?} in {} -> {}: {:?}",
                    w[0],
                    w[1],
                    r.source,
                    r.target,
                    r.points
                );
            }
        }
    }

    /// Manhattan length of an axis-aligned segment.
    fn seg_len(a: P, b: P) -> f64 {
        (a.0 - b.0).abs() + (a.1 - b.1).abs()
    }

    /// Every border side of `bx` that point `p` lies on (two at a corner).
    fn sides_on(bx: &Rect, p: P) -> Vec<Side> {
        let e = 1e-6;
        let mut v = Vec::new();
        let in_y = p.1 >= bx.y - e && p.1 <= bx.y + bx.h + e;
        let in_x = p.0 >= bx.x - e && p.0 <= bx.x + bx.w + e;
        if (p.0 - bx.x).abs() < e && in_y {
            v.push(Side::Left);
        }
        if (p.0 - (bx.x + bx.w)).abs() < e && in_y {
            v.push(Side::Right);
        }
        if (p.1 - bx.y).abs() < e && in_x {
            v.push(Side::Top);
        }
        if (p.1 - (bx.y + bx.h)).abs() < e && in_x {
            v.push(Side::Bottom);
        }
        v
    }

    /// The segment `on_pt -> other` is perpendicular to at least one border side
    /// that `on_pt` lies on (a parallel/hugging exit fails this).
    fn perp_to_border(bx: &Rect, on_pt: P, other: P) -> bool {
        let dx = (other.0 - on_pt.0).abs();
        let dy = (other.1 - on_pt.1).abs();
        let horizontal = dy < 1e-6 && dx > 1e-6;
        let vertical = dx < 1e-6 && dy > 1e-6;
        sides_on(bx, on_pt).iter().any(|s| match s {
            Side::Left | Side::Right => horizontal,
            Side::Top | Side::Bottom => vertical,
        })
    }

    /// Assert the perpendicular-stub invariants for every route in `out`.
    fn assert_perp_ends(out: &[Route], rects: &BTreeMap<BoxId, Rect>) {
        for rt in out {
            assert!(
                rt.points.len() >= 2,
                "{}->{} route degenerate: {:?}",
                rt.source,
                rt.target,
                rt.points
            );
            let src_bx = rects[&BoxId::Node(rt.source.clone())];
            let tgt_bx = rects[&BoxId::Node(rt.target.clone())];
            let n = rt.points.len();
            let (p0, p1) = (rt.points[0], rt.points[1]);
            let (plast, pprev) = (rt.points[n - 1], rt.points[n - 2]);
            assert!(
                perp_to_border(&src_bx, p0, p1),
                "{}->{} source exit not perpendicular: {:?}",
                rt.source,
                rt.target,
                rt.points
            );
            assert!(
                seg_len(p0, p1) >= ROUTE_MARGIN - 1e-6,
                "{}->{} source stub shorter than ROUTE_MARGIN: {:?}",
                rt.source,
                rt.target,
                rt.points
            );
            assert!(
                perp_to_border(&tgt_bx, plast, pprev),
                "{}->{} target entry not perpendicular: {:?}",
                rt.source,
                rt.target,
                rt.points
            );
            assert!(
                seg_len(plast, pprev) >= ROUTE_MARGIN - 1e-6,
                "{}->{} target stub shorter than ROUTE_MARGIN: {:?}",
                rt.source,
                rt.target,
                rt.points
            );
            for w in rt.points.windows(2) {
                assert!(
                    (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                    "{}->{} diagonal segment {:?}->{:?}",
                    rt.source,
                    rt.target,
                    w[0],
                    w[1]
                );
            }
        }
    }

    #[test]
    fn every_route_leaves_and_enters_perpendicular() {
        let cfg = SolveConfig::default();

        // 1. Clear line of sight.
        {
            let boxes = vec![leafbox("a"), leafbox("b")];
            let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
            rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
            rects.insert(BoxId::Node("b".into()), nrect(300.0, 0.0, 100.0, 60.0));
            let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
            let out = route(&boxes, &rects, &edges, &cfg);
            assert_perp_ends(&out, &rects);
        }

        // 2. Detour around a blocking obstacle.
        {
            let boxes = vec![leafbox("a"), leafbox("b"), leafbox("m")];
            let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
            rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
            rects.insert(BoxId::Node("b".into()), nrect(350.0, 0.0, 100.0, 60.0));
            rects.insert(BoxId::Node("m".into()), nrect(150.0, -30.0, 80.0, 120.0));
            let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
            let out = route(&boxes, &rects, &edges, &cfg);
            assert_perp_ends(&out, &rects);
        }

        // 3. Hub fan-out: five edges leaving the same side of a hub.
        {
            let boxes = vec![
                leafbox("h"),
                leafbox("t1"),
                leafbox("t2"),
                leafbox("t3"),
                leafbox("t4"),
                leafbox("t5"),
            ];
            let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
            rects.insert(BoxId::Node("h".into()), nrect(0.0, 200.0, 120.0, 120.0));
            rects.insert(BoxId::Node("t1".into()), nrect(400.0, 0.0, 100.0, 60.0));
            rects.insert(BoxId::Node("t2".into()), nrect(400.0, 120.0, 100.0, 60.0));
            rects.insert(BoxId::Node("t3".into()), nrect(400.0, 240.0, 100.0, 60.0));
            rects.insert(BoxId::Node("t4".into()), nrect(400.0, 360.0, 100.0, 60.0));
            rects.insert(BoxId::Node("t5".into()), nrect(400.0, 480.0, 100.0, 60.0));
            let edges: Vec<(BoxId, BoxId)> = ["t1", "t2", "t3", "t4", "t5"]
                .iter()
                .map(|t| (BoxId::Node("h".into()), BoxId::Node((*t).into())))
                .collect();
            let out = route(&boxes, &rects, &edges, &cfg);
            assert_perp_ends(&out, &rects);
        }
    }

    #[test]
    fn endpoints_never_land_on_a_node_corner() {
        // Diagonally offset nodes tempt the router to attach at the corner
        // nearest the other box. Corner attachments look wrong, so every
        // endpoint must sit a clear margin in from both corners of its side.
        let cfg = SolveConfig::default();
        let boxes = vec![leafbox("a"), leafbox("b")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("b".into()), nrect(260.0, 220.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &cfg);
        assert_eq!(out.len(), 1);
        let rt = &out[0];
        let n = rt.points.len();
        for (key, ep) in [(&rt.source, rt.points[0]), (&rt.target, rt.points[n - 1])] {
            let bx = rects[&BoxId::Node(key.clone())];
            let corners = [
                (bx.x, bx.y),
                (bx.x + bx.w, bx.y),
                (bx.x, bx.y + bx.h),
                (bx.x + bx.w, bx.y + bx.h),
            ];
            let nearest = corners
                .iter()
                .map(|c| seg_len(*c, ep))
                .fold(f64::INFINITY, f64::min);
            assert!(
                nearest >= 8.0,
                "{key} endpoint {ep:?} sits on/near a corner (nearest {nearest})"
            );
        }
    }

    #[test]
    fn stub_blocked_side_falls_back_to_open_side() {
        // `a` is flanked tightly on its right by blocker `k` (within ROUTE_MARGIN),
        // so every right-side stub is blocked. The target `b` sits below, reachable
        // out the open bottom side. The route must exist, stay orthogonal, and leave
        // `a` perpendicular out an open side — no panic, no right-side exit.
        let boxes = vec![leafbox("a"), leafbox("b"), leafbox("k")];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 100.0, 60.0));
        rects.insert(BoxId::Node("k".into()), nrect(105.0, -40.0, 40.0, 140.0));
        rects.insert(BoxId::Node("b".into()), nrect(0.0, 220.0, 100.0, 60.0));
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let out = route(&boxes, &rects, &edges, &SolveConfig::default());
        assert_eq!(out.len(), 1);
        let rt = &out[0];
        assert!(rt.points.len() >= 2, "no route: {:?}", rt.points);
        for w in rt.points.windows(2) {
            assert!(
                (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                "diagonal segment {:?}->{:?}",
                w[0],
                w[1]
            );
        }
        let a = rects[&BoxId::Node("a".into())];
        assert!(
            perp_to_border(&a, rt.points[0], rt.points[1]),
            "source exit not perpendicular: {:?}",
            rt.points
        );
        // The exit must NOT be the blocked right side.
        assert!(
            !sides_on(&a, rt.points[0]).contains(&Side::Right),
            "route left via the blocked right side: {:?}",
            rt.points
        );
        // The far end lands on the target border.
        let b = rects[&BoxId::Node("b".into())];
        let last = *rt.points.last().unwrap();
        assert!(
            !sides_on(&b, last).is_empty(),
            "endpoint not on target border: {last:?}"
        );
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

    // ---- P-3 equivalence: the slab-indexed OVG build must be byte-identical
    // ---- to the pre-optimization linear-scan build.

    /// Verbatim copy of `build_ovg` as it was BEFORE the P-3 slab-index
    /// optimization (linear scans everywhere). The equivalence tests below
    /// assert the optimized build produces the exact same graph — vertices,
    /// adjacency (including push ORDER, which A* tie-breaking depends on),
    /// and attach candidates — so routes cannot have moved.
    fn build_ovg_reference(
        obstacles: &[Obstacle],
        src: Rect,
        tgt: Rect,
    ) -> (Ovg, Vec<usize>, Vec<usize>) {
        let inflated: Vec<Rect> = obstacles
            .iter()
            .map(|o| inflate(o.rect, ROUTE_MARGIN))
            .collect();

        let mut xs = vec![
            src.x,
            src.x + src.w,
            src.x + src.w / 2.0,
            tgt.x,
            tgt.x + tgt.w,
            tgt.x + tgt.w / 2.0,
        ];
        let mut ys = vec![
            src.y,
            src.y + src.h,
            src.y + src.h / 2.0,
            tgt.y,
            tgt.y + tgt.h,
            tgt.y + tgt.h / 2.0,
        ];
        for r in &inflated {
            xs.push(r.x);
            xs.push(r.x + r.w);
            ys.push(r.y);
            ys.push(r.y + r.h);
        }
        let xs = axis_coords(xs);
        let ys = axis_coords(ys);

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

        let attach = |verts: &mut Vec<P>,
                      adj: &mut Vec<Vec<(usize, f64)>>,
                      on_border: &mut BTreeSet<usize>,
                      bx: Rect|
         -> Vec<usize> {
            let mut cands: Vec<(P, Side)> = Vec::new();
            for &y in &ys {
                if y >= bx.y + CORNER_INSET - 1e-9 && y <= bx.y + bx.h - CORNER_INSET + 1e-9 {
                    cands.push(((bx.x, y), Side::Left));
                    cands.push(((bx.x + bx.w, y), Side::Right));
                }
            }
            for &x in &xs {
                if x >= bx.x + CORNER_INSET - 1e-9 && x <= bx.x + bx.w - CORNER_INSET + 1e-9 {
                    cands.push(((x, bx.y), Side::Top));
                    cands.push(((x, bx.y + bx.h), Side::Bottom));
                }
            }
            cands.push(((bx.x, bx.y + bx.h / 2.0), Side::Left));
            cands.push(((bx.x + bx.w, bx.y + bx.h / 2.0), Side::Right));
            cands.push(((bx.x + bx.w / 2.0, bx.y), Side::Top));
            cands.push(((bx.x + bx.w / 2.0, bx.y + bx.h), Side::Bottom));
            cands.sort_by(|(pa, sa), (pb, sb)| {
                pa.0.total_cmp(&pb.0)
                    .then(pa.1.total_cmp(&pb.1))
                    .then(sa.cmp(sb))
            });
            cands.dedup_by(|(pa, sa), (pb, sb)| {
                (pa.0 - pb.0).abs() < 1e-9 && (pa.1 - pb.1).abs() < 1e-9 && sa == sb
            });

            let mut idxs = Vec::new();
            for (pt, side) in cands {
                let bi = verts.len();
                verts.push(pt);
                adj.push(Vec::new());
                on_border.insert(bi);
                let nrm = outward_normal(side);
                let stub = (pt.0 + ROUTE_MARGIN * nrm.0, pt.1 + ROUTE_MARGIN * nrm.1);
                let si = verts.len();
                verts.push(stub);
                adj.push(Vec::new());
                adj[bi].push((si, ROUTE_MARGIN));
                adj[si].push((bi, ROUTE_MARGIN));
                for gi in 0..si {
                    if on_border.contains(&gi) {
                        continue;
                    }
                    let g = verts[gi];
                    let aligned = (g.0 - stub.0).abs() < 1e-9 || (g.1 - stub.1).abs() < 1e-9;
                    if aligned && !segment_blocked(&inflated, stub, g) {
                        let len = (g.0 - stub.0).abs() + (g.1 - stub.1).abs();
                        adj[si].push((gi, len));
                        adj[gi].push((si, len));
                    }
                }
                idxs.push(bi);
            }
            idxs
        };

        let mut on_border: BTreeSet<usize> = BTreeSet::new();
        let srcv = attach(&mut verts, &mut adj, &mut on_border, src);
        let tgtv = attach(&mut verts, &mut adj, &mut on_border, tgt);
        (Ovg { verts, adj }, srcv, tgtv)
    }

    /// Verbatim copy of the pre-P-3 `route_keyed_with` edge loop: per-edge
    /// obstacle rebuild + reference OVG build, feeding the SAME `astar`,
    /// `hub_spread` and `nudge`.
    fn route_keyed_with_reference(
        boxes: &[Box],
        rects: &BTreeMap<BoxId, Rect>,
        edges: &[KeyedEdge],
        cost: &RouteCost,
    ) -> Vec<Route> {
        let membership = build_membership(boxes);
        let mut routes: Vec<Route> = Vec::new();
        for (s, t, key, label_size) in edges {
            let Some((source, target, src, tgt)) = routable(rects, s, t) else {
                continue;
            };
            let mut obstacles = leaf_obstacles(rects, &[s.clone(), t.clone()]);
            obstacles.extend(group_obstacles(rects, &membership, s, t));
            obstacles.sort_by(|a, b| a.id.cmp(&b.id));
            let (ovg, srcv, tgtv) = build_ovg_reference(&obstacles, src, tgt);
            let goal = (tgt.x + tgt.w / 2.0, tgt.y + tgt.h / 2.0);
            let inflated: Vec<Rect> = obstacles
                .iter()
                .map(|o| inflate(o.rect, ROUTE_MARGIN))
                .collect();
            let points = astar(&ovg, &srcv, &tgtv, goal, cost, &inflated, *label_size)
                .unwrap_or_else(|| fallback_l(src, tgt));
            routes.push(Route {
                points,
                source,
                target,
                key: key.clone(),
            });
        }
        hub_spread(&mut routes, rects);
        nudge(&mut routes);
        routes
    }

    /// Deterministic LCG so the random-scene tests need no dependency.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn range(&mut self, lo: i64, hi: i64) -> f64 {
            (lo + (self.next() % (hi - lo) as u64) as i64) as f64
        }
    }

    /// A pseudo-random scene: `n` leaf boxes (some snapped to a shared grid so
    /// coordinates coincide and exercise the dedup/tolerance paths, and boxes
    /// may overlap so the `fallback_l` path is exercised too), one group over
    /// the first few, and `m` edges (some parallel duplicates, some labelled).
    fn random_scene(
        seed: u64,
        n: usize,
        m: usize,
    ) -> (Vec<Box>, BTreeMap<BoxId, Rect>, Vec<KeyedEdge>) {
        let mut rng = Lcg(seed);
        let mut boxes: Vec<Box> = Vec::new();
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        for i in 0..n {
            let k = format!("n{i}");
            boxes.push(leafbox(&k));
            let snap = rng.next() % 2 == 0;
            let (x, y) = if snap {
                (rng.range(0, 8) * 220.0, rng.range(0, 8) * 160.0)
            } else {
                (rng.range(0, 1600), rng.range(0, 1200))
            };
            rects.insert(
                BoxId::Node(k),
                nrect(x, y, 60.0 + rng.range(0, 80), 40.0 + rng.range(0, 40)),
            );
        }
        let members: Vec<BoxId> = (0..n.min(3))
            .map(|i| BoxId::Node(format!("n{i}")))
            .collect();
        boxes.push(groupbox(0, members));
        rects.insert(BoxId::Group(0), nrect(-40.0, -40.0, 700.0, 500.0));
        let mut edges: Vec<KeyedEdge> = Vec::new();
        for e in 0..m {
            let a = (rng.next() as usize) % n;
            let b = (rng.next() as usize) % n;
            let label = if rng.next() % 3 == 0 {
                Some((30.0 + rng.range(0, 60), 14.0))
            } else {
                None
            };
            edges.push((
                BoxId::Node(format!("n{a}")),
                BoxId::Node(format!("n{b}")),
                Some(format!("e{e}")),
                label,
            ));
        }
        (boxes, rects, edges)
    }

    /// Non-overlapping jittered grid of leaf boxes with chained + long-range
    /// edges: no edge ever needs the centre-to-centre `fallback_l`, so the
    /// border/orthogonality invariants must hold on every route. Per-node
    /// jitter keeps every axis coordinate distinct — a snapped grid dedups to
    /// a handful of axis lines and hides the router's real cost.
    fn grid_scene(n: usize, m: usize) -> (Vec<Box>, BTreeMap<BoxId, Rect>, Vec<KeyedEdge>) {
        let mut boxes = Vec::new();
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        let cols = (n as f64).sqrt().ceil() as usize;
        for i in 0..n {
            let k = format!("n{i}");
            boxes.push(leafbox(&k));
            let (cx, cy) = (i % cols, i / cols);
            let (jx, jy) = ((i * 7 % 40) as f64, (i * 13 % 40) as f64);
            rects.insert(
                BoxId::Node(k),
                nrect(cx as f64 * 260.0 + jx, cy as f64 * 180.0 + jy, 100.0, 60.0),
            );
        }
        let mut rng = Lcg(0xC0FFEE);
        let mut edges: Vec<KeyedEdge> = Vec::new();
        for e in 0..m {
            let a = if e < n - 1 {
                e
            } else {
                (rng.next() as usize) % n
            };
            let b = if e < n - 1 {
                e + 1
            } else {
                (rng.next() as usize) % n
            };
            if a == b {
                continue;
            }
            let label = if e % 3 == 0 { Some((48.0, 14.0)) } else { None };
            edges.push((
                BoxId::Node(format!("n{a}")),
                BoxId::Node(format!("n{b}")),
                Some(format!("e{e}")),
                label,
            ));
        }
        (boxes, rects, edges)
    }

    #[test]
    fn p3_optimized_ovg_is_byte_identical_to_reference() {
        for seed in 0..40u64 {
            let mut rng = Lcg(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
            let n = 3 + (rng.next() as usize) % 14;
            let (_boxes, rects, _edges) = random_scene(seed + 1000, n, 0);
            let ids: Vec<BoxId> = rects.keys().cloned().collect();
            // Pick two distinct leaf endpoints; everything else is an obstacle.
            let leaves: Vec<&BoxId> = ids.iter().filter(|i| matches!(i, BoxId::Node(_))).collect();
            let (s, t) = (leaves[0].clone(), leaves[leaves.len() - 1].clone());
            let mut obstacles = leaf_obstacles(&rects, &[s.clone(), t.clone()]);
            obstacles.push(Obstacle {
                id: BoxId::Group(0),
                rect: rects[&BoxId::Group(0)],
            });
            obstacles.sort_by(|a, b| a.id.cmp(&b.id));
            let (src, tgt) = (rects[&s], rects[&t]);
            let (fast, fs, ft) = build_ovg(&obstacles, src, tgt);
            let (refr, rs, rt) = build_ovg_reference(&obstacles, src, tgt);
            assert_eq!(fast.verts, refr.verts, "verts differ (seed {seed})");
            assert_eq!(fast.adj, refr.adj, "adjacency differs (seed {seed})");
            assert_eq!(fs, rs, "src attach candidates differ (seed {seed})");
            assert_eq!(ft, rt, "tgt attach candidates differ (seed {seed})");
        }
    }

    #[test]
    fn p3_optimized_routes_are_identical_to_reference_on_random_multi_edge_scenes() {
        let cfg = SolveConfig::default();
        for seed in 0..12u64 {
            let (boxes, rects, edges) = random_scene(seed * 7 + 3, 10, 16);
            let cost = RouteCost {
                label_pressure: if seed % 2 == 0 { 0.0 } else { 50.0 },
                ..RouteCost::default()
            };
            let fast = route_keyed_with(&boxes, &rects, &edges, &cfg, &cost);
            let refr = route_keyed_with_reference(&boxes, &rects, &edges, &cost);
            assert_eq!(fast, refr, "routes diverged from reference (seed {seed})");
        }
    }

    /// Golden structural invariants on a stable multi-edge fixture: endpoints
    /// land ON their box borders, every segment orthogonal — the properties
    /// P-3 must not regress even if a future change legitimately moves routes.
    #[test]
    fn p3_multi_edge_fixture_keeps_endpoint_and_orthogonality_invariants() {
        let (boxes, rects, edges) = grid_scene(12, 20);
        let out = route_keyed_with(
            &boxes,
            &rects,
            &edges,
            &SolveConfig::default(),
            &RouteCost::default(),
        );
        assert!(!out.is_empty());
        for rt in &out {
            for w in rt.points.windows(2) {
                assert!(
                    (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6,
                    "{}->{} diagonal segment {:?}->{:?}",
                    rt.source,
                    rt.target,
                    w[0],
                    w[1]
                );
            }
            let src_bx = rects[&BoxId::Node(rt.source.clone())];
            let tgt_bx = rects[&BoxId::Node(rt.target.clone())];
            assert!(
                !sides_on(&src_bx, rt.points[0]).is_empty(),
                "{}->{} source endpoint off border: {:?}",
                rt.source,
                rt.target,
                rt.points[0]
            );
            assert!(
                !sides_on(&tgt_bx, *rt.points.last().unwrap()).is_empty(),
                "{}->{} target endpoint off border: {:?}",
                rt.source,
                rt.target,
                rt.points.last()
            );
        }
    }

    /// P-3 timing evidence. Run manually with
    /// `cargo test -p waml --release --lib p3_router_scales -- --ignored --nocapture`.
    /// Prints optimized vs pre-change (reference) wall time on the same scenes
    /// and asserts the outputs match while measuring.
    #[test]
    #[ignore = "perf measurement, run manually"]
    fn p3_router_scales_on_large_diagrams() {
        use std::time::Instant;
        let cfg = SolveConfig::default();
        let cost = RouteCost::default();

        for (n, m) in [(60, 120), (200, 400)] {
            let (boxes, rects, edges) = grid_scene(n, m);
            let t0 = Instant::now();
            let fast = route_keyed_with(&boxes, &rects, &edges, &cfg, &cost);
            let fast_ms = t0.elapsed().as_secs_f64() * 1e3;
            let t1 = Instant::now();
            let refr = route_keyed_with_reference(&boxes, &rects, &edges, &cost);
            let ref_ms = t1.elapsed().as_secs_f64() * 1e3;
            assert_eq!(fast, refr);
            println!(
                "P-3 {n} nodes / {m} edges: optimized {fast_ms:.1} ms, reference {ref_ms:.1} ms"
            );
            assert!(
                fast_ms < 60_000.0,
                "{n}-node/{m}-edge solve took {fast_ms:.0} ms (> 60 s bound)"
            );
        }
    }
}
