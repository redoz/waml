use waml::{
    action::{ActionBasis, SyntaxChangeBatch},
    analysis::{prepare_candidate, DocumentId, PreparedCandidate},
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle},
    uml::{repair_actions, ActionContext},
};

fn prepared(text: &str, revision: u64) -> PreparedCandidate {
    prepare_candidate(
        SourceBundle::try_from_pairs([("class.md", text)]).unwrap(),
        None,
        revision,
    )
    .unwrap()
}

fn document(candidate: &PreparedCandidate) -> DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("class.md").unwrap())
        .unwrap()
}

fn repaired_text(candidate: &PreparedCandidate, action: waml::action::CodeAction) -> String {
    let source = SyntaxChangeBatch::new(action)
        .unwrap()
        .lower(EditContext {
            source: candidate.source(),
            okf_analysis: candidate.okf(),
            session_revision: candidate.revision(),
            uml: candidate.uml(),
        })
        .unwrap();
    source
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text()
        .to_owned()
}

#[test]
fn repairs_missing_colon_type_and_invalid_multiplicity_at_typed_slots() {
    let cases = [
        ("- name String", "- name: String", "Insert missing `: `"),
        ("- name:", "- name: String", "Insert missing type `String`"),
        (
            "- name: String [oops 42]",
            "- name: String {42}",
            "Replace invalid multiplicity",
        ),
    ];
    for (line, fixed, title) in cases {
        let source = format!("---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Attributes\n{line}\n");
        let candidate = prepared(&source, 11);
        let actions = repair_actions(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
        )
        .unwrap();
        let action = actions
            .iter()
            .find(|action| action.title == title)
            .unwrap_or_else(|| panic!("missing {title:?} in {actions:?}"))
            .clone();
        assert!(matches!(
            action.basis,
            ActionBasis::Document {
                session_revision: 11,
                ..
            }
        ));
        assert_eq!(
            repaired_text(&candidate, action),
            source.replace(line, fixed)
        );
    }
}

#[test]
fn action_context_rejects_mixed_catalogs_and_wrong_revision() {
    let left = prepared(
        "---\ntype: uml.Class\n---\n# Left\n\n## Attributes\n- x:\n",
        7,
    );
    let right = prepared(
        "---\ntype: uml.Class\n---\n# Right\n\n## Attributes\n- y:\n",
        7,
    );
    assert!(matches!(
        ActionContext::new(left.okf(), right.uml(), 7),
        Err(waml::action::ActionError::MismatchedCatalog)
    ));
    assert!(matches!(
        ActionContext::new(left.okf(), left.uml(), 8),
        Err(waml::action::ActionError::MismatchedAnalysisRevision { .. })
    ));
}

#[test]
fn produced_action_is_rejected_after_a_new_session_revision() {
    let original = prepared(
        "---\ntype: uml.Class\n---\n# C\n\n## Attributes\n- name:\n",
        20,
    );
    let action = repair_actions(
        ActionContext::from_prepared(&original).unwrap(),
        document(&original),
    )
    .unwrap()
    .into_iter()
    .find(|action| action.title == "Insert missing type `String`")
    .unwrap();
    let changed = prepared(
        "---\ntype: uml.Class\n---\n# C changed\n\n## Attributes\n- name:\n",
        21,
    );
    let error = SyntaxChangeBatch::new(action)
        .unwrap()
        .lower(EditContext {
            source: changed.source(),
            okf_analysis: changed.okf(),
            session_revision: changed.revision(),
            uml: changed.uml(),
        })
        .unwrap_err();
    assert!(error.reason.contains("StaleSession"), "{error:?}");
}

#[test]
fn multiplicity_repair_reanalyzes_cleanly_with_canonical_braces() {
    let source = "---\r\ntype: uml.Class\r\ntitle: Café 😀\r\n---\r\n# Café 😀\r\n\r\n## Attributes\r\n- quantité: Number [oops 42]\r\n";
    let candidate = prepared(source, 30);
    let action = repair_actions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
    )
    .unwrap()
    .into_iter()
    .find(|action| action.title == "Replace invalid multiplicity")
    .unwrap();
    let repaired = SyntaxChangeBatch::new(action)
        .unwrap()
        .lower(EditContext {
            source: candidate.source(),
            okf_analysis: candidate.okf(),
            session_revision: candidate.revision(),
            uml: candidate.uml(),
        })
        .unwrap();
    let repaired_text = repaired
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text();
    assert!(repaired_text.contains("quantité: Number {42}\r\n"));
    assert!(repaired_text.starts_with("---\r\n"));

    let reparsed = prepare_candidate(repaired, None, 31).unwrap();
    assert!(
        reparsed.uml().diagnostics.iter().all(|diagnostic| {
            diagnostic.message != "invalid multiplicity"
                && diagnostic.message != "unterminated multiplicity"
        }),
        "{:?}",
        reparsed.uml().diagnostics
    );
    let attribute = &reparsed.uml().declared.concept("class").unwrap().attributes[0];
    assert!(matches!(
        attribute.multiplicity,
        waml::uml::DeclaredField::Valid { .. }
    ));
    assert_eq!(
        reparsed.uml().projection.node("class").unwrap().attributes[0]
            .multiplicity
            .as_ref()
            .unwrap()
            .as_str(),
        "42"
    );
    assert!(repair_actions(
        ActionContext::from_prepared(&reparsed).unwrap(),
        document(&reparsed)
    )
    .unwrap()
    .iter()
    .all(|action| action.title != "Replace invalid multiplicity"));
}

#[test]
fn brace_multiplicity_repairs_use_the_exact_typed_slot_range() {
    let cases = [
        ("{oops 42}", "{42}", "42"),
        ("{oops}", "{1}", "1"),
        ("{oops", "{1}", "1"),
    ];
    for (invalid, replacement, expected_value) in cases {
        let source = format!(
            "---\r\ntype: uml.Class\r\n---\r\n# Café 😀\r\n\r\n## Attributes\r\n- quantité: Number {invalid}\r\n"
        );
        let candidate = prepared(&source, 70);
        let action = repair_actions(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
        )
        .unwrap()
        .into_iter()
        .find(|action| action.title == "Replace invalid multiplicity")
        .unwrap_or_else(|| panic!("missing brace repair for {invalid:?}"));
        let edit = &action.changes[0].edits[0];
        let expected_start = source.find(invalid).unwrap();
        assert_eq!(edit.range.start().to_usize(), expected_start, "{invalid}");
        assert_eq!(
            edit.range.end().to_usize(),
            expected_start + invalid.len(),
            "{invalid}"
        );
        assert_eq!(&*edit.replacement, replacement, "{invalid}");

        let repaired = SyntaxChangeBatch::new(action)
            .unwrap()
            .lower(EditContext {
                source: candidate.source(),
                okf_analysis: candidate.okf(),
                session_revision: candidate.revision(),
                uml: candidate.uml(),
            })
            .unwrap();
        let repaired_text = repaired
            .document(&BundlePath::parse("class.md").unwrap())
            .unwrap()
            .text();
        assert_eq!(
            repaired_text,
            source.replace(invalid, replacement),
            "{invalid}"
        );
        let reparsed = prepare_candidate(repaired, None, 71).unwrap();
        assert!(
            reparsed
                .uml()
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.message != "invalid multiplicity"
                    && diagnostic.message != "unterminated multiplicity"),
            "{invalid}: {:?}",
            reparsed.uml().diagnostics
        );
        let attribute = &reparsed.uml().projection.node("class").unwrap().attributes[0];
        assert_eq!(
            attribute.multiplicity.as_ref().unwrap().as_str(),
            expected_value,
            "{invalid}"
        );
        assert!(repair_actions(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed)
        )
        .unwrap()
        .into_iter()
        .all(|action| action.title != "Replace invalid multiplicity"));
    }
}

use waml::uml::{declared::expected_for_slot, syntax::UmlSyntaxKind, ExpectedSyntax};

fn sequence(text: &str) -> PreparedCandidate {
    prepare_candidate(
        SourceBundle::try_from_pairs([
            ("class.md", text),
            ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ])
        .unwrap(),
        None,
        5,
    )
    .unwrap()
}

#[test]
fn a_missing_lifeline_alias_is_reported_as_an_alias_not_a_message_target() {
    let candidate = sequence(
        "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as\n",
    );
    let concept = candidate.uml().declared.concept("class").unwrap();
    assert!(
        matches!(
            concept.lifelines[0].alias,
            waml::uml::DeclaredField::Incomplete {
                expected: ExpectedSyntax::Alias,
                ..
            }
        ),
        "expected Alias, got something else"
    );
}

#[test]
fn a_missing_call_id_is_reported_as_a_call_id() {
    let candidate = sequence(
        "---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A calls A `x()` as\n",
    );
    let concept = candidate.uml().declared.concept("class").unwrap();
    assert!(matches!(
        concept.messages[0].call_id,
        waml::uml::DeclaredField::Incomplete {
            expected: ExpectedSyntax::CallId,
            ..
        }
    ));
}

#[test]
fn the_slot_kind_table_is_total_over_the_slots_it_names() {
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::LifelineAlias),
        ExpectedSyntax::Alias
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::InteractionUseAlias),
        ExpectedSyntax::Alias
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageCallId),
        ExpectedSyntax::CallId
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageReturnCall),
        ExpectedSyntax::CallId
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::MessageTarget),
        ExpectedSyntax::MessageTarget
    );
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::FlowTarget),
        ExpectedSyntax::FlowTarget
    );
    // An unlisted slot keeps the historical catch-all rather than panicking:
    // a new slot kind must not turn a half-typed document into a crash.
    assert_eq!(
        expected_for_slot(UmlSyntaxKind::Root),
        ExpectedSyntax::MessageTarget
    );
}
