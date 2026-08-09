use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode};

fn analyze(pairs: impl IntoIterator<Item = (&'static str, &'static str)>) -> uml::Analysis {
    let source = SourceBundle::try_from_pairs(pairs).unwrap();
    let okf = analyze_okf(&source, None, 9).unwrap();
    uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 9,
        },
        None,
    )
    .unwrap()
}

fn count<T: AstNode<uml::syntax::UmlLanguage>>(
    node: SyntaxNode<uml::syntax::UmlLanguage>,
) -> usize {
    usize::from(T::cast(node.clone()).is_some())
        + node
            .children()
            .filter_map(SyntaxElement::into_node)
            .map(count::<T>)
            .sum::<usize>()
}

fn typed<T: AstNode<uml::syntax::UmlLanguage>>(
    node: SyntaxNode<uml::syntax::UmlLanguage>,
) -> Vec<T> {
    let mut result = T::cast(node.clone()).into_iter().collect::<Vec<_>>();
    for child in node.children().filter_map(SyntaxElement::into_node) {
        result.extend(typed::<T>(child));
    }
    result
}

fn root(analysis: &uml::Analysis, path: &str) -> SyntaxNode<uml::syntax::UmlLanguage> {
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse(path).unwrap())
        .unwrap();
    analysis.syntax.document(id).unwrap().syntax().root()
}

fn written(analysis: &uml::Analysis, path: &str) -> String {
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse(path).unwrap())
        .unwrap();
    analysis
        .syntax
        .document(id)
        .unwrap()
        .syntax()
        .write_to_string()
}

#[test]
fn flow_fixed_slots_project_every_current_node_and_transition_form_losslessly() {
    let authored = "---\r\ntype: uml.ActivityDiagram\r\ntitle: Café flow\r\n---\r\n# Café flow\r\n\r\n## Nodes\r\n### initial Start\r\n- transitions to Work\r\n### Work\r\n- entry: `begin`\r\n- do: `serve ☕`\r\n- exit: `finish`\r\n- on `paid` when `ok` transitions to Choice carries [Order](./order.md): `record`\r\n- refines [Child](./child.md)\r\n- partition: Kitchen\r\n#### Notes\r\n- naïve note\r\n### decision Choice\r\n- else transitions to Done\r\n### final Done\r\n";
    let analysis = analyze([
        ("flow.md", authored),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ("child.md", "---\ntype: uml.ActivityDiagram\n---\n# Child\n"),
    ]);
    let syntax = root(&analysis, "flow.md");
    assert_eq!(count::<uml::FlowNodeSyntax>(syntax.clone()), 4);
    assert_eq!(count::<uml::FlowTransitionSyntax>(syntax.clone()), 3);
    assert_eq!(count::<uml::FlowBlockSyntax>(syntax.clone()), 1);
    assert_eq!(written(&analysis, "flow.md"), authored);
    let flow = analysis
        .projection
        .flows
        .iter()
        .find(|f| f.key == "flow")
        .unwrap();
    assert_eq!(flow.nodes.len(), 4);
    assert_eq!(flow.edges.len(), 3);
    assert_eq!(analysis.projection.activity_nodes.len(), 4);
    assert_eq!(analysis.projection.flow_edges.len(), 3);
}

#[test]
fn transition_traces_are_typed_and_lossless() {
    let authored = "---\r\ntype: uml.StateMachine\r\n---\r\n# Sign in\r\n\r\n## Nodes\r\n### Idle\r\n- on `authenticated` transitions to SignedIn traces [AUTH-OIDC-004](./sign-in-behavior.md#auth-oidc-004)\r\n- on `retry` transitions to Idle traces [Retry](#retry) traces [Policy](https://example.com/policy)\r\n- on `fallback` transitions to SignedIn\r\n  traces [Local](#fallback)\r\n  traces [External](https://openid.net/specs/openid-connect-core-1_0.html)\r\n### final SignedIn\r\n";
    let analysis = analyze([
        ("sign-in.md", authored),
        (
            "sign-in-behavior.md",
            "---\ntype: okf.Behavior\n---\n# Sign-in behavior\n\n## AUTH-OIDC-004\n",
        ),
    ]);

    let syntax = root(&analysis, "sign-in.md");
    let transitions = typed::<uml::FlowTransitionSyntax>(syntax.clone());
    let traces = typed::<uml::FlowTraceSyntax>(syntax);

    assert_eq!(transitions.len(), 3);
    assert_eq!(traces.len(), 5);
    assert_eq!(transitions[0].traces().count(), 1);
    assert_eq!(transitions[1].traces().count(), 2);
    assert_eq!(transitions[2].traces().count(), 2);
    for transition in transitions {
        assert_eq!(transition.syntax().children().count(), 11);
        assert_eq!(
            transition
                .syntax()
                .child_at(uml::FlowTransitionSyntax::TRACES_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::FlowTraces
        );
    }
    let declared = analysis.declared.concept("sign-in").unwrap();
    let declared_traces = &declared.flow_nodes[0].transitions[0].traces;
    assert_eq!(declared_traces.len(), 1);
    assert!(matches!(
        &declared_traces[0].label,
        uml::DeclaredField::Valid { value, .. } if value == "AUTH-OIDC-004"
    ));
    assert!(matches!(
        &declared_traces[0].href,
        uml::DeclaredField::Valid { value, .. }
            if value == "./sign-in-behavior.md#auth-oidc-004"
    ));

    let edges = &analysis.projection.flow_edges;
    assert_eq!(edges[0].traces.len(), 1);
    assert_eq!(edges[0].traces[0].label, "AUTH-OIDC-004");
    assert_eq!(
        edges[0].traces[0].href,
        "./sign-in-behavior.md#auth-oidc-004"
    );
    assert!(matches!(
        &edges[0].traces[0].target,
        waml::model::TraceTarget::InternalFragment { concept_id, fragment }
            if concept_id == "sign-in-behavior" && fragment == "auth-oidc-004"
    ));
    assert_eq!(written(&analysis, "sign-in.md"), authored);
}

#[test]
fn malformed_transition_traces_recover_at_flow_boundaries_losslessly() {
    let authored = "---\ntype: uml.StateMachine\n---\n# Flow\n\n## Nodes\n### A\n- transitions to B traces\n- transitions to C traces [Broken](\n  traces []()\n### B\n  traces [Orphan](#orphan)\n- transitions to C traces [Valid](#valid)\n### final C\n\n## Notes\nkeep\n";
    let analysis = analyze([("flow.md", authored)]);
    let syntax = root(&analysis, "flow.md");
    let transitions = typed::<uml::FlowTransitionSyntax>(syntax.clone());
    let traces = typed::<uml::FlowTraceSyntax>(syntax);

    assert_eq!(written(&analysis, "flow.md"), authored);
    assert_eq!(transitions.len(), 4);
    assert_eq!(traces.len(), 3);
    let trace_counts = transitions
        .iter()
        .map(|transition| transition.traces().count())
        .collect::<Vec<_>>();
    assert_eq!(trace_counts, vec![0, 2, 0, 1], "{trace_counts:?}");
    assert!(transitions[3].traces().next().unwrap().link().is_some());
    assert_eq!(
        analysis.declared.concept("flow").unwrap().flow_nodes.len(),
        3
    );
}

#[test]
fn sequence_nested_slots_project_current_forms_losslessly() {
    let authored = "---\r\ntype: uml.SequenceDiagram\r\ntitle: Checkout\r\n---\r\n# Checkout\r\n\r\n## Lifelines\r\n- [Buyer](./buyer.md) as buyer\r\n- [Order](./order.md)\r\n\r\n## Messages\r\n- buyer calls Order `place(é)`\r\n- alt\r\n  - when `valid`\r\n    - Order returns `ok` to buyer\r\n    - loop\r\n      - when `again`\r\n        - buyer signals Order\r\n  - else\r\n    - buyer destroys Order\r\n- par\r\n  - branch `self`\r\n    - buyer calls buyer\r\n  - branch `outside`\r\n    - outside signals Order `found`\r\n";
    let analysis = analyze([
        ("sequence.md", authored),
        ("buyer.md", "---\ntype: uml.Actor\n---\n# Buyer\n"),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);
    let syntax = root(&analysis, "sequence.md");
    assert_eq!(count::<uml::LifelineSyntax>(syntax.clone()), 2);
    assert_eq!(count::<uml::MessageSyntax>(syntax.clone()), 6);
    assert_eq!(count::<uml::SequenceOperandSyntax>(syntax.clone()), 5);
    assert_eq!(count::<uml::MessagesBlockSyntax>(syntax.clone()), 1);
    assert_eq!(written(&analysis, "sequence.md"), authored);
    let sequence = analysis
        .projection
        .interactions
        .iter()
        .find(|s| s.key == "sequence")
        .unwrap();
    assert_eq!(sequence.edges.len(), 6);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("sequence.md").unwrap())
        .unwrap();
    let unsupported = analysis
        .syntax
        .document(id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
        })
        .count();
    assert_eq!(unsupported, 0);
}

#[test]
fn malformed_behavior_recovers_at_confirmed_heading_and_keeps_precise_provenance() {
    let authored = include_str!("fixtures/parser-platform/recovery/flow.md");
    let analysis = analyze([
        ("flow.md", authored),
        ("target.md", "---\ntype: uml.Class\n---\n# Target\n"),
    ]);
    let syntax = root(&analysis, "flow.md");
    assert_eq!(written(&analysis, "flow.md"), authored);
    assert!(count::<uml::FlowNodeSyntax>(syntax) >= 2);
    assert!(analysis
        .declared
        .concept("flow")
        .unwrap()
        .flow_nodes
        .iter()
        .flat_map(|node| node.transitions.iter())
        .any(|transition| matches!(transition.target, uml::DeclaredField::Invalid { .. })));
    assert!(analysis
        .diagnostics
        .iter()
        .all(|d| { d.document.is_some() && d.document_revision.is_some() && d.range.is_some() }));

    let sequence = include_str!("fixtures/parser-platform/recovery/sequence.md");
    let analysis = analyze([
        ("sequence.md", sequence),
        ("target.md", "---\ntype: uml.Class\n---\n# Target\n"),
    ]);
    assert_eq!(written(&analysis, "sequence.md"), sequence);
    assert!(analysis
        .declared
        .concept("sequence")
        .unwrap()
        .lifelines
        .iter()
        .any(|lifeline| matches!(lifeline.target, uml::DeclaredField::Invalid { .. })));
}

#[test]
fn every_flow_heading_kind_and_claimed_link_state_is_declared_without_byte_loss() {
    let authored = "---\ntype: uml.StateMachineDiagram\n---\n# S\n\n## Nodes\n### initial I\n### final F\n### decision D\n### merge M\n### fork K\n### join J\n### object [Target](./target.md)\n### Plain state\n";
    let analysis = analyze([
        ("states.md", authored),
        ("target.md", "---\ntype: uml.Class\n---\n# Target\n"),
    ]);
    let declared = analysis.declared.concept("states").unwrap();
    assert_eq!(declared.flow_nodes.len(), 8);
    assert!(declared
        .flow_nodes
        .iter()
        .all(|node| matches!(node.kind, uml::DeclaredField::Valid { .. })));
    assert!(matches!(
        declared.flow_nodes[6].object_ref,
        uml::DeclaredField::Valid { .. }
    ));
    assert!(matches!(
        declared.flow_nodes[7].object_ref,
        uml::DeclaredField::Absent
    ));
    assert_eq!(written(&analysis, "states.md"), authored);
}

#[test]
fn behavior_productions_expose_direct_fixed_slots() {
    let analysis = analyze([
        (
            "flow.md",
            "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### object [Order](./order.md)\n- on `go` when `ready` transitions to Done carries [Order](./order.md): `effect`\n### final Done\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls Order `place()`\n- alt\n  - when `ready`\n    - order returns to Order\n",
        ),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);
    let flow_root = root(&analysis, "flow.md");
    let nodes = typed::<uml::FlowNodeSyntax>(flow_root.clone());
    assert_eq!(
        nodes[0].kind_token().unwrap().text().write_to_string(),
        "object"
    );
    assert_eq!(nodes[0].identity_token().text().write_to_string(), "Order");
    assert_eq!(
        nodes[0].object_link().unwrap().kind(),
        uml::syntax::UmlSyntaxKind::Link
    );
    let transition = typed::<uml::FlowTransitionSyntax>(flow_root)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(
        transition.trigger_token().unwrap().text().write_to_string(),
        "`go`"
    );
    assert_eq!(
        transition.guard_token().unwrap().text().write_to_string(),
        "`ready`"
    );
    assert_eq!(
        transition.target_token().unwrap().text().write_to_string(),
        "Done"
    );
    assert_eq!(
        transition.effect_token().unwrap().text().write_to_string(),
        "`effect`"
    );
    assert!(transition.carries_link().is_some());
    let declared_flow = analysis.declared.concept("flow").unwrap();
    let identity_range = match &declared_flow.flow_nodes[0].identity {
        uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
        _ => panic!("identity is valid"),
    };
    assert_ne!(
        identity_range,
        declared_flow.flow_nodes[0].syntax.syntax().range()
    );
    let target_range = match &declared_flow.flow_nodes[0].transitions[0].target {
        uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
        _ => panic!("target is valid"),
    };
    assert_ne!(
        target_range,
        declared_flow.flow_nodes[0].transitions[0]
            .syntax
            .syntax()
            .range()
    );

    let sequence_root = root(&analysis, "sequence.md");
    let lifeline = typed::<uml::LifelineSyntax>(sequence_root.clone()).remove(0);
    assert!(lifeline.link().is_some());
    assert_eq!(
        lifeline.alias_token().unwrap().text().write_to_string(),
        "order"
    );
    let message = typed::<uml::MessageSyntax>(sequence_root.clone()).remove(0);
    assert_eq!(message.source_token().text().write_to_string(), "order");
    assert_eq!(message.verb_token().text().write_to_string(), "calls");
    assert_eq!(
        message.target_token().unwrap().text().write_to_string(),
        "Order"
    );
    assert_eq!(
        message.value_token().unwrap().text().write_to_string(),
        "`place()`"
    );
    let fragment = typed::<uml::SequenceFragmentSyntax>(sequence_root.clone()).remove(0);
    assert_eq!(fragment.kind_token().text().write_to_string(), "alt");
    let operand = typed::<uml::SequenceOperandSyntax>(sequence_root).remove(0);
    assert_eq!(
        operand.guard_token().unwrap().text().write_to_string(),
        "`ready`"
    );
    let declared_sequence = analysis.declared.concept("sequence").unwrap();
    let ranges = [
        match &declared_sequence.messages[0].source {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("message source is valid"),
        },
        match &declared_sequence.messages[0].target {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("message target is valid"),
        },
        match &declared_sequence.messages[0].value {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("message value is valid"),
        },
    ];
    for range in ranges {
        assert_ne!(range, declared_sequence.messages[0].syntax.syntax().range());
    }
}

#[test]
fn self_par_outside_gate_and_ref_forms_are_accepted() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# Current\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Gates\n- entry\n\n## Messages\n- ref [Use](./use.md) as used\n  - bind order to order\n- par\n  - branch `self`\n    - order calls order `work()`\n  - branch `outside`\n    - outside signals order `found`\n  - branch `gate`\n    - @entry signals used@exit `through`\n";
    let analysis = analyze([
        ("current.md", authored),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        (
            "use.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Use\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Gates\n- exit\n\n## Messages\n- order signals @exit `ready`\n",
        ),
    ]);
    let syntax = root(&analysis, "current.md");
    assert_eq!(
        count::<uml::syntax::InteractionUseSyntax>(syntax.clone()),
        1
    );
    assert_eq!(count::<uml::syntax::BindingSyntax>(syntax.clone()), 1);
    assert_eq!(count::<uml::SequenceFragmentSyntax>(syntax.clone()), 1);
    assert_eq!(count::<uml::SequenceOperandSyntax>(syntax.clone()), 3);
    assert_eq!(count::<uml::MessageSyntax>(syntax), 3);
    assert_eq!(written(&analysis, "current.md"), authored);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("current.md").unwrap())
        .unwrap();
    assert!(analysis
        .syntax
        .document(id)
        .unwrap()
        .syntax()
        .diagnostics()
        .is_empty());
}

#[test]
fn deferred_and_removed_sequence_spellings_have_exact_unsupported_ranges() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# Removed\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- strict\n- seq\n- ignore\n- consider\n- coregion order, Order\n- order calls order: `old-call`\n- order replies order: `old-return`\n- order sends order: `old-signal`\n- -> order: `found`\n- order sends ->\n- gate entry -> order\n- order signals order `current`\n";
    let analysis = analyze([
        ("removed.md", authored),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);
    let syntax = root(&analysis, "removed.md");
    let messages = typed::<uml::MessageSyntax>(syntax);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].value_token().unwrap().text().write_to_string(),
        "`current`"
    );
    assert_eq!(written(&analysis, "removed.md"), authored);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("removed.md").unwrap())
        .unwrap();
    let unsupported = analysis
        .syntax
        .document(id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
        })
        .collect::<Vec<_>>();
    assert_eq!(unsupported.len(), 11);
    for spelling in [
        "- strict",
        "- seq",
        "- ignore",
        "- consider",
        "- coregion order, Order",
        "- order calls order: `old-call`",
        "- order replies order: `old-return`",
        "- order sends order: `old-signal`",
        "- -> order: `found`",
        "- order sends ->",
        "- gate entry -> order",
    ] {
        let start = authored.find(spelling).unwrap();
        assert!(unsupported.iter().any(|diagnostic| {
            diagnostic.range.start().to_usize() == start
                && diagnostic.range.end().to_usize() == start + spelling.len()
        }));
    }
}

#[test]
fn behavior_occurrence_indices_are_invariant_across_absent_and_recovery_slots() {
    let analysis = analyze([
        (
            "flow.md",
            "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### Plain\n- entry: `begin`\n- refines [Order](./order.md)\n- transitions to Done\n- on `go` when `ready` transitions to Done carries [Order](./order.md): `effect`\n- transitions to Done: broken\n### object [Order](./order.md)\n### final Done\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md)\n- [Order](./order.md) as order\n- [Order](./order.md) trailing\n\n## Messages\n- order calls Order\n- order calls Order `place()`\n- order calls Order `broken\n- alt\n  - else\n  - when `ready`\n  - when broken\n",
        ),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);

    let flow_root = root(&analysis, "flow.md");
    let nodes = typed::<uml::FlowNodeSyntax>(flow_root.clone());
    for node in &nodes {
        assert_eq!(
            node.syntax()
                .child_at(uml::FlowNodeSyntax::KIND_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::FlowNodeKindSlot
        );
        assert_eq!(
            node.syntax()
                .child_at(uml::FlowNodeSyntax::IDENTITY_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::FlowIdentity
        );
        assert_eq!(
            node.syntax()
                .child_at(uml::FlowNodeSyntax::RECOVERY_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::BehaviorRecovery
        );
    }
    assert!(nodes[0].kind_token().is_none());
    assert_eq!(
        nodes[1].kind_token().unwrap().text().write_to_string(),
        "object"
    );

    let transitions = typed::<uml::FlowTransitionSyntax>(flow_root);
    assert_eq!(transitions.len(), 3);
    for transition in &transitions {
        assert_eq!(transition.syntax().children().count(), 11);
        assert_eq!(
            transition
                .syntax()
                .child_at(uml::FlowTransitionSyntax::TRACES_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::FlowTraces
        );
        assert_eq!(
            transition
                .syntax()
                .child_at(uml::FlowTransitionSyntax::TARGET_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::FlowTarget
        );
        assert_eq!(
            transition
                .syntax()
                .child_at(uml::FlowTransitionSyntax::RECOVERY_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::BehaviorRecovery
        );
        assert_eq!(
            transition.target_token().unwrap().text().write_to_string(),
            "Done"
        );
    }
    assert!(transitions[0].trigger_token().is_none());
    assert_eq!(
        transitions[1]
            .trigger_token()
            .unwrap()
            .text()
            .write_to_string(),
        "`go`"
    );
    assert_eq!(transitions[2].recovery().count(), 1);
    let declared_flow = analysis.declared.concept("flow").unwrap();
    assert!(matches!(
        declared_flow.flow_nodes[0].transitions[0].trigger,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_flow.flow_nodes[0].transitions[0].guard,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_flow.flow_nodes[0].transitions[0].carries,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_flow.flow_nodes[0].transitions[0].effect,
        uml::DeclaredField::Absent
    ));
    let internals = typed::<uml::FlowInternalSyntax>(root(&analysis, "flow.md"));
    assert_eq!(internals.len(), 2);
    for internal in &internals {
        assert_eq!(internal.syntax().children().count(), 7);
        assert_eq!(
            internal
                .syntax()
                .child_at(uml::FlowInternalSyntax::RECOVERY_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::BehaviorRecovery
        );
    }
    assert_eq!(
        internals[0].value_token().unwrap().text().write_to_string(),
        "`begin`"
    );
    assert!(internals[1].link().is_some());

    let sequence_root = root(&analysis, "sequence.md");
    let lifelines = typed::<uml::LifelineSyntax>(sequence_root.clone());
    for lifeline in &lifelines {
        assert_eq!(lifeline.syntax().children().count(), 6);
        assert_eq!(
            lifeline
                .syntax()
                .child_at(uml::LifelineSyntax::LINK_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::Link
        );
    }
    assert!(lifelines[0].alias_token().is_none());
    assert_eq!(
        lifelines[1].alias_token().unwrap().text().write_to_string(),
        "order"
    );
    assert_eq!(lifelines[2].recovery().count(), 1);

    let messages = typed::<uml::MessageSyntax>(sequence_root.clone());
    for message in &messages {
        assert_eq!(message.syntax().children().count(), 15);
        assert_eq!(
            message
                .syntax()
                .child_at(uml::MessageSyntax::VALUE_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::MessageValue
        );
    }
    assert!(messages[0].value_token().is_none());
    assert_eq!(
        messages[1].value_token().unwrap().text().write_to_string(),
        "`place()`"
    );
    assert_eq!(messages[2].recovery().count(), 1);

    let fragments = typed::<uml::SequenceFragmentSyntax>(sequence_root.clone());
    assert_eq!(fragments[0].syntax().children().count(), 4);
    let operands = typed::<uml::SequenceOperandSyntax>(sequence_root);
    for operand in &operands {
        assert_eq!(operand.syntax().children().count(), 6);
        assert_eq!(
            operand
                .syntax()
                .child_at(uml::SequenceOperandSyntax::GUARD_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::OperandGuard
        );
    }
    assert!(operands[0].guard_token().is_none());
    assert_eq!(
        operands[1].guard_token().unwrap().text().write_to_string(),
        "`ready`"
    );
    assert_eq!(operands[2].recovery().count(), 1);
    let declared_sequence = analysis.declared.concept("sequence").unwrap();
    assert!(matches!(
        declared_sequence.lifelines[0].alias,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_sequence.messages[0].value,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_sequence.operands[0].spec,
        uml::DeclaredField::Valid { .. }
    ));
}

#[test]
fn missing_lifeline_link_is_incomplete_but_present_malformed_link_is_invalid() {
    let authored =
        "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Lifelines\n-\n- broken\n\n## Messages\n";
    let analysis = analyze([("sequence.md", authored)]);
    let lifelines = &analysis.declared.concept("sequence").unwrap().lifelines;
    assert_eq!(lifelines.len(), 2);

    let missing_at = authored.find("## Lifelines\n-\n").unwrap() + "## Lifelines\n-".len();
    for field in [&lifelines[0].target, &lifelines[0].title] {
        match field {
            uml::DeclaredField::Incomplete { syntax, expected } => {
                assert_eq!(*expected, uml::ExpectedSyntax::LinkTarget);
                assert_eq!(syntax.range().start().to_usize(), missing_at);
                assert_eq!(syntax.range().end().to_usize(), missing_at);
            }
            _ => panic!("entirely missing lifeline link must be incomplete"),
        }
    }

    let malformed_start = authored.find("broken").unwrap();
    for field in [&lifelines[1].target, &lifelines[1].title] {
        match field {
            uml::DeclaredField::Invalid { syntax, .. } => {
                assert_eq!(syntax.range().start().to_usize(), malformed_start);
                assert_eq!(
                    syntax.range().end().to_usize(),
                    malformed_start + "broken".len()
                );
            }
            _ => panic!("present malformed lifeline link must be invalid"),
        }
    }

    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("sequence.md").unwrap())
        .unwrap();
    let diagnostics = analysis.syntax.document(id).unwrap().syntax().diagnostics();
    let malformed = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedLifeline
        })
        .collect::<Vec<_>>();
    assert_eq!(malformed.len(), 2);
    assert!(malformed.iter().any(|diagnostic| {
        diagnostic.range.start().to_usize() == missing_at
            && diagnostic.range.end().to_usize() == missing_at
    }));
    assert!(malformed.iter().any(|diagnostic| {
        diagnostic.range.start().to_usize() == malformed_start
            && diagnostic.range.end().to_usize() == malformed_start + "broken".len()
    }));
}

#[test]
fn lifeline_as_without_an_alias_is_reported_on_the_keyword() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md) as\n\n## Messages\n";
    let analysis = analyze([("sequence.md", authored)]);
    let lifelines = &analysis.declared.concept("sequence").unwrap().lifelines;
    assert_eq!(lifelines.len(), 1);
    assert!(matches!(
        lifelines[0].alias,
        uml::DeclaredField::Incomplete { .. }
    ));

    let as_start = authored.find(" as\n").unwrap() + 1;
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("sequence.md").unwrap())
        .unwrap();
    let diagnostics = analysis.syntax.document(id).unwrap().syntax().diagnostics();
    let malformed = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedLifeline
        })
        .collect::<Vec<_>>();
    assert_eq!(malformed.len(), 1);
    assert_eq!(malformed[0].range.start().to_usize(), as_start);
    assert_eq!(malformed[0].range.end().to_usize(), as_start + "as".len());
    assert_eq!(
        malformed[0].message.as_ref(),
        "expected a lifeline alias after \"as\""
    );
}

#[test]
fn lifeline_without_an_as_keyword_is_not_reported() {
    let authored =
        "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md)\n\n## Messages\n";
    let analysis = analyze([("sequence.md", authored)]);
    let lifelines = &analysis.declared.concept("sequence").unwrap().lifelines;
    assert_eq!(lifelines.len(), 1);
    assert!(matches!(lifelines[0].alias, uml::DeclaredField::Absent));

    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("sequence.md").unwrap())
        .unwrap();
    assert!(!analysis
        .syntax
        .document(id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .any(
            |diagnostic| diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedLifeline
        ));
}

#[test]
fn required_behavior_accessors_return_indexed_missing_tokens_without_panicking() {
    let flow = "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### initial\n- entry: `begin`\n- transitions\n- transitions to [broken\n### object\n### object [broken\n";
    let sequence =
        "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Messages\n- sender\n- alt\n  - else\n";
    let analysis = analyze([("flow.md", flow), ("sequence.md", sequence)]);

    let flow_root = root(&analysis, "flow.md");
    let nodes = typed::<uml::FlowNodeSyntax>(flow_root.clone());
    let initial_identity = nodes[0].identity_token();
    let initial_at = flow.find("### initial").unwrap() + "### initial".len();
    assert_eq!(
        initial_identity.kind(),
        uml::syntax::UmlSyntaxKind::IdentityToken
    );
    assert!(initial_identity.flags().is_missing());
    assert_eq!(initial_identity.range().start().to_usize(), initial_at);
    assert_eq!(initial_identity.range().end().to_usize(), initial_at);

    let object_identity = nodes[1].identity_token();
    let object_at = flow.find("### object").unwrap() + "### object".len();
    assert_eq!(
        object_identity.kind(),
        uml::syntax::UmlSyntaxKind::IdentityToken
    );
    assert!(object_identity.flags().is_missing());
    assert_eq!(object_identity.range().start().to_usize(), object_at);
    assert_eq!(object_identity.range().end().to_usize(), object_at);

    let malformed_object_identity = nodes[2].identity_token();
    let malformed_object_at = flow.rfind("[broken").unwrap();
    assert_eq!(
        malformed_object_identity.kind(),
        uml::syntax::UmlSyntaxKind::LinkTextToken
    );
    assert!(malformed_object_identity.flags().is_missing());
    assert_eq!(
        malformed_object_identity.range().start().to_usize(),
        malformed_object_at
    );
    assert_eq!(
        malformed_object_identity.range().end().to_usize(),
        malformed_object_at
    );

    let transitions = typed::<uml::FlowTransitionSyntax>(flow_root);
    let missing_target = transitions[0].target_token().unwrap();
    let target_at = flow.find("- transitions\n").unwrap() + "- transitions".len();
    assert_eq!(
        missing_target.kind(),
        uml::syntax::UmlSyntaxKind::TargetToken
    );
    assert!(missing_target.flags().is_missing());
    assert_eq!(missing_target.range().start().to_usize(), target_at);
    assert_eq!(missing_target.range().end().to_usize(), target_at);

    let malformed_link_target = transitions[1].target_token().unwrap();
    let malformed_link_at = flow.find("[broken").unwrap();
    assert_eq!(
        malformed_link_target.kind(),
        uml::syntax::UmlSyntaxKind::LinkTargetToken
    );
    assert!(malformed_link_target.flags().is_missing());
    assert_eq!(
        malformed_link_target.range().start().to_usize(),
        malformed_link_at
    );
    assert_eq!(
        malformed_link_target.range().end().to_usize(),
        malformed_link_at
    );

    let internal = typed::<uml::FlowInternalSyntax>(root(&analysis, "flow.md")).remove(0);
    let internal_keyword = internal.keyword_token().unwrap();
    assert_eq!(
        internal_keyword.kind(),
        uml::syntax::UmlSyntaxKind::InternalKeywordToken
    );
    assert!(!internal_keyword.flags().is_missing());

    let sequence_root = root(&analysis, "sequence.md");
    let messages = typed::<uml::MessageSyntax>(sequence_root.clone());
    let sender_start = sequence.find("sender").unwrap();
    let sender_at = sender_start + "sender".len();
    assert_eq!(
        messages[0].source_token().kind(),
        uml::syntax::UmlSyntaxKind::SourceToken
    );
    assert!(!messages[0].source_token().flags().is_missing());
    assert_eq!(
        messages[0].source_token().range().start().to_usize(),
        sender_start - 1
    );
    assert_eq!(
        messages[0].source_token().range().end().to_usize(),
        sender_at
    );
    assert_eq!(
        messages[0].verb_token().kind(),
        uml::syntax::UmlSyntaxKind::VerbToken
    );
    assert!(messages[0].target_token().is_none());
    let target_slot = messages[0]
        .syntax()
        .child_at(uml::MessageSyntax::TARGET_SLOT)
        .and_then(SyntaxElement::into_node)
        .unwrap();
    let target_token = target_slot
        .child_at(0)
        .and_then(SyntaxElement::into_token)
        .unwrap();
    assert_eq!(target_token.kind(), uml::syntax::UmlSyntaxKind::TargetToken);
    for token in [messages[0].verb_token(), target_token] {
        assert!(token.flags().is_missing());
        assert_eq!(token.range().start().to_usize(), sender_at);
        assert_eq!(token.range().end().to_usize(), sender_at);
    }

    let fragment = typed::<uml::SequenceFragmentSyntax>(sequence_root.clone()).remove(0);
    assert_eq!(
        fragment.kind_token().kind(),
        uml::syntax::UmlSyntaxKind::FragmentKindToken
    );
    assert!(!fragment.kind_token().flags().is_missing());
    let operand = typed::<uml::SequenceOperandSyntax>(sequence_root).remove(0);
    assert_eq!(
        operand.keyword_token().kind(),
        uml::syntax::UmlSyntaxKind::OperandKeywordToken
    );
    assert!(!operand.keyword_token().flags().is_missing());
}
