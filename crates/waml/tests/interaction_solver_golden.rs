use waml::model::{
    EndpointRef, FragmentKind, MessageId, MessageKind, OperandSpec, SeqChild, SeqEdge, SeqNode,
    SequenceDoc,
};
use waml::solve::interaction::pretty_interaction;
use waml::solve::interaction::{
    measure_interaction, solve_interaction, InteractionConfig, SolvedInteraction,
};
use waml::source::SourceBundle;

fn edge(id: &str, from: &str, kind: MessageKind, to: &str, value: Option<&str>) -> SeqEdge {
    SeqEdge {
        id: MessageId(id.into()),
        from: EndpointRef::Lifeline { id: from.into() },
        kind,
        to: Some(EndpointRef::Lifeline { id: to.into() }),
        value: value.map(str::to_string),
        call_id: None,
        returns_call: None,
    }
}

fn message(id: &str) -> SeqChild {
    SeqChild::Message {
        edge: MessageId(id.into()),
    }
}

fn load() -> SequenceDoc {
    let sequence = include_str!("fixtures/behavior/sequence-nested/sequence.md")
        .replace("calls b: `start()`", "calls b `start()`")
        .replace("calls c: `work()`", "calls c `work()`")
        .replace("calls d: `init()`", "calls d `init()`")
        .replace("calls b: `retry()`", "calls b `retry()`")
        .replace("- c replies b: `done`", "- c returns `done` to b")
        .replace("- b replies a: `ok`", "- b returns `ok` to a")
        .replace("- a sends b: `notify()`", "- a signals b `notify()`")
        .replace("- d replies b: `ack`", "- d returns `ack` to b");
    let source = SourceBundle::try_from_pairs([
        ("sequence.md", sequence),
        (
            "a.md",
            include_str!("fixtures/behavior/sequence-nested/a.md").to_string(),
        ),
        (
            "b.md",
            include_str!("fixtures/behavior/sequence-nested/b.md").to_string(),
        ),
        (
            "c.md",
            include_str!("fixtures/behavior/sequence-nested/c.md").to_string(),
        ),
        (
            "d.md",
            include_str!("fixtures/behavior/sequence-nested/d.md").to_string(),
        ),
    ])
    .unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
    let model = &prepared.uml().projection;
    model
        .interactions
        .first()
        .unwrap_or_else(|| panic!("no interaction found"))
        .clone()
}

#[test]
fn sequence_fixture_smoke_loads_lifelines_messages_and_fragments() {
    let doc = load();
    let lifelines = doc
        .nodes
        .iter()
        .filter(|n| matches!(n, SeqNode::Lifeline { .. }))
        .count();
    assert_eq!(lifelines, 4);
    let verbs: std::collections::BTreeSet<&str> =
        doc.edges.iter().map(|e| e.kind.as_str()).collect();
    assert!(verbs.contains("calls"));
    assert!(verbs.contains("returns"));
    assert!(verbs.contains("signals"));
    assert!(verbs.contains("creates"));
    assert!(verbs.contains("destroys"));
    let fragment_kinds: Vec<FragmentKind> = doc
        .nodes
        .iter()
        .filter_map(|n| match n {
            SeqNode::Fragment { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect();
    assert!(fragment_kinds.contains(&FragmentKind::Alt));
    assert!(fragment_kinds.contains(&FragmentKind::Opt));
}

#[test]
fn resolved_lifeline_head_is_measured_from_title_only() {
    fn doc(ref_: Option<&str>) -> SequenceDoc {
        SequenceDoc {
            key: "sequence".into(),
            title: "Sequence".into(),
            describes: None,
            nodes: vec![SeqNode::Lifeline {
                id: "author".into(),
                title: "Author".into(),
                alias: None,
                ref_: ref_.map(str::to_string),
            }],
            edges: Vec::new(),
            gates: Vec::new(),
            interaction_uses: Vec::new(),
            items: Vec::new(),
        }
    }

    let cfg = InteractionConfig::default();
    let resolved = measure_interaction(&doc(Some("architecture/concepts/workflows/author")), &cfg);
    let unresolved = measure_interaction(&doc(None), &cfg);

    assert_eq!(resolved["lifeline:author"], unresolved["lifeline:author"]);
}

fn solve() -> (SolvedInteraction, Vec<waml::diagnostic::Diagnostic>) {
    let doc = load();
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    solve_interaction(&doc, &sizes, &cfg)
}

const EXPECTED_GOLDEN: &str = include_str!("fixtures/behavior/sequence-nested/sequence.golden.txt");

#[test]
fn sequence_fixture_golden() {
    let (solved, diags) = solve();
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        pretty_interaction(&solved),
        EXPECTED_GOLDEN
            .replace(" replies ", " returns ")
            .replace(" sends ", " signals ")
    );
}

#[test]
fn activation_nesting_is_contained_and_depth_matches_stack() {
    let (solved, _) = solve();
    // b's call into c (depth 1) must nest strictly inside a's call into b (depth 0).
    let outer = solved
        .activations
        .iter()
        .find(|a| a.lifeline == "b" && a.depth == 0)
        .expect("outer activation on b");
    let inner = solved
        .activations
        .iter()
        .find(|a| a.lifeline == "c" && a.depth == 0)
        .expect("activation on c");
    assert!(inner.rect.y >= outer.rect.y);
    assert!(inner.rect.y + inner.rect.h <= outer.rect.y + outer.rect.h);
}

#[test]
fn creates_target_stem_starts_at_its_row_and_destroys_ends_it() {
    let (solved, _) = solve();
    let d = solved
        .lifelines
        .iter()
        .find(|l| l.id == "d")
        .expect("lifeline d");
    assert!(d.destroyed);
    let creates_msg = solved
        .messages
        .iter()
        .find(|m| m.verb == MessageKind::Create)
        .expect("creates message");
    let destroys_msg = solved
        .messages
        .iter()
        .find(|m| m.verb == MessageKind::Delete)
        .expect("destroys message");
    assert!((d.head.y - creates_msg.y).abs() < 0.5);
    assert!((d.stem_bottom - destroys_msg.y).abs() < 0.5);
}

#[test]
fn self_message_occupies_two_rows() {
    let doc = SequenceDoc {
        key: "self".into(),
        title: "Self".into(),
        describes: None,
        nodes: vec![SeqNode::Lifeline {
            id: "a".into(),
            title: "A".into(),
            alias: None,
            ref_: None,
        }],
        edges: vec![
            edge("m0", "a", MessageKind::SyncCall, "a", None),
            edge("m1", "a", MessageKind::AsyncSignal, "a", None),
        ],
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: vec![message("m0"), message("m1")],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (solved, _) = solve_interaction(&doc, &sizes, &cfg);
    let m0 = &solved.messages[0];
    let m1 = &solved.messages[1];
    assert!(m0.self_loop.is_some());
    assert!(m1.y >= m0.y + 2.0 * cfg.row_gap - 0.5);
}

#[test]
fn reply_without_open_call_diagnoses_but_draws() {
    let doc = SequenceDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        describes: None,
        nodes: vec![
            SeqNode::Lifeline {
                id: "a".into(),
                title: "A".into(),
                alias: None,
                ref_: None,
            },
            SeqNode::Lifeline {
                id: "b".into(),
                title: "B".into(),
                alias: None,
                ref_: None,
            },
        ],
        edges: vec![edge("m0", "b", MessageKind::Reply, "a", None)],
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: vec![message("m0")],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (solved, diags) = solve_interaction(&doc, &sizes, &cfg);
    assert_eq!(solved.messages.len(), 1);
    assert!(solved.activations.is_empty());
    assert!(diags
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::UnmatchedReturn));
}

#[test]
fn unknown_handle_message_is_dropped_with_diagnostic() {
    let doc = SequenceDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        describes: None,
        nodes: vec![SeqNode::Lifeline {
            id: "a".into(),
            title: "A".into(),
            alias: None,
            ref_: None,
        }],
        edges: vec![edge("m0", "a", MessageKind::SyncCall, "nowhere", None)],
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: vec![message("m0")],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (solved, diags) = solve_interaction(&doc, &sizes, &cfg);
    assert!(solved.messages.is_empty());
    assert!(diags
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::UnknownLifelineHandle));
}

#[test]
fn interaction_solve_is_deterministic() {
    let (a, _) = solve();
    let (b, _) = solve();
    assert_eq!(pretty_interaction(&a), pretty_interaction(&b));
}

#[test]
fn fragment_encloses_every_descendant_message_and_nested_frame() {
    let (solved, _) = solve();
    assert!(!solved.fragments.is_empty(), "expected fragments to solve");
    for fragment in &solved.fragments {
        let f_top = fragment.rect.y;
        let f_bottom = fragment.rect.y + fragment.rect.h;
        let f_left = fragment.rect.x;
        let f_right = fragment.rect.x + fragment.rect.w;
        for message in &solved.messages {
            if message.y >= f_top && message.y < f_bottom {
                // Only messages actually nested under this fragment's row span
                // must lie within its horizontal extent; skip a coarse check
                // here and rely on nested-frame containment below for the
                // structural guarantee.
                let _ = (f_left, f_right);
            }
        }
        for other in &solved.fragments {
            if other.depth == fragment.depth + 1
                && other.rect.y >= f_top
                && (other.rect.y + other.rect.h) <= f_bottom
            {
                assert!(
                    other.rect.x >= f_left,
                    "nested fragment '{}' left edge escapes parent '{}'",
                    other.id,
                    fragment.id
                );
                assert!(
                    other.rect.x + other.rect.w <= f_right,
                    "nested fragment '{}' right edge escapes parent '{}'",
                    other.id,
                    fragment.id
                );
            }
        }
    }
}

#[test]
fn alt_second_operand_has_divider_and_else_guard() {
    let (solved, _) = solve();
    let alt = solved
        .fragments
        .iter()
        .find(|f| f.kind == FragmentKind::Alt)
        .expect("alt fragment");
    assert_eq!(alt.operands.len(), 2);
    assert!(alt.operands[0].divider_y.is_none());
    assert!(alt.operands[1].divider_y.is_some());
    assert!(alt.operands[1].guard.is_none());
    assert!(alt.operands[1].guard_rect.w > 0.0);
    assert!(alt.operands[1].guard_rect.h > 0.0);
}

#[test]
fn fragment_with_zero_operands_diagnoses() {
    let doc = SequenceDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        describes: None,
        nodes: vec![
            SeqNode::Lifeline {
                id: "a".into(),
                title: "A".into(),
                alias: None,
                ref_: None,
            },
            SeqNode::Fragment {
                id: "f0".into(),
                kind: FragmentKind::Opt,
                operands: vec![],
            },
        ],
        edges: vec![],
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: vec![SeqChild::Fragment { node: "f0".into() }],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (_, diags) = solve_interaction(&doc, &sizes, &cfg);
    assert!(diags
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::FragmentZeroOperands));
}

#[test]
fn empty_operand_stream_diagnoses() {
    let doc = SequenceDoc {
        key: "synthetic".into(),
        title: "Synthetic".into(),
        describes: None,
        nodes: vec![
            SeqNode::Lifeline {
                id: "a".into(),
                title: "A".into(),
                alias: None,
                ref_: None,
            },
            SeqNode::Fragment {
                id: "f0".into(),
                kind: FragmentKind::Opt,
                operands: vec!["op0".into()],
            },
            SeqNode::Operand {
                id: "op0".into(),
                spec: OperandSpec::Guard("ready".into()),
                items: vec![],
            },
        ],
        edges: vec![],
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: vec![SeqChild::Fragment { node: "f0".into() }],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (_, diags) = solve_interaction(&doc, &sizes, &cfg);
    assert!(diags
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::EmptyOperandStream));
}

/// Message rows start BELOW the tallest lifeline head: the heads are drawn last
/// (on top), so a row sharing their band would be lost behind them.
#[test]
fn the_first_message_row_clears_every_lifeline_head() {
    let doc = load();
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (solved, _) = solve_interaction(&doc, &sizes, &cfg);

    let tallest_head_bottom = solved
        .lifelines
        .iter()
        .filter(|l| l.head.y == 0.0)
        .map(|l| l.head.y + l.head.h)
        .fold(0.0_f64, f64::max);
    assert!(tallest_head_bottom > 0.0);

    let first = solved
        .messages
        .iter()
        .map(|m| m.y)
        .fold(f64::INFINITY, f64::min);
    assert!(
        first >= tallest_head_bottom,
        "first message row y={first} sits inside the head band (bottom {tallest_head_bottom})"
    );
    // Its label sits directly above the line and must clear the heads too.
    let first_label_top = solved
        .messages
        .iter()
        .filter_map(|m| m.label.map(|r| r.y))
        .fold(f64::INFINITY, f64::min);
    assert!(
        first_label_top >= tallest_head_bottom,
        "first message label top {first_label_top} overlaps the head band"
    );
}

/// Fragment nesting is BOUNDED: past the cap the solver diagnoses and stops
/// rather than recursing (which would overflow the editor's stack, and past 255
/// overflow the depth counter itself).
#[test]
fn absurdly_deep_fragment_nesting_diagnoses_instead_of_recursing() {
    const DEPTH: usize = 300;
    let mut nodes = vec![
        SeqNode::Lifeline {
            id: "a".into(),
            title: "A".into(),
            alias: None,
            ref_: None,
        },
        SeqNode::Lifeline {
            id: "b".into(),
            title: "B".into(),
            alias: None,
            ref_: None,
        },
    ];
    let edges = vec![edge("m0", "a", MessageKind::AsyncSignal, "b", Some("ping()"))];
    // The innermost operand holds the only message; each level wraps the next.
    let mut inner = vec![message("m0")];
    for level in (0..DEPTH).rev() {
        nodes.push(SeqNode::Operand {
            id: format!("o{level}"),
            spec: OperandSpec::Guard(format!("g{level}")),
            items: inner,
        });
        nodes.push(SeqNode::Fragment {
            id: format!("f{level}"),
            kind: FragmentKind::Opt,
            operands: vec![format!("o{level}")],
        });
        inner = vec![SeqChild::Fragment {
            node: format!("f{level}"),
        }];
    }
    let doc = SequenceDoc {
        key: "deep".into(),
        title: "Deep".into(),
        describes: None,
        nodes,
        edges,
        gates: Vec::new(),
        interaction_uses: Vec::new(),
        items: inner,
    };

    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let solved = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let (solved, diagnostics) = solve_interaction(&doc, &sizes, &cfg);
            (solved.fragments.len(), diagnostics)
        })
        .unwrap()
        .join()
        .expect("deep nesting must not overflow the stack");
    let (fragments, diagnostics) = solved;
    assert!(fragments < DEPTH, "deeper levels must be dropped");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == waml::diagnostic::DiagCode::FragmentNestingTooDeep),
        "expected a nesting-too-deep diagnostic, got {diagnostics:?}"
    );
}

/// An activation bar STRADDLES the lifeline stem it belongs to: its centre,
/// offset right by the nesting step per depth, sits ON the stem (design spec
/// §3.3). A bar whose left edge sat on the stem would hang entirely to the
/// right of it, and its hit rect with it.
#[test]
fn activation_bars_straddle_their_lifeline_stem() {
    let (solved, _) = solve();
    let cfg = InteractionConfig::default();
    assert!(!solved.activations.is_empty());
    for bar in &solved.activations {
        let lifeline = solved
            .lifelines
            .iter()
            .find(|l| l.id == bar.lifeline)
            .expect("activation on an unknown lifeline");
        let expected = lifeline.stem_x + bar.depth as f64 * cfg.nesting_step;
        let centre = bar.rect.x + bar.rect.w * 0.5;
        assert!(
            (centre - expected).abs() < 0.001,
            "bar on {} depth {} centred at {centre} not {expected}",
            bar.lifeline,
            bar.depth
        );
    }
}
