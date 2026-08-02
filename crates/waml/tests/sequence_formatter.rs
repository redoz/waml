use waml::{
    action::SyntaxChangeBatch,
    analysis::{prepare_candidate, DocumentId, PreparedCandidate},
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle},
    uml::{ActionContext, DeclaredField, Formatter},
};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode};

fn prepared(text: &str, revision: u64) -> PreparedCandidate {
    prepare_candidate(
        SourceBundle::try_from_pairs([("checkout.md", text)]).unwrap(),
        None,
        revision,
    )
    .unwrap()
}

fn document(candidate: &PreparedCandidate) -> DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("checkout.md").unwrap())
        .unwrap()
}

fn apply(candidate: &PreparedCandidate, action: waml::action::CodeAction) -> SourceBundle {
    SyntaxChangeBatch::new(action)
        .unwrap()
        .lower(EditContext {
            source: candidate.source(),
            okf_analysis: candidate.okf(),
            session_revision: candidate.revision(),
            uml: candidate.uml(),
        })
        .unwrap()
}

fn string_field(field: &DeclaredField<waml::uml::syntax::UmlLanguage, String>) -> String {
    match field {
        DeclaredField::Absent => "absent".into(),
        DeclaredField::Valid { value, .. } => format!("valid:{value}"),
        DeclaredField::Incomplete { .. } => "incomplete".into(),
        DeclaredField::Invalid { .. } => "invalid".into(),
    }
}

fn declared_fingerprint(candidate: &PreparedCandidate) -> Vec<String> {
    let concept = candidate.uml().declared.concept("checkout").unwrap();
    concept
        .lifelines
        .iter()
        .map(|lifeline| {
            format!(
                "lifeline:{}:{}:{}",
                string_field(&lifeline.target),
                string_field(&lifeline.title),
                string_field(&lifeline.alias)
            )
        })
        .chain(concept.messages.iter().map(|message| {
            format!(
                "message:{}:{}:{}:{}:{}",
                string_field(&message.from),
                match &message.verb {
                    DeclaredField::Absent => "absent".into(),
                    DeclaredField::Valid { value, .. } => format!("valid:{value:?}"),
                    DeclaredField::Incomplete { .. } => "incomplete".into(),
                    DeclaredField::Invalid { .. } => "invalid".into(),
                },
                string_field(&message.to),
                string_field(&message.signature),
                message.depth
            )
        }))
        .collect()
}

fn typed<T: AstNode<waml::uml::syntax::UmlLanguage>>(
    node: SyntaxNode<waml::uml::syntax::UmlLanguage>,
) -> Vec<T> {
    let mut result = T::cast(node.clone()).into_iter().collect::<Vec<_>>();
    for child in node.children().filter_map(SyntaxElement::into_node) {
        result.extend(typed::<T>(child));
    }
    result
}

fn syntax_fingerprint(candidate: &PreparedCandidate) -> Vec<String> {
    let root = candidate
        .uml()
        .syntax
        .document(document(candidate))
        .unwrap()
        .syntax()
        .root();
    typed::<waml::uml::syntax::GateSyntax>(root.clone())
        .iter()
        .map(|gate| format!("gate:{}", gate.name_token().text().write_to_string()))
        .chain(
            typed::<waml::uml::syntax::InteractionUseSyntax>(root.clone())
                .iter()
                .map(|interaction| {
                    let link = interaction.link().unwrap();
                    let link_text = link
                        .children()
                        .find(|element| {
                            element.kind() == waml::uml::syntax::UmlSyntaxKind::LinkTextToken
                        })
                        .and_then(SyntaxElement::into_token)
                        .unwrap()
                        .text()
                        .write_to_string();
                    let link_target = link
                        .children()
                        .find(|element| {
                            element.kind() == waml::uml::syntax::UmlSyntaxKind::LinkTargetToken
                        })
                        .and_then(SyntaxElement::into_token)
                        .unwrap()
                        .text()
                        .write_to_string();
                    format!(
                        "ref:{link_text}:{}:{}",
                        link_target.strip_prefix("./").unwrap_or(&link_target),
                        interaction.alias_token().text().write_to_string()
                    )
                })
                .collect::<Vec<_>>(),
        )
        .chain(
            typed::<waml::uml::syntax::BindingSyntax>(root.clone())
                .iter()
                .map(|binding| {
                    format!(
                        "bind:{}:{}",
                        binding.local_token().text().write_to_string(),
                        binding.target_token().text().write_to_string()
                    )
                })
                .collect::<Vec<_>>(),
        )
        .chain(
            typed::<waml::uml::SequenceFragmentSyntax>(root.clone())
                .iter()
                .map(|fragment| {
                    format!(
                        "fragment:{}",
                        fragment.kind_token().text().write_to_string()
                    )
                })
                .collect::<Vec<_>>(),
        )
        .chain(
            typed::<waml::uml::SequenceOperandSyntax>(root)
                .iter()
                .map(|operand| {
                    format!(
                        "operand:{}:{}:{}",
                        operand.keyword_token().text().write_to_string(),
                        operand
                            .guard_token()
                            .map(|token| token.text().write_to_string())
                            .unwrap_or_default(),
                        operand
                            .branch_label_token()
                            .map(|token| token.text().write_to_string())
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>(),
        )
        .collect()
}

#[test]
fn canonical_sequence_format_is_idempotent_and_semantic() {
    let source = "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Messages\n- ref [Authorize](authorize.md)  as auth\n  - bind order  to caller\n- alt\n  - when `ready`\n  - else\n- opt\n- loop\n- par\n  - branch `payment`\n  - branch\n- break\n- critical\n- assert\n- neg\n- order   calls payment async `charge( as, to, for )`  as charge\n- payment returns `approved`  to order  for charge\n\n## Gates\n- request\n\n## Lifelines\n- [Order](order.md)  as order\n- [Payment](payment.md) as payment\n";
    let expected = "---\ntype: uml.Sequence\n---\n\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as order\n- [Payment](./payment.md) as payment\n\n## Gates\n- request\n\n## Messages\n- ref [Authorize](./authorize.md) as auth\n  - bind order to caller\n- alt\n  - when `ready`\n  - else\n- opt\n- loop\n- par\n  - branch `payment`\n  - branch\n- break\n- critical\n- assert\n- neg\n- order calls payment async `charge( as, to, for )` as charge\n- payment returns `approved` to order for charge\n";
    let candidate = prepared(source, 80);
    let before = declared_fingerprint(&candidate);
    let syntax_before = syntax_fingerprint(&candidate);
    assert_eq!(
        syntax_before,
        [
            "gate:request",
            "ref:Authorize:authorize.md:auth",
            "bind:order:caller",
            "fragment:alt",
            "fragment:opt",
            "fragment:loop",
            "fragment:par",
            "fragment:break",
            "fragment:critical",
            "fragment:assert",
            "fragment:neg",
            "operand:when:`ready`:",
            "operand:else::",
            "operand:branch::`payment`",
            "operand:branch::",
        ]
    );
    let formatted = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate),
            )
            .unwrap(),
    );
    let actual = formatted
        .document(&BundlePath::parse("checkout.md").unwrap())
        .unwrap()
        .text();
    assert_eq!(actual, expected);

    let reparsed = prepared(actual, 81);
    assert_eq!(declared_fingerprint(&reparsed), before);
    assert_eq!(syntax_fingerprint(&reparsed), syntax_before);
    let second = Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed),
        )
        .unwrap();
    assert!(second.changes[0].edits.is_empty());
}

#[test]
fn malformed_canonical_sequence_line_prevents_all_formatter_edits() {
    let cases = [
        "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls payment `charge()` as\n",
        "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Messages\nalt\n- sender signals receiver `Ready`\n",
        "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Messages\n- alt\n  when `ready`\n  - else\n",
    ];
    for (index, source) in cases.into_iter().enumerate() {
        let candidate = prepared(source, 82 + index as u64);
        assert!(!candidate
            .uml()
            .syntax
            .document(document(&candidate))
            .unwrap()
            .syntax()
            .diagnostics()
            .is_empty());
        let action = Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate),
            )
            .unwrap();
        assert!(action.changes[0].edits.is_empty());
    }
}

#[test]
fn mixed_tab_sequence_indentation_prevents_all_formatter_edits() {
    let source = "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Messages\n- alt\n  \t- branch `mixed`\n- ref [Authorize](./authorize.md) as auth\n  \t- bind order to caller\n";
    let candidate = prepared(source, 85);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
        )
        .unwrap();
    assert!(action.changes[0].edits.is_empty());
}
