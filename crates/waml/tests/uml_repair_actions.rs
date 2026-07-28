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
