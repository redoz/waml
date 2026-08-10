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

fn declared_value<T>(field: &uml::DeclaredField<uml::syntax::UmlLanguage, T>) -> &T {
    match field {
        uml::DeclaredField::Valid { value, .. } => value,
        _ => panic!("expected a valid declared field"),
    }
}

#[test]
fn declared_messages_preserve_authored_fields_without_matching() {
    let analysis = analyze([(
        "s.md",
        "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- customer calls order `submit()` as submission\n- order returns `accepted` to intruder for submission\n",
    )]);
    let message = &analysis.declared.concept("s").unwrap().messages[1];

    assert_eq!(
        declared_value(&message.source),
        &uml::DeclaredEndpointRef::Lifeline("order".into())
    );
    assert_eq!(declared_value(&message.value), "accepted");
    assert_eq!(
        declared_value(&message.return_to),
        &uml::DeclaredEndpointRef::Lifeline("intruder".into())
    );
    assert_eq!(declared_value(&message.return_for), "submission");
}

#[test]
fn canonical_messages_have_fixed_lossless_slots() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- customer calls order `submit()` as submission\n- order calls worker async `run()` as work\n- order returns `accepted` to customer for submission\n- order signals bus `OrderPlaced`\n- order creates worker: `OrderWorker`\n- order destroys worker\n";
    let analysis = analyze([("s.md", authored)]);
    let messages = typed::<uml::MessageSyntax>(root(&analysis, "s.md"));
    assert_eq!(messages.len(), 6);
    assert!(messages
        .iter()
        .all(|message| message.syntax().children().count() == 15));
    assert_eq!(
        messages[0].source_token().text().write_to_string(),
        "customer"
    );
    assert_eq!(messages[0].verb_token().text().write_to_string(), "calls");
    assert_eq!(
        messages[0].target_token().unwrap().text().write_to_string(),
        "order"
    );
    assert_eq!(
        messages[0].value_token().unwrap().text().write_to_string(),
        "`submit()`"
    );
    assert_eq!(
        messages[0]
            .call_id_token()
            .unwrap()
            .text()
            .write_to_string(),
        "submission"
    );
    assert_eq!(
        messages[0].as_token().unwrap().text().write_to_string(),
        "as"
    );
    assert_eq!(
        messages[1].async_token().unwrap().text().write_to_string(),
        "async"
    );
    assert_eq!(
        messages[2]
            .return_target_token()
            .unwrap()
            .text()
            .write_to_string(),
        "customer"
    );
    assert_eq!(
        messages[2]
            .return_call_token()
            .unwrap()
            .text()
            .write_to_string(),
        "submission"
    );
    assert_eq!(
        messages[2].to_token().unwrap().text().write_to_string(),
        "to"
    );
    assert_eq!(
        messages[2].for_token().unwrap().text().write_to_string(),
        "for"
    );
    assert_eq!(
        messages[3].value_token().unwrap().text().write_to_string(),
        "`OrderPlaced`"
    );
    assert_eq!(
        messages[4].colon_token().unwrap().text().write_to_string(),
        ":"
    );
    let colon_slot = messages[4]
        .syntax()
        .child_at(uml::MessageSyntax::COLON_SLOT)
        .and_then(SyntaxElement::into_token)
        .unwrap();
    assert_eq!(colon_slot.kind(), uml::syntax::UmlSyntaxKind::ColonToken);
    assert!(!colon_slot.flags().is_missing());
    assert_eq!(colon_slot.text().write_to_string(), ":");
    let value_slot = messages[4]
        .syntax()
        .child_at(uml::MessageSyntax::VALUE_SLOT)
        .and_then(SyntaxElement::into_node)
        .unwrap();
    assert_eq!(value_slot.kind(), uml::syntax::UmlSyntaxKind::MessageValue);
    assert_eq!(
        messages[4].value_token().unwrap().text().write_to_string(),
        "`OrderWorker`"
    );
    assert!(matches!(
        analysis.declared.concept("s").unwrap().messages[4].value,
        uml::DeclaredField::Valid { .. }
    ));
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn create_and_destroy_reject_async_as_the_target() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- a creates async: `Worker`\n- a destroys async\n- a creates worker: `Worker`\n";
    let analysis = analyze([("s.md", authored)]);
    let messages = typed::<uml::MessageSyntax>(root(&analysis, "s.md"));
    assert_eq!(messages.len(), 3);
    assert!(messages[0].target_token().is_none());
    assert!(messages[1].target_token().is_none());
    assert_eq!(messages[0].recovery().count(), 1);
    assert_eq!(messages[1].recovery().count(), 1);
    assert_eq!(
        messages[2].target_token().unwrap().text().write_to_string(),
        "worker"
    );
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let malformed = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
        })
        .count();
    assert_eq!(malformed, 2);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn malformed_message_recovers_at_the_next_sibling() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- a calls b `work()` as\n- b returns `done` to\n- b returns `done` for\n- a calls\n- a signals\n- b returns `done` to for work\n- a returns async\n- a signals async b\n- a signals bus `Ready`\n";
    let analysis = analyze([("s.md", authored)]);
    let messages = typed::<uml::MessageSyntax>(root(&analysis, "s.md"));
    assert_eq!(messages.len(), 9);
    assert_eq!(
        messages
            .iter()
            .map(|message| message.syntax().children().count())
            .collect::<Vec<_>>(),
        vec![15; 9]
    );
    assert_eq!(messages[6].recovery().count(), 1);
    assert_eq!(messages[7].recovery().count(), 1);
    assert_eq!(
        messages[8].target_token().unwrap().text().write_to_string(),
        "bus"
    );
    assert_eq!(
        messages[8].value_token().unwrap().text().write_to_string(),
        "`Ready`"
    );
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let malformed = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
        })
        .count();
    assert_eq!(malformed, 8);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn removed_message_spellings_are_not_messages() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- a replies b: `old`\n- a sends b: `old`\n- a calls b: `old`\n- a signals b `new`\n";
    let analysis = analyze([("s.md", authored)]);
    let messages = typed::<uml::MessageSyntax>(root(&analysis, "s.md"));
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].value_token().unwrap().text().write_to_string(),
        "`new`"
    );
    assert_eq!(analysis.declared.concept("s").unwrap().messages.len(), 1);
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let unsupported = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
        })
        .count();
    assert_eq!(unsupported, 3);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn all_fragment_heads_and_operand_forms_are_typed() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- alt\n  - when `ready`\n  - else\n- opt\n- loop\n- par\n  - branch `payment`\n    - par\n  - branch\n- break\n- critical\n- assert\n- neg\n";
    let analysis = analyze([("s.md", authored)]);
    let root = root(&analysis, "s.md");
    let fragments = typed::<uml::SequenceFragmentSyntax>(root.clone());
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.kind_token().text().write_to_string())
            .collect::<Vec<_>>(),
        ["alt", "opt", "loop", "par", "par", "break", "critical", "assert", "neg",]
    );
    let operands = typed::<uml::SequenceOperandSyntax>(root);
    assert_eq!(
        operands
            .iter()
            .map(|operand| (
                operand.keyword_token().text().write_to_string(),
                operand
                    .guard_token()
                    .map(|token| token.text().write_to_string()),
                operand
                    .branch_label_token()
                    .map(|token| token.text().write_to_string()),
            ))
            .collect::<Vec<_>>(),
        [
            ("when".into(), Some("`ready`".into()), None),
            ("else".into(), None, None),
            ("branch".into(), None, Some("`payment`".into())),
            ("branch".into(), None, None),
        ]
    );
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn deferred_fragment_heads_recover_without_consuming_the_next_message() {
    let cases = [
        (
            "strict",
            "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- strict\n- sender signals receiver `Ready`\n",
        ),
        (
            "seq",
            "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- seq\n- sender signals receiver `Ready`\n",
        ),
        (
            "ignore",
            "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- ignore\n- sender signals receiver `Ready`\n",
        ),
        (
            "consider",
            "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- consider\n- sender signals receiver `Ready`\n",
        ),
        (
            "coregion",
            "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- coregion\n- sender signals receiver `Ready`\n",
        ),
    ];

    for (head, authored) in cases {
        let analysis = analyze([("s.md", authored)]);
        let root = root(&analysis, "s.md");
        assert!(typed::<uml::SequenceFragmentSyntax>(root.clone()).is_empty());
        let messages = typed::<uml::MessageSyntax>(root);
        assert_eq!(messages.len(), 1, "{head}");
        assert_eq!(
            messages[0].verb_token().text().write_to_string(),
            "signals",
            "{head}"
        );
        let document_id = analysis
            .syntax
            .catalog()
            .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
            .unwrap();
        let unsupported = analysis
            .syntax
            .document(document_id)
            .unwrap()
            .syntax()
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
            })
            .count();
        assert_eq!(unsupported, 1, "{head}");
        assert_eq!(written(&analysis, "s.md"), authored, "{head}");
    }
}

#[test]
fn unknown_operand_words_recover_without_becoming_messages() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- alt\n  - otherwise `not-ready`\n  - sender signals receiver `Ready`\n";
    let analysis = analyze([("s.md", authored)]);
    let root = root(&analysis, "s.md");
    assert_eq!(typed::<uml::SequenceFragmentSyntax>(root.clone()).len(), 1);
    let messages = typed::<uml::MessageSyntax>(root);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].verb_token().text().write_to_string(), "signals");
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn operands_require_exactly_one_more_indentation_level() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- alt\n    - when `too-deep`\n  - else\n";
    let analysis = analyze([("s.md", authored)]);
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let malformed = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
        })
        .count();
    assert_eq!(malformed, 1);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn gates_refs_and_bindings_have_fixed_lossless_slots() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# Checkout\n\n## Gates\n- request\n\n## Messages\n- ref [Authorize payment](./authorize-payment.md) as auth\n  - bind order to caller\n  - bind payment to payment\n- order calls auth@request `authorize()` as authorization\n- auth@request returns `approved` for authorization\n";
    let target = "---\ntype: uml.SequenceDiagram\n---\n# Authorize payment\n";
    let analysis = analyze([("checkout.md", authored), ("authorize-payment.md", target)]);
    let root = root(&analysis, "checkout.md");

    let gates = typed::<uml::syntax::GateSyntax>(root.clone());
    assert_eq!(gates.len(), 1);
    assert_eq!(gates[0].name_token().text().write_to_string(), "request");

    let uses = typed::<uml::syntax::InteractionUseSyntax>(root.clone());
    assert_eq!(uses.len(), 1);
    let link = uses[0].link().unwrap();
    assert_eq!(
        link.children()
            .filter_map(SyntaxElement::into_token)
            .map(|token| token.text().write_to_string())
            .collect::<String>(),
        "[Authorize payment](./authorize-payment.md)"
    );
    assert_eq!(uses[0].alias_token().text().write_to_string(), "auth");

    let bindings = typed::<uml::syntax::BindingSyntax>(root.clone());
    assert_eq!(bindings.len(), 2);
    assert_eq!(
        bindings
            .iter()
            .map(|binding| (
                binding.local_token().text().write_to_string(),
                binding.target_token().text().write_to_string(),
            ))
            .collect::<Vec<_>>(),
        [
            ("order".into(), "caller".into()),
            ("payment".into(), "payment".into())
        ]
    );

    let messages = typed::<uml::MessageSyntax>(root);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].target_token().unwrap().text().write_to_string(),
        "auth@request"
    );
    assert_eq!(
        messages[1].source_token().text().write_to_string(),
        "auth@request"
    );
    assert_eq!(written(&analysis, "checkout.md"), authored);
}

#[test]
fn sequence_reference_errors_recover_at_the_next_typed_line() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# Checkout\n\n## Messages\n- ref as missing-link\n- ref [Authorize](./authorize.md) as\n- ref [Authorize](./authorize.md) as auth\n  - bind order caller\n  - bind payment to payment\n- sender calls @ `bad()` as invalid_target\n- use@ signals receiver `bad`\n- sender signals receiver `Ready`\n- ref [Later](./later.md) as later\n";
    let target = "---\ntype: uml.SequenceDiagram\n---\n# Target\n";
    let analysis = analyze([
        ("checkout.md", authored),
        ("authorize.md", target),
        ("later.md", target),
    ]);
    let root = root(&analysis, "checkout.md");

    let uses = typed::<uml::syntax::InteractionUseSyntax>(root.clone());
    assert_eq!(uses.len(), 4);
    assert_eq!(uses[3].alias_token().text().write_to_string(), "later");
    let bindings = typed::<uml::syntax::BindingSyntax>(root.clone());
    assert_eq!(bindings.len(), 2);
    assert_eq!(
        bindings[1].local_token().text().write_to_string(),
        "payment"
    );
    assert_eq!(
        bindings[1].target_token().text().write_to_string(),
        "payment"
    );
    let messages = typed::<uml::MessageSyntax>(root);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].target_token().is_none());
    assert_eq!(messages[0].recovery().count(), 1);
    assert_eq!(messages[1].recovery().count(), 1);
    assert_eq!(messages[2].verb_token().text().write_to_string(), "signals");

    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("checkout.md").unwrap())
        .unwrap();
    let malformed = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
        })
        .count();
    assert_eq!(malformed, 6);
    assert_eq!(written(&analysis, "checkout.md"), authored);
}

#[test]
fn bare_fragment_and_operand_keep_their_text_and_recover_locally() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- alt\n  when `ready`\n  - else\nalt\n- sender signals receiver `Ready`\n";
    let analysis = analyze([("s.md", authored)]);
    let root = root(&analysis, "s.md");
    let fragments = typed::<uml::SequenceFragmentSyntax>(root.clone());
    assert_eq!(fragments.len(), 2);
    assert_eq!(fragments[1].kind_token().text().write_to_string(), "alt");
    assert!(fragments[1]
        .syntax()
        .child_at(uml::SequenceFragmentSyntax::BULLET_SLOT)
        .and_then(SyntaxElement::into_token)
        .unwrap()
        .flags()
        .is_missing());
    let operands = typed::<uml::SequenceOperandSyntax>(root.clone());
    assert_eq!(operands.len(), 2);
    assert_eq!(operands[0].keyword_token().text().write_to_string(), "when");
    assert!(operands[0]
        .syntax()
        .child_at(uml::SequenceOperandSyntax::BULLET_SLOT)
        .and_then(SyntaxElement::into_token)
        .unwrap()
        .flags()
        .is_missing());
    let messages = typed::<uml::MessageSyntax>(root);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].verb_token().text().write_to_string(), "signals");
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let malformed = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
        })
        .count();
    assert_eq!(malformed, 2);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn mixed_tab_indentation_recovers_before_operand_and_binding_ownership() {
    let authored = "---\ntype: uml.SequenceDiagram\n---\n# S\n\n## Messages\n- alt\n  \t- branch `mixed`\n  - branch `valid`\n- ref [Use](./use.md) as used\n  \t- bind bad to bad\n  - bind good to good\n- sender signals receiver `Ready`\n";
    let target = "---\ntype: uml.SequenceDiagram\n---\n# Use\n";
    let analysis = analyze([("s.md", authored), ("use.md", target)]);
    let root = root(&analysis, "s.md");
    let operands = typed::<uml::SequenceOperandSyntax>(root.clone());
    assert_eq!(operands.len(), 1);
    assert_eq!(
        operands[0]
            .branch_label_token()
            .unwrap()
            .text()
            .write_to_string(),
        "`valid`"
    );
    let bindings = typed::<uml::syntax::BindingSyntax>(root.clone());
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].local_token().text().write_to_string(), "good");
    let messages = typed::<uml::MessageSyntax>(root);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].verb_token().text().write_to_string(), "signals");
    let document_id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("s.md").unwrap())
        .unwrap();
    let malformed_indent = analysis
        .syntax
        .document(document_id)
        .unwrap()
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedIndentation
        })
        .count();
    assert_eq!(malformed_indent, 2);
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn malformed_sequence_forms_recover_at_operand_heading_and_eof_boundaries() {
    #[derive(Clone, Copy)]
    enum Form {
        Message,
        Operand,
        InteractionUse,
        Binding,
    }

    #[derive(Clone, Copy)]
    enum Boundary {
        Operand,
        Heading,
        Eof,
    }

    let cases = [
        (
            "message-operand.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Message operand\n\n## Messages\n- alt\n  - branch `first`\n    - sender calls\n  - branch `next`\n    - sender signals receiver `kept`\n",
            Form::Message,
            Boundary::Operand,
        ),
        (
            "operand-operand.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Operand operand\n\n## Messages\n- alt\n  - when\n  - branch `next`\n    - sender signals receiver `kept`\n",
            Form::Operand,
            Boundary::Operand,
        ),
        (
            "ref-operand.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Ref operand\n\n## Messages\n- alt\n  - branch `first`\n    - ref [Target](./target.md) as\n  - branch `next`\n    - sender signals receiver `kept`\n",
            Form::InteractionUse,
            Boundary::Operand,
        ),
        (
            "bind-operand.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Bind operand\n\n## Messages\n- alt\n  - branch `first`\n    - ref [Target](./target.md) as target\n      - bind local\n  - branch `next`\n    - sender signals receiver `kept`\n",
            Form::Binding,
            Boundary::Operand,
        ),
        (
            "message-heading.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Message heading\n\n## Messages\n- sender calls\n\n## Gates\n- next\n",
            Form::Message,
            Boundary::Heading,
        ),
        (
            "operand-heading.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Operand heading\n\n## Messages\n- alt\n  - when\n\n## Gates\n- next\n",
            Form::Operand,
            Boundary::Heading,
        ),
        (
            "ref-heading.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Ref heading\n\n## Messages\n- ref [Target](./target.md) as\n\n## Gates\n- next\n",
            Form::InteractionUse,
            Boundary::Heading,
        ),
        (
            "bind-heading.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Bind heading\n\n## Messages\n- ref [Target](./target.md) as target\n  - bind local\n\n## Gates\n- next\n",
            Form::Binding,
            Boundary::Heading,
        ),
        (
            "message-eof.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Message EOF\n\n## Messages\n- sender calls",
            Form::Message,
            Boundary::Eof,
        ),
        (
            "operand-eof.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Operand EOF\n\n## Messages\n- alt\n  - when",
            Form::Operand,
            Boundary::Eof,
        ),
        (
            "ref-eof.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Ref EOF\n\n## Messages\n- ref [Target](./target.md) as",
            Form::InteractionUse,
            Boundary::Eof,
        ),
        (
            "bind-eof.md",
            "---\ntype: uml.SequenceDiagram\n---\n# Bind EOF\n\n## Messages\n- ref [Target](./target.md) as target\n  - bind local",
            Form::Binding,
            Boundary::Eof,
        ),
    ];
    let target = "---\ntype: uml.SequenceDiagram\n---\n# Target\n";

    for (path, authored, form, boundary) in cases {
        let analysis = analyze([(path, authored), ("target.md", target)]);
        let syntax_root = root(&analysis, path);
        let has_form = match form {
            Form::Message => !typed::<uml::MessageSyntax>(syntax_root.clone()).is_empty(),
            Form::Operand => !typed::<uml::SequenceOperandSyntax>(syntax_root.clone()).is_empty(),
            Form::InteractionUse => {
                !typed::<uml::syntax::InteractionUseSyntax>(syntax_root.clone()).is_empty()
            }
            Form::Binding => !typed::<uml::syntax::BindingSyntax>(syntax_root.clone()).is_empty(),
        };
        assert!(has_form, "{path} lost its malformed form");

        match boundary {
            Boundary::Operand => assert!(typed::<uml::SequenceOperandSyntax>(syntax_root.clone())
                .iter()
                .any(|operand| operand
                    .branch_label_token()
                    .is_some_and(|token| { token.text().write_to_string() == "`next`" }))),
            Boundary::Heading => {
                let gates = typed::<uml::syntax::GateSyntax>(syntax_root.clone());
                assert_eq!(gates.len(), 1, "{path} swallowed the next heading");
                assert_eq!(gates[0].name_token().text().write_to_string(), "next");
            }
            Boundary::Eof => {}
        }

        let document_id = analysis
            .syntax
            .catalog()
            .id_for_path(&waml::source::BundlePath::parse(path).unwrap())
            .unwrap();
        assert!(
            analysis
                .syntax
                .document(document_id)
                .unwrap()
                .syntax()
                .diagnostics()
                .iter()
                .any(|diagnostic| {
                    diagnostic.code == uml::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
                }),
            "{path} did not report malformed sequence syntax"
        );
        assert_eq!(
            written(&analysis, path),
            authored,
            "{path} was not lossless"
        );
    }
}
