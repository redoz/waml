use waml::{
    action::SyntaxChangeBatch,
    analysis::{prepare_candidate, DocumentId, PreparedCandidate},
    edit::{EditBatch, EditContext},
    parse::parse_document,
    serialize::serialize_document,
    source::{BundlePath, SourceBundle},
    uml::{ActionContext, Formatter},
};

fn prepared(path: &str, text: &str, revision: u64) -> PreparedCandidate {
    prepare_candidate(
        SourceBundle::try_from_pairs([(path, text)]).unwrap(),
        None,
        revision,
    )
    .unwrap()
}

fn document(candidate: &PreparedCandidate, path: &str) -> DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse(path).unwrap())
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

#[test]
fn valid_fixture_matches_the_legacy_canonical_bytes_and_is_idempotent() {
    let fixtures = [
        ("enum.md", include_str!("fixtures/parser-platform/enum.md")),
        (
            "object.md",
            include_str!("fixtures/parser-platform/object.md"),
        ),
        (
            "diagram.md",
            include_str!("fixtures/parser-platform/diagram.md"),
        ),
        (
            "activity.md",
            include_str!("fixtures/parser-platform/activity.md"),
        ),
        (
            "state-machine.md",
            include_str!("fixtures/parser-platform/state-machine.md"),
        ),
        (
            "sequence.md",
            include_str!("fixtures/parser-platform/sequence.md"),
        ),
    ];
    for (path, source) in fixtures {
        let candidate = prepared(path, source, 17);
        let action = Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, path),
            )
            .unwrap();
        let formatted = apply(&candidate, action);
        let formatted_text = formatted
            .document(&BundlePath::parse(path).unwrap())
            .unwrap()
            .text();
        assert_eq!(
            formatted_text,
            serialize_document(&parse_document(source)),
            "{path}: formatter must retain the frozen broad canonical behavior"
        );

        let second = prepared(path, formatted_text, 18);
        let second_action = Formatter
            .format(
                ActionContext::from_prepared(&second).unwrap(),
                document(&second, path),
            )
            .unwrap();
        assert!(
            second_action.changes[0].edits.is_empty(),
            "{path}: second format"
        );
    }
}

#[test]
fn raw_unknown_markdown_bytes_remain_present_verbatim() {
    let source = include_str!("fixtures/parser-platform/class.md");
    let raw = "## Operations\n\nThis **raw Markdown** is deliberately not semantic.\n\n### Nested heading\n\nTrailing whitespace stays here.   \n";
    assert!(source.contains(raw));
    let candidate = prepared("class.md", source, 19);
    let formatted = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, "class.md"),
            )
            .unwrap(),
    );
    assert!(formatted
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text()
        .contains(raw));
}

#[test]
fn malformed_recovery_and_unclaimed_generic_source_are_not_rewritten() {
    let malformed = include_str!("fixtures/parser-platform/malformed-crlf-unicode.md");
    let candidate = prepared("broken.md", malformed, 3);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "broken.md"),
        )
        .unwrap();
    assert!(action.changes[0].edits.is_empty());

    let generic = include_str!("fixtures/parser-platform/generic.md");
    let generic_candidate = prepared("generic.md", generic, 4);
    let error = Formatter
        .format(
            ActionContext::from_prepared(&generic_candidate).unwrap(),
            document(&generic_candidate, "generic.md"),
        )
        .unwrap_err();
    assert!(matches!(error, waml::uml::FormatError::NotClaimed { .. }));
}
