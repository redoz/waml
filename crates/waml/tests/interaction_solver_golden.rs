use waml::model::{FragmentKind, MessageVerb, SeqEdge, SeqNode, SequenceDoc};
use waml::solve::interaction::pretty_interaction;
use waml::solve::interaction::{
    measure_interaction, solve_interaction, InteractionConfig, SolvedInteraction,
};
use waml::source::SourceBundle;

fn load() -> SequenceDoc {
    let source = SourceBundle::try_from_pairs([
        (
            "sequence.md",
            include_str!("fixtures/behavior/sequence-nested/sequence.md"),
        ),
        (
            "a.md",
            include_str!("fixtures/behavior/sequence-nested/a.md"),
        ),
        (
            "b.md",
            include_str!("fixtures/behavior/sequence-nested/b.md"),
        ),
        (
            "c.md",
            include_str!("fixtures/behavior/sequence-nested/c.md"),
        ),
        (
            "d.md",
            include_str!("fixtures/behavior/sequence-nested/d.md"),
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
        doc.edges.iter().map(|e| e.verb.as_str()).collect();
    assert!(verbs.contains("calls"));
    assert!(verbs.contains("replies"));
    assert!(verbs.contains("sends"));
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
    assert_eq!(pretty_interaction(&solved), EXPECTED_GOLDEN);
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
        .find(|m| m.verb == MessageVerb::Creates)
        .expect("creates message");
    let destroys_msg = solved
        .messages
        .iter()
        .find(|m| m.verb == MessageVerb::Destroys)
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
            SeqEdge {
                id: "m0".into(),
                from: "a".into(),
                verb: MessageVerb::Calls,
                to: "a".into(),
                signature: None,
            },
            SeqEdge {
                id: "m1".into(),
                from: "a".into(),
                verb: MessageVerb::Sends,
                to: "a".into(),
                signature: None,
            },
        ],
        items: vec![
            waml::model::SeqChild::Message { edge: "m0".into() },
            waml::model::SeqChild::Message { edge: "m1".into() },
        ],
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
        edges: vec![SeqEdge {
            id: "m0".into(),
            from: "b".into(),
            verb: MessageVerb::Replies,
            to: "a".into(),
            signature: None,
        }],
        items: vec![waml::model::SeqChild::Message { edge: "m0".into() }],
    };
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(&doc, &cfg);
    let (solved, diags) = solve_interaction(&doc, &sizes, &cfg);
    assert_eq!(solved.messages.len(), 1);
    assert!(solved.activations.is_empty());
    assert!(diags
        .iter()
        .any(|d| d.code == waml::diagnostic::DiagCode::UnmatchedReply));
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
        edges: vec![SeqEdge {
            id: "m0".into(),
            from: "a".into(),
            verb: MessageVerb::Calls,
            to: "nowhere".into(),
            signature: None,
        }],
        items: vec![waml::model::SeqChild::Message { edge: "m0".into() }],
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
