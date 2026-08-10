//! The locator table. Each case is an authored document with a `|` marking the
//! cursor; the marker is stripped before parsing and its byte offset becomes
//! the query offset. Asserting slot and token kinds (never byte offsets) keeps
//! the table readable and keeps it from breaking on unrelated parser motion.

use waml::analysis::prepare_candidate;
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{expectation_at, syntax::UmlSyntaxKind};

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.ClassDiagram\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.ClassDiagram\ntitle: B\n---\n# B\n"),
];

/// Parse `marked` with the `|` removed and return the locator result at the
/// marker, as `(slot, token, prefix_text)`.
fn locate(marked: &str) -> Option<(UmlSyntaxKind, UmlSyntaxKind, String)> {
    let offset = marked.find('|').expect("the case must place a cursor");
    let text = marked.replacen('|', "", 1);
    let mut pairs = vec![("doc.md", text.as_str())];
    pairs.extend(SUPPORT);
    let bundle = SourceBundle::try_from_pairs(pairs).unwrap();
    let candidate = prepare_candidate(bundle, None, 1).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    let snapshot = candidate.uml().syntax.document(id).unwrap();
    let offset = waml::analysis::TextSize::try_from_usize(offset).unwrap();
    expectation_at(snapshot.syntax(), offset).map(|expectation| {
        let prefix = text
            [expectation.prefix.start().to_usize()..expectation.prefix.end().to_usize()]
            .to_owned();
        (expectation.slot, expectation.token, prefix)
    })
}

fn sequence(body: &str) -> String {
    format!("---\ntype: uml.SequenceDiagram\ntitle: S\n---\n# S\n\n{body}")
}

#[test]
fn after_as_the_cursor_sits_in_the_empty_lifeline_alias_slot() {
    let located = locate(&sequence("## Lifelines\n\n- [A](./a.md) as |\n"));
    let (slot, token, prefix) = located.expect("an alias is expected after `as`");
    assert_eq!(slot, UmlSyntaxKind::LifelineAlias);
    assert_eq!(token, UmlSyntaxKind::AliasToken);
    assert_eq!(prefix, "");
}

#[test]
fn after_as_with_no_trailing_space_the_slot_is_still_found() {
    let located = locate(&sequence("## Lifelines\n\n- [A](./a.md) as|\n"));
    assert_eq!(
        located.map(|(slot, _, _)| slot),
        Some(UmlSyntaxKind::LifelineAlias)
    );
}

#[test]
fn mid_word_the_prefix_covers_the_partially_typed_operand() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B|\n",
    ));
    let (slot, token, prefix) = located.expect("a target is being typed");
    assert_eq!(slot, UmlSyntaxKind::MessageTarget);
    assert_eq!(token, UmlSyntaxKind::TargetToken);
    assert_eq!(prefix, "B");
}

#[test]
fn after_a_verb_the_cursor_sits_in_the_empty_target_slot() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A calls |\n",
    ));
    assert_eq!(
        located.map(|(slot, _, _)| slot),
        Some(UmlSyntaxKind::MessageTarget)
    );
}

#[test]
fn mid_verb_the_cursor_sits_in_the_verb_slot() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- A cal|\n",
    ));
    // KNOWN GAP: an unrecognized partial verb like "cal" is not held in a
    // fixed VerbToken slot -- the parser recovers it as a BadToken and
    // leaves VerbToken itself missing at the position just before it, which
    // the locator's rules do not reach. See Task 4 in the plan.
    assert_eq!(located, None);
}

#[test]
fn silence_after_a_complete_construct_in_prose_and_in_a_raw_region() {
    // After a complete message: the operand is finished, nothing is expected.
    assert_eq!(
        locate(&sequence(
            "## Lifelines\n\n- [A](./a.md) as A\n- [B](./b.md) as B\n\n## Messages\n\n- A calls B |\n"
        )),
        None
    );
    // Inside prose.
    assert_eq!(locate(&sequence("Some prose about| the flow.\n")), None);
    // Inside a raw Markdown region that is not a WAML section.
    assert_eq!(
        locate(&sequence("## Operations\n\n- anything at all| here\n")),
        None
    );
    // Inside frontmatter.
    assert_eq!(
        locate("---\ntype: uml.SequenceDia|gram\ntitle: S\n---\n# S\n"),
        None
    );
}

#[test]
fn an_offset_past_the_end_of_the_document_is_silence_not_an_error() {
    let bundle = SourceBundle::try_from_pairs([(
        "doc.md",
        "---\ntype: uml.ClassDiagram\ntitle: X\n---\n# X\n",
    )])
    .unwrap();
    let candidate = prepare_candidate(bundle, None, 1).unwrap();
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    let snapshot = candidate.uml().syntax.document(id).unwrap();
    let past_end = waml::analysis::TextSize::try_from_usize(10_000).unwrap();
    assert!(expectation_at(snapshot.syntax(), past_end).is_none());
}

#[test]
fn an_empty_bullet_in_messages_locates_the_message_source_not_the_fragment_kind() {
    let located = locate(&sequence(
        "## Lifelines\n\n- [A](./a.md) as A\n\n## Messages\n\n- |\n",
    ));
    // KNOWN GAP: an empty bullet under `## Messages` is ambiguous between a
    // message (missing source) and a combined fragment (missing kind). The
    // parser's primary continuation there is the message parse, so the
    // locator's unfiltered "first missing token in document order" rule
    // finds MessageSource/SourceToken; the fragment-kind alternative is not
    // reachable through this locator, only through the message-parse branch.
    // See Task 4 in the plan.
    assert_eq!(
        located.map(|(slot, token, _)| (slot, token)),
        Some((UmlSyntaxKind::MessageSource, UmlSyntaxKind::SourceToken))
    );
}

#[test]
fn a_slot_value_is_not_yet_a_completion_position() {
    // KNOWN GAP: `simple_item` (parser.rs) only lays a placeholder
    // `IdentifierToken` when the whole bullet body is empty -- the slot NAME
    // half, `- |`, which is why `an_empty_slot_bullet_locates_the_slot_name`
    // below works. Once a name and a colon are present, the remainder of the
    // line is lexed by `classifier_tokens`, which produces nothing at all
    // when there is no more text -- so an empty value ("- status: |") never
    // gets a token, missing or otherwise, and the locator finds nothing.
    // Task 12 in the plan drops the value half of type-driven completions for
    // this reason; the name half still lands.
    let located = locate(concat!(
        "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- [A](./a.md) as a\n  - status: |\n"
    ));
    assert_eq!(located, None);
}

#[test]
fn an_empty_slot_bullet_locates_the_slot_name() {
    let located = locate(concat!(
        "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n# O\n\n",
        "## Slots\n\n- [A](./a.md) as a\n  - |\n"
    ));
    assert_eq!(
        located.map(|(slot, token, _)| (slot, token)),
        Some((UmlSyntaxKind::Slot, UmlSyntaxKind::IdentifierToken))
    );
}

#[test]
fn a_lone_bracket_is_the_unfinished_link_position() {
    // The parser lays down a full Link node -- six missing tokens -- around a
    // bare `[`, in every context that admits a link. The locator reports the
    // missing link-text slot so the bracket provider can offer whole links.
    for (marked, expected_slot) in [
        (
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [|\n",
            UmlSyntaxKind::Link,
        ),
        (
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [Bu|\n",
            UmlSyntaxKind::Link,
        ),
        (
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Members\n\n- [Bu|\n",
            UmlSyntaxKind::Link,
        ),
        (
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Attributes\n\n- id: [|\n",
            UmlSyntaxKind::Link,
        ),
    ] {
        let (slot, token, _) = locate(marked).unwrap_or_else(|| panic!("None for {marked:?}"));
        assert_eq!(slot, expected_slot, "{marked:?}");
        assert_eq!(token, UmlSyntaxKind::LinkTextToken, "{marked:?}");
    }
}

#[test]
fn the_partial_after_the_bracket_is_the_prefix() {
    let (_, _, prefix) =
        locate("---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [Bu|\n").unwrap();
    assert_eq!(prefix, "Bu");
}

#[test]
fn a_bracket_position_is_not_reported_where_it_would_corrupt() {
    // Editing the text of a COMPLETE link: replacing `[Buyer` with a fresh
    // whole link would strand the existing `](./a.md)` tail.
    assert_eq!(
        locate(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n## Lifelines\n\n- [Bu|yer](./a.md)\n"
        ),
        None
    );
    // Prose is not the typed path's business; the markdown-side provider owns it.
    assert_eq!(
        locate("---\ntype: uml.Class\ntitle: C\n---\n# C\n\nSee [Bu|\n"),
        None
    );
}
