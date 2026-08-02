use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    diagnostic::DiagCode,
    model::{EndpointRef, FragmentKind, InteractionUseId, MessageId, MessageKind, OperandSpec, SeqChild, SeqNode},
    source::SourceBundle,
    uml,
};

fn analyze(pairs: impl IntoIterator<Item = (&'static str, &'static str)>) -> uml::Analysis {
    let source = SourceBundle::try_from_pairs(pairs).unwrap();
    let okf = analyze_okf(&source, None, 1).unwrap();
    uml::analyze(
        DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 1,
        },
        None,
    )
    .unwrap()
}

#[test]
fn endpoint_kinds_resolve() {
    let analysis = analyze([
        (
            "a.md",
            "---\ntype: uml.Class\n---\n# A\n",
        ),
        (
            "b.md",
            "---\ntype: uml.Class\n---\n# B\n",
        ),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Gates\n- request\n",
        ),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Gates\n- frame\n\n## Messages\n- ref [Target](./target.md) as auth\n- a calls b `one()`\n- outside signals b `two`\n- @frame signals b `three`\n- auth@request signals b `four`\n- a calls b async `five()`\n- a returns `six` to b\n- a creates b: `B`\n- a destroys b\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "s")
        .unwrap();

    assert_eq!(doc.edges[0].from, EndpointRef::Lifeline { id: "a".into() });
    assert_eq!(doc.edges[1].from, EndpointRef::Outside);
    assert_eq!(
        doc.edges[2].from,
        EndpointRef::LocalGate { gate: "frame".into() }
    );
    assert_eq!(
        doc.edges[3].from,
        EndpointRef::UseGate {
            interaction_use: InteractionUseId("u0".into()),
            gate: "request".into(),
        }
    );
    assert_eq!(
        doc.edges.iter().map(|edge| edge.kind).collect::<Vec<_>>(),
        [
            MessageKind::SyncCall,
            MessageKind::AsyncSignal,
            MessageKind::AsyncSignal,
            MessageKind::AsyncSignal,
            MessageKind::AsyncCall,
            MessageKind::Reply,
            MessageKind::Create,
            MessageKind::Delete,
        ]
    );
}

#[test]
fn returns_follow_the_locked_candidate_algorithm() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `first()` as first\n- b returns `ok` to a for first\n- a calls b async `second()` as second\n- b returns `ok` for second\n- a calls b `third()`\n- a calls b `fourth()`\n- b returns `ambiguous`\n- b returns `unknown` for missing\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "s")
        .unwrap();

    assert_eq!(doc.edges[1].returns_call, Some(MessageId("m0".into())));
    assert_eq!(doc.edges[3].returns_call, Some(MessageId("m2".into())));
    assert_eq!(doc.edges[6].returns_call, None);
    assert_eq!(doc.edges[7].returns_call, None);
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::AmbiguousReturn));
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::UnknownCallIdentity));
}

#[test]
fn fragment_operand_rules_are_exact() {
    let valid = "---\ntype: uml.Sequence\n---\n# Valid\n\n## Messages\n- alt\n  - when `a`\n  - else\n- opt\n  - when `a`\n- loop\n  - when `a`\n- break\n  - when `a`\n- par\n  - branch `a`\n  - branch\n- critical\n  - branch\n- assert\n  - branch\n- neg\n  - branch\n";
    let invalid = "---\ntype: uml.Sequence\n---\n# Invalid\n\n## Messages\n- alt\n  - else\n  - when `late`\n  - else\n- opt\n  - else\n- loop\n  - branch\n- break\n  - when `a`\n  - when `b`\n- par\n  - branch\n- critical\n  - branch\n  - branch\n- assert\n  - when `a`\n- neg\n  - else\n";
    let analysis = analyze([("valid.md", valid), ("invalid.md", invalid)]);
    let invalid_count = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidFragmentOperands)
        .count();
    assert_eq!(invalid_count, 8);
    let valid_doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "valid")
        .unwrap();
    assert_eq!(
        valid_doc
            .nodes
            .iter()
            .filter_map(|node| match node {
                SeqNode::Fragment { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            FragmentKind::Alt,
            FragmentKind::Opt,
            FragmentKind::Loop,
            FragmentKind::Break,
            FragmentKind::Par,
            FragmentKind::Critical,
            FragmentKind::Assert,
            FragmentKind::Neg,
        ]
    );
}

#[test]
fn nested_fragments_keep_order_and_branch_boundaries() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "nested.md",
            "---\ntype: uml.Sequence\n---\n# Nested\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- alt\n  - when `ready`\n    - a calls b `outer()`\n    - par\n      - branch `left`\n        - a signals b `left`\n      - branch `right`\n        - b signals a `right`\n  - else\n    - b returns `fallback` to a\n- a signals b `after`\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "nested")
        .unwrap();
    assert!(matches!(
        doc.items.as_slice(),
        [SeqChild::Fragment { node }, SeqChild::Message { edge }]
            if node == "f0" && edge.0 == "m4"
    ));
    let first_alt_operand = doc.nodes.iter().find_map(|node| match node {
        SeqNode::Operand { id, spec: OperandSpec::Guard(_), items } if id == "f0.o0" => {
            Some(items)
        }
        _ => None,
    }).unwrap();
    assert!(matches!(
        first_alt_operand.as_slice(),
        [SeqChild::Message { edge }, SeqChild::Fragment { node }]
            if edge.0 == "m0" && node == "f1"
    ));
    for (id, expected) in [("f1.o0", "m1"), ("f1.o1", "m2")] {
        let items = doc.nodes.iter().find_map(|node| match node {
            SeqNode::Operand { id: operand_id, items, .. } if operand_id == id => Some(items),
            _ => None,
        }).unwrap();
        assert!(matches!(items.as_slice(), [SeqChild::Message { edge }] if edge.0 == expected));
    }
}

#[test]
fn parallel_branches_do_not_infer_returns_from_siblings() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "branches.md",
            "---\ntype: uml.Sequence\n---\n# Branches\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- par\n  - branch `call`\n    - a calls b `work()` as work\n  - branch `return`\n    - b returns `wrong sibling` to a\n- b returns `after join` for work\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "branches")
        .unwrap();
    assert_eq!(doc.edges[1].returns_call, None);
    assert_eq!(doc.edges[2].returns_call, Some(MessageId("m0".into())));
}

#[test]
fn conditional_join_keeps_calls_that_can_remain_open() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "conditional.md",
            "---\ntype: uml.Sequence\n---\n# Conditional\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `work()` as work\n- opt\n  - when `early`\n    - b returns `early` for work\n- b returns `later` for work\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "conditional")
        .unwrap();
    assert_eq!(doc.edges[1].returns_call, Some(MessageId("m0".into())));
    assert_eq!(doc.edges[2].returns_call, Some(MessageId("m0".into())));
}
