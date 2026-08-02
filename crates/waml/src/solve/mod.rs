//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use crate::diagnostic::Diagnostic;
use crate::layout::{Axis, Direction, Edge, Margin, Shape};
use std::collections::{BTreeMap, BTreeSet};

pub mod flow;
pub mod geometry;
pub mod interaction;
pub mod label;
pub mod potentials;
pub mod resolve;
pub mod route;
pub mod sizing;
pub mod stress;

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
        /// Labels placed in world space by `place_labels`. `default` so a
        /// payload serialized before labels existed still deserializes.
        #[cfg_attr(feature = "serde", serde(default))]
        pub labels: Vec<crate::solve::label::PlacedLabel>,
        /// How many edges `place_labels_with_reroute` sent back through the
        /// router because their label could not be placed on the first pass.
        /// `default` so a payload serialized before this existed still
        /// deserializes; plain `place_labels` never reroutes, so this stays 0
        /// unless the caller opts into the reroute path.
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

/// Top-level entry: resolve the diagram to a `Scene`, then solve it. Keeps the
/// 2-tuple shape the wasm crate depends on; drops the placement report.
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (solved, diags, _dropped) = solve_diagram_reported(diagram, edges, sizes, cfg);
    (solved, diags)
}

/// Native-only entry: like `solve_diagram` but also returns the solver's
/// dropped-placement report (unsatisfiable placements + their contradiction sets).
/// The editor's conflict error list consumes this; the wasm path uses
/// `solve_diagram` and never sees it.
pub fn solve_diagram_reported(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    solve_diagram_reported_labeled(diagram, edges, sizes, &[], cfg)
}

/// Like `solve_diagram_reported`, but also factors each connected pair's
/// terminal-label widths into the connected-gap floor (see
/// `geometry::solve_with_rects_labeled`). `label_requests`' `edge` field
/// indexes into `edges`, matching the convention `place_labels` uses against
/// `solved.routes`. Callers with no labels to size for (the wasm path,
/// `flow.rs`) get exactly `solve_diagram_reported`'s behaviour via the empty
/// wrapper above.
pub fn solve_diagram_reported_labeled(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    label_requests: &[label::LabelRequest],
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let label_widths = connected_label_widths(edges, label_requests);
    let (mut solved, rects, mut geo_diags, dropped) =
        geometry::solve_with_rects_labeled(&scene, edges, sizes, &label_widths, cfg);
    diags.append(&mut geo_diags);
    solved.routes = route::route(&scene.boxes, &rects, edges, cfg);
    (solved, diags, dropped)
}

/// Terminal-label width floor per connected pair.
///
/// Within ONE edge the two terminal labels share the gap, so their widths add.
/// ACROSS parallel edges between the same pair they do not: each edge's labels
/// ride its own route, so the gap only has to hold the widest edge. Summing
/// there would blow a facing-border gap out by a multiple of what one edge needs.
fn connected_label_widths(
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

/// Place `requests` against the already-solved geometry, filling `solved.labels`.
///
/// `routes` is the polyline list `LabelRequest::edge` indexes into, and the
/// caller passes it EXPLICITLY rather than letting this read `solved.routes`:
/// a frontend may presence-filter or substitute routes (the editor falls back
/// to a straight polyline when the ordered route stream desyncs), and resolving
/// label indices against a different list than the one they were built from
/// silently places labels on the wrong edge.
///
/// Kept separate from `solve_diagram_reported` on purpose: composing the label
/// TEXT is display policy (which toggles are on, how a role and a multiplicity
/// combine), and that belongs to the frontend. The solver only ever sees final
/// strings, so the display model does not have to move into this crate.
/// Returns the requests that could not be positioned at all (a degenerate
/// route); labels that merely collided everywhere are still placed, at a
/// degraded last-resort position.
pub fn place_labels(
    solved: &mut Solved,
    routes: &[Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
) -> Vec<label::LabelRequest> {
    let obstacles = label_obstacles(solved);
    // A label that fits nowhere beside its route gets a leader line into free
    // space instead of silently disappearing; only a truly degenerate route
    // (no candidates at all) comes back unresolved.
    let placement = label::place_with_leaders(routes, requests, &obstacles, cfg);
    solved.label_leaders = placement
        .placed
        .iter()
        .filter(|p| p.leader.is_some())
        .count();
    solved.labels = placement.placed;
    placement.unplaced
}

/// The hard-obstacle set `place_labels`/`place_labels_with_reroute` place
/// against: every solved node, plus each group's title strip (its interior is
/// not solid -- a group legitimately holds edges and their labels).
fn label_obstacles(solved: &Solved) -> label::Obstacles {
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

/// Like `place_labels`, but when a request cannot be placed at all on the
/// first pass, re-routes just the edge(s) it belongs to with `label_pressure`
/// weighted so the router leaves room for the label, then retries placement.
/// Bounded to `MAX_REROUTE_ROUNDS` rounds: a reroute that frees one label can
/// block another, so this loop is not guaranteed to converge. Every edge that
/// was NOT rerouted keeps its original polyline byte-identical -- rerouting
/// is surgical, not global.
///
/// `routing.edges[i]` must correspond to `routes[i]` (and `solved.routes[i]`)
/// -- `routing` is the exact routing inputs `routes` was built from.
/// Sets `solved.label_reroutes` to the number of edges actually rerouted.
pub fn place_labels_with_reroute(
    solved: &mut Solved,
    routing: &RoutingContext,
    routes: &mut [Vec<(f64, f64)>],
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
) -> Vec<label::LabelRequest> {
    solved.label_reroutes = 0;
    let mut placement = label::place(routes, requests, &label_obstacles(solved), cfg);

    let mut round = 0;
    while round < MAX_REROUTE_ROUNDS && !placement.unplaced.is_empty() {
        round += 1;
        let blocked: BTreeSet<usize> = placement.unplaced.iter().map(|r| r.edge).collect();

        // The label band height to route room for on a blocked edge: the
        // tallest of any request it carries, so one weight fits every slot.
        let mut label_heights: BTreeMap<usize, f64> = BTreeMap::new();
        for req in requests {
            if blocked.contains(&req.edge) {
                let h = label::measure(&req.text, cfg).h;
                let slot = label_heights.entry(req.edge).or_insert(0.0);
                *slot = slot.max(h);
            }
        }

        let cost = route::RouteCost {
            label_pressure: REROUTE_LABEL_PRESSURE,
            ..route::RouteCost::default()
        };
        let mut rerouted_any = false;
        for &edge_idx in &blocked {
            let Some((s, t, key)) = routing.edges.get(edge_idx) else {
                continue;
            };
            let keyed = [(
                s.clone(),
                t.clone(),
                key.clone(),
                label_heights.get(&edge_idx).copied(),
            )];
            let Some(new_route) =
                route::route_keyed_with(routing.boxes, routing.rects, &keyed, routing.cfg, &cost)
                    .into_iter()
                    .next()
            else {
                continue;
            };
            if let Some(slot) = routes.get_mut(edge_idx) {
                *slot = new_route.points.clone();
            }
            if let Some(slot) = solved.routes.get_mut(edge_idx) {
                *slot = new_route;
            }
            solved.label_reroutes += 1;
            rerouted_any = true;
        }
        if !rerouted_any {
            break;
        }
        placement = label::place(routes, requests, &label_obstacles(solved), cfg);
    }

    let placement = label::place_with_leaders(routes, requests, &label_obstacles(solved), cfg);
    solved.label_leaders = placement
        .placed
        .iter()
        .filter(|p| p.leader.is_some())
        .count();
    solved.labels = placement.placed;
    placement.unplaced
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route_points(solved: &Solved) -> Vec<Vec<(f64, f64)>> {
        solved.routes.iter().map(|r| r.points.clone()).collect()
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
        let edges: Vec<(BoxId, BoxId, Option<String>)> =
            vec![(BoxId::Node("a".into()), BoxId::Node("b".into()), None)];
        let plain_edges: Vec<(BoxId, BoxId)> = edges
            .iter()
            .map(|(s, t, _)| (s.clone(), t.clone()))
            .collect();
        let routes = route::route(&boxes, &rects, &plain_edges, &SolveConfig::default());
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

        assert!(c.solved.label_reroutes <= MAX_REROUTE_ROUNDS);
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
