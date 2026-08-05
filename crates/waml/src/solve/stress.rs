//! Semi-smart default layout via stress majorization (SMACOF).
//!
//! Standalone, edge-aware placement for diagrams with **no** authored layout.
//! Connected nodes are pulled toward each other so the result reflects the
//! model's relationships instead of the edge-blind left-to-right strip that
//! `geometry::solve` produces for unconstrained roots. Fully deterministic
//! (circular seed, fixed iteration order, no RNG) — same input, same pixels.
//!
//! See docs/superpowers/specs/2026-07-21-default-layout-stress-majorization-design.md.
//! Not yet wired into `solve_diagram`; that is Phase 3, gated on screenshot review.

use super::{BoxId, Rect, Size, SolveConfig};
use crate::layout::Margin;
use std::collections::{HashMap, VecDeque};
use std::f64::consts::PI;

/// Tunables for the stress solve. Defaults are a first pass; the spec calls for
/// tuning `edge_len`/`gap` from a real-model screenshot (Phase 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StressConfig {
    /// Ideal pixels per graph hop (the SMACOF target-distance unit, `L`).
    pub edge_len: f64,
    /// Hard cap on majorization iterations per component.
    pub max_iter: u32,
    /// Convergence threshold on the absolute stress delta between iterations.
    pub epsilon: f64,
    /// Minimum pixels between node boxes after overlap removal.
    pub gap: f64,
    /// Ideal co-member separation for grouped nodes (the group's target-distance
    /// unit). Shorter than `edge_len` so groups pull tighter than a bare edge.
    pub group_len: f64,
    /// Weight multiplier applied to co-member pairs, per group-nesting depth.
    pub group_weight: f64,
    /// Padding from a group's member bounding box out to its hull.
    pub hull_pad: f64,
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            edge_len: 120.0,
            max_iter: 300,
            epsilon: 1e-4,
            gap: SolveConfig::default().min_sep,
            group_len: 120.0 * 0.75,
            group_weight: 4.0,
            hull_pad: SolveConfig::default().margin(Margin::Medium),
        }
    }
}

/// One diagram group's membership for the cohesion force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSpec {
    /// Indices into `ids`/`sizes` of every member, including members of nested
    /// children — a nested group's members are also listed in its ancestors.
    pub members: Vec<usize>,
    /// Nesting depth; 0 is top level. Deeper groups bind tighter.
    pub depth: u8,
}

/// Guard against division by zero when two points coincide (Guttman denom).
const COINCIDENT_EPS: f64 = 1e-9;

/// Scalar half-extent proxy for a box: the mean of its half-width and
/// half-height. Used to inflate target distances so adjacent boxes leave room
/// for their own footprints before the final scan-line push-apart.
fn half_extent(s: &Size) -> f64 {
    (s.w + s.h) / 4.0
}

/// Lay out `ids` (with matching `sizes`) under undirected `edges` (index pairs
/// into `ids`/`sizes`). Returns one `Rect` per input id, in input order, with
/// the min corner translated to the origin (matching `assemble`'s convention).
pub fn layout(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    cfg: &StressConfig,
) -> Vec<Rect> {
    layout_grouped(ids, sizes, edges, &[], cfg).0
}

/// Lay out `ids` under undirected `edges` plus a soft cohesion force from
/// `groups`: co-members are pulled toward a shorter target distance and
/// weighted more heavily in the SMACOF solve, without hard-constraining them —
/// a strong outside edge can still pull a member away. Returns `(node rects,
/// one hull rect per group in `groups` order)`. With `groups` empty this is
/// exactly `layout`'s behavior (the regression guard for "groups change
/// nothing when there are none").
pub fn layout_grouped(
    ids: &[BoxId],
    sizes: &[Size],
    edges: &[(usize, usize)],
    groups: &[GroupSpec],
    cfg: &StressConfig,
) -> (Vec<Rect>, Vec<Rect>) {
    let n = ids.len();
    assert_eq!(n, sizes.len(), "ids and sizes length mismatch");
    if n == 0 {
        return (vec![], vec![]);
    }
    if n == 1 {
        let rects = vec![Rect {
            x: 0.0,
            y: 0.0,
            w: sizes[0].w,
            h: sizes[0].h,
        }];
        let hulls = group_hulls(&rects, groups, cfg);
        return (rects, hulls);
    }

    let clean = dedup_edges(n, edges);
    let co_depth = comembership_depths(groups);
    let co_edges: Vec<(usize, usize)> = co_depth.keys().copied().collect();
    // Merged adjacency: real edges plus a clique per group, so a group can
    // never be split across two independently shelf-packed components, and
    // `bfs_hops` defines a distance for every co-member pair.
    let merged: Vec<(usize, usize)> = {
        let mut m = clean.clone();
        m.extend(co_edges.iter().copied());
        dedup_edges(n, &m)
    };
    if merged.is_empty() {
        // No meaningful distances — degenerate. Fall back to the grid, but still
        // honor the hull contract: one hull per group, in `groups` order. This
        // arm is reachable with non-empty `groups` whenever every group has
        // fewer than two members (no clique edges) and there are no real edges.
        let mut rects = grid_pack(ids, sizes, cfg);
        separate_hulls(&mut rects, groups, cfg);
        normalize_to_origin(&mut rects);
        let hulls = group_hulls(&rects, groups, cfg);
        debug_assert_eq!(hulls.len(), groups.len());
        return (rects, hulls);
    }

    let adj = adjacency(n, &merged);
    let comps = components(n, &adj);

    // Solve each component independently, normalizing its min corner to the
    // origin and recording its bounding box for packing.
    struct Laid {
        comp: Vec<usize>,
        rects: Vec<Rect>, // local, min corner at (0,0)
        w: f64,
        h: f64,
    }
    let mut laid: Vec<Laid> = Vec::with_capacity(comps.len());
    for comp in comps {
        let mut rects = component_layout(&comp, sizes, &adj, cfg, &co_depth);
        remove_overlaps(&mut rects, cfg.gap);
        let (min_x, min_y) = rects
            .iter()
            .fold((f64::INFINITY, f64::INFINITY), |(mx, my), r| {
                (mx.min(r.x), my.min(r.y))
            });
        let (mut w, mut h) = (0.0_f64, 0.0_f64);
        for r in &mut rects {
            r.x -= min_x;
            r.y -= min_y;
            w = w.max(r.x + r.w);
            h = h.max(r.y + r.h);
        }
        laid.push(Laid { comp, rects, w, h });
    }

    // Shelf-pack the components toward a roughly landscape aspect rather than a
    // single left-to-right row (which strings singletons into a long tail).
    // Target row width = sqrt(total area) biased wide; deterministic order.
    let total_area: f64 = laid.iter().map(|l| l.w * l.h).sum();
    let widest = laid.iter().fold(0.0_f64, |m, l| m.max(l.w));
    let target_w = (total_area.sqrt() * 1.4).max(widest);

    let zero = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut out = vec![zero; n];
    let (mut cursor_x, mut cursor_y, mut shelf_h) = (0.0_f64, 0.0_f64, 0.0_f64);
    for l in &laid {
        if cursor_x > 0.0 && cursor_x + l.w > target_w {
            cursor_x = 0.0;
            cursor_y += shelf_h + cfg.gap;
            shelf_h = 0.0;
        }
        for (local, r) in l.rects.iter().enumerate() {
            out[l.comp[local]] = Rect {
                x: r.x + cursor_x,
                y: r.y + cursor_y,
                w: r.w,
                h: r.h,
            };
        }
        cursor_x += l.w + cfg.gap;
        shelf_h = shelf_h.max(l.h);
    }
    separate_hulls(&mut out, groups, cfg);

    // Normalize the min corner to the origin, matching `layout`'s convention;
    // the separation pass can push rects negative.
    normalize_to_origin(&mut out);

    let hulls = group_hulls(&out, groups, cfg);
    debug_assert_eq!(hulls.len(), groups.len());
    (out, hulls)
}

/// Translate `rects` so their bounding box's min corner sits at the origin.
fn normalize_to_origin(rects: &mut [Rect]) {
    let (min_x, min_y) = rects
        .iter()
        .fold((f64::INFINITY, f64::INFINITY), |(mx, my), r| {
            (mx.min(r.x), my.min(r.y))
        });
    if min_x.is_finite() {
        for r in rects {
            r.x -= min_x;
            r.y -= min_y;
        }
    }
}

/// Bounded separation pass: sibling group hulls that overlap are pushed apart
/// by translating every member of the deeper/later group, and any *ungrouped*
/// node that lands inside a hull is pushed out through its nearest edge.
/// Group pairs that share any member — nested (ancestor/descendant) pairs, and
/// siblings that merely intersect — are left alone: containment is expected for
/// the former, and neither can be separated by translating one set without
/// dragging the shared nodes out of the other. Deterministic: groups are always visited deepest-first
/// then by index, and the loop is capped at 6 passes, exiting early once a
/// pass makes no translation.
///
/// Node-node overlap removal runs at the *start* of each pass, before the
/// checks, so a pass that reports "nothing moved" leaves the rects both
/// overlap-free and hull-separated — the post-condition the callers assert.
/// Only nodes that belong to no group at all are pushed out of foreign hulls:
/// translating a member of some other group in isolation would skew that
/// group's own hull and undo its just-established separation.
fn separate_hulls(rects: &mut [Rect], groups: &[GroupSpec], cfg: &StressConfig) {
    if groups.is_empty() {
        return;
    }
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| groups[b].depth.cmp(&groups[a].depth).then(a.cmp(&b)));

    let member_sets: Vec<std::collections::BTreeSet<usize>> = groups
        .iter()
        .map(|g| g.members.iter().copied().collect())
        .collect();
    // Only groups with *disjoint* member sets can be pulled apart: translating
    // one of two sets that share a node drags that node out of the other group
    // too, so the pair can never separate and the passes just fight until the
    // cap. Nested (ancestor/descendant) pairs share by definition and are
    // expected to overlap; merely-intersecting siblings (the same element under
    // two `###` headings) are the same story geometrically.
    let is_entangled =
        |i: usize, j: usize| -> bool { !member_sets[i].is_disjoint(&member_sets[j]) };

    // A node in any group is moved only with its group, never in isolation.
    let ungrouped: Vec<bool> = (0..rects.len())
        .map(|i| !member_sets.iter().any(|s| s.contains(&i)))
        .collect();

    for _ in 0..6 {
        let mut moved = false;
        remove_overlaps(rects, cfg.gap);

        let hulls = group_hulls(rects, groups, cfg);
        for (oi, &gi) in order.iter().enumerate() {
            for &gj in &order[oi + 1..] {
                if is_entangled(gi, gj) {
                    continue;
                }
                let a = &hulls[gi];
                let b = &hulls[gj];
                let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
                let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                moved = true;
                let a_cx = a.x + a.w / 2.0;
                let a_cy = a.y + a.h / 2.0;
                let b_cx = b.x + b.w / 2.0;
                let b_cy = b.y + b.h / 2.0;
                if ox < oy {
                    let dir = if b_cx >= a_cx { 1.0 } else { -1.0 };
                    let delta = dir * (ox + cfg.gap);
                    for &m in &groups[gj].members {
                        if let Some(r) = rects.get_mut(m) {
                            r.x += delta;
                        }
                    }
                } else {
                    let dir = if b_cy >= a_cy { 1.0 } else { -1.0 };
                    let delta = dir * (oy + cfg.gap);
                    for &m in &groups[gj].members {
                        if let Some(r) = rects.get_mut(m) {
                            r.y += delta;
                        }
                    }
                }
            }
        }

        // Hulls stay valid across this loop: only ungrouped nodes move, and no
        // hull depends on them. Each node is re-tested against every hull until
        // it is clear of all of them (bounded), so being pushed out of one hull
        // and straight into another resolves within the same pass.
        let hulls = group_hulls(rects, groups, cfg);
        for (idx, r) in rects.iter_mut().enumerate() {
            if !ungrouped[idx] {
                continue;
            }
            for _ in 0..groups.len().max(1) {
                let mut hit = false;
                for h in &hulls {
                    let ox = (r.x + r.w).min(h.x + h.w) - r.x.max(h.x);
                    let oy = (r.y + r.h).min(h.y + h.h) - r.y.max(h.y);
                    if ox <= 0.0 || oy <= 0.0 {
                        continue;
                    }
                    hit = true;
                    moved = true;
                    if ox < oy {
                        let dir = if r.x + r.w / 2.0 >= h.x + h.w / 2.0 {
                            1.0
                        } else {
                            -1.0
                        };
                        r.x += dir * (ox + cfg.gap);
                    } else {
                        let dir = if r.y + r.h / 2.0 >= h.y + h.h / 2.0 {
                            1.0
                        } else {
                            -1.0
                        };
                        r.y += dir * (oy + cfg.gap);
                    }
                }
                if !hit {
                    break;
                }
            }
        }

        if !moved {
            break;
        }
    }
}

/// For every pair of co-members across `groups`, the depth of the *deepest*
/// group that contains both (0 = top level). Pairs never sharing a group are
/// absent. Keyed with the smaller index first. Groups are small, so the O(k^2)
/// clique per group is fine.
fn comembership_depths(groups: &[GroupSpec]) -> HashMap<(usize, usize), u8> {
    let mut depths: HashMap<(usize, usize), u8> = HashMap::new();
    for g in groups {
        for (i, &a) in g.members.iter().enumerate() {
            for &b in &g.members[i + 1..] {
                if a == b {
                    continue;
                }
                let key = if a < b { (a, b) } else { (b, a) };
                let entry = depths.entry(key).or_insert(g.depth);
                *entry = (*entry).max(g.depth);
            }
        }
    }
    depths
}

/// Bounding box of each group's member rects, grown by `cfg.hull_pad`. A
/// group with no members (all keys missing from `sizes`, or genuinely empty)
/// gets a degenerate zero-size hull at the origin.
fn group_hulls(rects: &[Rect], groups: &[GroupSpec], cfg: &StressConfig) -> Vec<Rect> {
    groups
        .iter()
        .map(|g| {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for &m in &g.members {
                let Some(r) = rects.get(m) else { continue };
                min_x = min_x.min(r.x);
                min_y = min_y.min(r.y);
                max_x = max_x.max(r.x + r.w);
                max_y = max_y.max(r.y + r.h);
            }
            if !min_x.is_finite() {
                return Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                };
            }
            Rect {
                x: min_x - cfg.hull_pad,
                y: min_y - cfg.hull_pad,
                w: (max_x - min_x) + 2.0 * cfg.hull_pad,
                h: (max_y - min_y) + 2.0 * cfg.hull_pad,
            }
        })
        .collect()
}

// --- helpers -------------------------------------------------------------

/// Drop self-edges, dedup (undirected), and clamp indices in range. Output is
/// sorted for a fully deterministic downstream ordering.
fn dedup_edges(n: usize, edges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut seen: Vec<(usize, usize)> = edges
        .iter()
        .filter(|&&(a, b)| a != b && a < n && b < n)
        .map(|&(a, b)| if a < b { (a, b) } else { (b, a) })
        .collect();
    seen.sort_unstable();
    seen.dedup();
    seen
}

/// Undirected adjacency lists, each sorted ascending.
fn adjacency(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for &(a, b) in edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    for a in &mut adj {
        a.sort_unstable();
        a.dedup();
    }
    adj
}

/// Connected components, each sorted ascending, ordered by smallest member.
fn components(n: usize, adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; n];
    let mut comps = Vec::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(node) = queue.pop_front() {
            comp.push(node);
            for &nb in &adj[node] {
                if !seen[nb] {
                    seen[nb] = true;
                    queue.push_back(nb);
                }
            }
        }
        comp.sort_unstable();
        comps.push(comp);
    }
    // `start` scans ascending so comps already order by smallest member.
    comps
}

/// BFS hop counts from `src` over the whole graph; `None` for unreachable.
fn bfs_hops(n: usize, adj: &[Vec<usize>], src: usize) -> Vec<Option<u32>> {
    let mut dist = vec![None; n];
    dist[src] = Some(0);
    let mut queue = VecDeque::from([src]);
    while let Some(node) = queue.pop_front() {
        let d = dist[node].unwrap();
        for &nb in &adj[node] {
            if dist[nb].is_none() {
                dist[nb] = Some(d + 1);
                queue.push_back(nb);
            }
        }
    }
    dist
}

/// Deterministic circular seed: node `k` of `m` at angle `2*PI*k/m`, radius set
/// so the ring circumference is about `edge_len * m`.
fn circular_seed(m: usize, edge_len: f64) -> Vec<(f64, f64)> {
    let radius = (edge_len * m as f64 / (2.0 * PI)).max(edge_len);
    (0..m)
        .map(|k| {
            let theta = 2.0 * PI * k as f64 / m as f64;
            (radius * theta.cos(), radius * theta.sin())
        })
        .collect()
}

/// Weighted raw stress: sum over a<b of w_ab * (d_ab - dist(p_a, p_b))^2.
fn stress_value(pos: &[(f64, f64)], dist: &[Vec<f64>], w: &[Vec<f64>]) -> f64 {
    let m = pos.len();
    let mut s = 0.0;
    for a in 0..m {
        for b in (a + 1)..m {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let actual = dx.hypot(dy);
            let e = dist[a][b] - actual;
            s += w[a][b] * e * e;
        }
    }
    s
}

/// Run the Guttman-transform majorization to convergence. Returns the final
/// positions and the per-iteration stress trace (trace[0] = seed stress).
fn majorize(
    seed: &[(f64, f64)],
    dist: &[Vec<f64>],
    w: &[Vec<f64>],
    wsum: &[f64],
    cfg: &StressConfig,
) -> (Vec<(f64, f64)>, Vec<f64>) {
    let m = seed.len();
    let mut pos = seed.to_vec();
    let mut trace = vec![stress_value(&pos, dist, w)];
    for _ in 0..cfg.max_iter {
        // Simultaneous (Jacobi) update — the standard SMACOF majorizer; stress
        // is guaranteed non-increasing.
        let mut next = vec![(0.0, 0.0); m];
        for a in 0..m {
            if wsum[a] <= 0.0 {
                next[a] = pos[a];
                continue;
            }
            let (mut sx, mut sy) = (0.0, 0.0);
            for b in 0..m {
                if b == a {
                    continue;
                }
                let dx = pos[a].0 - pos[b].0;
                let dy = pos[a].1 - pos[b].1;
                let actual = dx.hypot(dy);
                let inv = if actual < COINCIDENT_EPS {
                    0.0
                } else {
                    dist[a][b] / actual
                };
                sx += w[a][b] * (pos[b].0 + inv * dx);
                sy += w[a][b] * (pos[b].1 + inv * dy);
            }
            next[a] = (sx / wsum[a], sy / wsum[a]);
        }
        pos = next;
        let s = stress_value(&pos, dist, w);
        let prev = *trace.last().unwrap();
        trace.push(s);
        if (prev - s).abs() < cfg.epsilon {
            break;
        }
    }
    (pos, trace)
}

/// Solve one connected component to node-centered `Rect`s (in `comp` order).
/// `co_depth` maps a co-member pair (global indices, smaller first) to the
/// depth of its deepest common group, for the cohesion override below.
fn component_layout(
    comp: &[usize],
    sizes: &[Size],
    adj: &[Vec<usize>],
    cfg: &StressConfig,
    co_depth: &HashMap<(usize, usize), u8>,
) -> Vec<Rect> {
    let m = comp.len();
    if m == 1 {
        let s = sizes[comp[0]];
        return vec![Rect {
            x: -s.w / 2.0,
            y: -s.h / 2.0,
            w: s.w,
            h: s.h,
        }];
    }

    // Target distances: hops * edge_len, inflated by combined half-extents so
    // boxes have room for their footprints. Co-member pairs are then pulled in
    // to `group_len` (whichever is tighter) — cohesion is soft: an edge-adjacent
    // pair that is also co-member keeps the tighter of the two distances.
    // Weights w = 1 / d^2, with co-member pairs weighted up by
    // `group_weight^(depth+1)` using the deepest group containing both.
    let n = adj.len();
    let mut dist = vec![vec![0.0; m]; m];
    let mut w = vec![vec![0.0; m]; m];
    for (la, &ga) in comp.iter().enumerate() {
        let hops = bfs_hops(n, adj, ga);
        for (lb, &gb) in comp.iter().enumerate() {
            if la == lb {
                continue;
            }
            let h = hops[gb].expect("connected component is fully reachable") as f64;
            let mut d = h * cfg.edge_len + half_extent(&sizes[ga]) + half_extent(&sizes[gb]);
            let key = if ga < gb { (ga, gb) } else { (gb, ga) };
            let depth = co_depth.get(&key).copied();
            if depth.is_some() {
                let group_d = cfg.group_len + half_extent(&sizes[ga]) + half_extent(&sizes[gb]);
                d = d.min(group_d);
            }
            dist[la][lb] = d;
            let mut weight = 1.0 / (d * d);
            if let Some(depth) = depth {
                weight *= cfg.group_weight.powi(depth as i32 + 1);
            }
            w[la][lb] = weight;
        }
    }
    let wsum: Vec<f64> = (0..m).map(|a| w[a].iter().sum()).collect();

    let seed = circular_seed(m, cfg.edge_len);
    let (pos, _trace) = majorize(&seed, &dist, &w, &wsum, cfg);

    // Centers → top-left rects.
    comp.iter()
        .enumerate()
        .map(|(local, &g)| {
            let s = sizes[g];
            Rect {
                x: pos[local].0 - s.w / 2.0,
                y: pos[local].1 - s.h / 2.0,
                w: s.w,
                h: s.h,
            }
        })
        .collect()
}

/// Deterministic scan-line push-apart guaranteeing no rectangle overlaps.
///
/// Sweep boxes left-to-right by center-x. Each box is pushed right just enough
/// to clear every earlier box it overlaps in y (within `gap`). After the pass
/// every pair is separated by at least `gap` on at least one axis, so no pair
/// overlaps. Earlier boxes never move, so a single deterministic sweep suffices.
fn remove_overlaps(rects: &mut [Rect], gap: f64) {
    let m = rects.len();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| {
        let ca = rects[a].x + rects[a].w / 2.0;
        let cb = rects[b].x + rects[b].w / 2.0;
        ca.partial_cmp(&cb)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    for pi in 0..m {
        let i = order[pi];
        let mut min_left = f64::NEG_INFINITY;
        for &j in order.iter().take(pi) {
            let i_top = rects[i].y;
            let i_bot = rects[i].y + rects[i].h;
            let j_top = rects[j].y;
            let j_bot = rects[j].y + rects[j].h;
            let y_overlap = i_top < j_bot + gap && j_top < i_bot + gap;
            if y_overlap {
                min_left = min_left.max(rects[j].x + rects[j].w + gap);
            }
        }
        if min_left.is_finite() && rects[i].x < min_left {
            rects[i].x = min_left;
        }
    }
}

/// Edgeless fallback: wrap the flat node list into a `ceil(sqrt(n))`-column
/// grid. Column widths are per-column maxima, row heights per-row maxima, with
/// `gap` between cells; each box is centered in its cell. Min corner at origin.
pub fn grid_pack(ids: &[BoxId], sizes: &[Size], cfg: &StressConfig) -> Vec<Rect> {
    let n = ids.len();
    if n == 0 {
        return vec![];
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);

    let mut col_w = vec![0.0_f64; cols];
    let mut row_h = vec![0.0_f64; rows];
    for (k, s) in sizes.iter().enumerate() {
        let (r, c) = (k / cols, k % cols);
        col_w[c] = col_w[c].max(s.w);
        row_h[r] = row_h[r].max(s.h);
    }

    // Cell origins from prefix sums plus inter-cell gaps.
    let mut col_x = vec![0.0_f64; cols];
    for c in 1..cols {
        col_x[c] = col_x[c - 1] + col_w[c - 1] + cfg.gap;
    }
    let mut row_y = vec![0.0_f64; rows];
    for r in 1..rows {
        row_y[r] = row_y[r - 1] + row_h[r - 1] + cfg.gap;
    }

    (0..n)
        .map(|k| {
            let (r, c) = (k / cols, k % cols);
            let s = sizes[k];
            Rect {
                x: col_x[c] + (col_w[c] - s.w) / 2.0,
                y: row_y[r] + (row_h[r] - s.h) / 2.0,
                w: s.w,
                h: s.h,
            }
        })
        .collect()
}

/// Deterministic, `solve::pretty`-style dump: one `node <id> @ x,y wxh` line per
/// box, sorted by id. Used by tests and the harness.
pub fn pretty(ids: &[BoxId], rects: &[Rect]) -> String {
    let mut pairs: Vec<(&BoxId, &Rect)> = ids.iter().zip(rects.iter()).collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::new();
    for (id, r) in pairs {
        let name = match id {
            BoxId::Node(k) => k.clone(),
            BoxId::Group(g) => format!("group{g}"),
            BoxId::Inline(i) => format!("inline{i}"),
        };
        out.push_str(&format!(
            "node {name} @ {:.0},{:.0} {:.0}x{:.0}\n",
            r.x, r.y, r.w, r.h
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(k: &str) -> BoxId {
        BoxId::Node(k.into())
    }
    fn ids(keys: &[&str]) -> Vec<BoxId> {
        keys.iter().map(|k| node(k)).collect()
    }
    fn sizes(n: usize, w: f64, h: f64) -> Vec<Size> {
        vec![Size { w, h }; n]
    }
    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn bfs_hops_on_a_path() {
        // 0-1-2-3 path.
        let adj = adjacency(4, &[(0, 1), (1, 2), (2, 3)]);
        let d = bfs_hops(4, &adj, 0);
        assert_eq!(d, vec![Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn bfs_hops_marks_unreachable() {
        // 0-1 and isolated 2.
        let adj = adjacency(3, &[(0, 1)]);
        assert_eq!(bfs_hops(3, &adj, 0), vec![Some(0), Some(1), None]);
    }

    #[test]
    fn dedup_edges_drops_self_and_duplicates() {
        let e = dedup_edges(3, &[(0, 1), (1, 0), (2, 2), (0, 1)]);
        assert_eq!(e, vec![(0, 1)]);
    }

    #[test]
    fn components_split_and_order_by_smallest_member() {
        // {0,2} and {1,3}, discovered so smallest member leads.
        let adj = adjacency(4, &[(0, 2), (1, 3)]);
        let comps = components(4, &adj);
        assert_eq!(comps, vec![vec![0, 2], vec![1, 3]]);
    }

    #[test]
    fn circular_seed_places_first_node_on_positive_x_axis() {
        let seed = circular_seed(4, 120.0);
        assert_eq!(seed.len(), 4);
        // Ring circumference formula gives 76.4 < edge_len, so the min clamp
        // pins the radius at edge_len.
        let radius = (120.0 * 4.0 / (2.0 * PI)).max(120.0);
        assert_eq!(radius, 120.0);
        assert!((seed[0].0 - radius).abs() < 1e-9);
        assert!(seed[0].1.abs() < 1e-9);
        // Quarter turn → (0, radius).
        assert!(seed[1].0.abs() < 1e-9);
        assert!((seed[1].1 - radius).abs() < 1e-9);
    }

    #[test]
    fn majorization_monotonically_decreases_stress() {
        // Square graph 0-1-2-3-0 plus a diagonal.
        let adj = adjacency(4, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]);
        let cfg = StressConfig::default();
        let szs = sizes(4, 100.0, 40.0);
        // Rebuild the dist/weight matrices the same way component_layout does.
        let comp = [0usize, 1, 2, 3];
        let m = comp.len();
        let mut dist = vec![vec![0.0; m]; m];
        let mut w = vec![vec![0.0; m]; m];
        for (la, &ga) in comp.iter().enumerate() {
            let hops = bfs_hops(4, &adj, ga);
            for (lb, &gb) in comp.iter().enumerate() {
                if la == lb {
                    continue;
                }
                let h = hops[gb].unwrap() as f64;
                let d = h * cfg.edge_len + half_extent(&szs[ga]) + half_extent(&szs[gb]);
                dist[la][lb] = d;
                w[la][lb] = 1.0 / (d * d);
            }
        }
        let wsum: Vec<f64> = (0..m).map(|a| w[a].iter().sum()).collect();
        let seed = circular_seed(m, cfg.edge_len);
        let (_pos, trace) = majorize(&seed, &dist, &w, &wsum, &cfg);
        assert!(trace.len() >= 2);
        for pair in trace.windows(2) {
            assert!(
                pair[1] <= pair[0] + 1e-6,
                "stress rose: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn overlap_removal_leaves_no_overlaps() {
        // Five boxes clustered on nearly the same point.
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            };
            5
        ];
        for (i, r) in rects.iter_mut().enumerate() {
            r.x = i as f64 * 5.0;
            r.y = i as f64 * 3.0;
        }
        remove_overlaps(&mut rects, 24.0);
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(&rects[i], &rects[j]), "boxes {i},{j} overlap");
            }
        }
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(layout(&[], &[], &[], &StressConfig::default()).is_empty());
    }

    #[test]
    fn single_node_sits_at_origin() {
        let r = layout(
            &ids(&["a"]),
            &sizes(1, 200.0, 90.0),
            &[],
            &StressConfig::default(),
        );
        assert_eq!(
            r,
            vec![Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 90.0
            }]
        );
    }

    #[test]
    fn no_edges_falls_back_to_grid() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d"]);
        let szs = sizes(4, 100.0, 40.0);
        let via_layout = layout(&g, &szs, &[], &cfg);
        let via_grid = grid_pack(&g, &szs, &cfg);
        assert_eq!(via_layout, via_grid);
        // ceil(sqrt(4)) = 2 columns, 2 rows.
        assert_eq!(
            via_grid[0],
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            }
        );
        assert_eq!(
            via_grid[1],
            Rect {
                x: 140.0,
                y: 0.0,
                w: 100.0,
                h: 40.0
            }
        );
        assert_eq!(
            via_grid[2],
            Rect {
                x: 0.0,
                y: 80.0,
                w: 100.0,
                h: 40.0
            }
        );
    }

    #[test]
    fn self_and_duplicate_edges_do_not_crash() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b"]);
        let szs = sizes(2, 100.0, 40.0);
        // Only self/dup edges → collapses to no-edge grid.
        let r = layout(&g, &szs, &[(0, 0), (1, 1)], &cfg);
        assert_eq!(r, grid_pack(&g, &szs, &cfg));
    }

    #[test]
    fn output_has_no_overlaps_and_is_normalized() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 160.0, 80.0);
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
        let r = layout(&g, &szs, &edges, &cfg);
        let (min_x, min_y) = r
            .iter()
            .fold((f64::INFINITY, f64::INFINITY), |(mx, my), q| {
                (mx.min(q.x), my.min(q.y))
            });
        assert!(min_x.abs() < 1e-6, "min x normalized to 0, got {min_x}");
        assert!(min_y.abs() < 1e-6, "min y normalized to 0, got {min_y}");
        for i in 0..r.len() {
            for j in (i + 1)..r.len() {
                assert!(!overlaps(&r[i], &r[j]), "nodes {i},{j} overlap");
            }
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e", "f"]);
        let szs = sizes(6, 120.0, 60.0);
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5)];
        let a = layout(&g, &szs, &edges, &cfg);
        let b = layout(&g, &szs, &edges, &cfg);
        assert_eq!(a, b);
    }

    #[test]
    fn disconnected_components_occupy_disjoint_regions() {
        // Two triangles; the shelf-packer must place the components in disjoint
        // regions (side-by-side or stacked), never overlapping.
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e", "f"]);
        let szs = sizes(6, 100.0, 40.0);
        let edges = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)];
        let r = layout(&g, &szs, &edges, &cfg);
        let bbox = |sl: &[Rect]| {
            sl.iter().fold(
                (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ),
                |(x0, y0, x1, y1), q| {
                    (
                        x0.min(q.x),
                        y0.min(q.y),
                        x1.max(q.x + q.w),
                        y1.max(q.y + q.h),
                    )
                },
            )
        };
        let (ax0, ay0, ax1, ay1) = bbox(&r[0..3]);
        let (bx0, by0, bx1, by1) = bbox(&r[3..6]);
        let disjoint =
            bx0 >= ax1 - 1e-6 || ax0 >= bx1 - 1e-6 || by0 >= ay1 - 1e-6 || ay0 >= by1 - 1e-6;
        assert!(
            disjoint,
            "component bounding boxes overlap: a=({ax0},{ay0},{ax1},{ay1}) b=({bx0},{by0},{bx1},{by1})"
        );
    }

    #[test]
    fn layout_grouped_with_no_groups_matches_layout() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 160.0, 80.0);
        let edges = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];
        let plain = layout(&g, &szs, &edges, &cfg);
        let (grouped, hulls) = layout_grouped(&g, &szs, &edges, &[], &cfg);
        assert_eq!(plain, grouped);
        assert!(hulls.is_empty());
    }

    #[test]
    fn layout_grouped_edgeless_with_no_groups_matches_grid() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d"]);
        let szs = sizes(4, 100.0, 40.0);
        let via_grid = grid_pack(&g, &szs, &cfg);
        let (grouped, hulls) = layout_grouped(&g, &szs, &[], &[], &cfg);
        assert_eq!(grouped, via_grid);
        assert!(hulls.is_empty());
    }

    #[test]
    fn group_hulls_bound_members_with_padding() {
        let cfg = StressConfig::default();
        let rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 40.0,
            },
            Rect {
                x: 200.0,
                y: 100.0,
                w: 100.0,
                h: 40.0,
            },
        ];
        let groups = vec![GroupSpec {
            members: vec![0, 1],
            depth: 0,
        }];
        let hulls = group_hulls(&rects, &groups, &cfg);
        assert_eq!(hulls.len(), 1);
        let h = hulls[0];
        assert!((h.x - (0.0 - cfg.hull_pad)).abs() < 1e-9);
        assert!((h.y - (0.0 - cfg.hull_pad)).abs() < 1e-9);
        assert!((h.w - (300.0 + 2.0 * cfg.hull_pad)).abs() < 1e-9);
        assert!((h.h - (140.0 + 2.0 * cfg.hull_pad)).abs() < 1e-9);
    }

    #[test]
    fn comembership_depths_uses_deepest_shared_group() {
        // Outer group {0,1,2} depth 0, inner group {0,1} depth 1.
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![0, 1],
                depth: 1,
            },
        ];
        let d = comembership_depths(&groups);
        assert_eq!(d.get(&(0, 1)), Some(&1)); // deepest shared group wins
        assert_eq!(d.get(&(0, 2)), Some(&0));
        assert_eq!(d.get(&(1, 2)), Some(&0));
    }

    #[test]
    fn cohesion_pulls_co_members_closer_than_bare_hops_would() {
        // Star: hub 0 connects to a (1) and to the group {b(2), c(3), d(4)} via b
        // only — so c and d have no real edge at all, just co-membership. Without
        // cohesion c/d would sit far apart (2+ hops through b); with cohesion they
        // must land within a `group_len`-ish target distance of each other.
        let cfg = StressConfig::default();
        let g = ids(&["hub", "a", "b", "c", "d"]);
        let szs = sizes(5, 80.0, 40.0);
        let edges = [(0, 1), (0, 2)];
        let groups = vec![GroupSpec {
            members: vec![2, 3, 4], // b, c, d
            depth: 0,
        }];
        // c (3) and d (4) are unreachable via `edges` alone.
        let plain_adj = adjacency(5, &dedup_edges(5, &edges));
        assert!(bfs_hops(5, &plain_adj, 3)[4].is_none());

        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), 1);
        let c = &rects[3];
        let d = &rects[4];
        let cx = c.x + c.w / 2.0;
        let cy = c.y + c.h / 2.0;
        let dx = d.x + d.w / 2.0;
        let dy = d.y + d.h / 2.0;
        let dist = (cx - dx).hypot(cy - dy);
        // c and d have no edge at all — without cohesion their target distance
        // through the merged graph would be 2 hops (b-c, b-d), i.e. ~2*edge_len.
        // Cohesion pulls the *target* distance down to ~group_len instead, so the
        // solved distance should land well under the bare-hop distance.
        assert!(
            dist < 2.0 * cfg.edge_len,
            "c/d too far apart: {dist} (expected well under {})",
            2.0 * cfg.edge_len
        );

        // Sibling group members' bboxes must sit inside the emitted hull.
        let hull = hulls[0];
        for m in [2usize, 3, 4] {
            let r = &rects[m];
            assert!(
                r.x >= hull.x - 1e-6
                    && r.y >= hull.y - 1e-6
                    && r.x + r.w <= hull.x + hull.w + 1e-6
                    && r.y + r.h <= hull.y + hull.h + 1e-6,
                "member {m} rect not inside hull"
            );
        }
    }

    #[test]
    fn strong_outside_edge_still_pulls_a_member_away() {
        // Two members b,c grouped, but c also carries a direct edge to an
        // outside anchor `x` — cohesion is soft, so c should sit closer to its
        // real edge-neighbor x than an ungrouped member would to nothing, i.e.
        // adding the group must not force c on top of b regardless of the edge.
        let cfg = StressConfig::default();
        let g = ids(&["b", "c", "x"]);
        let szs = sizes(3, 80.0, 40.0);
        let edges = [(1, 2)]; // c -- x
        let groups = vec![GroupSpec {
            members: vec![0, 1], // b, c
            depth: 0,
        }];
        let (rects, _hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let b = &rects[0];
        let c = &rects[1];
        let x = &rects[2];
        let dist = |p: &Rect, q: &Rect| {
            let px = p.x + p.w / 2.0;
            let py = p.y + p.h / 2.0;
            let qx = q.x + q.w / 2.0;
            let qy = q.y + q.h / 2.0;
            (px - qx).hypot(py - qy)
        };
        // c's real edge to x still shapes the result: c is not glued to b.
        assert!(dist(c, x) > 0.0);
        assert!(dist(b, c) > 0.0);
    }

    fn rects_overlap(a: &Rect, b: &Rect) -> bool {
        let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
        let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
        ox > 0.0 && oy > 0.0
    }

    #[test]
    fn sibling_hulls_never_overlap() {
        // Two groups with a single cross-group edge pulling them together —
        // without separation their hulls would collide.
        let cfg = StressConfig::default();
        let g = ids(&["a1", "a2", "a3", "b1", "b2", "b3"]);
        let szs = sizes(6, 100.0, 50.0);
        let edges = [(2, 3)]; // a3 -- b1, the only cross-group pull
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![3, 4, 5],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), 2);
        assert!(
            !rects_overlap(&hulls[0], &hulls[1]),
            "sibling hulls overlap: {:?} vs {:?}",
            hulls[0],
            hulls[1]
        );
        // Members still sit inside their own hull.
        for (gi, group) in groups.iter().enumerate() {
            let hull = hulls[gi];
            for &m in &group.members {
                let r = &rects[m];
                assert!(
                    r.x >= hull.x - 1e-6
                        && r.y >= hull.y - 1e-6
                        && r.x + r.w <= hull.x + hull.w + 1e-6
                        && r.y + r.h <= hull.y + hull.h + 1e-6,
                    "member {m} rect not inside its own hull"
                );
            }
        }
    }

    #[test]
    fn nested_group_hull_stays_inside_parent_hull() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c"]);
        let szs = sizes(3, 100.0, 50.0);
        let edges: [(usize, usize); 0] = [];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1, 2],
                depth: 0,
            },
            GroupSpec {
                members: vec![0, 1],
                depth: 1,
            },
        ];
        let (_rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let outer = hulls[0];
        let inner = hulls[1];
        assert!(
            inner.x >= outer.x - 1e-6
                && inner.y >= outer.y - 1e-6
                && inner.x + inner.w <= outer.x + outer.w + 1e-6
                && inner.y + inner.h <= outer.y + outer.h + 1e-6,
            "inner hull {inner:?} not inside outer hull {outer:?}"
        );
    }

    #[test]
    fn separate_hulls_is_deterministic_with_multiple_groups() {
        let cfg = StressConfig::default();
        let g = ids(&["a1", "a2", "b1", "b2", "c1", "c2"]);
        let szs = sizes(6, 90.0, 45.0);
        let edges = [(1, 2), (3, 4)];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
            GroupSpec {
                members: vec![4, 5],
                depth: 0,
            },
        ];
        let one = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let two = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(one, two);
    }

    #[test]
    fn singleton_groups_without_edges_still_emit_one_hull_each() {
        // Every group has a single member and there are no edges, so the merged
        // graph is empty and the grid fallback fires — the hull contract ("one
        // hull per group, in `groups` order") must still hold.
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c"]);
        let szs = sizes(3, 100.0, 40.0);
        let groups = vec![
            GroupSpec {
                members: vec![0],
                depth: 0,
            },
            GroupSpec {
                members: vec![1],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &[], &groups, &cfg);
        assert_eq!(hulls.len(), groups.len(), "one hull per group");
        for (gi, group) in groups.iter().enumerate() {
            let hull = hulls[gi];
            for &m in &group.members {
                let r = &rects[m];
                assert!(
                    r.x >= hull.x - 1e-6
                        && r.y >= hull.y - 1e-6
                        && r.x + r.w <= hull.x + hull.w + 1e-6
                        && r.y + r.h <= hull.y + hull.h + 1e-6,
                    "member {m} rect not inside its own hull"
                );
            }
        }
        assert!(
            !rects_overlap(&hulls[0], &hulls[1]),
            "sibling hulls overlap: {:?} vs {:?}",
            hulls[0],
            hulls[1]
        );
    }

    #[test]
    fn three_interleaved_groups_end_separated_and_overlap_free() {
        // Three groups chained by cross-group edges, plus two ungrouped nodes
        // seeded between them. The post-conditions must hold on the *returned*
        // rects: sibling hulls disjoint, no node-node overlap, and no ungrouped
        // node left sitting inside a hull.
        let cfg = StressConfig::default();
        let g = ids(&[
            "a1", "a2", "b1", "b2", "c1", "c2", "loose1", "loose2", "loose3",
        ]);
        let szs = sizes(9, 120.0, 60.0);
        let edges = [(1, 2), (3, 4), (5, 6), (6, 0), (7, 2), (8, 4)];
        let groups = vec![
            GroupSpec {
                members: vec![0, 1],
                depth: 0,
            },
            GroupSpec {
                members: vec![2, 3],
                depth: 0,
            },
            GroupSpec {
                members: vec![4, 5],
                depth: 0,
            },
        ];
        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(hulls.len(), groups.len());
        for i in 0..hulls.len() {
            for j in (i + 1)..hulls.len() {
                assert!(
                    !rects_overlap(&hulls[i], &hulls[j]),
                    "hulls {i},{j} overlap: {:?} vs {:?}",
                    hulls[i],
                    hulls[j]
                );
            }
        }
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!overlaps(&rects[i], &rects[j]), "nodes {i},{j} overlap");
            }
        }
        for loose in [6usize, 7, 8] {
            for (gi, hull) in hulls.iter().enumerate() {
                assert!(
                    !rects_overlap(&rects[loose], hull),
                    "ungrouped node {loose} sits inside hull {gi}"
                );
            }
        }
    }

    /// The separation post-conditions ("sibling hulls never overlap" and "no
    /// node-node overlap") must hold on the *returned* rects for every shape of
    /// input, not just the hand-picked fixtures. Deterministic sweep; the case
    /// n=11 wk=1 ek=4 gk=1 used to fail when overlap removal ran *after* the
    /// last separation check.
    #[test]
    fn hull_separation_post_conditions_hold_across_configurations() {
        let cfg = StressConfig::default();
        for n in 6..12usize {
            for wk in 0..4 {
                let szs: Vec<Size> = (0..n)
                    .map(|i| Size {
                        w: 60.0 + (wk * 60) as f64 + (i % 3) as f64 * 40.0,
                        h: 30.0 + (i % 2) as f64 * 50.0,
                    })
                    .collect();
                for ek in 0..6usize {
                    let edges: Vec<(usize, usize)> = (0..n)
                        .map(|i| (i, (i * (ek + 1) + 1) % n))
                        .filter(|(a, b)| a != b)
                        .collect();
                    for gk in 1..4usize {
                        let mut groups = Vec::new();
                        let per = 2 + gk;
                        let mut i = 0;
                        while i + per <= n {
                            groups.push(GroupSpec {
                                members: (i..i + per).collect(),
                                depth: 0,
                            });
                            i += per;
                        }
                        if groups.len() < 2 {
                            continue;
                        }
                        let g = ids(&vec!["x"; n]);
                        let (rects, hulls) = layout_grouped(&g, &szs, &edges, &groups, &cfg);
                        for a in 0..hulls.len() {
                            for b in (a + 1)..hulls.len() {
                                assert!(
                                    !rects_overlap(&hulls[a], &hulls[b]),
                                    "n={n} wk={wk} ek={ek} gk={gk}: hulls {a},{b} overlap"
                                );
                            }
                        }
                        for a in 0..rects.len() {
                            for b in (a + 1)..rects.len() {
                                assert!(
                                    !overlaps(&rects[a], &rects[b]),
                                    "n={n} wk={wk} ek={ek} gk={gk}: nodes {a},{b} overlap"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn layout_grouped_is_deterministic() {
        let cfg = StressConfig::default();
        let g = ids(&["a", "b", "c", "d", "e"]);
        let szs = sizes(5, 100.0, 40.0);
        let edges = [(0, 1), (2, 3)];
        let groups = vec![GroupSpec {
            members: vec![0, 1, 2],
            depth: 0,
        }];
        let one = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        let two = layout_grouped(&g, &szs, &edges, &groups, &cfg);
        assert_eq!(one, two);
    }
}
