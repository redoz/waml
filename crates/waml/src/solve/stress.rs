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

use super::{BoxId, Rect, Size};
use std::collections::VecDeque;
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
}

impl Default for StressConfig {
    fn default() -> Self {
        StressConfig {
            edge_len: 120.0,
            max_iter: 300,
            epsilon: 1e-4,
            gap: 24.0,
        }
    }
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
    let n = ids.len();
    assert_eq!(n, sizes.len(), "ids and sizes length mismatch");
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![Rect {
            x: 0.0,
            y: 0.0,
            w: sizes[0].w,
            h: sizes[0].h,
        }];
    }

    let clean = dedup_edges(n, edges);
    if clean.is_empty() {
        // No meaningful distances — degenerate. Fall back to the grid.
        return grid_pack(ids, sizes, cfg);
    }

    let adj = adjacency(n, &clean);
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
        let mut rects = component_layout(&comp, sizes, &adj, cfg);
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
    out
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
fn component_layout(
    comp: &[usize],
    sizes: &[Size],
    adj: &[Vec<usize>],
    cfg: &StressConfig,
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
    // boxes have room for their footprints. Weights w = 1 / d^2.
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
            let d = h * cfg.edge_len + half_extent(&sizes[ga]) + half_extent(&sizes[gb]);
            dist[la][lb] = d;
            w[la][lb] = 1.0 / (d * d);
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
        let mut rects = vec![Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 }; 5];
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
        assert_eq!(via_grid[0], Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 });
        assert_eq!(via_grid[1], Rect { x: 124.0, y: 0.0, w: 100.0, h: 40.0 });
        assert_eq!(via_grid[2], Rect { x: 0.0, y: 64.0, w: 100.0, h: 40.0 });
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
                (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
                |(x0, y0, x1, y1), q| {
                    (x0.min(q.x), y0.min(q.y), x1.max(q.x + q.w), y1.max(q.y + q.h))
                },
            )
        };
        let (ax0, ay0, ax1, ay1) = bbox(&r[0..3]);
        let (bx0, by0, bx1, by1) = bbox(&r[3..6]);
        let disjoint = bx0 >= ax1 - 1e-6
            || ax0 >= bx1 - 1e-6
            || by0 >= ay1 - 1e-6
            || ay0 >= by1 - 1e-6;
        assert!(
            disjoint,
            "component bounding boxes overlap: a=({ax0},{ay0},{ax1},{ay1}) b=({bx0},{by0},{bx1},{by1})"
        );
    }
}
