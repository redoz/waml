//! Flow-substrate layout: activity/state-machine rank+order+x placement plus
//! partition lanes. Routing (Task 3) is layered on top of the `Solved` this
//! module produces. See docs/superpowers/specs/2026-07-12-... §2.1-2.4.

use super::route;
use super::sizing::{self, Font};
use super::{
    Box, BoxId, BoxKind, FlagSet, Rect, Route, Size, SizeMap, SolveConfig, Solved, SolvedGroup,
};
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::layout::{Margin, Shape};
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
    /// Height of ONE drawn line, from [`sizing::chrome_metrics`]. Sizes a
    /// single-line box; do NOT multiply it to stack lines.
    pub line_height: f64,
    /// Top-edge to top-edge step between stacked lines, from
    /// [`sizing::chrome_metrics`]. Larger than `line_height` because makepad
    /// scales the baseline-to-baseline distance by `line_spacing`.
    pub line_advance: f64,
    pub diamond_min: f64,
    pub bar_thickness: f64,
    pub bar_min_len: f64,
    pub initial_r: f64,
    pub final_r: f64,
}

impl Default for FlowConfig {
    fn default() -> Self {
        // Both heights come off the SAME measurement the editor draws with, so
        // a `mod.fonts` change moves box and glyph together instead of leaving
        // text hanging out of its box.
        let font_size = 13.0 * sizing::PT_TO_LPX;
        let metrics = sizing::chrome_metrics(font_size, Font::Sans);
        FlowConfig {
            row_gap: 56.0,
            node_gap: 32.0,
            lane_pad: 24.0,
            pad_x: 14.0,
            pad_y: 10.0,
            font_size,
            line_height: metrics.row_height,
            line_advance: metrics.line_advance,
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
    let mut diagnostics = Vec::new();
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
        } else {
            diagnostics.push(Diagnostic::new(
                DiagCode::UnknownFlowTarget,
                format!("edge '{}' targets unknown local id '{}'", edge.key, edge.to),
                doc.key.clone(),
                0,
            ));
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

/// A dangling cross-document edge target, rendered as a short outbound stub
/// leaving the source node's border (spec §2.1, §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct OffPageStub {
    pub edge_key: String,
    /// 2-3 points, leaving the source border.
    pub points: Vec<(f64, f64)>,
    /// Resolved from `to_ref`'s document title, else the raw `to` text.
    pub target_title: String,
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
                // A node title is drawn in the `text_heading` SemiBold cut, so
                // it must be measured in that cut too.
                let text_w = sizing::text_width(label_for(node), cfg.font_size, Font::SansSemiBold);
                let side = (text_w + cfg.pad_x * 2.0).max(cfg.diamond_min);
                Size { w: side, h: side }
            }
            FlowNodeKind::Fork | FlowNodeKind::Join => Size {
                w: cfg.bar_min_len,
                h: cfg.bar_thickness,
            },
            FlowNodeKind::Object | FlowNodeKind::Plain => {
                // The box has to hold EVERY line the renderer draws inside it,
                // not just the title: the state-machine `entry:`/`do:`/`exit:`
                // behavior lines and an object node's `:Type` line all sit in
                // this same glyph, so the widest of them sets the width.
                // The title is SemiBold (`text_heading`); every line under it is
                // Regular (`text_body`).
                let mut text_w =
                    sizing::text_width(label_for(node), cfg.font_size, Font::SansSemiBold);
                let mut lines = 1.0;
                if flavor == FlowFlavor::StateMachine {
                    for (keyword, body) in [
                        ("entry", &node.entry),
                        ("do", &node.do_),
                        ("exit", &node.exit),
                    ] {
                        if let Some(body) = body {
                            let line = format!("{keyword}: {body}");
                            text_w =
                                text_w.max(sizing::text_width(&line, cfg.font_size, Font::Sans));
                            lines += 1.0;
                        }
                    }
                }
                if let Some(object_ref) = &node.object_ref {
                    // The renderer draws the RESOLVED classifier title here;
                    // this pure pass only has the pool key, which is the same
                    // text modulo case, so it is the right-length stand-in.
                    let line = format!(":{object_ref}");
                    text_w = text_w.max(sizing::text_width(&line, cfg.font_size, Font::Sans));
                    // The classifier line is a DRAWN row like the behavior
                    // lines above, so it has to raise the box too; counting it
                    // for width alone left it hanging under the box.
                    lines += 1.0;
                }
                // One drawn row, plus one STACKING advance per line after it.
                // Multiplying the row height by the line count under-sizes the
                // box, because makepad steps further than one row between
                // baselines (`line_spacing`).
                Size {
                    w: (text_w + cfg.pad_x * 2.0).max(cfg.diamond_min * 2.0),
                    h: cfg.pad_y * 2.0 + cfg.line_height + cfg.line_advance * (lines - 1.0),
                }
            }
        };
        sizes.insert(node.key.clone(), size);
    }
    sizes
}

/// The traversal seed for a flow, per spec §2.3: `Initial` nodes, else every
/// in-degree-0 node, else the first declared node.
///
/// SINGLE SOURCE OF TRUTH. Both cycle-breaking and the reachability check that
/// feeds `UnreachableFlowNode` must use this same set. When the reachability
/// pass had its own copy that omitted the in-degree-0 tier, a legitimate flow
/// with no `initial` and two roots reported every node of the second component
/// as unreachable.
fn traversal_starts<'a>(rf: &'a ResolvedFlow) -> Vec<&'a str> {
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
    starts
}

/// Cycle-break by DFS from `traversal_starts`. Returns the set of edge keys
/// whose direction was reversed for ranking purposes.
fn break_cycles(rf: &ResolvedFlow, adj: &BTreeMap<&str, Vec<&FlowEdge>>) -> BTreeSet<String> {
    let mut reversed = BTreeSet::new();
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut on_stack: BTreeSet<&str> = BTreeSet::new();

    let starts = traversal_starts(rf);

    /// Iterative (explicit-stack) DFS: a user document can carry an arbitrarily
    /// long node chain, and recursion here would overflow the editor's stack.
    fn dfs<'a>(
        start: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a FlowEdge>>,
        visited: &mut BTreeSet<&'a str>,
        on_stack: &mut BTreeSet<&'a str>,
        reversed: &mut BTreeSet<String>,
    ) {
        if visited.contains(start) {
            return;
        }
        visited.insert(start);
        on_stack.insert(start);
        let mut stack: Vec<(&'a str, usize)> = vec![(start, 0)];
        while let Some((node, idx)) = stack.pop() {
            let edges: &[&'a FlowEdge] = adj.get(node).map(|v| v.as_slice()).unwrap_or(&[]);
            let Some(e) = edges.get(idx).copied() else {
                on_stack.remove(node);
                continue;
            };
            stack.push((node, idx + 1));
            let to = e.to.as_str();
            if on_stack.contains(to) {
                reversed.insert(e.key.clone());
            } else if !visited.contains(to) {
                visited.insert(to);
                on_stack.insert(to);
                stack.push((to, 0));
            }
        }
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
    /// Iterative (explicit-stack) pre-order DFS, for the same stack-depth
    /// reason `break_cycles::dfs` is iterative.
    fn dfs_seed<'a>(
        start: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a str>>,
        seeded: &mut BTreeSet<&'a str>,
        rank: &BTreeMap<&'a str, usize>,
        rows: &mut [Vec<&'a str>],
    ) {
        let seed = |k: &'a str, rows: &mut [Vec<&'a str>], seeded: &mut BTreeSet<&'a str>| {
            if let Some(r) = rank.get(k).copied() {
                if r < rows.len() {
                    rows[r].push(k);
                }
            }
            seeded.insert(k);
        };
        if seeded.contains(start) {
            return;
        }
        seed(start, rows, seeded);
        let mut stack: Vec<(&'a str, usize)> = vec![(start, 0)];
        while let Some((node, idx)) = stack.pop() {
            let tos: &[&'a str] = adj.get(node).map(|v| v.as_slice()).unwrap_or(&[]);
            let Some(to) = tos.get(idx).copied() else {
                continue;
            };
            stack.push((node, idx + 1));
            if !seeded.contains(to) {
                seed(to, rows, seeded);
                stack.push((to, 0));
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

/// How many alternating (down, up) barycenter passes `refine_x` runs.
const X_REFINE_PASSES: usize = 4;

/// Reverse an adjacency map.
fn reverse_adj<'a>(adj: &BTreeMap<&'a str, Vec<&'a str>>) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut rev: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, tos) in adj.iter() {
        for to in tos {
            rev.entry(to).or_default().push(from);
        }
    }
    rev
}

/// The bounded priority passes of spec §2.4 step 4: alternate down/up sweeps,
/// pulling every node toward the barycenter of its adjacent-rank neighbours
/// while keeping in-rank order and a `node_gap` separation. `bounds_of` clamps
/// a node into its band (a partition lane, or the whole row when unbanded) and
/// `group_of` names the band whose left-to-right cursor the node shares.
#[allow(clippy::too_many_arguments)]
fn refine_x<'a>(
    rows: &[Vec<&'a str>],
    sizes: &SizeMap,
    cfg: &FlowConfig,
    adj: &BTreeMap<&'a str, Vec<&'a str>>,
    rev_adj: &BTreeMap<&'a str, Vec<&'a str>>,
    bounds_of: &dyn Fn(&str) -> (f64, f64),
    group_of: &dyn Fn(&str) -> usize,
    x: &mut BTreeMap<&'a str, f64>,
    passes: usize,
) {
    let width = |k: &str| sizes.get(k).map(|s| s.w).unwrap_or(0.0);
    for pass in 0..passes {
        let down = pass % 2 == 0;
        let indices: Vec<usize> = if down {
            (1..rows.len()).collect()
        } else {
            (0..rows.len().saturating_sub(1)).rev().collect()
        };
        for i in indices {
            let neighbor_row = if down { i - 1 } else { i + 1 };
            let neighbors_of = if down { rev_adj } else { adj };
            let centers: BTreeMap<&str, f64> = rows[neighbor_row]
                .iter()
                .map(|k| (*k, x.get(k).copied().unwrap_or(0.0) + width(k) / 2.0))
                .collect();
            // What every member still to this node's RIGHT (in the same band)
            // needs, so centring a node never pushes its followers out of the
            // band's right edge.
            let mut tail: BTreeMap<&str, f64> = BTreeMap::new();
            let mut right_need: BTreeMap<usize, f64> = BTreeMap::new();
            for k in rows[i].iter().rev() {
                let group = group_of(k);
                let right = right_need.get(&group).copied().unwrap_or(0.0);
                let need = width(k)
                    + if right > 0.0 {
                        cfg.node_gap + right
                    } else {
                        0.0
                    };
                tail.insert(k, need);
                right_need.insert(group, need);
            }
            let mut cursors: BTreeMap<usize, f64> = BTreeMap::new();
            for k in &rows[i] {
                let w = width(k);
                let (lo, hi) = bounds_of(k);
                let group = group_of(k);
                let cursor = cursors.get(&group).copied().unwrap_or(lo);
                let desired = neighbors_of.get(k).and_then(|ns| {
                    let vals: Vec<f64> =
                        ns.iter().filter_map(|n| centers.get(*n).copied()).collect();
                    (!vals.is_empty()).then(|| vals.iter().sum::<f64>() / vals.len() as f64)
                });
                let current = x.get(k).copied().unwrap_or(lo);
                let target = desired.map(|c| c - w / 2.0).unwrap_or(current);
                let room = hi - tail.get(k).copied().unwrap_or(w);
                let nx = target.max(lo).max(cursor).min(room.max(lo)).max(cursor);
                x.insert(k, nx);
                cursors.insert(group, nx + w + cfg.node_gap);
            }
        }
    }
}

/// x placement: pack left-to-right by measured width + `node_gap`, then a
/// bounded number of priority passes toward adjacent-rank barycenters.
fn place_x<'a>(
    rows: &[Vec<&'a str>],
    sizes: &SizeMap,
    cfg: &FlowConfig,
    adj: &BTreeMap<&'a str, Vec<&'a str>>,
    rev_adj: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeMap<&'a str, f64> {
    let mut x: BTreeMap<&str, f64> = BTreeMap::new();
    for row in rows {
        let mut cursor = 0.0;
        for k in row {
            x.insert(k, cursor);
            let w = sizes.get(*k).map(|s| s.w).unwrap_or(0.0);
            cursor += w + cfg.node_gap;
        }
    }
    refine_x(
        rows,
        sizes,
        cfg,
        adj,
        rev_adj,
        &|_| (0.0, f64::INFINITY),
        &|_| 0,
        &mut x,
        X_REFINE_PASSES,
    );
    x
}

/// Resolves a document key (a cross-document edge's `to_ref`) to that
/// document's title, for off-page stub labels. `&|_| None` keeps the raw `to`
/// text.
pub type TitleLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Assemble the flow into a `Solved`.
pub fn solve_flow(
    doc: &FlowDoc,
    nodes: &[ActivityNode],
    edges: &[FlowEdge],
    sizes: &SizeMap,
    cfg: &FlowConfig,
    titles: TitleLookup<'_>,
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
                labels: Vec::new(),
                label_reroutes: 0,
                label_leaders: 0,
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
        // Same seed as `break_cycles` — see `traversal_starts`. Diverging here
        // reports whole components of a multi-root flow as unreachable.
        let mut queue: VecDeque<&str> = traversal_starts(&rf).into_iter().collect();
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

    // Ranking adjacency: reversed edges flipped. Self-edges are NEVER part of
    // it -- a self-loop would keep its own node's indegree above 0 forever, so
    // `rank_nodes`' topo queue would never reach it and everything downstream
    // would stay pinned at rank 0.
    let mut adj_ranking: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &rf.edges {
        if e.from == e.to {
            continue;
        }
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
    let rev_ranking = reverse_adj(&adj_ranking);
    let x = place_x(&rows, sizes, cfg, &adj_ranking, &rev_ranking);

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

    // Partitions: first-appearance order named lanes. Nodes without a
    // partition are control-flow glue, not members of a fake trailing lane;
    // they may float across the named lane stack toward their neighbours.
    let mut lane_order: Vec<String> = Vec::new();
    for n in &rf.nodes {
        if let Some(partition) = &n.partition {
            if !lane_order.contains(partition) {
                lane_order.push(partition.clone());
            }
        }
    }

    let mut groups = Vec::new();
    // A single named partition still gets its band and x clamp. An all-
    // unpartitioned activity keeps the ordinary unbanded placement above.
    if !lane_order.is_empty() {
        // Each lane's band width is the widest a single row ever needs for
        // that lane's members (packed left-to-right within the row); node x
        // is repacked per (row, lane) so members sharing a rank never
        // overlap. Lanes are laid out as contiguous bands left to right in
        // `lane_order`.
        let lane_of: BTreeMap<&str, Option<usize>> = rf
            .nodes
            .iter()
            .map(|n| {
                let index = n
                    .partition
                    .as_ref()
                    .and_then(|partition| lane_order.iter().position(|lane| lane == partition));
                (n.key.as_str(), index)
            })
            .collect();

        // Per-row, per-lane packed width (sum of member widths + gaps).
        let mut lane_band_w = vec![0.0_f64; lane_order.len()];
        for row in &rows {
            let mut row_lane_w = vec![0.0_f64; lane_order.len()];
            for k in row {
                let Some(li) = lane_of[k] else {
                    continue;
                };
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

        let mut lane_x: BTreeMap<&str, f64> = BTreeMap::new();
        for row in &rows {
            let mut cursor_per_lane = vec![0.0_f64; lane_order.len()];
            for k in row {
                if let Some(li) = lane_of[k] {
                    let local = cursor_per_lane[li];
                    let w = sizes.get(*k).map(|s| s.w).unwrap_or(0.0);
                    lane_x.insert(k, lane_x_offset[li] + cfg.lane_pad + local);
                    cursor_per_lane[li] = local + w + cfg.node_gap;
                } else {
                    lane_x.insert(k, x.get(k).copied().unwrap_or(0.0));
                }
            }
        }
        let stack_width = lane_band_w
            .iter()
            .map(|width| width + cfg.lane_pad * 2.0)
            .sum::<f64>();
        // Named nodes stay in their lane. Partition-less nodes share the whole
        // stack and are pulled toward their neighbours by the same bounded
        // priority passes, which keeps initial/final chains straight.
        let lane_bounds = |k: &str| {
            if let Some(li) = lane_of[k] {
                let lo = lane_x_offset[li] + cfg.lane_pad;
                (lo, lo + lane_band_w[li])
            } else {
                (0.0, stack_width)
            }
        };
        let free_group = lane_order.len();
        refine_x(
            &rows,
            sizes,
            cfg,
            &adj_ranking,
            &rev_ranking,
            &lane_bounds,
            &|k| lane_of[k].unwrap_or(free_group),
            &mut lane_x,
            X_REFINE_PASSES + 1,
        );
        // `refine_x` separates nodes inside one lane. A partition-less node
        // deliberately has no lane, so give it the nearest clear horizontal
        // slot after refinement. This keeps it visually free of the named
        // bands without allowing a mixed rank to overlap.
        for row in &rows {
            let mut occupied = row
                .iter()
                .filter(|key| lane_of[*key].is_some())
                .map(|key| {
                    let left = lane_x.get(key).copied().unwrap_or(0.0);
                    let width = sizes.get(*key).map(|size| size.w).unwrap_or(0.0);
                    (left, left + width)
                })
                .collect::<Vec<_>>();
            for key in row.iter().filter(|key| lane_of[**key].is_none()) {
                let width = sizes.get(*key).map(|size| size.w).unwrap_or(0.0);
                let desired = lane_x.get(key).copied().unwrap_or(0.0).max(0.0);
                let mut candidates = vec![desired, 0.0];
                for &(left, right) in &occupied {
                    candidates.push(right + cfg.node_gap);
                    if left >= width + cfg.node_gap {
                        candidates.push(left - cfg.node_gap - width);
                    }
                }
                candidates.sort_by(f64::total_cmp);
                candidates.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
                let left = candidates
                    .into_iter()
                    .filter(|candidate| {
                        occupied.iter().all(|&(other_left, other_right)| {
                            candidate + width + cfg.node_gap <= other_left
                                || *candidate >= other_right + cfg.node_gap
                        })
                    })
                    .min_by(|a, b| {
                        (a - desired)
                            .abs()
                            .total_cmp(&(b - desired).abs())
                            .then(a.total_cmp(b))
                    })
                    .unwrap_or_else(|| {
                        occupied
                            .iter()
                            .map(|(_, right)| right + cfg.node_gap)
                            .fold(0.0_f64, f64::max)
                    });
                lane_x.insert(key, left);
                occupied.push((left, left + width));
            }
        }
        for (k, xv) in &lane_x {
            if let Some(rect) = node_rects.get_mut(*k) {
                rect.x = *xv;
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
                title: Some(lane.clone()),
                depth: 0,
            });
            lane_x_start += band_w;
        }
    }

    let flags: BTreeMap<String, FlagSet> = node_keys
        .iter()
        .map(|k| (k.to_string(), FlagSet::default()))
        .collect();

    let routes = route_edges(&rf, &node_rects, &reversed, cfg);
    let off_page = build_off_page_stubs(&rf, &node_rects, cfg, titles);

    FlowSolution {
        solved: Solved {
            nodes: node_rects,
            groups,
            flags,
            routes,
            labels: Vec::new(),
            label_reroutes: 0,
            label_leaders: 0,
        },
        diagnostics,
        reversed,
        off_page,
    }
}

/// Left/right border midpoint of `rect`.
fn border_mid(rect: Rect, right: bool) -> (f64, f64) {
    if right {
        (rect.x + rect.w, rect.y + rect.h / 2.0)
    } else {
        (rect.x, rect.y + rect.h / 2.0)
    }
}

/// A 4-point orthogonal loop out the node's right border into a side channel
/// and back in (spec §2.5); the router skips self-edges, flow owns them.
fn self_edge_route(rect: Rect, cfg: &FlowConfig) -> Vec<(f64, f64)> {
    use crate::solve::route::{CORNER_INSET, ROUTE_MARGIN};
    // Half the node gap alone put the loop 16px off the border while every
    // routed connector keeps ROUTE_MARGIN (24px) clear -- and the router
    // INFLATES each node by that same margin before searching, so an edge
    // routed past this node runs down a corridor the shallow loop sits inside,
    // and crosses it. The loop has to clear the corridor it shares.
    let out_x = rect.x + rect.w + (cfg.node_gap / 2.0).max(ROUTE_MARGIN);
    // Keep the two ends off the corners for the same reason `attach` does, and
    // never let them cross over on a short node.
    let inset = CORNER_INSET.min(rect.h / 2.0);
    let y_top = (rect.y + rect.h * 0.3).max(rect.y + inset);
    let y_bot = (rect.y + rect.h * 0.7).min(rect.y + rect.h - inset);
    vec![
        (rect.x + rect.w, y_top),
        (out_x, y_top),
        (out_x, y_bot),
        (rect.x + rect.w, y_bot),
    ]
}

/// Route a reversed (loop) back-edge outside the whole rank-stack's bounding
/// column via a side channel, biased toward whichever side the source sits
/// closer to (spec §2.5). `Route.source`/`target` stay the TRUE direction.
fn back_edge_uses_right(src: Rect, all_rects: &BTreeMap<String, Rect>) -> bool {
    let min_left = all_rects
        .values()
        .map(|rect| rect.x)
        .fold(f64::INFINITY, f64::min);
    let max_right = all_rects
        .values()
        .map(|rect| rect.x + rect.w)
        .fold(f64::NEG_INFINITY, f64::max);
    src.x + src.w / 2.0 >= (min_left + max_right) / 2.0
}

/// Half the gap between two back edges' attachment points on one border.
const BACK_EDGE_PORT_STEP: f64 = 8.0;

/// Where a back edge meets `rect`'s border.
///
/// Back edges bypass the shared router entirely (see `route_edges`): they are
/// built here and appended to the route list, so the router's endpoint-spreading
/// phase never sees them. Every one of them took its side's exact MIDPOINT --
/// which is also the one attach candidate the router guarantees on every side.
/// So a back edge and the ordinary edge coming the other way between the same
/// two nodes attached at exactly the same two points and drew one connector on
/// top of the other for the whole run out to the channel. The activity fixture
/// ships that: `Retry -> Check` and `Check -> Retry` share 31px at Retry and
/// 29px at Check.
///
/// The channels are already separated by `channel_index`; fan the PORTS out by
/// the same index, alternating either side of the midpoint, never taking the
/// midpoint itself, and never crossing into the corner band the router's own
/// attach rule keeps clear.
fn back_edge_port(rect: Rect, right: bool, channel_index: usize) -> (f64, f64) {
    let (x, mid) = border_mid(rect, right);
    let rank = (channel_index + 1).div_ceil(2) as f64;
    let sign = if channel_index % 2 == 0 { 1.0 } else { -1.0 };
    let room = (rect.h / 2.0 - crate::solve::route::CORNER_INSET).max(0.0);
    let offset = (sign * rank * BACK_EDGE_PORT_STEP).clamp(-room, room);
    (x, mid + offset)
}

fn back_edge_route(
    src: Rect,
    tgt: Rect,
    all_rects: &BTreeMap<String, Rect>,
    cfg: &FlowConfig,
    channel_index: usize,
) -> Vec<(f64, f64)> {
    let min_left = all_rects
        .values()
        .map(|r| r.x)
        .fold(f64::INFINITY, f64::min);
    let max_right = all_rects
        .values()
        .map(|r| r.x + r.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let use_right = back_edge_uses_right(src, all_rects);
    let channel_step = cfg.node_gap / 2.0;
    let channel_x = if use_right {
        max_right + cfg.node_gap + channel_step * channel_index as f64
    } else {
        min_left - cfg.node_gap - channel_step * channel_index as f64
    };
    let source_port = back_edge_port(src, use_right, channel_index);
    let target_port = back_edge_port(tgt, use_right, channel_index);
    vec![
        source_port,
        (channel_x, source_port.1),
        (channel_x, target_port.1),
        target_port,
    ]
}

/// Build `Solved.routes` for a resolved flow: normal edges via the shared
/// orthogonal router, self-edges and reversed back-edges routed directly
/// (spec §2.5).
fn route_edges(
    rf: &ResolvedFlow,
    node_rects: &BTreeMap<String, Rect>,
    reversed: &BTreeSet<String>,
    cfg: &FlowConfig,
) -> Vec<Route> {
    let mut routes = Vec::new();
    let mut normal_pairs: Vec<(BoxId, BoxId, Option<String>)> = Vec::new();
    let mut left_back_edges = 0usize;
    let mut right_back_edges = 0usize;

    for e in &rf.edges {
        if e.from == e.to {
            if let Some(&rect) = node_rects.get(&e.from) {
                routes.push(Route {
                    points: self_edge_route(rect, cfg),
                    source: e.from.clone(),
                    target: e.to.clone(),
                    key: Some(e.key.clone()),
                });
            }
            continue;
        }
        if reversed.contains(&e.key) {
            if let (Some(&src), Some(&tgt)) = (node_rects.get(&e.from), node_rects.get(&e.to)) {
                let use_right = back_edge_uses_right(src, node_rects);
                let channel_index = if use_right {
                    let index = right_back_edges;
                    right_back_edges += 1;
                    index
                } else {
                    let index = left_back_edges;
                    left_back_edges += 1;
                    index
                };
                routes.push(Route {
                    points: back_edge_route(src, tgt, node_rects, cfg, channel_index),
                    source: e.from.clone(),
                    target: e.to.clone(),
                    key: Some(e.key.clone()),
                });
            }
            continue;
        }
        normal_pairs.push((
            BoxId::Node(e.from.clone()),
            BoxId::Node(e.to.clone()),
            Some(e.key.clone()),
        ));
    }

    if !normal_pairs.is_empty() {
        let boxes: Vec<Box> = node_rects
            .keys()
            .map(|k| Box {
                id: BoxId::Node(k.clone()),
                kind: BoxKind::Leaf,
                children: Vec::new(),
                axis: None,
                shape: Shape::Box,
                margin: Margin::Small,
                flags: FlagSet::default(),
                title: None,
                depth: 0,
            })
            .collect();
        let rects: BTreeMap<BoxId, Rect> = node_rects
            .iter()
            .map(|(k, r)| (BoxId::Node(k.clone()), *r))
            .collect();
        let normal_routes =
            route::route_keyed(&boxes, &rects, &normal_pairs, &SolveConfig::default());
        routes.extend(normal_routes);
    }

    routes
}

/// Build the off-page stub for each cross-document edge: a short outbound
/// polyline leaving the source node's border (spec §2.1, §4.1).
fn build_off_page_stubs(
    rf: &ResolvedFlow,
    node_rects: &BTreeMap<String, Rect>,
    cfg: &FlowConfig,
    titles: TitleLookup<'_>,
) -> Vec<OffPageStub> {
    let mut stubs = Vec::new();
    for e in &rf.off_page {
        let Some(&src) = node_rects.get(&e.from) else {
            continue;
        };
        let out_x = src.x + src.w + cfg.node_gap / 2.0;
        let y = src.y + src.h / 2.0;
        // The target document's own title, resolved through `to_ref`; the raw
        // `to` text is only the fallback (spec §2.1, §4.1).
        let target_title = e
            .to_ref
            .as_deref()
            .and_then(titles)
            .unwrap_or_else(|| e.to.clone());
        stubs.push(OffPageStub {
            edge_key: e.key.clone(),
            points: vec![(src.x + src.w, y), (out_x, y)],
            target_title,
        });
    }
    stubs
}
