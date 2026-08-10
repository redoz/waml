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
    // An empty bullet under `## Messages` locates the message parse's missing
    // source, not the fragment-kind alternative -- see
    // `an_empty_bullet_in_messages_locates_the_message_source_not_the_fragment_kind`
    // in uml_completion_locator.rs. Since Task 8, `in_document_refs` offers the
    // declared lifeline handle there instead of silence.
    let offered = labels(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- |\n",
    ));
    assert!(
        offered
            .iter()
            .any(|(label, kind)| label == "A" && *kind == CompletionKind::Reference),
        "{offered:?}"
    );
    assert!(
        !offered
            .iter()
            .any(|(_, kind)| *kind == CompletionKind::Keyword),
        "no fragment keywords expected at the message source slot: {offered:?}"
    );
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

const SEQUENCE_BODY: &str = concat!(
    "## Lifelines\n\n",
    "- [A](./a.md) as buyer\n",
    "- [B](./b.md) as order\n\n",
    "## Gates\n\n",
    "- inbound\n\n",
    "## Messages\n\n",
);

#[test]
fn a_message_target_offers_every_declared_handle_plus_outside_and_gates() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- buyer calls |\n")));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"buyer"), "{references:?}");
    assert!(references.contains(&"order"), "{references:?}");
    assert!(references.contains(&"outside"), "{references:?}");
    assert!(references.contains(&"@inbound"), "{references:?}");
}

#[test]
fn a_message_source_offers_the_same_handles() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- |\n")));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"buyer"), "{references:?}");
    assert!(references.contains(&"order"), "{references:?}");
}

#[test]
fn a_handle_prefix_narrows_the_offered_handles() {
    let offered = labels(&sequence(&format!("{SEQUENCE_BODY}- buyer calls or|\n")));
    assert_eq!(
        offered
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>(),
        ["order"]
    );
}

#[test]
fn a_use_gate_handle_is_offered_after_an_interaction_use() {
    let offered = labels(&sequence(concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n\n",
        "## Messages\n\n- ref [B](./b.md) as inner\n- buyer calls |\n"
    )));
    assert!(
        offered.iter().any(|(label, _)| label == "inner@"),
        "the use alias must be offered as a gate prefix: {offered:?}"
    );
}

#[test]
fn for_offers_declared_call_ids_and_as_offers_none() {
    let body = concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n- [B](./b.md) as order\n\n",
        "## Messages\n\n- buyer calls order `submit()` as submission\n"
    );
    let referenced = labels(&sequence(&format!(
        "{body}- order returns to buyer for |\n"
    )));
    assert!(
        referenced.iter().any(|(label, _)| label == "submission"),
        "a declared call id must be offered after `for`: {referenced:?}"
    );

    // `as` declares a call id; there is nothing in the document to reference,
    // so no Reference candidate is offered there.
    let declared = labels(&sequence(&format!(
        "{body}- buyer calls order `pay()` as |\n"
    )));
    assert!(
        !declared
            .iter()
            .any(|(label, kind)| label == "submission" && *kind == CompletionKind::Reference),
        "a declaration site must not offer existing ids: {declared:?}"
    );
}

#[test]
fn transitions_to_offers_declared_flow_node_identities() {
    let offered = labels(concat!(
        "---\ntype: uml.Activity\ntitle: F\n---\n# F\n\n",
        "## Nodes\n\n",
        "### Receive\n\n- transitions to Check\n\n",
        "### Check\n\n- transitions to |\n"
    ));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"Receive"), "{references:?}");
    assert!(references.contains(&"Check"), "{references:?}");
}

#[test]
fn bind_to_offers_the_used_interactions_handles() {
    let offered = labels(&sequence(concat!(
        "## Lifelines\n\n- [A](./a.md) as buyer\n\n",
        "## Messages\n\n- ref [B](./b.md) as inner\n  - bind buyer to |\n"
    )));
    // With no analysis of the used document's handles available, the provider
    // offers this document's handles, which is the accept set the diagnostic
    // uses for the local half of a binding.
    assert!(
        offered.iter().any(|(label, _)| label == "buyer"),
        "{offered:?}"
    );
}

#[test]
fn layout_offers_diagram_member_names() {
    let offered = labels(concat!(
        "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
        "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n",
        "## Layout\n\n- A above |\n"
    ));
    let references = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Reference)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(references.contains(&"B"), "{references:?}");
}

#[test]
fn a_link_target_offers_catalog_documents_labelled_by_title() {
    let offered = labels(&sequence("## Lifelines\n\n- [Buyer](|)\n"));
    let links = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Link)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(links.contains(&"A"), "{links:?}");
    assert!(links.contains(&"B"), "{links:?}");
    assert!(
        !links.contains(&"S"),
        "a document must not link to itself: {links:?}"
    );
}

#[test]
fn a_link_target_inserts_a_bundle_relative_path_that_round_trips() {
    let marked = sequence("## Lifelines\n\n- [Buyer](|)\n");
    let offset = marked.find('|').unwrap();
    let text = marked.replacen('|', "", 1);
    let candidate = prepared(&text, 3);
    let offered = completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        document(&candidate),
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap();
    let chosen = offered
        .iter()
        .find(|completion| completion.label.as_ref() == "A")
        .expect("A is offered");
    let mut applied = text.clone();
    applied.replace_range(
        chosen.replace.start().to_usize()..chosen.replace.end().to_usize(),
        &chosen.insert,
    );
    let reanalyzed = prepared(&applied, 4);
    assert!(
        reanalyzed
            .uml()
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != waml::diagnostic::DiagCode::UnresolvedTarget),
        "{:?} in {applied}",
        reanalyzed.uml().diagnostics
    );
}

#[test]
fn a_stale_session_revision_is_an_error_before_any_candidate_is_computed() {
    let candidate = prepared("---\ntype: uml.Class\n---\n# C\n", 3);
    assert!(matches!(
        ActionContext::new(candidate.okf(), candidate.uml(), 4),
        Err(waml::action::ActionError::MismatchedAnalysisRevision { .. })
    ));
}

const TYPED: [(&str, &str); 2] = [
    (
        "order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n\n- status: Status\n- total: Number\n",
    ),
    (
        "status.md",
        "---\ntype: uml.Enum\ntitle: Status\n---\n# Status\n\n## Values\n\n- Draft\n- Placed\n",
    ),
];

fn typed_labels(marked: &str) -> Vec<(String, CompletionKind)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let mut pairs = vec![("doc.md", text.as_str())];
    pairs.extend(TYPED);
    let candidate =
        prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, 6).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    completions(
        ActionContext::from_prepared(&candidate).unwrap(),
        id,
        TextSize::try_from_usize(offset).unwrap(),
    )
    .unwrap()
    .into_iter()
    .map(|completion| (completion.label.to_string(), completion.kind))
    .collect()
}

#[test]
fn an_empty_slot_offers_the_instance_of_classifiers_attributes() {
    // The document is itself the instance -- `## Slots` are its own, and the
    // classifier they're checked against comes from `## Relationships`'
    // `instance of`, mirroring `tests/fixtures/parser-platform/object.md`.
    let offered = typed_labels(concat!(
        "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- |\n\n## Relationships\n\n- instance of [Order](./order.md)\n"
    ));
    let fields = offered
        .iter()
        .filter(|(_, kind)| *kind == CompletionKind::Field)
        .map(|(label, _)| label.as_str())
        .collect::<Vec<_>>();
    assert!(fields.contains(&"status"), "{fields:?}");
    assert!(fields.contains(&"total"), "{fields:?}");
}
