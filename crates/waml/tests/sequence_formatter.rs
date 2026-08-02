use waml::{
    action::SyntaxChangeBatch,
    analysis::{prepare_candidate, DocumentId, PreparedCandidate},
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle},
    uml::{ActionContext, DeclaredField, Formatter},
};

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

#[test]
fn canonical_sequence_format_is_idempotent_and_semantic() {
    let source = "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Messages\n- ref [Authorize](authorize.md)  as auth\n  - bind order  to caller\n- order   calls payment async `charge( as, to, for )`  as charge\n- payment returns `approved`  to order  for charge\n\n## Gates\n- request\n\n## Lifelines\n- [Order](order.md)  as order\n- [Payment](payment.md) as payment\n";
    let expected = "---\ntype: uml.Sequence\n---\n\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as order\n- [Payment](./payment.md) as payment\n\n## Gates\n- request\n\n## Messages\n- ref [Authorize](./authorize.md) as auth\n  - bind order to caller\n- order calls payment async `charge( as, to, for )` as charge\n- payment returns `approved` to order for charge\n";
    let candidate = prepared(source, 80);
    let before = declared_fingerprint(&candidate);
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
    let source = "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as order\n\n## Messages\n- order calls payment `charge()` as\n";
    let candidate = prepared(source, 82);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
        )
        .unwrap();
    assert!(action.changes[0].edits.is_empty());
}
