use waml::analysis::{prepare_candidate, DocumentId, PreparedCandidate, TextSize};
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{completions, ActionContext, CompletionKind};

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
];

fn prepared(text: &str, revision: u64) -> PreparedCandidate {
    let mut pairs = vec![("doc.md", text)];
    pairs.extend(SUPPORT);
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, revision).unwrap()
}

fn document(candidate: &PreparedCandidate) -> DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap()
}

/// Complete `marked` at its `|`, returning `(label, kind)` pairs.
fn labels(marked: &str) -> Vec<(String, CompletionKind)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let candidate = prepared(&text, 3);
    completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap()
    .into_iter()
    .map(|completion| (completion.label.to_string(), completion.kind))
    .collect()
}

fn sequence(body: &str) -> String {
    format!("---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n{body}")
}

#[test]
fn an_empty_verb_slot_offers_every_message_verb() {
    let offered = labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A |\n",
    ));
    let mut words = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Keyword)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    words.sort_unstable();
    assert_eq!(
        words,
        ["calls", "creates", "destroys", "returns", "signals"]
    );
}

#[test]
fn a_partially_typed_verb_is_silence_due_to_a_known_locator_gap() {
    // KNOWN GAP: an unrecognized partial verb like "cr" is not held in a fixed
    // VerbToken slot -- the parser recovers it as a BadToken and leaves
    // VerbToken itself missing at the position just before it, which the
    // locator's rules do not reach. See Task 4 in the plan and
    // `mid_verb_the_cursor_sits_in_the_verb_slot` in uml_completion_locator.rs.
    assert!(labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A cr|\n"
    ))
    .is_empty());
}

#[test]
fn an_empty_bullet_in_messages_offers_message_source_candidates_not_fragment_keywords() {
    // KNOWN GAP: an empty bullet under `## Messages` locates the message
    // parse's missing source, not the fragment-kind alternative -- see
    // `an_empty_bullet_in_messages_locates_the_message_source_not_the_fragment_kind`
    // in uml_completion_locator.rs. `fixed_vocabulary` has no provider for
    // SourceToken yet (a later slice's in_document_refs), so today this is
    // silence, not the fragment keywords.
    assert!(labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- |\n"
    ))
    .is_empty());
}

#[test]
fn a_relationship_kind_slot_offers_every_relationship_keyword() {
    let offered =
        labels("---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Relationships\n\n- | [A](./a.md)\n");
    for keyword in ["associates", "composes", "instance of", "specializes"] {
        assert!(
            offered.iter().any(|(label, _)| label == keyword),
            "{keyword} missing from {offered:?}"
        );
    }
}

#[test]
fn nothing_is_offered_in_prose_or_after_a_complete_construct() {
    assert!(labels(&sequence("Some prose about| the flow.\n")).is_empty());
    assert!(labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B |\n"
    ))
    .is_empty());
}

#[test]
fn an_offset_past_the_end_of_the_document_is_empty_not_an_error() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    assert_eq!(
        completions(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate),
            TextSize::try_from_usize(9_999).unwrap(),
        )
        .unwrap(),
        Vec::new()
    );
}

#[test]
fn an_unknown_document_is_an_error() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    // `candidate`'s catalog has three documents (doc.md, a.md, b.md), so their
    // ids are 0..=2 -- both catalogs assign DocumentId from zero when built
    // with no previous catalog. Three filler documents ahead of "other.md" in
    // this bundle push its id to 3, which is genuinely absent from
    // `candidate`'s catalog rather than coinciding with one of its documents.
    let other = prepare_candidate(
        SourceBundle::try_from_pairs([
            ("filler1.md", "# One\n"),
            ("filler2.md", "# Two\n"),
            ("filler3.md", "# Three\n"),
            ("other.md", "# Other\n"),
        ])
        .unwrap(),
        None,
        3,
    )
    .unwrap();
    let stranger = other
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("other.md").unwrap())
        .unwrap();
    assert!(matches!(
        completions(
            ActionContext::from_prepared(&candidate).unwrap(),
            stranger,
            TextSize::try_from_usize(0).unwrap(),
        ),
        Err(waml::action::ActionError::UnknownDocument { .. })
    ));
}

#[test]
fn a_stale_session_revision_is_an_error_before_any_candidate_is_computed() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    assert!(matches!(
        ActionContext::new(candidate.okf(), candidate.uml(), 4),
        Err(waml::action::ActionError::MismatchedAnalysisRevision { .. })
    ));
}
