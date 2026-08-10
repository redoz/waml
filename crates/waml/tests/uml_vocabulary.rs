use waml::analysis::prepare_candidate;
use waml::source::SourceBundle;
use waml::uml::vocabulary;

fn diagnostics_for<I, P, T>(pairs: I) -> Vec<String>
where
    I: IntoIterator<Item = (P, T)>,
    P: Into<String>,
    T: Into<String>,
{
    let bundle = SourceBundle::try_from_pairs(pairs).unwrap();
    prepare_candidate(bundle, None, 1)
        .unwrap()
        .uml()
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn every_layout_hint_phrase_parses_without_a_diagnostic() {
    for phrase in vocabulary::LAYOUT_HINT_PHRASES {
        let text = format!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n### G\n- [A](./a.md)\n\n## Layout\n- A with {phrase}\n"
        );
        let diagnostics = diagnostics_for([
            ("doc.md".to_string(), text),
            (
                "a.md".to_string(),
                "---\ntype: uml.Class\n---\n# A\n".to_string(),
            ),
        ]);
        assert!(
            diagnostics
                .iter()
                .all(|message| !message.contains("layout")),
            "{phrase:?} produced {diagnostics:?}"
        );
    }
}

/// The direction tables are the parser's own vocabulary, not a copy of it: a
/// word added to `LAYOUT_DIRECTION_VERTICALS`/`_LATERALS` (and so to
/// `LAYOUT_DIRECTION_PHRASES`) must parse. A one-way `debug_assert!` inside the
/// parser could not catch a table that grew past the match arms.
#[test]
fn every_layout_direction_phrase_parses_without_a_diagnostic() {
    for phrase in vocabulary::LAYOUT_DIRECTION_PHRASES {
        let text = format!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- A {phrase} B\n"
        );
        let diagnostics = diagnostics_for([
            ("doc.md".to_string(), text),
            (
                "a.md".to_string(),
                "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string(),
            ),
            (
                "b.md".to_string(),
                "---\ntype: uml.Class\ntitle: B\n---\n# B\n".to_string(),
            ),
        ]);
        assert!(
            diagnostics
                .iter()
                .all(|message| !message.contains("layout")),
            "{phrase:?} produced {diagnostics:?}"
        );
    }
}

#[test]
fn every_message_verb_parses_without_a_malformed_message_diagnostic() {
    for verb in vocabulary::MESSAGE_VERBS {
        // `returns` has no direct target of its own -- only an optional
        // `to <target>` -- so it cannot share the `<verb> B` shape the other
        // four verbs take.
        let message_line = if *verb == "returns" {
            "A returns to B".to_string()
        } else {
            format!("A {verb} B")
        };
        let text = format!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- {message_line}\n"
        );
        let diagnostics = diagnostics_for([
            ("doc.md".to_string(), text),
            (
                "a.md".to_string(),
                "---\ntype: uml.Class\n---\n# A\n".to_string(),
            ),
            (
                "b.md".to_string(),
                "---\ntype: uml.Class\n---\n# B\n".to_string(),
            ),
        ]);
        assert!(
            diagnostics
                .iter()
                .all(|message| !message.contains("message")),
            "{verb:?} produced {diagnostics:?}"
        );
    }
}

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
        .chain(vocabulary::LAYOUT_DIRECTION_VERTICALS)
        .chain(vocabulary::LAYOUT_DIRECTION_LATERALS)
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
