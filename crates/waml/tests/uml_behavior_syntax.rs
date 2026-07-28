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
    assert_eq!(count::<uml::MessageSyntax>(syntax.clone()), 6);
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
        .any(|transition| matches!(transition.target, uml::DeclaredField::Incomplete { .. })));
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
