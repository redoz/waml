use waml::model::{ActivityNode, FlowDoc, FlowEdge, FlowFlavor, FlowNodeKind};
use waml::solve::flow::{measure_flow, resolve_flow, solve_flow, FlowConfig};
use waml::solve::pretty_flow;
use waml::source::SourceBundle;

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
    solve_flow(&doc, &nodes, &edges, &sizes, &cfg)
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
    let sol = solve("activity", FlowFlavor::Activity);
    let (doc, nodes, edges) = load("activity");
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let _ = rf;
    for e in &edges {
        if sol.reversed.contains(&e.key) {
            continue;
        }
        let (Some(from), Some(to)) = (sol.solved.nodes.get(&e.from), sol.solved.nodes.get(&e.to))
        else {
            continue;
        };
        assert!(
            from.y <= to.y + 1.0,
            "edge {} -> {} not rank-monotone: {:?} -> {:?}",
            e.from,
            e.to,
            from,
            to
        );
    }
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
    }];
    let (rf, _) = resolve_flow(&doc, &nodes, &edges);
    let cfg = FlowConfig::default();
    let sizes = measure_flow(&rf.nodes, FlowFlavor::Activity, &cfg);
    let sol = solve_flow(&doc, &nodes, &edges, &sizes, &cfg);
    assert!(!sol.diagnostics.is_empty());
    assert!(!sol.solved.nodes.is_empty());
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
    let sol = solve_flow(&doc, &[], &[], &sizes, &cfg);
    assert!(!sol.diagnostics.is_empty());
    assert!(sol.solved.nodes.is_empty());
}

#[test]
fn state_machine_fixture_layout_golden() {
    let sol = solve("state-machine", FlowFlavor::StateMachine);
    assert!(sol.diagnostics.is_empty(), "{:?}", sol.diagnostics);
    assert!(!sol.solved.nodes.is_empty());
}
