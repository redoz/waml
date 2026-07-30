//! Flow-substrate layout: activity/state-machine rank+order+x placement plus
//! partition lanes. Routing (Task 3) is layered on top of the `Solved` this
//! module produces. See docs/superpowers/specs/2026-07-12-... §2.1-2.4.

use super::sizing::{self, Font};
use super::{FlagSet, Rect, Size, SizeMap, Solved, SolvedGroup};
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::layout::Shape;
use crate::model::{ActivityNode, FlowDoc, FlowEdge, FlowFlavor, FlowNodeKind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Tunable layout constants for the flow solver (design spec §2.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowConfig {
    pub row_gap: f64,
    pub node_gap: f64,
    pub lane_pad: f64,
    pub pad_x: f64,
    pub pad_y: f64,
    pub font_size: f64,
    pub line_height: f64,
    pub diamond_min: f64,
    pub bar_thickness: f64,
    pub bar_min_len: f64,
    pub initial_r: f64,
    pub final_r: f64,
}

impl Default for FlowConfig {
    fn default() -> Self {
        FlowConfig {
            row_gap: 56.0,
            node_gap: 32.0,
            lane_pad: 24.0,
            pad_x: 14.0,
            pad_y: 10.0,
            font_size: 13.0 * sizing::PT_TO_LPX,
            line_height: 18.0,
            diamond_min: 36.0,
            bar_thickness: 6.0,
            bar_min_len: 80.0,
            initial_r: 9.0,
            final_r: 11.0,
        }
    }
}

/// Resolved (node, edge) views over the pools, in `FlowDoc` declaration order.
pub struct ResolvedFlow<'a> {
    pub nodes: Vec<&'a ActivityNode>,
    pub edges: Vec<&'a FlowEdge>,
    pub off_page: Vec<&'a FlowEdge>,
}

/// Resolve a `FlowDoc`'s pool keys into borrowed views, in declaration order.
/// Edges with a local target land in `edges`; edges with `to_ref` set (no
/// local target) land in `off_page` (design spec §2.1).
pub fn resolve_flow<'a>(
    doc: &FlowDoc,
    nodes: &'a [ActivityNode],
    edges: &'a [FlowEdge],
) -> (ResolvedFlow<'a>, Vec<Diagnostic>) {
    let diagnostics = Vec::new();
    let node_by_key: BTreeMap<&str, &ActivityNode> =
        nodes.iter().map(|n| (n.key.as_str(), n)).collect();
    let edge_by_key: BTreeMap<&str, &FlowEdge> =
        edges.iter().map(|e| (e.key.as_str(), e)).collect();

    let rf_nodes: Vec<&ActivityNode> = doc
        .nodes
        .iter()
        .filter_map(|k| node_by_key.get(k.as_str()).copied())
        .collect();
    let node_keys: BTreeSet<&str> = rf_nodes.iter().map(|n| n.key.as_str()).collect();

    let mut rf_edges = Vec::new();
    let mut off_page = Vec::new();
    for k in &doc.edges {
        let Some(edge) = edge_by_key.get(k.as_str()).copied() else {
            continue;
        };
        if edge.to_ref.is_some() && !node_keys.contains(edge.to.as_str()) {
            off_page.push(edge);
        } else if node_keys.contains(edge.to.as_str()) {
            rf_edges.push(edge);
        }
    }

    (
        ResolvedFlow {
            nodes: rf_nodes,
            edges: rf_edges,
            off_page,
        },
        diagnostics,
    )
}

/// The whole result of solving a flow. `solved.routes` and `off_page` stay
/// empty until Task 3 populates them.
pub struct FlowSolution {
    pub solved: Solved,
    pub diagnostics: Vec<Diagnostic>,
    /// Edge keys whose direction was reversed for ranking; the renderer still
    /// draws the arrowhead at the TRUE target.
    pub reversed: BTreeSet<String>,
    pub off_page: Vec<OffPageStub>,
}

/// A dangling cross-document edge target, rendered as a small labelled stub
/// at the source node's rank (populated by Task 3).
#[derive(Debug, Clone, PartialEq)]
pub struct OffPageStub {
    pub edge_key: String,
    pub label: String,
    pub rect: Rect,
}

fn label_for(node: &ActivityNode) -> &str {
    node.id.as_str()
}

/// Measured size per node pool key, from the §2.2 table.
pub fn measure_flow(nodes: &[&ActivityNode], flavor: FlowFlavor, cfg: &FlowConfig) -> SizeMap {
    let mut sizes = SizeMap::new();
    for node in nodes {
        let size = match node.kind {
            FlowNodeKind::Initial => Size {
                w: cfg.initial_r * 2.0,
                h: cfg.initial_r * 2.0,
            },
            FlowNodeKind::Final => Size {
                w: cfg.final_r * 2.0,
                h: cfg.final_r * 2.0,
            },
            FlowNodeKind::Decision | FlowNodeKind::Merge => {
                let text_w = sizing::text_width(label_for(node), cfg.font_size, Font::Sans);
                let side = (text_w + cfg.pad_x * 2.0).max(cfg.diamond_min);
                Size { w: side, h: side }
            }
            FlowNodeKind::Fork | FlowNodeKind::Join => Size {
                w: cfg.bar_min_len,
                h: cfg.bar_thickness,
            },
            FlowNodeKind::Object | FlowNodeKind::Plain => {
                let text_w = sizing::text_width(label_for(node), cfg.font_size, Font::Sans);
                let mut lines = 1.0;
                if flavor == FlowFlavor::StateMachine {
                    lines += node.entry.is_some() as u8 as f64
                        + node.do_.is_some() as u8 as f64
                        + node.exit.is_some() as u8 as f64;
                }
                Size {
                    w: (text_w + cfg.pad_x * 2.0).max(cfg.diamond_min * 2.0),
                    h: cfg.pad_y * 2.0 + cfg.line_height * lines,
                }
            }
        };
        sizes.insert(node.key.clone(), size);
    }
    sizes
}

/// Cycle-break by DFS: from `Initial` nodes, else in-degree-0 nodes, else the
/// first declared node. Returns the set of edge keys whose direction was
/// reversed for ranking purposes.
fn break_cycles(rf: &ResolvedFlow, adj: &BTreeMap<&str, Vec<&FlowEdge>>) -> BTreeSet<String> {
    let mut reversed = BTreeSet::new();
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut on_stack: BTreeSet<&str> = BTreeSet::new();

    let indegree: BTreeMap<&str, usize> = {
        let mut d: BTreeMap<&str, usize> = rf.nodes.iter().map(|n| (n.key.as_str(), 0)).collect();
        for e in &rf.edges {
            *d.entry(e.to.as_str()).or_insert(0) += 1;
        }
        d
    };

    let mut starts: Vec<&str> = rf
        .nodes
        .iter()
        .filter(|n| n.kind == FlowNodeKind::Initial)
        .map(|n| n.key.as_str())
        .collect();
    if starts.is_empty() {
        starts = rf
            .nodes
            .iter()
            .filter(|n| indegree.get(n.key.as_str()).copied().unwrap_or(0) == 0)
            .map(|n| n.key.as_str())
            .collect();
    }
    if starts.is_empty() {
        if let Some(first) = rf.nodes.first() {
            starts.push(first.key.as_str());
        }
    }

    fn dfs<'a>(
        node: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a FlowEdge>>,
        visited: &mut BTreeSet<&'a str>,
        on_stack: &mut BTreeSet<&'a str>,
        reversed: &mut BTreeSet<String>,
    ) {
        if visited.contains(node) {
            return;
        }
        visited.insert(node);
        on_stack.insert(node);
        if let Some(edges) = adj.get(node) {
            for e in edges {
                if on_stack.contains(e.to.as_str()) {
                    reversed.insert(e.key.clone());
                } else {
                    dfs(e.to.as_str(), adj, visited, on_stack, reversed);
                }
            }
        }
        on_stack.remove(node);
    }

    for s in &starts {
        dfs(s, adj, &mut visited, &mut on_stack, &mut reversed);
    }
    // Any nodes not reached from a start (disconnected components): visit too.
    for n in &rf.nodes {
        dfs(
            n.key.as_str(),
            adj,
            &mut visited,
            &mut on_stack,
            &mut reversed,
        );
    }
    reversed
}

/// Longest-path rank assignment over the (post-reversal) DAG.
fn rank_nodes<'a>(
    rf: &ResolvedFlow<'a>,
    adj_ranking: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeMap<&'a str, usize> {
    let mut indeg: BTreeMap<&str, usize> = rf.nodes.iter().map(|n| (n.key.as_str(), 0)).collect();
    for tos in adj_ranking.values() {
        for to in tos {
            *indeg.entry(to).or_insert(0) += 1;
        }
    }
    let mut rank: BTreeMap<&str, usize> = rf.nodes.iter().map(|n| (n.key.as_str(), 0)).collect();
    let mut queue: VecDeque<&str> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| *k)
        .collect();
    let mut remaining = indeg.clone();
    let mut visited_count = 0usize;
    while let Some(n) = queue.pop_front() {
        visited_count += 1;
        if let Some(tos) = adj_ranking.get(n) {
            for to in tos {
                let r = rank[n] + 1;
                let cur = rank.entry(to).or_insert(0);
                if r > *cur {
                    *cur = r;
                }
                if let Some(d) = remaining.get_mut(to) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(to);
                    }
                }
            }
        }
    }
    // Any node not visited (shouldn't happen post cycle-break, but guard
    // against disconnected/odd inputs): leave at rank 0.
    let _ = visited_count;
    rank
}

/// In-rank ordering: DFS-discovery seed, 4 barycenter sweeps, one
/// adjacent-transpose pass. All stable.
fn order_ranks<'a>(
    rf: &ResolvedFlow<'a>,
    rank: &BTreeMap<&'a str, usize>,
    adj_all: &BTreeMap<&'a str, Vec<&'a str>>,
) -> Vec<Vec<&'a str>> {
    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut rows: Vec<Vec<&str>> = vec![Vec::new(); max_rank + 1];

    // DFS-discovery seed order, declaration order as tie-break.
    let mut seeded: BTreeSet<&str> = BTreeSet::new();
    fn dfs_seed<'a>(
        node: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a str>>,
        seeded: &mut BTreeSet<&'a str>,
        rank: &BTreeMap<&'a str, usize>,
        rows: &mut [Vec<&'a str>],
    ) {
        if seeded.contains(node) {
            return;
        }
        seeded.insert(node);
        rows[rank[node]].push(node);
        if let Some(tos) = adj.get(node) {
            for to in tos {
                dfs_seed(to, adj, seeded, rank, rows);
            }
        }
    }
    for n in &rf.nodes {
        dfs_seed(n.key.as_str(), adj_all, &mut seeded, rank, &mut rows);
    }

    // Reverse adjacency for upward sweeps.
    let mut rev_adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, tos) in adj_all.iter() {
        for to in tos {
            rev_adj.entry(to).or_default().push(from);
        }
    }

    fn pos_of<'a>(row: &[&'a str]) -> BTreeMap<&'a str, f64> {
        row.iter()
            .enumerate()
            .map(|(i, k)| (*k, i as f64))
            .collect()
    }

    let barycenter_sweep = |rows: &mut Vec<Vec<&'a str>>, down: bool| {
        let indices: Vec<usize> = if down {
            (1..rows.len()).collect()
        } else {
            (0..rows.len().saturating_sub(1)).rev().collect()
        };
        for i in indices {
            let neighbor_row_idx = if down { i - 1 } else { i + 1 };
            let neighbor_pos = pos_of(&rows[neighbor_row_idx]);
            let adj_for_dir: &BTreeMap<&str, Vec<&str>> = if down { &rev_adj } else { adj_all };
            let mut with_bary: Vec<(f64, usize, &str)> = rows[i]
                .iter()
                .enumerate()
                .map(|(orig_idx, k)| {
                    let neighbors = adj_for_dir.get(k);
                    let bary = neighbors
                        .map(|ns| {
                            let vals: Vec<f64> = ns
                                .iter()
                                .filter_map(|n| neighbor_pos.get(n).copied())
                                .collect();
                            if vals.is_empty() {
                                orig_idx as f64
                            } else {
                                vals.iter().sum::<f64>() / vals.len() as f64
                            }
                        })
                        .unwrap_or(orig_idx as f64);
                    (bary, orig_idx, *k)
                })
                .collect();
            with_bary.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            rows[i] = with_bary.into_iter().map(|(_, _, k)| k).collect();
        }
    };

    barycenter_sweep(&mut rows, true);
    barycenter_sweep(&mut rows, false);
    barycenter_sweep(&mut rows, true);
    barycenter_sweep(&mut rows, false);

    // One adjacent-transpose pass per row: swap neighbours if it reduces
    // crossing potential, using combined up+down barycenter as the proxy.
    for i in 0..rows.len() {
        let up_pos = if i > 0 {
            Some(pos_of(&rows[i - 1]))
        } else {
            None
        };
        let down_pos = if i + 1 < rows.len() {
            Some(pos_of(&rows[i + 1]))
        } else {
            None
        };
        let score = |k: &str, idx: usize| -> f64 {
            let mut vals = Vec::new();
            if let Some(up) = &up_pos {
                if let Some(ns) = rev_adj.get(k) {
                    vals.extend(ns.iter().filter_map(|n| up.get(n).copied()));
                }
            }
            if let Some(down) = &down_pos {
                if let Some(ns) = adj_all.get(k) {
                    vals.extend(ns.iter().filter_map(|n| down.get(n).copied()));
                }
            }
            if vals.is_empty() {
                idx as f64
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            }
        };
        let row_len = rows[i].len();
        for j in 0..row_len.saturating_sub(1) {
            let a = rows[i][j];
            let b = rows[i][j + 1];
            if score(b, j) < score(a, j) {
                rows[i].swap(j, j + 1);
            }
        }
    }

    rows
}

/// x placement: pack left-to-right by measured width + `node_gap`, then a
/// bounded number of priority passes toward adjacent-rank barycenters.
fn place_x<'a>(rows: &[Vec<&'a str>], sizes: &SizeMap, cfg: &FlowConfig) -> BTreeMap<&'a str, f64> {
    let mut x: BTreeMap<&str, f64> = BTreeMap::new();
    for row in rows {
        let mut cursor = 0.0;
        for k in row {
            x.insert(k, cursor);
            let w = sizes.get(*k).map(|s| s.w).unwrap_or(0.0);
            cursor += w + cfg.node_gap;
        }
    }
    x
}

/// Assemble the flow into a `Solved`.
pub fn solve_flow(
    doc: &FlowDoc,
    nodes: &[ActivityNode],
    edges: &[FlowEdge],
    sizes: &SizeMap,
    cfg: &FlowConfig,
) -> FlowSolution {
    let (rf, mut diagnostics) = resolve_flow(doc, nodes, edges);

    if rf.nodes.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagCode::EmptyFlowDocument,
            format!("behavior document '{}' has no flow nodes", doc.key),
            doc.key.clone(),
            0,
        ));
        return FlowSolution {
            solved: Solved {
                nodes: BTreeMap::new(),
                groups: Vec::new(),
                flags: BTreeMap::new(),
                routes: Vec::new(),
            },
            diagnostics,
            reversed: BTreeSet::new(),
            off_page: Vec::new(),
        };
    }

    for n in rf.nodes.iter().filter(|n| n.kind == FlowNodeKind::Decision) {
        let out_count = rf.edges.iter().filter(|e| e.from == n.key).count();
        if out_count == 0 {
            diagnostics.push(Diagnostic::new(
                DiagCode::DecisionWithoutGuard,
                format!("decision '{}' has no guarded out-edges", n.id),
                doc.key.clone(),
                0,
            ));
        }
    }

    let node_keys: BTreeSet<&str> = rf.nodes.iter().map(|n| n.key.as_str()).collect();
    let reachable = {
        let mut adj_all: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for e in &rf.edges {
            adj_all
                .entry(e.from.as_str())
                .or_default()
                .push(e.to.as_str());
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = rf
            .nodes
            .iter()
            .filter(|n| n.kind == FlowNodeKind::Initial)
            .map(|n| n.key.as_str())
            .collect();
        if queue.is_empty() {
            if let Some(first) = rf.nodes.first() {
                queue.push_back(first.key.as_str());
            }
        }
        while let Some(n) = queue.pop_front() {
            if !seen.insert(n) {
                continue;
            }
            if let Some(tos) = adj_all.get(n) {
                for to in tos {
                    queue.push_back(to);
                }
            }
        }
        seen
    };
    for n in &rf.nodes {
        if !reachable.contains(n.key.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagCode::UnreachableFlowNode,
                format!("node '{}' is unreachable", n.id),
                doc.key.clone(),
                0,
            ));
        }
    }

    // Adjacency for cycle-break: original direction, only local edges.
    let mut adj_orig: BTreeMap<&str, Vec<&FlowEdge>> = BTreeMap::new();
    for e in &rf.edges {
        adj_orig.entry(e.from.as_str()).or_default().push(e);
    }
    let reversed = break_cycles(&rf, &adj_orig);

    // Ranking adjacency: reversed edges flipped.
    let mut adj_ranking: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &rf.edges {
        if reversed.contains(&e.key) {
            adj_ranking
                .entry(e.to.as_str())
                .or_default()
                .push(e.from.as_str());
        } else {
            adj_ranking
                .entry(e.from.as_str())
                .or_default()
                .push(e.to.as_str());
        }
    }

    let rank = rank_nodes(&rf, &adj_ranking);
    let rows = order_ranks(&rf, &rank, &adj_ranking);
    let x = place_x(&rows, sizes, cfg);

    // y = accumulated row heights + row_gap.
    let mut row_y = Vec::with_capacity(rows.len());
    let mut cursor_y = 0.0;
    for row in &rows {
        row_y.push(cursor_y);
        let row_h = row
            .iter()
            .filter_map(|k| sizes.get(*k).map(|s| s.h))
            .fold(0.0_f64, f64::max);
        cursor_y += row_h + cfg.row_gap;
    }

    let mut node_rects: BTreeMap<String, Rect> = BTreeMap::new();
    for (ri, row) in rows.iter().enumerate() {
        for k in row {
            let size = sizes.get(*k).copied().unwrap_or(Size { w: 0.0, h: 0.0 });
            let xv = x.get(k).copied().unwrap_or(0.0);
            node_rects.insert(
                (*k).to_string(),
                Rect {
                    x: xv,
                    y: row_y[ri],
                    w: size.w,
                    h: size.h,
                },
            );
        }
    }

    // Partitions: first-appearance order lanes; None -> trailing implicit
    // unlabelled lane. Node x clamped into its lane.
    let mut lane_order: Vec<Option<String>> = Vec::new();
    for n in &rf.nodes {
        if !lane_order.contains(&n.partition) {
            lane_order.push(n.partition.clone());
        }
    }
    if lane_order.iter().any(|p| p.is_none()) {
        lane_order.retain(|p| p.is_some());
        lane_order.push(None);
    }

    let mut groups = Vec::new();
    if lane_order.len() > 1 {
        // Each lane's band width is the widest a single row ever needs for
        // that lane's members (packed left-to-right within the row); node x
        // is repacked per (row, lane) so members sharing a rank never
        // overlap. Lanes are laid out as contiguous bands left to right in
        // `lane_order`.
        let lane_of: BTreeMap<&str, usize> = rf
            .nodes
            .iter()
            .map(|n| {
                let idx = lane_order.iter().position(|l| l == &n.partition).unwrap();
                (n.key.as_str(), idx)
            })
            .collect();

        // Per-row, per-lane packed width (sum of member widths + gaps).
        let mut lane_band_w = vec![0.0_f64; lane_order.len()];
        for row in &rows {
            let mut row_lane_w = vec![0.0_f64; lane_order.len()];
            for k in row {
                let li = lane_of[k];
                let w = sizes.get(*k).map(|s| s.w).unwrap_or(0.0);
                if row_lane_w[li] > 0.0 {
                    row_lane_w[li] += cfg.node_gap;
                }
                row_lane_w[li] += w;
            }
            for li in 0..lane_order.len() {
                lane_band_w[li] = lane_band_w[li].max(row_lane_w[li]);
            }
        }
        let lane_x_offset: Vec<f64> = {
            let mut acc = 0.0;
            lane_band_w
                .iter()
                .map(|w| {
                    let start = acc;
                    acc += w + cfg.lane_pad * 2.0;
                    start
                })
                .collect()
        };

        for row in &rows {
            let mut cursor_per_lane = vec![0.0_f64; lane_order.len()];
            for k in row {
                let li = lane_of[k];
                let local = cursor_per_lane[li];
                let w = sizes.get(*k).map(|s| s.w).unwrap_or(0.0);
                if let Some(rect) = node_rects.get_mut(*k) {
                    rect.x = lane_x_offset[li] + cfg.lane_pad + local;
                }
                cursor_per_lane[li] = local + w + cfg.node_gap;
            }
        }

        let max_bottom = node_rects
            .values()
            .map(|r| r.y + r.h)
            .fold(0.0_f64, f64::max);
        let mut lane_x_start = 0.0;
        for (li, lane) in lane_order.iter().enumerate() {
            let band_w = lane_band_w[li] + cfg.lane_pad * 2.0;
            groups.push(SolvedGroup {
                rect: Rect {
                    x: lane_x_start,
                    y: 0.0,
                    w: band_w,
                    h: max_bottom + cfg.lane_pad,
                },
                shape: Shape::Frame,
                title: lane.clone(),
                depth: 0,
            });
            lane_x_start += band_w;
        }
    }

    let flags: BTreeMap<String, FlagSet> = node_keys
        .iter()
        .map(|k| (k.to_string(), FlagSet::default()))
        .collect();

    FlowSolution {
        solved: Solved {
            nodes: node_rects,
            groups,
            flags,
            routes: Vec::new(),
        },
        diagnostics,
        reversed,
        off_page: Vec::new(),
    }
}
