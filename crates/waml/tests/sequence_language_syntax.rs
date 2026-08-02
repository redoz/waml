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

#[test]
fn canonical_messages_have_fixed_lossless_slots() {
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- customer calls order `submit()` as submission\n- order calls worker async `run()` as work\n- order returns `accepted` to customer for submission\n- order signals bus `OrderPlaced`\n- order creates worker: `OrderWorker`\n- order destroys worker\n";
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
        analysis.declared.concept("s").unwrap().messages[4].signature,
        uml::DeclaredField::Valid { .. }
    ));
    assert_eq!(written(&analysis, "s.md"), authored);
}

#[test]
fn create_and_destroy_reject_async_as_the_target() {
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- a creates async: `Worker`\n- a destroys async\n- a creates worker: `Worker`\n";
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
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- a calls b `work()` as\n- b returns `done` to\n- b returns `done` for\n- a calls\n- a signals\n- b returns `done` to for work\n- a returns async\n- a signals async b\n- a signals bus `Ready`\n";
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
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- a replies b: `old`\n- a sends b: `old`\n- a calls b: `old`\n- a signals b `new`\n";
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
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- alt\n  - when `ready`\n  - else\n- opt\n- loop\n- par\n  - branch `payment`\n    - par\n  - branch\n- break\n- critical\n- assert\n- neg\n";
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
            "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- strict\n- sender signals receiver `Ready`\n",
        ),
        (
            "seq",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- seq\n- sender signals receiver `Ready`\n",
        ),
        (
            "ignore",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- ignore\n- sender signals receiver `Ready`\n",
        ),
        (
            "consider",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- consider\n- sender signals receiver `Ready`\n",
        ),
        (
            "coregion",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- coregion\n- sender signals receiver `Ready`\n",
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
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- alt\n  - otherwise `not-ready`\n  - sender signals receiver `Ready`\n";
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
    let authored = "---\ntype: uml.Sequence\n---\n# S\n\n## Messages\n- alt\n    - when `too-deep`\n  - else\n";
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
