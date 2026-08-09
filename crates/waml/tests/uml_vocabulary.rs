use waml::uml::vocabulary;

#[test]
fn layout_keywords_are_sorted_unique_and_cover_every_phrase_word() {
    let mut sorted = vocabulary::LAYOUT_KEYWORDS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), vocabulary::LAYOUT_KEYWORDS);

    // Every word of every phrase table is itself a keyword, so the formatter's
    // word-by-word canonicalisation can never lower-case a phrase only halfway.
    for phrase in vocabulary::LAYOUT_HINT_PHRASES
        .iter()
        .chain(vocabulary::LAYOUT_DIRECTION_PHRASES)
    {
        for word in phrase.split(' ') {
            assert!(
                vocabulary::LAYOUT_KEYWORDS.contains(&word),
                "{phrase:?} uses non-keyword {word:?}"
            );
        }
    }
    for word in vocabulary::LAYOUT_EDGE_WORDS
        .iter()
        .chain(vocabulary::LAYOUT_AXIS_WORDS)
        .chain(vocabulary::LAYOUT_DIRECTION_HEADS)
        .chain(vocabulary::LAYOUT_SHAPE_HINTS)
        .chain(vocabulary::LAYOUT_MARGIN_SIZES)
    {
        assert!(
            vocabulary::LAYOUT_KEYWORDS.contains(word),
            "{word:?} is used by the grammar but is not a layout keyword"
        );
    }
}

#[test]
fn canonical_layout_keyword_lowercases_and_folds_margins() {
    assert_eq!(vocabulary::canonical_layout_keyword("ABOVE"), Some("above"));
    assert_eq!(
        vocabulary::canonical_layout_keyword("margins"),
        Some("margin")
    );
    assert_eq!(
        vocabulary::canonical_layout_keyword("Margins"),
        Some("margin")
    );
    assert_eq!(vocabulary::canonical_layout_keyword("Order"), None);
}

#[test]
fn model_derived_vocabularies_list_every_variant_exactly_once() {
    let relationships = vocabulary::relationship_keywords().collect::<Vec<_>>();
    assert_eq!(relationships.len(), 11);
    for keyword in &relationships {
        assert!(
            waml::model::RelationshipKind::parse(keyword).is_some(),
            "{keyword:?} does not round-trip through RelationshipKind::parse"
        );
    }
    // `instance of` must be offered -- and matched -- before any single word,
    // so a two-word kind is never truncated to a one-word prefix.
    assert_eq!(relationships[0], "instance of");

    let flow = vocabulary::flow_node_keywords().collect::<Vec<_>>();
    assert_eq!(flow.len(), 7);
    assert!(!flow.contains(&"plain"), "Plain has no keyword");
    for keyword in &flow {
        assert!(
            waml::model::FlowNodeKind::from_keyword(keyword).is_some(),
            "{keyword}"
        );
    }

    let fragments = vocabulary::fragment_keywords().collect::<Vec<_>>();
    assert_eq!(fragments.len(), 8);
    for keyword in &fragments {
        assert!(
            waml::model::FragmentKind::parse(keyword).is_some(),
            "{keyword}"
        );
    }
}

#[test]
fn message_verbs_are_the_five_the_grammar_accepts() {
    assert_eq!(
        vocabulary::MESSAGE_VERBS,
        ["calls", "returns", "signals", "creates", "destroys"]
    );
}
