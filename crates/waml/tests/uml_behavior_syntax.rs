use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode};

fn analyze(pairs: impl IntoIterator<Item = (&'static str, &'static str)>) -> uml::Analysis {
    let source = SourceBundle::try_from_pairs(pairs).unwrap();
    let okf = analyze_okf(&source, None, 9).unwrap();
    uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
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

fn root<'a>(analysis: &'a uml::Analysis, path: &str) -> SyntaxNode<uml::syntax::UmlLanguage> {
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
    let authored = "---\r\ntype: uml.Activity\r\ntitle: Café flow\r\n---\r\n# Café flow\r\n\r\n## Nodes\r\n### initial Start\r\n- transitions to Work\r\n### Work\r\n- entry: `begin`\r\n- do: `serve ☕`\r\n- exit: `finish`\r\n- on `paid` when `ok` transitions to Choice carries [Order](./order.md): `record`\r\n- refines [Child](./child.md)\r\n- partition: Kitchen\r\n#### Notes\r\n- naïve note\r\n### decision Choice\r\n- else transitions to Done\r\n### final Done\r\n";
    let analysis = analyze([
        ("flow.md", authored),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ("child.md", "---\ntype: uml.Activity\n---\n# Child\n"),
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
fn sequence_nested_slots_project_supported_forms_and_diagnose_deferred_forms() {
    let authored = "---\r\ntype: uml.Sequence\r\ntitle: Checkout\r\n---\r\n# Checkout\r\n\r\n## Lifelines\r\n- [Buyer](./buyer.md) as buyer\r\n- [Order](./order.md)\r\n\r\n## Messages\r\n- buyer calls Order: `place(é)`\r\n- alt\r\n  - when `valid`\r\n    - Order replies buyer: `ok`\r\n    - loop\r\n      - when `again`\r\n        - buyer sends Order\r\n  - else\r\n    - buyer destroys Order\r\n- par\r\n- buyer calls buyer\r\n- -> Order: `found`\r\n";
    let analysis = analyze([
        ("sequence.md", authored),
        ("buyer.md", "---\ntype: uml.Actor\n---\n# Buyer\n"),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);
    let syntax = root(&analysis, "sequence.md");
    assert_eq!(count::<uml::LifelineSyntax>(syntax.clone()), 2);
    assert_eq!(count::<uml::MessageSyntax>(syntax.clone()), 4);
    assert_eq!(count::<uml::SequenceOperandSyntax>(syntax.clone()), 3);
    assert_eq!(count::<uml::MessagesBlockSyntax>(syntax.clone()), 1);
    assert_eq!(written(&analysis, "sequence.md"), authored);
    let sequence = analysis
        .projection
        .interactions
        .iter()
        .find(|s| s.key == "sequence")
        .unwrap();
    assert_eq!(sequence.edges.len(), 4);
    assert!(analysis.diagnostics.iter().any(|d| d.line == 22));
    assert!(analysis.diagnostics.iter().any(|d| d.line == 23));
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
    let authored = "---\ntype: uml.StateMachine\n---\n# S\n\n## Nodes\n### initial I\n### final F\n### decision D\n### merge M\n### fork K\n### join J\n### object [Target](./target.md)\n### Plain state\n";
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
            "---\ntype: uml.Activity\n---\n# Flow\n\n## Nodes\n### object [Order](./order.md)\n- on `go` when `ready` transitions to Done carries [Order](./order.md): `effect`\n### final Done\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.Sequence\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls Order: `place()`\n- alt\n  - when `ready`\n    - order replies Order\n",
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
    assert_eq!(message.target_token().text().write_to_string(), "Order");
    assert_eq!(
        message.signature_token().unwrap().text().write_to_string(),
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
    for field in [
        &declared_sequence.messages[0].from,
        &declared_sequence.messages[0].to,
        &declared_sequence.messages[0].signature,
    ] {
        let range = match field {
            uml::DeclaredField::Valid { syntax, .. } => syntax.range(),
            _ => panic!("message field is valid"),
        };
        assert_ne!(range, declared_sequence.messages[0].syntax.syntax().range());
    }
}

#[test]
fn every_deferred_sequence_form_has_unsupported_code_and_exact_range() {
    let authored = "---\ntype: uml.Sequence\n---\n# Deferred\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- par\n- order calls order\n- -> order: `found`\n- order sends ->\n- gate entry -> order\n- coregion order, Order\n";
    let analysis = analyze([
        ("deferred.md", authored),
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ]);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("deferred.md").unwrap())
        .unwrap();
    let diagnostics = analysis.syntax.document(id).unwrap().syntax().diagnostics();
    let unsupported = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
        })
        .collect::<Vec<_>>();
    assert_eq!(unsupported.len(), 6);
    for spelling in [
        "- par",
        "- order calls order",
        "- -> order: `found`",
        "- order sends ->",
        "- gate entry -> order",
        "- coregion order, Order",
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
            "---\ntype: uml.Activity\n---\n# Flow\n\n## Nodes\n### Plain\n- entry: `begin`\n- refines [Order](./order.md)\n- transitions to Done\n- on `go` when `ready` transitions to Done carries [Order](./order.md): `effect`\n- transitions to Done: broken\n### object [Order](./order.md)\n### final Done\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.Sequence\n---\n# Sequence\n\n## Lifelines\n- [Order](./order.md)\n- [Order](./order.md) as order\n- [Order](./order.md) trailing\n\n## Messages\n- order calls Order\n- order calls Order: `place()`\n- order calls Order: broken\n- alt\n  - else\n  - when `ready`\n  - when broken\n",
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
        assert_eq!(transition.syntax().children().count(), 10);
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
        assert_eq!(message.syntax().children().count(), 8);
        assert_eq!(
            message
                .syntax()
                .child_at(uml::MessageSyntax::SIGNATURE_SLOT)
                .unwrap()
                .kind(),
            uml::syntax::UmlSyntaxKind::MessageSignature
        );
    }
    assert!(messages[0].signature_token().is_none());
    assert_eq!(
        messages[1]
            .signature_token()
            .unwrap()
            .text()
            .write_to_string(),
        "`place()`"
    );
    assert_eq!(messages[2].recovery().count(), 1);

    let fragments = typed::<uml::SequenceFragmentSyntax>(sequence_root.clone());
    assert_eq!(fragments[0].syntax().children().count(), 4);
    let operands = typed::<uml::SequenceOperandSyntax>(sequence_root);
    for operand in &operands {
        assert_eq!(operand.syntax().children().count(), 5);
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
        declared_sequence.messages[0].signature,
        uml::DeclaredField::Absent
    ));
    assert!(matches!(
        declared_sequence.sequence_operands[0].guard,
        uml::DeclaredField::Absent
    ));
}

#[test]
fn missing_lifeline_link_is_incomplete_but_present_malformed_link_is_invalid() {
    let authored =
        "---\ntype: uml.Sequence\n---\n# Sequence\n\n## Lifelines\n-\n- broken\n\n## Messages\n";
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
fn required_behavior_accessors_return_indexed_missing_tokens_without_panicking() {
    let flow = "---\ntype: uml.Activity\n---\n# Flow\n\n## Nodes\n### initial\n- entry: `begin`\n- transitions\n- transitions to [broken\n### object\n### object [broken\n";
    let sequence =
        "---\ntype: uml.Sequence\n---\n# Sequence\n\n## Messages\n- sender\n- alt\n  - else\n";
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
    assert_eq!(
        messages[0].target_token().kind(),
        uml::syntax::UmlSyntaxKind::TargetToken
    );
    for token in [messages[0].verb_token(), messages[0].target_token()] {
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
