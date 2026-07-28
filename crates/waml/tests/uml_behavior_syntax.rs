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
