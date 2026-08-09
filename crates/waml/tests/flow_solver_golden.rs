use waml::model::{ActivityNode, FlowDoc, FlowEdge, FlowFlavor, FlowNodeKind};
use waml::solve::flow::{measure_flow, resolve_flow, solve_flow, FlowConfig};
use waml::solve::pretty_flow;
use waml::solve::Rect;
use waml::source::SourceBundle;

fn point_on_border(p: (f64, f64), rect: Rect) -> bool {
    let eps = 0.5;
    let on_vertical = (p.0 - rect.x).abs() <= eps || (p.0 - (rect.x + rect.w)).abs() <= eps;
    let on_horizontal = (p.1 - rect.y).abs() <= eps || (p.1 - (rect.y + rect.h)).abs() <= eps;
    let within_x = p.0 >= rect.x - eps && p.0 <= rect.x + rect.w + eps;
    let within_y = p.1 >= rect.y - eps && p.1 <= rect.y + rect.h + eps;
    (on_vertical && within_y) || (on_horizontal && within_x)
}

fn load(name: &str) -> (FlowDoc, Vec<ActivityNode>, Vec<FlowEdge>) {
    let source = match name {
        "activity" => SourceBundle::try_from_pairs([
            (
                "flow.md",
                include_str!("fixtures/behavior/activity/flow.md"),
            ),
            (
                "order.md",
                include_str!("fixtures/behavior/activity/order.md"),
            ),
        ])
        .unwrap(),
        "state-machine" => SourceBundle::try_from_pairs([(
            "states.md",
            include_str!("fixtures/behavior/state-machine/states.md"),
        )])
        .unwrap(),
        other => panic!("unknown fixture {other}"),
    };
    let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
    let model = &prepared.uml().projection;
    let doc = model
        .flows
        .iter()
        .find(|f| match name {
            "activity" => f.flavor == FlowFlavor::Activity,
            _ => f.flavor == FlowFlavor::StateMachine,
        })
        .unwrap_or_else(|| panic!("no flow doc found for {name}"))
        .clone();
    let nodes: Vec<ActivityNode> = doc
        .nodes
        .iter()
        .map(|key| {
            model
                .activity_nodes
                .iter()
                .find(|n| &n.key == key)
                .unwrap()
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
                .unwrap()
                .clone()
        })
        .collect();
    (doc, nodes, edges)
}

#[test]
fn activity_fixture_smoke_loads_nodes_and_edges() {
    let (doc, nodes, edges) = load("activity");
    assert_eq!(doc.flavor, FlowFlavor::Activity);
    assert_eq!(nodes.len(), 13);
    assert!(edges.len() >= 13);
    assert!(nodes.iter().any(|n| n.kind == FlowNodeKind::Decision));
    assert!(nodes.iter().any(|n| n.kind == FlowNodeKind::Fork));
    assert!(nodes.iter().any(|n| n.kind == FlowNodeKind::Join));
    assert!(nodes.iter().any(|n| n.kind == FlowNodeKind::Object));
    assert!(nodes.iter().any(|n| n.partition.is_none()));
    assert!(
        nodes
            .iter()
            .filter_map(|n| n.partition.as_deref())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= 2
    );
}

#[test]
fn state_machine_fixture_smoke_loads_nodes_and_edges() {
    let (doc, nodes, edges) = load("state-machine");
    assert_eq!(doc.flavor, FlowFlavor::StateMachine);
    assert!(nodes.iter().any(|n| n.entry.is_some()));
    assert!(edges
        .iter()
        .any(|e| e.trigger.is_some() && e.guard.is_some()));
    assert!(edges.iter().any(|e| e.from == e.to));
}

fn solve(name: &str, flavor: FlowFlavor) -> waml::solve::flow::FlowSolution {
    let (doc, nodes, edges) = load(name);
    let (rf, resolve_diags) = resolve_flow(&doc, &nodes, &edges);
    assert!(resolve_diags.is_empty(), "{resolve_diags:?}");
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&rf.nodes, flavor, &cfg);
    solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None)
}

const EXPECTED_ACTIVITY_GOLDEN: &str = include_str!("fixtures/behavior/activity/flow.golden.txt");

#[test]
fn activity_fixture_layout_golden() {
    let sol = solve("activity", FlowFlavor::Activity);
    assert!(sol.diagnostics.is_empty(), "{:?}", sol.diagnostics);
    assert_eq!(pretty_flow(&sol.solved), EXPECTED_ACTIVITY_GOLDEN);
}

#[test]
fn ranks_are_monotone_along_non_reversed_edges() {
    for (name, flavor) in [
        ("activity", FlowFlavor::Activity),
        ("state-machine", FlowFlavor::StateMachine),
    ] {
        let sol = solve(name, flavor);
        let (_, _, edges) = load(name);
        for e in &edges {
            if sol.reversed.contains(&e.key) || e.from == e.to {
                continue;
            }
            let (Some(from), Some(to)) =
                (sol.solved.nodes.get(&e.from), sol.solved.nodes.get(&e.to))
            else {
                continue;
            };
            assert!(
                from.y <= to.y + 1.0,
                "{name}: edge {} -> {} not rank-monotone: {:?} -> {:?}",
                e.from,
                e.to,
                from,
                to
            );
        }
    }
}

/// A self-transition must not flatten the ranks of everything downstream of it:
/// it is excluded from the ranking adjacency entirely, so `Done` (three
/// transitions past `Start`) sits strictly below `Start`.
#[test]
fn self_transition_does_not_flatten_downstream_ranks() {
    let sol = solve("state-machine", FlowFlavor::StateMachine);
    let (_, nodes, _) = load("state-machine");
    let y_of = |id: &str| {
        let node = nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("fixture has no node {id}"));
        sol.solved.nodes[&node.key].y
    };
    assert!(y_of("Start") < y_of("Idle"), "Start must rank above Idle");
    assert!(y_of("Idle") < y_of("Active"), "Idle must rank above Active");
    assert!(y_of("Active") < y_of("Done"), "Active must rank above Done");
}

#[test]
fn nodes_lie_inside_their_partition_band() {
    let sol = solve("activity", FlowFlavor::Activity);
    let (_, nodes, _) = load("activity");
    for n in &nodes {
        let Some(partition) = &n.partition else {
            continue;
        };
        let Some(rect) = sol.solved.nodes.get(&n.key) else {
            continue;
        };
        let group = sol
            .solved
            .groups
            .iter()
            .find(|g| g.title.as_deref() == Some(partition.as_str()))
            .unwrap_or_else(|| panic!("no lane group for partition {partition}"));
        assert!(
            rect.x >= group.rect.x - 0.5
                && rect.x + rect.w <= group.rect.x + group.rect.w + 0.5
                && rect.y >= group.rect.y - 0.5
                && rect.y + rect.h <= group.rect.y + group.rect.h + 0.5,
            "node {} rect {:?} not inside lane {:?}",
            n.key,
            rect,
            group.rect
        );
    }
}

#[test]
fn no_overlapping_node_rects_within_a_rank() {
    let sol = solve("activity", FlowFlavor::Activity);
    let mut by_y: std::collections::BTreeMap<i64, Vec<(f64, f64)>> = Default::default();
    for rect in sol.solved.nodes.values() {
        by_y.entry(rect.y.round() as i64)
            .or_default()
            .push((rect.x, rect.x + rect.w));
    }
    for spans in by_y.values() {
        let mut spans = spans.clone();
        spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 <= pair[1].0 + 0.5,
                "overlapping rects in rank: {pair:?}"
            );
        }
    }
}

#[test]
fn solving_twice_is_byte_identical() {
    let a = solve("activity", FlowFlavor::Activity);
    let b = solve("activity", FlowFlavor::Activity);
    assert_eq!(pretty_flow(&a.solved), pretty_flow(&b.solved));
}

#[test]
fn decision_without_guards_diagnoses_but_still_solves() {
    let doc = FlowDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: vec!["synthetic#Start".into(), "synthetic#Choice".into()],
        edges: vec!["synthetic#e0".into()],
    };
    let nodes = vec![
        ActivityNode {
            key: "synthetic#Start".into(),
            id: "Start".into(),
            behavior: "synthetic".into(),
            kind: FlowNodeKind::Initial,
            object_ref: None,
            partition: None,
            entry: None,
            do_: None,
            exit: None,
            refines: None,
            notes: vec![],
        },
        ActivityNode {
            key: "synthetic#Choice".into(),
            id: "Choice".into(),
            behavior: "synthetic".into(),
            kind: FlowNodeKind::Decision,
            object_ref: None,
            partition: None,
            entry: None,
            do_: None,
            exit: None,
            refines: None,
            notes: vec![],
        },
    ];
    let edges = vec![FlowEdge {
        key: "synthetic#e0".into(),
        kind: waml::model::FlowEdgeKind::ControlFlow,
        behavior: "synthetic".into(),
        from: "synthetic#Start".into(),
        to: "synthetic#Choice".into(),
        to_ref: None,
        trigger: None,
        guard: None,
        is_else: false,
        effect: None,
        carries: None,
        traces: Vec::new(),
    }];
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    assert!(!sol.diagnostics.is_empty());
    assert!(!sol.solved.nodes.is_empty());
}

/// Regression: a flow with NO `initial` node and two disconnected roots must not
/// report the second component as unreachable. The reachability BFS once seeded
/// itself with `Initial` nodes only (falling back to the FIRST declared node),
/// while `break_cycles` also fell back to every in-degree-0 node — so `C -> D`
/// below produced "node 'C' is unreachable" / "node 'D' is unreachable", which
/// reached the status bar verbatim and inflated the empty-state count. Both now
/// share `traversal_starts`.
#[test]
fn multi_root_flow_without_initial_reports_nothing_unreachable() {
    fn plain(id: &str) -> ActivityNode {
        ActivityNode {
            key: format!("synthetic#{id}"),
            id: id.into(),
            behavior: "synthetic".into(),
            kind: FlowNodeKind::Plain,
            object_ref: None,
            partition: None,
            entry: None,
            do_: None,
            exit: None,
            refines: None,
            notes: vec![],
        }
    }
    fn control(key: &str, from: &str, to: &str) -> FlowEdge {
        FlowEdge {
            key: format!("synthetic#{key}"),
            kind: waml::model::FlowEdgeKind::ControlFlow,
            behavior: "synthetic".into(),
            from: format!("synthetic#{from}"),
            to: format!("synthetic#{to}"),
            to_ref: None,
            trigger: None,
            guard: None,
            is_else: false,
            effect: None,
            carries: None,
            traces: Vec::new(),
        }
    }

    // Two independent chains, no Initial anywhere: A -> B and C -> D.
    let doc = FlowDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: ["A", "B", "C", "D"]
            .iter()
            .map(|id| format!("synthetic#{id}"))
            .collect(),
        edges: vec!["synthetic#e0".into(), "synthetic#e1".into()],
    };
    let nodes = vec![plain("A"), plain("B"), plain("C"), plain("D")];
    let edges = vec![control("e0", "A", "B"), control("e1", "C", "D")];

    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);

    let unreachable: Vec<&str> = sol
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .filter(|m| m.contains("unreachable"))
        .collect();
    assert!(
        unreachable.is_empty(),
        "multi-root flow wrongly reported unreachable nodes: {unreachable:?}"
    );
    // All four nodes still laid out.
    assert_eq!(sol.solved.nodes.len(), 4);
}

#[test]
fn empty_flow_doc_diagnoses_and_returns_empty_solved() {
    let doc = FlowDoc {
        key: "empty".into(),
        title: "Empty".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: vec![],
        edges: vec![],
    };
    let (rf, _) = resolve_flow(&doc, &[], &[]);
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &[], &[], &sizes, &cfg, &|_| None);
    assert!(!sol.diagnostics.is_empty());
    assert!(sol.solved.nodes.is_empty());
}

const EXPECTED_STATE_MACHINE_GOLDEN: &str =
    include_str!("fixtures/behavior/state-machine/states.golden.txt");

#[test]
fn state_machine_fixture_layout_golden() {
    let sol = solve("state-machine", FlowFlavor::StateMachine);
    assert!(sol.diagnostics.is_empty(), "{:?}", sol.diagnostics);
    assert_eq!(pretty_flow(&sol.solved), EXPECTED_STATE_MACHINE_GOLDEN);
}

#[test]
fn every_route_endpoint_lies_on_its_node_border() {
    for (name, flavor) in [
        ("activity", FlowFlavor::Activity),
        ("state-machine", FlowFlavor::StateMachine),
    ] {
        let sol = solve(name, flavor);
        for route in &sol.solved.routes {
            let src_rect = sol.solved.nodes.get(&route.source).copied();
            let tgt_rect = sol.solved.nodes.get(&route.target).copied();
            if let Some(src_rect) = src_rect {
                let first = *route.points.first().unwrap();
                assert!(
                    point_on_border(first, src_rect),
                    "{name}: route {} -> {} start {:?} not on source border {:?}",
                    route.source,
                    route.target,
                    first,
                    src_rect
                );
            }
            if let Some(tgt_rect) = tgt_rect {
                let last = *route.points.last().unwrap();
                assert!(
                    point_on_border(last, tgt_rect),
                    "{name}: route {} -> {} end {:?} not on target border {:?}",
                    route.source,
                    route.target,
                    last,
                    tgt_rect
                );
            }
        }
    }
}

#[test]
fn self_transition_routes_out_and_back() {
    let sol = solve("state-machine", FlowFlavor::StateMachine);
    let (_, _, edges) = load("state-machine");
    let self_edge = edges
        .iter()
        .find(|e| e.from == e.to)
        .expect("fixture has a self-transition");
    let route = sol
        .solved
        .routes
        .iter()
        .find(|r| r.source == self_edge.from && r.target == self_edge.to)
        .expect("self-edge route exists");
    assert!(route.points.len() >= 4, "{:?}", route.points);
    let rect = sol.solved.nodes[&self_edge.from];
    for (i, p) in route.points.iter().enumerate() {
        let is_endpoint = i == 0 || i == route.points.len() - 1;
        if !is_endpoint {
            assert!(
                p.0 >= rect.x + rect.w - 0.5,
                "interior point {p:?} should be outside node interior {rect:?}"
            );
        }
    }
}

#[test]
fn loop_back_edge_routes_outside_the_rank_stack() {
    let sol = solve("activity", FlowFlavor::Activity);
    let max_right = sol
        .solved
        .nodes
        .values()
        .map(|r| r.x + r.w)
        .fold(0.0_f64, f64::max);
    let min_left = sol
        .solved
        .nodes
        .values()
        .map(|r| r.x)
        .fold(f64::INFINITY, f64::min);
    let has_outside_route = sol.solved.routes.iter().any(|r| {
        sol.reversed.iter().any(|k| {
            let (_, _, edges) = load("activity");
            edges
                .iter()
                .find(|e| &e.key == k)
                .is_some_and(|e| e.from == r.source && e.to == r.target)
        }) && r
            .points
            .iter()
            .any(|p| p.0 > max_right + 0.5 || p.0 < min_left - 0.5)
    });
    assert!(
        has_outside_route,
        "expected at least one reversed-edge route to leave the rank-stack column: {:?}",
        sol.solved.routes
    );
}

#[test]
fn unknown_target_without_to_ref_drops_with_diagnostic() {
    let doc = FlowDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: vec!["synthetic#Start".into()],
        edges: vec!["synthetic#e0".into()],
    };
    let nodes = vec![ActivityNode {
        key: "synthetic#Start".into(),
        id: "Start".into(),
        behavior: "synthetic".into(),
        kind: FlowNodeKind::Initial,
        object_ref: None,
        partition: None,
        entry: None,
        do_: None,
        exit: None,
        refines: None,
        notes: vec![],
    }];
    let edges = vec![FlowEdge {
        key: "synthetic#e0".into(),
        kind: waml::model::FlowEdgeKind::ControlFlow,
        behavior: "synthetic".into(),
        from: "synthetic#Start".into(),
        to: "Nowhere".into(),
        to_ref: None,
        trigger: None,
        guard: None,
        is_else: false,
        effect: None,
        carries: None,
        traces: Vec::new(),
    }];
    let (rf, diags) = resolve_flow(&doc, &nodes, &edges);
    assert!(rf.edges.is_empty());
    assert!(rf.off_page.is_empty());
    assert!(!diags.is_empty());
}

#[test]
fn cross_document_edge_becomes_off_page_stub() {
    let doc = FlowDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: vec!["synthetic#Start".into()],
        edges: vec!["synthetic#e0".into()],
    };
    let nodes = vec![ActivityNode {
        key: "synthetic#Start".into(),
        id: "Start".into(),
        behavior: "synthetic".into(),
        kind: FlowNodeKind::Initial,
        object_ref: None,
        partition: None,
        entry: None,
        do_: None,
        exit: None,
        refines: None,
        notes: vec![],
    }];
    let edges = vec![FlowEdge {
        key: "synthetic#e0".into(),
        kind: waml::model::FlowEdgeKind::ControlFlow,
        behavior: "synthetic".into(),
        from: "synthetic#Start".into(),
        to: "Other Behavior".into(),
        to_ref: Some("other".into()),
        trigger: None,
        guard: None,
        is_else: false,
        effect: None,
        carries: None,
        traces: Vec::new(),
    }];
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    assert!(sol
        .solved
        .routes
        .iter()
        .all(|r| r.source != "synthetic#Start" || r.target != "Other Behavior"));
    assert_eq!(sol.off_page.len(), 1);
    assert_eq!(sol.off_page[0].edge_key, "synthetic#e0");
    assert_eq!(sol.off_page[0].target_title, "Other Behavior");
    assert!(sol.off_page[0].points.len() >= 2);
}

/// `to_ref` resolves the stub label to the TARGET DOCUMENT's title; the raw
/// `to` text is only the fallback (spec 2.1, 4.1).
#[test]
fn off_page_stub_label_resolves_the_target_document_title() {
    let doc = FlowDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: vec!["synthetic#Start".into()],
        edges: vec!["synthetic#e0".into()],
    };
    let nodes = vec![ActivityNode {
        key: "synthetic#Start".into(),
        id: "Start".into(),
        behavior: "synthetic".into(),
        kind: FlowNodeKind::Initial,
        object_ref: None,
        partition: None,
        entry: None,
        do_: None,
        exit: None,
        refines: None,
        notes: vec![],
    }];
    let edges = vec![FlowEdge {
        key: "synthetic#e0".into(),
        kind: waml::model::FlowEdgeKind::ControlFlow,
        behavior: "synthetic".into(),
        from: "synthetic#Start".into(),
        to: "other".into(),
        to_ref: Some("other".into()),
        trigger: None,
        guard: None,
        is_else: false,
        effect: None,
        carries: None,
        traces: Vec::new(),
    }];
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);

    let resolved = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|key| {
        (key == "other").then(|| "Fulfil Order".to_string())
    });
    assert_eq!(resolved.off_page[0].target_title, "Fulfil Order");

    let unresolved = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    assert_eq!(unresolved.off_page[0].target_title, "other");
}

fn chain(len: usize, partition: Option<&str>) -> (FlowDoc, Vec<ActivityNode>, Vec<FlowEdge>) {
    let nodes: Vec<ActivityNode> = (0..len)
        .map(|i| ActivityNode {
            key: format!("chain#n{i}"),
            id: format!("n{i}"),
            behavior: "chain".into(),
            kind: if i == 0 {
                FlowNodeKind::Initial
            } else {
                FlowNodeKind::Plain
            },
            object_ref: None,
            partition: partition.map(str::to_string),
            entry: None,
            do_: None,
            exit: None,
            refines: None,
            notes: vec![],
        })
        .collect();
    let edges: Vec<FlowEdge> = (1..len)
        .map(|i| FlowEdge {
            key: format!("chain#e{i}"),
            kind: waml::model::FlowEdgeKind::ControlFlow,
            behavior: "chain".into(),
            from: format!("chain#n{}", i - 1),
            to: format!("chain#n{i}"),
            to_ref: None,
            trigger: None,
            guard: None,
            is_else: false,
            effect: None,
            carries: None,
            traces: Vec::new(),
        })
        .collect();
    let doc = FlowDoc {
        key: "chain".into(),
        title: "Chain".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: nodes.iter().map(|n| n.key.clone()).collect(),
        edges: edges.iter().map(|e| e.key.clone()).collect(),
    };
    (doc, nodes, edges)
}

/// A long authored chain must not overflow the stack: every graph walk in the
/// flow solver is iterative. Solved on a deliberately SMALL stack, so a
/// per-node recursive frame would blow it long before the chain ends.
#[test]
fn a_long_node_chain_solves_on_a_small_stack() {
    const CHAIN: usize = 400;
    let solved = std::thread::Builder::new()
        .stack_size(96 * 1024)
        .spawn(|| {
            let (doc, nodes, edges) = chain(CHAIN, None);
            let cfg = FlowConfig::default();
            let (rf, _) = resolve_flow(&doc, &nodes, &edges);
            let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
            let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
            sol.solved.nodes.len()
        })
        .unwrap()
        .join()
        .expect("a long chain must not overflow the stack");
    assert_eq!(solved, CHAIN);
}

/// One named partition still draws its lane band (and clamps its nodes into
/// it) -- the band is not conditional on there being a SECOND lane.
#[test]
fn a_single_named_partition_still_emits_its_lane_band() {
    let (doc, nodes, edges) = chain(3, Some("Sales"));
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    let lane = sol
        .solved
        .groups
        .iter()
        .find(|g| g.title.as_deref() == Some("Sales"))
        .expect("single named partition must still get a lane band");
    for rect in sol.solved.nodes.values() {
        assert!(
            rect.x >= lane.rect.x - 0.5 && rect.x + rect.w <= lane.rect.x + lane.rect.w + 0.5,
            "node {rect:?} outside lane {:?}",
            lane.rect
        );
    }
}

fn plain(key: &str, id: &str, kind: FlowNodeKind) -> ActivityNode {
    ActivityNode {
        key: key.into(),
        id: id.into(),
        behavior: "fan".into(),
        kind,
        object_ref: None,
        partition: None,
        entry: None,
        do_: None,
        exit: None,
        refines: None,
        notes: vec![],
    }
}

/// Spec 2.4 step 4: a rank is centred under its parents rather than packed hard
/// left. A single `Initial` with two children must sit over their midpoint.
#[test]
fn a_rank_is_centred_under_its_parents() {
    let nodes = vec![
        plain("fan#Start", "Start", FlowNodeKind::Initial),
        plain("fan#Left", "Left", FlowNodeKind::Plain),
        plain("fan#Right", "Right", FlowNodeKind::Plain),
    ];
    let edges: Vec<FlowEdge> = ["Left", "Right"]
        .iter()
        .enumerate()
        .map(|(i, target)| FlowEdge {
            key: format!("fan#e{i}"),
            kind: waml::model::FlowEdgeKind::ControlFlow,
            behavior: "fan".into(),
            from: "fan#Start".into(),
            to: format!("fan#{target}"),
            to_ref: None,
            trigger: None,
            guard: None,
            is_else: false,
            effect: None,
            carries: None,
            traces: Vec::new(),
        })
        .collect();
    let doc = FlowDoc {
        key: "fan".into(),
        title: "Fan".into(),
        flavor: FlowFlavor::Activity,
        describes: None,
        nodes: nodes.iter().map(|n| n.key.clone()).collect(),
        edges: edges.iter().map(|e| e.key.clone()).collect(),
    };
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    let center = |key: &str| {
        let r = sol.solved.nodes[key];
        r.x + r.w / 2.0
    };
    let children_mid = (center("fan#Left") + center("fan#Right")) / 2.0;
    assert!(
        (center("fan#Start") - children_mid).abs() < 1.0,
        "Start centre {} not over its children's midpoint {children_mid}",
        center("fan#Start")
    );
}

/// Two transitions between the SAME pair of nodes each get their own route,
/// tagged with their own edge key, so a consumer can label and hit-test both.
#[test]
fn parallel_edges_between_one_pair_each_carry_their_own_route_key() {
    let nodes = vec![
        plain("fan#A", "A", FlowNodeKind::Initial),
        plain("fan#B", "B", FlowNodeKind::Plain),
    ];
    let edges: Vec<FlowEdge> = ["press", "timeout"]
        .iter()
        .enumerate()
        .map(|(i, trigger)| FlowEdge {
            key: format!("fan#e{i}"),
            kind: waml::model::FlowEdgeKind::ControlFlow,
            behavior: "fan".into(),
            from: "fan#A".into(),
            to: "fan#B".into(),
            to_ref: None,
            trigger: Some((*trigger).to_string()),
            guard: None,
            is_else: false,
            effect: None,
            carries: None,
            traces: Vec::new(),
        })
        .collect();
    let doc = FlowDoc {
        key: "fan".into(),
        title: "Fan".into(),
        flavor: FlowFlavor::StateMachine,
        describes: None,
        nodes: nodes.iter().map(|n| n.key.clone()).collect(),
        edges: edges.iter().map(|e| e.key.clone()).collect(),
    };
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::StateMachine, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg, &|_| None);
    let keys: Vec<String> = sol
        .solved
        .routes
        .iter()
        .filter_map(|r| r.key.clone())
        .collect();
    assert_eq!(keys, vec!["fan#e0".to_string(), "fan#e1".to_string()]);
}

/// A state's box must be wide enough for the `entry:`/`do:`/`exit:` behavior
/// lines the renderer draws inside it, not merely for its title.
#[test]
fn a_state_box_fits_its_entry_do_exit_lines() {
    use waml::solve::sizing::{self, Font};

    let (doc, nodes, edges) = load("state-machine");
    let cfg = FlowConfig::default();
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let sizes = measure_flow(&rf.nodes, FlowFlavor::StateMachine, &cfg);
    let mut checked = 0;
    for node in &nodes {
        let Some(size) = sizes.get(&node.key) else {
            continue;
        };
        for (keyword, body) in [
            ("entry", &node.entry),
            ("do", &node.do_),
            ("exit", &node.exit),
        ] {
            let Some(body) = body else { continue };
            let line = format!("{keyword}: {body}");
            let width = sizing::text_width(&line, cfg.font_size, Font::Sans);
            assert!(
                size.w >= width,
                "state {} box {:.1}px cannot hold {line:?} ({width:.1}px)",
                node.id,
                size.w
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "fixture has no behavior lines to check");
}
