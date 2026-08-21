//! QUALITY invariants over routed connectors.
//!
//! The structural tests next door check that a route SATISFIES its constraints.
//! They cannot see whether the result is any good. A connector that grazes a
//! node it was routed around, doubles back through the node it just left,
//! leaves its border with a 10px stub instead of the mandatory 24, or lies on
//! top of another connector for half its length satisfies every assertion the
//! solver makes about itself. `fd8f305f` is the proof: two connector defects
//! shipped with a green suite because every structural invariant still held.
//!
//! So this module states the quality rules as executable predicates — `audit` —
//! and runs them over three substrates:
//!
//! * the repo's authored behavior fixtures, solved end to end;
//! * hand-built dense scenes (grids, fans, corridors) that concentrate the
//!   conditions the repair phases were written for;
//! * randomly generated scenes, via proptest, where nothing was chosen to make
//!   the router look good.
//!
//! Every rule is derived from the router's own stated contract: `attach` builds
//! a mandatory perpendicular ROUTE_MARGIN stub on every border candidate and
//! drops candidates inside the CORNER_INSET band, A* routes around every
//! inflated obstacle, and the polyline is orthogonal throughout.
//!
//! Two gaps are known and deliberately not asserted away:
//!
//! * Two connectors sharing part of a run. `nudge` separates a channel by
//!   sliding runs sideways; where every free slot would cross a node the run
//!   stays put, and a shared run is the lesser defect. Held on the authored
//!   fixtures, exempted (by name, so nothing else can hide behind it) on the
//!   dense synthetic scenes.
//! * Nodes closer together than two stubs. Facing borders 47px apart cannot
//!   both keep a 24px perpendicular stub on one line, so the search cannot go
//!   straight across without reversing -- which it refuses -- and takes a
//!   sub-pixel staircase between the two boxes' centre lines instead. The
//!   generated scenes are laid out on a lattice loose enough that the case does
//!   not arise; a tighter one reproduces it, and the artefact measures under a
//!   pixel.

use super::*;
use crate::layout::{Margin, Shape};
use crate::solve::{BoxKind, FlagSet, Solved};

/// One quality-rule breach, with enough context to reproduce it.
#[derive(Debug, Clone)]
struct Breach {
    rule: &'static str,
    detail: String,
}

/// Is `p` on the border of `r` — on the boundary, not in the interior, not
/// floating off the rect entirely?
fn on_border(r: &Rect, p: P) -> bool {
    let e = 1e-6;
    let within_x = p.0 >= r.x - e && p.0 <= r.x + r.w + e;
    let within_y = p.1 >= r.y - e && p.1 <= r.y + r.h + e;
    let on_vertical = (p.0 - r.x).abs() < e || (p.0 - (r.x + r.w)).abs() < e;
    let on_horizontal = (p.1 - r.y).abs() < e || (p.1 - (r.y + r.h)).abs() < e;
    (on_vertical && within_y) || (on_horizontal && within_x)
}

/// Distance from a border point to the nearer corner of the side it sits on.
fn corner_clearance(r: &Rect, p: P, side: Side) -> f64 {
    match side {
        Side::Left | Side::Right => (p.1 - r.y).min(r.y + r.h - p.1),
        Side::Top | Side::Bottom => (p.0 - r.x).min(r.x + r.w - p.0),
    }
}

/// Length of the side an endpoint attaches to.
fn side_len(r: &Rect, side: Side) -> f64 {
    match side {
        Side::Left | Side::Right => r.h,
        Side::Top | Side::Bottom => r.w,
    }
}

/// Overlap length of two axis-aligned segments that share a line; `0.0` when
/// they are not parallel, or parallel but on different lines.
fn collinear_overlap(a: (P, P), b: (P, P)) -> f64 {
    let e = 1e-6;
    let horizontal = |s: (P, P)| (s.0 .1 - s.1 .1).abs() < e;
    let vertical = |s: (P, P)| (s.0 .0 - s.1 .0).abs() < e;
    if horizontal(a) && horizontal(b) && (a.0 .1 - b.0 .1).abs() < e {
        let (a0, a1) = (a.0 .0.min(a.1 .0), a.0 .0.max(a.1 .0));
        let (b0, b1) = (b.0 .0.min(b.1 .0), b.0 .0.max(b.1 .0));
        (a1.min(b1) - a0.max(b0)).max(0.0)
    } else if vertical(a) && vertical(b) && (a.0 .0 - b.0 .0).abs() < e {
        let (a0, a1) = (a.0 .1.min(a.1 .1), a.0 .1.max(a.1 .1));
        let (b0, b1) = (b.0 .1.min(b.1 .1), b.0 .1.max(b.1 .1));
        (a1.min(b1) - a0.max(b0)).max(0.0)
    } else {
        0.0
    }
}

/// Manhattan gap between two rects: how far apart their nearest borders are.
fn manhattan_gap(a: Rect, b: Rect) -> f64 {
    let dx = (b.x - (a.x + a.w)).max(a.x - (b.x + b.w)).max(0.0);
    let dy = (b.y - (a.y + a.h)).max(a.y - (b.y + b.h)).max(0.0);
    dx + dy
}

fn path_len(points: &[P]) -> f64 {
    points
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum()
}

/// A solved scene reduced to what the quality rules need: where each node ended
/// up, and the polyline for each connector.
struct Scene {
    rects: BTreeMap<String, Rect>,
    routes: Vec<Route>,
}

/// Every quality rule, evaluated over one solved scene.
///
/// Collects every breach rather than panicking on the first, so a failing case
/// reports the whole picture instead of whichever symptom came first.
fn audit(scene: &Scene) -> Vec<Breach> {
    let mut out = Vec::new();
    let mut push = |rule: &'static str, detail: String| out.push(Breach { rule, detail });

    for route in &scene.routes {
        let tag = format!("{}->{}", route.source, route.target);
        let pts = &route.points;
        if pts.len() < 2 {
            push("degenerate", format!("{tag}: {} points", pts.len()));
            continue;
        }
        let (Some(&src), Some(&tgt)) = (
            scene.rects.get(&route.source),
            scene.rects.get(&route.target),
        ) else {
            continue; // endpoint outside this diagram: not this rule's business
        };
        let last = pts.len() - 1;

        // Rule 1 -- the polyline is orthogonal throughout. Every rule below
        // assumes axis-aligned segments.
        for w in pts.windows(2) {
            if (w[0].0 - w[1].0).abs() > 1e-6 && (w[0].1 - w[1].1).abs() > 1e-6 {
                push(
                    "orthogonal",
                    format!("{tag}: diagonal {:?}->{:?}", w[0], w[1]),
                );
            }
        }

        // Rule 2 -- each endpoint sits ON its node's border: not floating
        // beside it, not sunk into its interior.
        for (label, p, r) in [("source", pts[0], src), ("target", pts[last], tgt)] {
            if !on_border(&r, p) {
                push(
                    "endpoint-on-border",
                    format!("{tag}: {label} {p:?} is not on the border of {r:?}"),
                );
            }
        }

        for (label, r, ep, nb) in [
            ("source", src, 0usize, 1usize),
            ("target", tgt, last, last - 1),
        ] {
            let Some(side) = attach_side(&r, pts[ep], pts[nb]) else {
                push(
                    "stub-side",
                    format!("{tag}: {label} end {:?} is on no side of {r:?}", pts[ep]),
                );
                continue;
            };
            let normal = outward_normal(side);
            let delta = (pts[nb].0 - pts[ep].0, pts[nb].1 - pts[ep].1);
            let outward = delta.0 * normal.0 + delta.1 * normal.1;
            let lateral = if perp_is_horizontal(side) {
                delta.1.abs()
            } else {
                delta.0.abs()
            };

            // Rule 3 -- the first hop off a border is perpendicular to it.
            // `attach` makes this structural; only a later phase can lose it.
            if lateral > 1e-6 {
                push(
                    "stub-perpendicular",
                    format!("{tag}: {label} stub runs {delta:?}, not square to its border"),
                );
            }

            // Rule 4 -- and it runs a full ROUTE_MARGIN before the first turn.
            // A negative value means the connector bends back INTO its node.
            if outward < ROUTE_MARGIN - 1e-6 {
                push(
                    "stub-length",
                    format!(
                        "{tag}: {label} stub is {outward:.3}px, needs {ROUTE_MARGIN} \
                         (endpoint {:?}, bend {:?}, node {r:?})",
                        pts[ep], pts[nb]
                    ),
                );
            }

            // Rule 5 -- endpoints keep CORNER_INSET clear of a corner, the same
            // band `attach` drops grid candidates in. A side with no room for
            // the band (shorter than two insets) is exempt, exactly as `attach`
            // exempts it by always keeping the side midpoint.
            let clearance = corner_clearance(&r, pts[ep], side);
            if side_len(&r, side) > 2.0 * CORNER_INSET && clearance < CORNER_INSET - 1e-6 {
                push(
                    "corner-inset",
                    format!(
                        "{tag}: {label} end {:?} is {clearance:.3}px from a corner of {r:?}",
                        pts[ep]
                    ),
                );
            }
        }

        // Rule 6 -- no segment passes through a node. A* routes around every
        // obstacle by construction, so a breach means a repair phase dragged a
        // segment somewhere A* never put it. The route's OWN endpoints count:
        // they are deliberately not obstacles while it is being routed, which
        // leaves nothing else guarding them.
        for (key, rect) in &scene.rects {
            let own = *key == route.source || *key == route.target;
            for w in pts.windows(2) {
                if segment_cuts(rect, w[0], w[1]) {
                    push(
                        if own { "cuts-own-node" } else { "cuts-node" },
                        format!("{tag}: segment {:?}->{:?} cuts {key} {rect:?}", w[0], w[1]),
                    );
                }
            }
        }

        // Rule 7 -- the polyline is proportional to the gap it spans. A route
        // that wanders breaks no structural rule; it just looks wrong. The
        // budget is deliberately generous (three times the gap, plus both
        // boxes, plus eight stubs) so it only fires on a real excursion.
        let gap = manhattan_gap(src, tgt);
        let len = path_len(pts);
        let budget = 3.0 * gap + 8.0 * ROUTE_MARGIN + src.w + src.h + tgt.w + tgt.h;
        if len > budget {
            push(
                "path-length",
                format!("{tag}: {len:.1}px path across a {gap:.1}px gap (budget {budget:.1})"),
            );
        }

        // Rule 8 -- turn count. An orthogonal connector between two boxes needs
        // a handful of bends at most; more than eight is a route threading a
        // maze, and reads as one.
        if pts.len() - 2 > 8 {
            push(
                "turn-count",
                format!("{tag}: {} turns {pts:?}", pts.len() - 2),
            );
        }
    }

    // Rule 9 -- no two connectors are drawn on top of each other. Sharing a
    // channel coordinate is fine; overlapping ON it means two edges render as
    // one line and a reader cannot tell them apart.
    for (i, left) in scene.routes.iter().enumerate() {
        for (j, right) in scene.routes.iter().enumerate().skip(i + 1) {
            for a in left.points.windows(2) {
                for b in right.points.windows(2) {
                    let overlap = collinear_overlap((a[0], a[1]), (b[0], b[1]));
                    if overlap > 1.0 {
                        push(
                            "parallel-overlap",
                            format!(
                                "routes {i} ({}->{}) and {j} ({}->{}) share {overlap:.1}px \
                                 of {:?}->{:?}",
                                left.source, left.target, right.source, right.target, a[0], a[1]
                            ),
                        );
                    }
                }
            }
        }
    }
    out
}

/// Panic listing every breach, grouped by rule.
fn assert_clean(name: &str, breaches: &[Breach]) {
    assert!(breaches.is_empty(), "{}", render(name, breaches));
}

/// Panic unless the only rule still breached is the one named. Used where a
/// known, documented gap remains, so a regression in ANY other rule is still a
/// hard failure rather than being lost in the noise of the known one.
fn assert_only(name: &str, breaches: &[Breach], allowed: &'static str) {
    let others: Vec<Breach> = breaches
        .iter()
        .filter(|b| b.rule != allowed)
        .cloned()
        .collect();
    assert!(others.is_empty(), "{}", render(name, &others));
}

fn render(name: &str, breaches: &[Breach]) -> String {
    let mut by_rule: BTreeMap<&str, Vec<&Breach>> = BTreeMap::new();
    for b in breaches {
        by_rule.entry(b.rule).or_default().push(b);
    }
    let mut msg = format!("{name}: {} quality breaches\n", breaches.len());
    for (rule, list) in by_rule {
        msg.push_str(&format!("  [{rule}] x{}\n", list.len()));
        for b in list.iter().take(4) {
            msg.push_str(&format!("      {}\n", b.detail));
        }
    }
    msg
}

// ---------------------------------------------------------------------------
// scene builders
// ---------------------------------------------------------------------------

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

type Placement = (BTreeMap<String, Rect>, Vec<(String, String)>);

/// Route a hand-placed scene through the real entry point and reduce it to what
/// `audit` needs.
fn solve_scene(rects: &BTreeMap<String, Rect>, edges: &[(String, String)]) -> Scene {
    let boxes: Vec<Box> = rects.keys().map(|k| leafbox(k)).collect();
    let by_id: BTreeMap<BoxId, Rect> = rects
        .iter()
        .map(|(k, r)| (BoxId::Node(k.clone()), *r))
        .collect();
    let keyed: Vec<KeyedEdge> = edges
        .iter()
        .enumerate()
        .map(|(i, (s, t))| KeyedEdge::at(i, BoxId::Node(s.clone()), BoxId::Node(t.clone())))
        .collect();
    Scene {
        rects: rects.clone(),
        routes: route(
            &boxes,
            &by_id,
            &keyed,
            &RouteCost::default(),
            &RoutePolicy::default(),
        ),
    }
}

/// Deterministic LCG, so a generated scene needs no dependency and reproduces
/// exactly from its seed.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
    fn range(&mut self, lo: i64, hi: i64) -> f64 {
        (lo + (self.next() % (hi - lo) as u64) as i64) as f64
    }
}

/// A jittered grid of boxes wired into a chain plus long-range hops. Every axis
/// coordinate is distinct -- a snapped grid dedups to a handful of axis lines
/// and hides how the router really behaves.
fn grid(n: usize, m: usize) -> Placement {
    let mut rects = BTreeMap::new();
    let cols = (n as f64).sqrt().ceil() as usize;
    for i in 0..n {
        let (cx, cy) = (i % cols, i / cols);
        let (jx, jy) = ((i * 7 % 40) as f64, (i * 13 % 40) as f64);
        rects.insert(
            format!("n{i}"),
            nrect(cx as f64 * 260.0 + jx, cy as f64 * 180.0 + jy, 100.0, 60.0),
        );
    }
    let mut edges = Vec::new();
    for e in 0..m {
        let (a, b) = if e < n - 1 {
            (e, e + 1)
        } else {
            (e % n, (e * 7 + 3) % n)
        };
        if a != b {
            edges.push((format!("n{a}"), format!("n{b}")));
        }
    }
    (rects, edges)
}

/// A hub with `m` spokes coming in from every direction: the fan-in case
/// `hub_spread` exists for, and the one that exposed a connector bending back
/// into its own node.
fn fan(m: usize) -> Placement {
    let mut rects = BTreeMap::new();
    rects.insert("hub".to_string(), nrect(400.0, 400.0, 120.0, 90.0));
    let mut edges = Vec::new();
    for i in 0..m {
        let angle = i as f64 / m as f64 * std::f64::consts::TAU;
        rects.insert(
            format!("s{i}"),
            nrect(
                460.0 + 420.0 * angle.cos() - 50.0,
                445.0 + 340.0 * angle.sin() - 30.0,
                100.0,
                60.0,
            ),
        );
        edges.push((format!("s{i}"), "hub".to_string()));
    }
    (rects, edges)
}

/// Random non-overlapping placement: at most one box per lattice cell, so every
/// edge is genuinely routable and no breach can be blamed on overlapping input.
fn scatter(seed: u64, n: usize, m: usize) -> Placement {
    let mut rng = Lcg(seed);
    let mut rects = BTreeMap::new();
    let mut used: BTreeSet<(i64, i64)> = BTreeSet::new();
    let span = (n as f64).sqrt().ceil() as i64 + 2;
    let (mut placed, mut guard) = (0usize, 0usize);
    while placed < n && guard < 10_000 {
        guard += 1;
        let (cx, cy) = (rng.range(0, span) as i64, rng.range(0, span) as i64);
        if !used.insert((cx, cy)) {
            continue;
        }
        rects.insert(
            format!("n{placed}"),
            nrect(
                cx as f64 * 300.0 + rng.range(0, 60),
                cy as f64 * 220.0 + rng.range(0, 60),
                80.0 + rng.range(0, 90),
                50.0 + rng.range(0, 60),
            ),
        );
        placed += 1;
    }
    let mut edges = Vec::new();
    for _ in 0..m {
        let a = (rng.next() as usize) % placed;
        let b = (rng.next() as usize) % placed;
        if a != b {
            edges.push((format!("n{a}"), format!("n{b}")));
        }
    }
    (rects, edges)
}

// ---------------------------------------------------------------------------
// the repo's authored fixtures, solved end to end
// ---------------------------------------------------------------------------

fn flow_fixture(name: &str) -> Scene {
    use crate::model::{ActivityNode, FlowEdge, FlowFlavor};
    use crate::solve::flow::{measure_flow, resolve_flow, solve_flow, FlowConfig};
    use crate::source::SourceBundle;

    let source = match name {
        "activity" => SourceBundle::try_from_pairs([
            (
                "flow.md",
                include_str!("../../../tests/fixtures/behavior/activity/flow.md"),
            ),
            (
                "order.md",
                include_str!("../../../tests/fixtures/behavior/activity/order.md"),
            ),
        ]),
        "state-machine" => SourceBundle::try_from_pairs([(
            "states.md",
            include_str!("../../../tests/fixtures/behavior/state-machine/states.md"),
        )]),
        other => panic!("unknown fixture {other}"),
    }
    .expect("fixture bundle parses");
    let prepared = crate::analysis::prepare_candidate(source, None, 1).expect("fixture analyses");
    let model = &prepared.uml().projection;
    let flavor = if name == "activity" {
        FlowFlavor::Activity
    } else {
        FlowFlavor::StateMachine
    };
    let doc = model
        .flows
        .iter()
        .find(|f| f.flavor == flavor)
        .expect("fixture declares a flow")
        .clone();
    let nodes: Vec<ActivityNode> = doc
        .nodes
        .iter()
        .map(|key| {
            model
                .activity_nodes
                .iter()
                .find(|n| &n.key == key)
                .expect("declared node exists")
                .clone()
        })
        .collect();
    let edges: Vec<FlowEdge> = doc
        .edges
        .iter()
        .map(|key| {
            model
                .flow_edges
                .iter()
                .find(|e| &e.key == key)
                .expect("declared edge exists")
                .clone()
        })
        .collect();
    let (resolved, diagnostics) = resolve_flow(&doc, &nodes, &edges);
    assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&resolved.nodes, flavor, &cfg);
    let solved: Solved = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None).solved;
    Scene {
        rects: solved.nodes.clone(),
        routes: solved.routes.clone(),
    }
}

/// The repo's own authored behavior fixtures, solved through the real pipeline.
/// These are diagrams a reader actually sees, so every rule holds here with no
/// exemption at all -- the overlap rule included.
#[test]
fn authored_fixtures_satisfy_every_connector_quality_rule() {
    for name in ["activity", "state-machine"] {
        let scene = flow_fixture(name);
        assert!(!scene.routes.is_empty(), "{name}: fixture routes nothing");
        assert_clean(name, &audit(&scene));
    }
}

// ---------------------------------------------------------------------------
// dense hand-built scenes
// ---------------------------------------------------------------------------

/// A fan-in is where `hub_spread` does its work, and where spreading one end of
/// a three-point route used to rewrite the OTHER end's stub -- bending the
/// connector straight back into the node it had just left. Every fan size from
/// two spokes to thirteen must come out clean.
#[test]
fn a_fan_in_of_any_size_keeps_every_connector_sound() {
    for m in 2..14 {
        let (rects, edges) = fan(m);
        let scene = solve_scene(&rects, &edges);
        assert_clean(&format!("fan {m}"), &audit(&scene));
    }
}

/// Grids concentrate the other repair phase: many routes sharing few corridors,
/// so `nudge` has to move runs, and every move is a chance to push one through
/// a node.
///
/// Two connectors still sharing a run is the one rule this scene cannot hold
/// yet. `nudge` separates a channel by sliding runs sideways; where every free
/// slot would cross a node the run stays put, and a shared run is the lesser
/// defect. Everything else must hold exactly.
#[test]
fn dense_grids_keep_every_connector_off_every_node() {
    for (n, m) in [(6, 8), (9, 14), (12, 20), (16, 28), (25, 45)] {
        let (rects, edges) = grid(n, m);
        let scene = solve_scene(&rects, &edges);
        assert_only(&format!("grid {n}/{m}"), &audit(&scene), "parallel-overlap");
    }
}

/// Random placements find shapes no hand-built fixture thinks of: boxes of
/// different sizes at unrelated coordinates, edges between any pair.
#[test]
fn scattered_scenes_keep_every_connector_off_every_node() {
    for seed in 0..40u64 {
        let (rects, edges) = scatter(seed, 10, 16);
        let scene = solve_scene(&rects, &edges);
        assert_only(
            &format!("scatter {seed}"),
            &audit(&scene),
            "parallel-overlap",
        );
    }
}

// ---------------------------------------------------------------------------
// regressions: one test per defect these rules found
// ---------------------------------------------------------------------------

/// A* used to be free to double back on its own line: the search's direction
/// state was axis-only, so a reversal cost no bend, and `simplify` then folded
/// the reversal away as "collinear". The only trace left was a first segment
/// SHORTER than the mandatory ROUTE_MARGIN stub -- here 19px instead of 24,
/// because the useful grid line sat between the border and the stub end.
#[test]
fn a_route_never_doubles_back_to_reach_a_line_behind_its_own_stub() {
    let (rects, edges) = grid(25, 45);
    let scene = solve_scene(&rects, &edges);
    let route = scene
        .routes
        .iter()
        .find(|r| r.source == "n2" && r.target == "n17")
        .expect("the n2 -> n17 edge is routed");
    let src = scene.rects["n2"];
    assert!(
        (route.points[0].0 - (src.x + src.w)).abs() < 1e-6,
        "expected the right border, got {:?}",
        route.points[0]
    );
    let stub = route.points[1].0 - route.points[0].0;
    assert!(
        stub >= ROUTE_MARGIN - 1e-6,
        "stub collapsed to {stub}px: {:?}",
        route.points
    );
    assert_only("n2 -> n17 scene", &audit(&scene), "parallel-overlap");
}

/// `hub_spread` spreads the endpoints crowded onto one side of a node and pulls
/// the adjacent bend along to keep the exit square. On a THREE-point route both
/// ends share that single bend, so moving it for one end rewrote the other
/// end's stub: nine spokes into one hub gave `s2` a stub of -13px -- a
/// connector whose first move was back inside its own box.
#[test]
fn spreading_one_end_never_bends_the_other_end_into_its_own_node() {
    let (rects, edges) = fan(9);
    let scene = solve_scene(&rects, &edges);
    for route in &scene.routes {
        let src = scene.rects[&route.source];
        let side = attach_side(&src, route.points[0], route.points[1])
            .unwrap_or_else(|| panic!("{}: endpoint on no side", route.source));
        let normal = outward_normal(side);
        let out = (route.points[1].0 - route.points[0].0) * normal.0
            + (route.points[1].1 - route.points[0].1) * normal.1;
        assert!(
            out >= ROUTE_MARGIN - 1e-6,
            "{}: stub is {out}px: {:?}",
            route.source,
            route.points
        );
    }
    assert_clean("fan 9", &audit(&scene));
}

/// `nudge` slides runs sideways to break up a shared channel, with no obstacle
/// model of its own -- so a run that was clear where A* left it could be slid
/// straight through a node. Two columns fully cross-connected through a wall of
/// boxes puts many runs in one narrow corridor with nowhere free to slide.
#[test]
fn separating_a_crowded_channel_never_slides_a_run_through_a_node() {
    let mut rects = BTreeMap::new();
    for i in 0..4 {
        rects.insert(format!("l{i}"), nrect(0.0, i as f64 * 150.0, 110.0, 70.0));
        rects.insert(format!("r{i}"), nrect(700.0, i as f64 * 150.0, 110.0, 70.0));
    }
    rects.insert("wall0".into(), nrect(330.0, -40.0, 90.0, 190.0));
    rects.insert("wall1".into(), nrect(330.0, 260.0, 90.0, 190.0));
    let mut edges = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            edges.push((format!("l{i}"), format!("r{j}")));
        }
    }
    let scene = solve_scene(&rects, &edges);
    assert_only("crossing wall", &audit(&scene), "parallel-overlap");
}

// ---------------------------------------------------------------------------
// properties over generated scenes
// ---------------------------------------------------------------------------

use proptest::prelude::*;

/// Non-overlapping boxes on a lattice plus an arbitrary edge set. The lattice
/// keeps boxes apart -- overlapping nodes make "route around a node"
/// meaningless -- while leaving position, size and connectivity free.
fn arbitrary_scene() -> impl Strategy<Value = Placement> {
    (
        proptest::collection::vec((0usize..6, 0usize..6, 60u32..170, 40u32..120), 2..9),
        proptest::collection::vec((0usize..9, 0usize..9), 1..14),
    )
        .prop_map(|(cells, pairs)| {
            let mut rects: BTreeMap<String, Rect> = BTreeMap::new();
            let mut used: BTreeSet<(usize, usize)> = BTreeSet::new();
            for (cx, cy, w, h) in cells {
                if !used.insert((cx, cy)) {
                    continue;
                }
                let key = format!("n{}", rects.len());
                rects.insert(
                    key,
                    nrect(
                        cx as f64 * 320.0,
                        cy as f64 * 240.0,
                        f64::from(w),
                        f64::from(h),
                    ),
                );
            }
            let n = rects.len();
            let edges = pairs
                .into_iter()
                .filter(|(a, b)| a != b && *a < n && *b < n)
                .map(|(a, b)| (format!("n{a}"), format!("n{b}")))
                .collect();
            (rects, edges)
        })
}

/// The hard rules, for any generated scene. `parallel-overlap` is filtered out
/// for the reason spelled out on `dense_grids_keep_every_connector_off_every_node`:
/// the router has no phase that can always separate two runs, and a shared run
/// is the lesser defect. Everything else must hold for every input.
fn hard_rule_breaches(rects: &BTreeMap<String, Rect>, edges: &[(String, String)]) -> Vec<Breach> {
    audit(&solve_scene(rects, edges))
        .into_iter()
        .filter(|b| b.rule != "parallel-overlap")
        .collect()
}

proptest! {
    // Kept small so the default suite stays fast; `stress_` below is the same
    // property at a case count worth running deliberately.
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// For ANY placement and ANY edge set, no connector may cross a node, lose
    /// its perpendicular stub, land in a corner band, leave its border, wander,
    /// or go diagonal.
    #[test]
    fn any_scene_keeps_every_connector_off_every_node((rects, edges) in arbitrary_scene()) {
        let breaches = hard_rule_breaches(&rects, &edges);
        prop_assert!(
            breaches.is_empty(),
            "{}\nrects: {rects:?}\nedges: {edges:?}",
            render("generated scene", &breaches)
        );
    }
}

/// The same property at a case count worth waiting for. Ignored by default so
/// the everyday suite stays fast:
/// `cargo test -p waml --lib stress_any_scene -- --ignored`.
#[test]
#[ignore = "stress run: thousands of cases, run deliberately"]
fn stress_any_scene_keeps_every_connector_off_every_node() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: 50_000,
        ..ProptestConfig::default()
    });
    runner
        .run(&arbitrary_scene(), |(rects, edges)| {
            let breaches = hard_rule_breaches(&rects, &edges);
            prop_assert!(
                breaches.is_empty(),
                "{}\nrects: {rects:?}\nedges: {edges:?}",
                render("generated scene", &breaches)
            );
            Ok(())
        })
        .expect("4000 generated scenes keep every connector off every node");
}
