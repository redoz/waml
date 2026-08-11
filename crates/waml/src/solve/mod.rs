//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use crate::layout::{Axis, Direction, Edge, Margin, Shape};
use std::collections::{BTreeMap, BTreeSet};

pub mod constrain;
pub mod crossing;
pub mod flow;
pub mod geometry;
pub mod interaction;
pub mod label;
pub mod resolve;
pub mod route;
pub mod sizing;
pub mod stress;
pub mod use_case;
pub mod vpsc;

// Wire (solver IO) types live in a nested module so that the `Tsify` derive's
// generated `VectorIntoWasmAbi`/`VectorFromWasmAbi` impls — which reference the
// unqualified `std::boxed::Box<[Self]>` — resolve to the prelude `Box`, not the
// internal IR type `solve::Box` defined below (which would otherwise shadow it
// in this module's scope). Re-exported below so all existing `solve::X` paths
// (including `super::X` imports in `resolve.rs`/`geometry.rs`) are unaffected.
mod wire {
    use crate::layout::Shape;
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Size {
        pub w: f64,
        pub h: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Rect {
        pub x: f64,
        pub y: f64,
        pub w: f64,
        pub h: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct SolveConfig {
        pub margin_px: [f64; 4],
        pub chip: Size,
        /// Floor for the gap between two UNCONNECTED node neighbours. Defaulted
        /// on deserialize so a payload serialized before this field existed
        /// still deserializes.
        #[cfg_attr(feature = "serde", serde(default = "super::default_min_sep"))]
        pub min_sep: f64,
        /// Floor for the facing-border gap between two nodes joined by an edge.
        /// Defaulted on deserialize for the same reason as `min_sep`.
        #[cfg_attr(feature = "serde", serde(default = "super::default_min_assoc"))]
        pub min_assoc: f64,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct FlagSet {
        pub emphasized: bool,
        pub collapsed: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct SolvedGroup {
        pub rect: Rect,
        pub shape: Shape,
        pub title: Option<String>,
        pub depth: u8,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Route {
        pub points: Vec<(f64, f64)>,
        pub source: String,
        pub target: String,
        /// The authored edge this route was built for, when the caller knows it.
        /// Two edges between the SAME pair of boxes are otherwise
        /// indistinguishable, so consumers that must map a route back to one
        /// edge (labels, hit-testing) key off this instead of `source`/`target`.
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        pub key: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Solved {
        pub nodes: BTreeMap<String, Rect>,
        pub groups: Vec<SolvedGroup>,
        pub flags: BTreeMap<String, FlagSet>,
        #[cfg_attr(feature = "serde", serde(default))]
        pub routes: Vec<Route>,
        /// Labels placed in world space by `place_labels_with_reroute`. `default` so a
        /// payload serialized before labels existed still deserializes.
        #[cfg_attr(feature = "serde", serde(default))]
        pub labels: Vec<crate::solve::label::PlacedLabel>,
        /// How many edges came back from `place_labels_with_reroute` with a
        /// DIFFERENT polyline than the first routing pass gave them -- edges,
        /// not rounds, so the number stays comparable across diagrams.
        /// `default` so a payload serialized before this existed still
        /// deserializes; a caller whose route list has desynced from the router's
        /// own gets no reroute, so this stays 0 there.
        #[cfg_attr(feature = "serde", serde(default))]
        pub label_reroutes: usize,
        /// How many labels needed a leader line into free space because
        /// nothing beside their route was clear. `default` so a payload
        /// serialized before this existed still deserializes.
        #[cfg_attr(feature = "serde", serde(default))]
        pub label_leaders: usize,
    }
}
pub use geometry::DroppedPlacement;
pub use wire::{FlagSet, Rect, Route, Size, SolveConfig, Solved, SolvedGroup};

pub type SizeMap = BTreeMap<String, Size>;

#[cfg(feature = "serde")]
fn default_min_sep() -> f64 {
    40.0
}

#[cfg(feature = "serde")]
fn default_min_assoc() -> f64 {
    72.0
}

impl Default for SolveConfig {
    fn default() -> Self {
        SolveConfig {
            margin_px: [0.0, 8.0, 16.0, 32.0],
            chip: Size { w: 96.0, h: 28.0 },
            min_sep: 40.0,
            min_assoc: 72.0,
        }
    }
}

impl SolveConfig {
    /// Pixel gap for a margin level.
    pub fn margin(&self, m: Margin) -> f64 {
        match m {
            Margin::No => self.margin_px[0],
            Margin::Small => self.margin_px[1],
            Margin::Medium => self.margin_px[2],
            Margin::Large => self.margin_px[3],
        }
    }
}

/// Stable identity of a box in the scene.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoxId {
    Node(String),
    Group(u32),
    Inline(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxKind {
    Leaf,
    Group,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Box {
    pub id: BoxId,
    pub kind: BoxKind,
    pub children: Vec<BoxId>,
    pub axis: Option<Axis>,
    pub shape: Shape,
    pub margin: Margin,
    pub flags: FlagSet,
    pub title: Option<String>,
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Place {
        a: BoxId,
        b: BoxId,
        dir: Direction,
    },
    Align {
        a: BoxId,
        a_edge: Edge,
        b: BoxId,
        b_edge: Edge,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub boxes: Vec<Box>,
    pub constraints: Vec<Constraint>,
}

/// Deterministic, human-readable dump of a solved layout. Used by tests.
pub fn pretty(solved: &Solved) -> String {
    let mut out = String::new();
    for (k, r) in &solved.nodes {
        out.push_str(&format!(
            "node {k} @ {:.0},{:.0} {:.0}x{:.0}\n",
            r.x, r.y, r.w, r.h
        ));
    }
    for g in &solved.groups {
        let title = g.title.as_deref().unwrap_or("");
        out.push_str(&format!(
            "group {:?} \"{}\" d{} @ {:.0},{:.0} {:.0}x{:.0}\n",
            g.shape, title, g.depth, g.rect.x, g.rect.y, g.rect.w, g.rect.h
        ));
    }
    for (k, f) in &solved.flags {
        if f.emphasized || f.collapsed {
            out.push_str(&format!(
                "flags {k} emphasized={} collapsed={}\n",
                f.emphasized, f.collapsed
            ));
        }
    }
    for l in &solved.labels {
        let leader = if l.leader.is_some() { " +leader" } else { "" };
        out.push_str(&format!(
            "label {} {:?} @ {:.0},{:.0} {:.0}x{:.0}{leader}\n",
            l.edge, l.slot, l.rect.x, l.rect.y, l.rect.w, l.rect.h
        ));
    }
    if !solved.labels.is_empty() || solved.label_reroutes > 0 || solved.label_leaders > 0 {
        out.push_str(&format!(
            "label-reroutes {} label-leaders {}\n",
            solved.label_reroutes, solved.label_leaders
        ));
    }
    out
}

/// Deterministic dump of a solved flow: `pretty(solved)` plus one line per
/// route (`route <source> -> <target> : x,y x,y ...`, coords `{:.0}`), in
/// `routes` order.
pub fn pretty_flow(solved: &Solved) -> String {
    let mut out = pretty(solved);
    for r in &solved.routes {
        out.push_str(&format!("route {} -> {} : ", r.source, r.target));
        let pts: Vec<String> = r
            .points
            .iter()
            .map(|(x, y)| format!("{x:.0},{y:.0}"))
            .collect();
        out.push_str(&pts.join(" "));
        out.push('\n');
    }
    out
}

/// The routing inputs a solve fed the router, handed back so a caller can
/// replay edges through it (`place_labels_with_reroute`) instead of rebuilding
/// the box forest and rect map — which only the solver knows — for itself.
pub struct SolvedRouting {
    pub boxes: Vec<Box>,
    pub rects: BTreeMap<BoxId, Rect>,
    pub edges: Vec<(BoxId, BoxId, Option<String>)>,
    /// Frontend-projected solid rectangles used by both routing and labels.
    pub hard_obstacles: Vec<Rect>,
}

impl SolvedRouting {
    pub fn context<'a>(&'a self, cfg: &'a SolveConfig) -> RoutingContext<'a> {
        RoutingContext {
            boxes: &self.boxes,
            rects: &self.rects,
            edges: &self.edges,
            cfg,
        }
    }
}

/// Terminal-label width floor per connected pair.
///
/// Within ONE edge the two terminal labels share the gap, so their widths
/// add. ACROSS parallel edges between the same pair they do not: each edge's
/// labels ride its own route, so the gap only has to hold the widest edge.
/// Feeds `constrain::compile`'s `label_widths` input on the unified stress
/// path, so every frontend computes this floor identically.
pub fn connected_label_widths(
    edges: &[(BoxId, BoxId)],
    label_requests: &[label::LabelRequest],
) -> BTreeMap<(BoxId, BoxId), f64> {
    let label_cfg = label::LabelConfig::default();
    let mut per_edge: BTreeMap<usize, f64> = BTreeMap::new();
    for req in label_requests {
        if !matches!(
            req.slot,
            label::LabelSlot::TerminalFrom | label::LabelSlot::TerminalTo
        ) {
            continue;
        }
        if req.edge >= edges.len() {
            continue;
        }
        let w = label::measure(&req.text, &label_cfg).w;
        *per_edge.entry(req.edge).or_insert(0.0) += w + label_cfg.slack / 2.0;
    }
    let mut out: BTreeMap<(BoxId, BoxId), f64> = BTreeMap::new();
    for (edge, width) in per_edge {
        let (a, b) = &edges[edge];
        let slot = out.entry(geometry::pair(a, b)).or_insert(0.0);
        *slot = slot.max(width);
    }
    out
}

/// The hard-obstacle set `place_labels_with_reroute` places
/// against: every solved node, plus each group's title strip (its interior is
/// not solid -- a group legitimately holds edges and their labels).
fn label_obstacles_with(solved: &Solved, extra_hard: &[Rect]) -> label::Obstacles {
    label::Obstacles {
        hard: solved
            .nodes
            .values()
            .copied()
            .chain(
                solved
                    .groups
                    .iter()
                    .filter(|g| g.title.is_some())
                    .map(|g| Rect {
                        h: label::GROUP_TITLE_BAND.min(g.rect.h),
                        ..g.rect
                    }),
            )
            .chain(extra_hard.iter().copied())
            .collect(),
    }
}

/// How many times a failed label may ask the router to try again. Bounded
/// because the loop is not guaranteed to converge: a reroute that frees one
/// label can block another, and without a ceiling that oscillates.
pub const MAX_REROUTE_ROUNDS: usize = 2;

/// `label_pressure` weight used only for a reroute attempt. The first routing
/// pass (`route::route`/`route::route_keyed`) always uses
/// `RouteCost::default()` (weight 0), so nothing moves until a label has
/// actually failed to place.
const REROUTE_LABEL_PRESSURE: f64 = 50.0;

/// The routing inputs a reroute pass replays edges against. Bundled because
/// they always travel together and `place_labels_with_reroute` already has
/// enough parameters without them spelled out individually.
pub struct RoutingContext<'a> {
    pub boxes: &'a [Box],
    pub rects: &'a BTreeMap<BoxId, Rect>,
    pub edges: &'a [(BoxId, BoxId, Option<String>)],
    pub cfg: &'a SolveConfig,
}

/// Route behavior and frontend-projected obstacles reused by label rerouting.
pub struct LabelRoutingPolicy<'a> {
    pub cost: route::RouteCost,
    pub route: &'a route::RoutePolicy,
    pub hard_obstacles: &'a [Rect],
}

/// How badly a placement went, smaller is better: labels with no route at all
/// first, then labels that only fit via a leader line. The reroute loop keeps
/// the best-scoring round rather than whatever the last round happened to
/// produce, so a reroute that made placement WORSE is never kept.
fn placement_score(placement: &label::Placement) -> (usize, usize) {
    (
        placement.unplaced.len(),
        placement
            .placed
            .iter()
            .filter(|p| p.leader.is_some())
            .count(),
    )
}

/// Place `requests` against the already-solved geometry, filling
/// `solved.labels`, and when a label will not fit beside its route, replay
/// the routing with `label_pressure` weighted on the offending edges so the
/// router leaves room for them, then retry placement. Bounded to
/// `MAX_REROUTE_ROUNDS` rounds, and stops early once the blocked set stops
/// changing (the router's inputs are then unchanged too, so the answer would
/// be byte-identical) or once a round produces no new geometry.
///
/// The replay routes the WHOLE edge set, not just the blocked edges: `hub_spread`
/// and parallel-edge `nudge` are cross-edge passes, and running them over a
/// one-edge slice strips the rerouted edge of its hub spread and parallel
/// separation, letting it land exactly on top of a sibling. Pressure still
/// applies only to blocked edges (every other edge carries no label size), so
/// an unblocked edge's own path is unchanged.
///
/// `routes` is the polyline list `LabelRequest::edge` indexes into, and the
/// caller passes it EXPLICITLY rather than letting this read `solved.routes`:
/// a frontend may presence-filter or substitute routes (the editor falls back
/// to a straight polyline when the ordered route stream desyncs), and resolving
/// label indices against a different list than the one they were built from
/// silently places labels on the wrong edge.
///
/// Composing the label TEXT stays with the caller on purpose: which toggles are
/// on and how a role and a multiplicity combine is display policy. The solver
/// only ever sees final strings, so the display model does not move into this
/// crate.
///
/// Rerouting additionally requires that `routes[i]` and `solved.routes[i]` are
/// the routes `routing` produced, in the router's own order. The router SKIPS
/// edges (self-edges, group endpoints, endpoints missing from `routing.rects`),
/// so `routing.edges` may be longer; `route::routable_edge_indices` supplies
/// the mapping. Any desync -- a differing length, or an equal-length list whose
/// polylines are not `solved.routes`' polylines (the editor substitutes a
/// straight fallback WITHOUT advancing its cursor, which shifts the two lists
/// against each other while keeping their lengths equal) -- disables rerouting
/// rather than rerouting the wrong edge. Placement itself still happens, so a
/// desynced caller degrades to a plain leader-line placement pass.
///
/// Sets `solved.label_reroutes` to the number of edges whose polyline actually
/// changed.
pub fn place_labels_with_reroute(
    solved: &mut Solved,
    routing: &RoutingContext,
    routes: &mut [Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
) -> Vec<label::LabelRequest> {
    place_labels_with_reroute_cost(
        solved,
        routing,
        routes,
        requests,
        cfg,
        route::RouteCost::default(),
    )
}

/// `place_labels_with_reroute` with an explicit base route cost. Callers that
/// opt into crossing-aware routing use the same policy when a label requests a
/// replay; all existing callers keep byte-identical default geometry.
pub fn place_labels_with_reroute_cost(
    solved: &mut Solved,
    routing: &RoutingContext,
    routes: &mut [Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
    base_cost: route::RouteCost,
) -> Vec<label::LabelRequest> {
    place_labels_with_reroute_policy(
        solved,
        routing,
        routes,
        requests,
        cfg,
        &LabelRoutingPolicy {
            cost: base_cost,
            route: &route::RoutePolicy::default(),
            hard_obstacles: &[],
        },
    )
}

/// Label placement and bounded rerouting with an explicit route policy.
pub fn place_labels_with_reroute_policy(
    solved: &mut Solved,
    routing: &RoutingContext,
    routes: &mut [Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
    policy: &LabelRoutingPolicy,
) -> Vec<label::LabelRequest> {
    // Only the ROUTES move here, never the nodes or groups the labels dodge.
    let obstacles = label_obstacles_with(solved, policy.hard_obstacles);
    let original: Vec<Vec<(f64, f64)>> = routes.to_vec();
    let mut placement = label::place_with_leaders(routes, requests, &obstacles, cfg);

    // Route position -> index into `routing.edges`.
    let order = route::routable_edge_indices(routing.rects, routing.edges);
    // Element-wise, not just by length: a caller that substitutes a route in
    // place keeps the lengths equal while shifting the two lists against each
    // other, and a length-only guard would then reroute -- and overwrite --
    // another edge's route from this edge's label.
    let aligned = order.len() == routes.len()
        && order.len() == solved.routes.len()
        && routes
            .iter()
            .zip(&solved.routes)
            .all(|(points, route)| *points == route.points);

    let cost = route::RouteCost {
        label_pressure: REROUTE_LABEL_PRESSURE,
        ..policy.cost
    };
    let mut best_score = placement_score(&placement);
    let mut best_routes = original.clone();
    let mut best_solved_routes = solved.routes.clone();
    let mut best_placement = placement.clone();

    let mut round = 0;
    let mut prev_blocked: Option<BTreeSet<usize>> = None;
    while aligned && round < MAX_REROUTE_ROUNDS && best_score > (0, 0) {
        round += 1;
        // Route positions whose label did not simply fit: no home at all, or a
        // home only reachable by a leader line.
        let mut blocked: BTreeSet<usize> = placement.unplaced.iter().map(|r| r.edge).collect();
        blocked.extend(
            placement
                .placed
                .iter()
                .filter(|p| p.leader.is_some())
                .map(|p| p.edge),
        );
        // Same blocked set means identical router inputs, so an identical
        // answer: stop instead of burning the round on a byte-for-byte redo.
        if prev_blocked.as_ref() == Some(&blocked) {
            break;
        }
        prev_blocked = Some(blocked.clone());

        // The label band to route room for on a blocked edge: the widest and
        // tallest of any request it carries, so one weight fits every slot.
        let mut label_sizes: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
        for req in requests {
            if blocked.contains(&req.edge) {
                let size = label::measure(&req.text, cfg);
                let slot = label_sizes.entry(req.edge).or_insert((0.0, 0.0));
                slot.0 = slot.0.max(size.w);
                slot.1 = slot.1.max(size.h);
            }
        }

        let mut keyed: Vec<route::KeyedEdge> = routing
            .edges
            .iter()
            .map(|(s, t, key)| (s.clone(), t.clone(), key.clone(), None))
            .collect();
        for (pos, &edge_idx) in order.iter().enumerate() {
            keyed[edge_idx].3 = label_sizes.get(&pos).copied();
        }

        let replayed = route::route_keyed_with_policy(
            routing.boxes,
            routing.rects,
            &keyed,
            routing.cfg,
            &cost,
            policy.route,
        );
        if replayed.len() != routes.len() {
            break; // the router disagrees with `order`: leave the routes alone
        }
        let mut changed = false;
        for (i, route) in replayed.into_iter().enumerate() {
            changed |= routes[i] != route.points;
            routes[i].clone_from(&route.points);
            solved.routes[i] = route;
        }
        if !changed {
            break;
        }

        placement = label::place_with_leaders(routes, requests, &obstacles, cfg);
        let score = placement_score(&placement);
        if score < best_score {
            best_score = score;
            best_routes.clone_from_slice(routes);
            best_solved_routes.clone_from(&solved.routes);
            best_placement = placement.clone();
        }
    }

    routes.clone_from_slice(&best_routes);
    solved.routes = best_solved_routes;
    solved.label_reroutes = routes
        .iter()
        .zip(&original)
        .filter(|(now, before)| now != before)
        .count();
    solved.label_leaders = best_score.1;
    solved.labels = best_placement.placed;
    best_placement.unplaced
}

/// Place labels against the exact polylines a frontend will draw.
///
/// Use this after frontend-specific endpoint clipping. It does not reroute, so
/// the saved attachment points, tangents, collisions, and leaders all describe
/// the final visible geometry.
pub fn place_labels_final(
    solved: &mut Solved,
    routes: &[Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
    hard_obstacles: &[Rect],
) -> Vec<label::LabelRequest> {
    let obstacles = label_obstacles_with(solved, hard_obstacles);
    let placement = label::place_with_leaders(routes, requests, &obstacles, cfg);
    solved.label_leaders = placement
        .placed
        .iter()
        .filter(|label| label.leader.is_some())
        .count();
    solved.labels = placement.placed;
    placement.unplaced
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_label_placement_uses_the_clipped_terminal_segment() {
        let mut solved = Solved {
            nodes: BTreeMap::new(),
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let routes = vec![vec![(40.0, 0.0), (100.0, 0.0)]];
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::TerminalFrom,
            text: "role".into(),
        }];

        let unresolved = place_labels_final(
            &mut solved,
            &routes,
            &requests,
            &label::LabelConfig::default(),
            &[],
        );

        assert!(unresolved.is_empty());
        assert_eq!(solved.labels.len(), 1);
        assert!(
            solved.labels[0].attach.0 >= 40.0,
            "terminal label must attach after the clipped endpoint: {:?}",
            solved.labels[0]
        );
    }

    fn route_points(solved: &Solved) -> Vec<Vec<(f64, f64)>> {
        solved.routes.iter().map(|r| r.points.clone()).collect()
    }

    /// Placement WITHOUT the reroute stage -- exactly what
    /// `place_labels_with_reroute` degrades to when it has no aligned routing
    /// to replay. Test-only: production has ONE entry point, so this is not a
    /// second public API with its own fallback semantics.
    fn place_labels(
        solved: &mut Solved,
        routes: &[Vec<(f64, f64)>],
        requests: &[label::LabelRequest],
        cfg: &label::LabelConfig,
    ) -> Vec<label::LabelRequest> {
        let obstacles = label_obstacles_with(solved, &[]);
        let placement = label::place_with_leaders(routes, requests, &obstacles, cfg);
        solved.label_leaders = placement
            .placed
            .iter()
            .filter(|p| p.leader.is_some())
            .count();
        solved.labels = placement.placed;
        placement.unplaced
    }

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

    /// A corridor with a shorter "below" detour that leaves no room for a
    /// label beside it, and a slightly longer "above" detour that does --
    /// same layout `route.rs`'s `corridor_with_tight_and_roomy_paths` uses,
    /// duplicated here so `place_labels_with_reroute`'s reroute path can be
    /// exercised against solved geometry rather than bare routing.
    struct Crowded {
        solved: Solved,
        boxes: Vec<Box>,
        rects: BTreeMap<BoxId, Rect>,
        edges: Vec<(BoxId, BoxId, Option<String>)>,
        routes: Vec<Vec<(f64, f64)>>,
    }

    fn crowded_scene_where_one_label_cannot_fit() -> Crowded {
        crowded_scene(vec![(
            BoxId::Node("a".into()),
            BoxId::Node("b".into()),
            None,
        )])
    }

    fn crowded_scene(edges: Vec<(BoxId, BoxId, Option<String>)>) -> Crowded {
        let boxes = vec![
            leafbox("a"),
            leafbox("b"),
            leafbox("wall"),
            leafbox("squeeze"),
        ];
        let mut rects: BTreeMap<BoxId, Rect> = BTreeMap::new();
        rects.insert(BoxId::Node("a".into()), nrect(0.0, 0.0, 40.0, 40.0));
        rects.insert(BoxId::Node("b".into()), nrect(400.0, 0.0, 40.0, 40.0));
        rects.insert(BoxId::Node("wall".into()), nrect(180.0, -60.0, 40.0, 130.0));
        rects.insert(
            BoxId::Node("squeeze".into()),
            nrect(180.0, 120.0, 40.0, 100.0),
        );
        let routes = route::route_keyed(&boxes, &rects, &edges, &SolveConfig::default());
        let nodes: BTreeMap<String, Rect> = rects
            .iter()
            .map(|(id, r)| {
                let BoxId::Node(k) = id else { unreachable!() };
                (k.clone(), *r)
            })
            .collect();
        let solved = Solved {
            nodes,
            groups: vec![],
            flags: BTreeMap::new(),
            routes,
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let route_points = route_points(&solved);
        Crowded {
            solved,
            boxes,
            rects,
            edges,
            routes: route_points,
        }
    }

    fn crowded_label_requests() -> Vec<label::LabelRequest> {
        vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::MidRoute,
            text: "a rather long relationship label".into(),
        }]
    }

    /// Tall enough (with `crowded_scene_where_one_label_cannot_fit`'s corridor)
    /// that BOTH sides of the shorter "below" detour collide -- `ROUTE_MARGIN`
    /// only guarantees 24 units of clearance from the obstacle a route hugs,
    /// and this label needs more than that. The longer "above" detour has open
    /// sky on its far side, so it always has room.
    fn crowded_label_config() -> label::LabelConfig {
        label::LabelConfig {
            font_size: 40.0,
            ..label::LabelConfig::default()
        }
    }

    #[test]
    fn a_label_that_cannot_be_placed_triggers_a_bounded_reroute() {
        let mut c = crowded_scene_where_one_label_cannot_fit();
        let before = c.solved.routes.clone();
        let requests = crowded_label_requests();

        let unresolved = place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        assert!(unresolved.is_empty(), "the label must still be drawn");
        assert!(
            c.solved.label_reroutes >= 1,
            "the failure should have been retried"
        );
        assert_ne!(
            before, c.solved.routes,
            "the blocked edge should have moved"
        );
        assert_eq!(before.len(), c.solved.routes.len());
    }

    #[test]
    fn crossing_aware_label_reroute_is_bounded_and_deterministic() {
        let solve = || {
            let mut c = crowded_scene_where_one_label_cannot_fit();
            let requests = crowded_label_requests();
            let unresolved = place_labels_with_reroute_cost(
                &mut c.solved,
                &RoutingContext {
                    boxes: &c.boxes,
                    rects: &c.rects,
                    edges: &c.edges,
                    cfg: &SolveConfig::default(),
                },
                &mut c.routes,
                &requests,
                &crowded_label_config(),
                route::RouteCost {
                    crossing: 2_048.0,
                    ..route::RouteCost::default()
                },
            );
            assert!(unresolved.is_empty(), "the label must still be drawn");
            assert_eq!(c.solved.label_reroutes, 1);
            (c.solved.routes, c.solved.labels)
        };

        let first = solve();
        for _ in 0..5 {
            assert_eq!(solve(), first);
        }
    }

    #[test]
    fn rerouting_stops_after_the_bound_even_when_it_never_succeeds() {
        // Both detours are boxed in on their open side too, so no reroute can
        // ever succeed -- the loop must still terminate rather than spin.
        let mut c = crowded_scene_where_one_label_cannot_fit();
        c.solved
            .nodes
            .insert("lid".to_string(), nrect(20.0, -160.0, 400.0, 80.0));
        let requests = crowded_label_requests();

        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        assert_eq!(
            c.solved.label_reroutes, 0,
            "one edge that never moves is zero reroutes, not one per round"
        );
    }

    #[test]
    fn an_equal_length_but_shifted_route_list_disables_the_reroute() {
        // The editor substitutes a straight fallback polyline IN PLACE without
        // advancing its route cursor, so its list can be the same LENGTH as
        // `solved.routes` while being element-wise shifted against it. A
        // length-only guard rerouted anyway and overwrote `solved.routes[i]`
        // from another edge's label. Degrade instead -- and never panic, which
        // is what the debug build did.
        let mut c = crowded_scene_where_one_label_cannot_fit();
        let before = c.solved.routes.clone();
        // A caller-substituted polyline: same count, different geometry.
        c.routes[0] = vec![(20.0, 20.0), (420.0, 20.0)];
        let substituted = c.routes.clone();
        let requests = crowded_label_requests();

        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        assert_eq!(c.routes, substituted, "the caller's routes must stand");
        assert_eq!(before, c.solved.routes, "no route may be overwritten");
        assert_eq!(c.solved.label_reroutes, 0);
        assert_eq!(c.solved.labels.len(), 1, "the label is still placed");
    }

    #[test]
    fn the_reroute_counts_edges_not_rounds() {
        // `label_reroutes` is documented as the number of edges actually
        // rerouted. Counting one per (round x edge) reported rounds instead,
        // and inflated the very instrumentation the reroute stage is judged by.
        let mut c = crowded_scene_where_one_label_cannot_fit();
        let requests = crowded_label_requests();
        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );
        assert!(
            c.solved.label_reroutes <= c.routes.len(),
            "at most one reroute per edge, got {}",
            c.solved.label_reroutes
        );
    }

    #[test]
    fn a_reroute_maps_route_positions_back_through_the_router_s_skips() {
        // The router SKIPS self-edges, so `routes[0]` here comes from
        // `edges[1]`. Indexing the edge list with the route position rerouted
        // the self-edge (a no-op) and the real blocked edge never moved.
        let mut c = crowded_scene(vec![
            (BoxId::Node("a".into()), BoxId::Node("a".into()), None), // skipped
            (BoxId::Node("a".into()), BoxId::Node("b".into()), None),
        ]);
        assert_eq!(c.routes.len(), 1, "the self-edge yields no route");
        let before = c.solved.routes.clone();
        let requests = crowded_label_requests();

        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        assert_eq!(
            c.solved.label_reroutes, 1,
            "the real edge should have moved"
        );
        assert_ne!(before, c.solved.routes);
    }

    #[test]
    fn a_reroute_keeps_parallel_edges_apart() {
        // Rerouting a blocked edge on its own skips the cross-edge `hub_spread`
        // and `nudge` passes, so the edge came back stripped of its parallel
        // separation and landed exactly on top of its sibling.
        let mut c = crowded_scene(vec![
            (
                BoxId::Node("a".into()),
                BoxId::Node("b".into()),
                Some("e1".into()),
            ),
            (
                BoxId::Node("a".into()),
                BoxId::Node("b".into()),
                Some("e2".into()),
            ),
        ]);
        let requests = vec![
            label::LabelRequest {
                edge: 0,
                slot: label::LabelSlot::MidRoute,
                text: "a rather long relationship label".into(),
            },
            label::LabelRequest {
                edge: 1,
                slot: label::LabelSlot::MidRoute,
                text: "a rather long relationship label".into(),
            },
        ];

        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        assert_eq!(c.solved.routes.len(), 2);
        assert_ne!(
            c.solved.routes[0].points, c.solved.routes[1].points,
            "two parallel edges must not share one polyline"
        );
    }

    #[test]
    fn parallel_edges_take_the_widest_label_floor_not_the_sum() {
        let edges = vec![
            (BoxId::Node("a".into()), BoxId::Node("b".into())),
            (BoxId::Node("a".into()), BoxId::Node("b".into())),
        ];
        let requests = vec![
            label::LabelRequest {
                edge: 0,
                slot: label::LabelSlot::TerminalFrom,
                text: "order {1}".into(),
            },
            label::LabelRequest {
                edge: 1,
                slot: label::LabelSlot::TerminalFrom,
                text: "order {1}".into(),
            },
        ];
        let one = connected_label_widths(&edges, &requests[..1]);
        let both = connected_label_widths(&edges, &requests);
        assert_eq!(
            one, both,
            "a second parallel edge must not add its label width to the gap"
        );
    }

    #[test]
    fn both_terminals_of_one_edge_still_add_up() {
        let edges = vec![(BoxId::Node("a".into()), BoxId::Node("b".into()))];
        let key = (BoxId::Node("a".into()), BoxId::Node("b".into()));
        let from = label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::TerminalFrom,
            text: "order {1}".into(),
        };
        let to = label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::TerminalTo,
            text: "customer {1}".into(),
        };
        let one = connected_label_widths(&edges, std::slice::from_ref(&from))[&key];
        let two = connected_label_widths(&edges, &[from, to])[&key];
        assert!(two > one, "the two ends of one edge share the same gap");
    }

    #[test]
    fn a_label_that_fits_nowhere_is_still_drawn() {
        // Before: it silently vanished from the diagram. A degraded placement
        // beats no label at all.
        let mut solved = Solved {
            nodes: BTreeMap::from([(
                "a".to_string(),
                Rect {
                    x: -500.0,
                    y: -500.0,
                    w: 2000.0,
                    h: 2000.0,
                },
            )]),
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(0.0, 50.0), (300.0, 50.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let routes = route_points(&solved);
        let unresolved = place_labels(
            &mut solved,
            &routes,
            &requests,
            &label::LabelConfig::default(),
        );
        assert!(unresolved.is_empty());
        assert_eq!(solved.labels.len(), 1, "the label must still be drawn");
        assert_eq!(solved.labels[0].text, "places");
    }

    #[test]
    fn placed_labels_avoid_the_solved_node_rects() {
        let mut solved = Solved {
            nodes: BTreeMap::from([(
                "a".to_string(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 120.0,
                    h: 80.0,
                },
            )]),
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(120.0, 40.0), (400.0, 40.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::TerminalFrom,
            text: "order {1}".into(),
        }];

        let routes = route_points(&solved);
        place_labels(
            &mut solved,
            &routes,
            &requests,
            &label::LabelConfig::default(),
        );

        assert_eq!(solved.labels.len(), 1);
        let card = solved.nodes["a"];
        assert!(!label::collides(solved.labels[0].rect, &[card]));
    }

    #[test]
    fn a_group_title_band_is_a_hard_obstacle_but_its_interior_is_not() {
        let mut solved = Solved {
            nodes: BTreeMap::from([
                (
                    "a".to_string(),
                    Rect {
                        x: 20.0,
                        y: 40.0,
                        w: 120.0,
                        h: 80.0,
                    },
                ),
                (
                    "b".to_string(),
                    Rect {
                        x: 20.0,
                        y: 200.0,
                        w: 120.0,
                        h: 80.0,
                    },
                ),
            ]),
            groups: vec![SolvedGroup {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 400.0,
                    h: 320.0,
                },
                shape: Shape::Frame,
                title: Some("Users".into()),
                depth: 0,
            }],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(80.0, 120.0), (80.0, 200.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::MidRoute,
            text: "places".into(),
        }];

        let routes = route_points(&solved);
        place_labels(
            &mut solved,
            &routes,
            &requests,
            &label::LabelConfig::default(),
        );

        assert_eq!(solved.labels.len(), 1);
        let group = solved.groups[0].rect;
        let title_band = Rect {
            h: label::GROUP_TITLE_BAND,
            ..group
        };
        let placed = solved.labels[0].rect;
        assert!(
            !label::collides(placed, &[title_band]),
            "must clear the title"
        );
        // But the label IS allowed inside the group body -- a group legitimately
        // contains edges and their labels.
        assert!(placed.y > group.y);
    }

    #[test]
    fn labels_resolve_against_the_caller_s_route_list_not_solved_routes() {
        // The editor's drawable-edge list and `solved.routes` are explicitly
        // allowed to desync (route::route presence-filters, and the editor
        // substitutes a straight fallback polyline without advancing its
        // cursor). Requests index the CALLER's list, so placement must follow
        // the polylines it is handed.
        let mut solved = Solved {
            nodes: BTreeMap::new(),
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(0.0, 1000.0), (300.0, 1000.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let routes = vec![vec![(0.0, 50.0), (300.0, 50.0)]];
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::MidRoute,
            text: "places".into(),
        }];

        place_labels(
            &mut solved,
            &routes,
            &requests,
            &label::LabelConfig::default(),
        );

        assert_eq!(solved.labels.len(), 1);
        assert!(
            (solved.labels[0].attach.1 - 50.0).abs() < 1.0,
            "label must hang off the passed route, not solved.routes: {:?}",
            solved.labels[0]
        );
    }

    #[test]
    fn the_pretty_dump_reports_label_placement_effort() {
        let mut c = crowded_scene_where_one_label_cannot_fit();
        let requests = crowded_label_requests();
        place_labels_with_reroute(
            &mut c.solved,
            &RoutingContext {
                boxes: &c.boxes,
                rects: &c.rects,
                edges: &c.edges,
                cfg: &SolveConfig::default(),
            },
            &mut c.routes,
            &requests,
            &crowded_label_config(),
        );

        let dump = pretty(&c.solved);
        assert!(dump.contains("label-reroutes "), "dump: {dump}");
        assert!(dump.contains("label-leaders "), "dump: {dump}");
    }

    #[test]
    fn pretty_dumps_nodes_deterministically() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "b".to_string(),
            Rect {
                x: 10.0,
                y: 0.0,
                w: 200.0,
                h: 90.0,
            },
        );
        nodes.insert(
            "a".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 90.0,
            },
        );
        let solved = Solved {
            nodes,
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        // BTreeMap orders keys: a before b.
        assert_eq!(
            pretty(&solved),
            "node a @ 0,0 200x90\nnode b @ 10,0 200x90\n"
        );
    }

    #[test]
    fn solve_config_maps_margin_levels() {
        let cfg = SolveConfig::default();
        assert_eq!(cfg.margin(Margin::No), 0.0);
        assert_eq!(cfg.margin(Margin::Large), 32.0);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn solve_io_types_serde_roundtrip() {
        // Inputs deserialize from a JS-shaped object.
        let cfg: SolveConfig =
            serde_json::from_str(r#"{"margin_px":[0,8,16,32],"chip":{"w":96,"h":28}}"#).unwrap();
        assert_eq!(cfg, SolveConfig::default());

        let sizes: SizeMap = serde_json::from_str(r#"{"a":{"w":200,"h":90}}"#).unwrap();
        assert_eq!(sizes["a"], Size { w: 200.0, h: 90.0 });

        // Output serializes with maps as JSON objects (serde_json default).
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_string(),
            Rect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            },
        );
        let solved = Solved {
            nodes,
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![],
            labels: vec![],
            label_reroutes: 0,
            label_leaders: 0,
        };
        let v: serde_json::Value = serde_json::to_value(&solved).unwrap();
        assert_eq!(v["nodes"]["a"]["x"], 1.0);
        assert_eq!(v["nodes"]["a"]["w"], 3.0);
    }
}
