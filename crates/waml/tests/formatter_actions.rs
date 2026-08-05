use waml::{
    action::SyntaxChangeBatch,
    analysis::{prepare_candidate, DocumentId, PreparedCandidate},
    edit::{EditBatch, EditContext},
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
fn valid_fixtures_keep_exact_canonical_bytes_and_are_idempotent() {
    let fixtures = [
        (
            "enum.md",
            include_str!("fixtures/parser-platform/enum.md"),
            "---\ntype: uml.Enum\ntitle: Status\n---\n\n# Status\n\n## Values\n- Draft\n- Submitted\n",
        ),
        (
            "object.md",
            include_str!("fixtures/parser-platform/object.md"),
            "---\ntype: uml.InstanceSpecification\ntitle: Draft Order\n---\n\n# Draft Order\n\n## Slots\n- id: ORD-1\n- label: cafe\u{301}\n\n## Relationships\n- instance of [Café Order](./class.md)\n",
        ),
        (
            "diagram.md",
            include_str!("fixtures/parser-platform/diagram.md"),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n\n# Orders\n\n## Members\n- [Café Order](./class.md)\n- [Status](./enum.md)\n\n## Layout\n- class right of enum\n",
        ),
        (
            "activity.md",
            include_str!("fixtures/parser-platform/activity.md"),
            "---\ntype: uml.Activity\ntitle: Fulfil\n---\n\n# Fulfil\n\n## Nodes\n\n### Start\n- transitions to Pack\n\n### Pack\n- transitions to Done\n\n### Done\n",
        ),
        (
            "state-machine.md",
            include_str!("fixtures/parser-platform/state-machine.md"),
            "---\ntype: uml.StateMachine\ntitle: Order State\n---\n\n# Order State\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.Sequence\ntitle: Checkout\n---\n# Checkout\n\n## Lifelines\n\n- [Buyer](./object.md)\n- [Order](./class.md)\n\n## Messages\n\n- Buyer calls Order `submit()` as submission\n",
            "---\ntype: uml.Sequence\ntitle: Checkout\n---\n\n# Checkout\n\n## Lifelines\n- [Buyer](./object.md)\n- [Order](./class.md)\n\n## Messages\n- Buyer calls Order `submit()` as submission\n",
        ),
    ];
    for (path, source, expected) in fixtures {
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
        assert_eq!(formatted_text, expected, "{path}: exact canonical bytes");
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

#[test]
fn removed_sequence_spellings_are_not_rewritten_as_canonical_messages() {
    let source = "---\ntype: uml.Sequence\n---\n# Sequence\n\n## Messages\n- a sends b: `old`\n- a replies b: `old`\n- a calls b: `old`\n";
    let candidate = prepared("sequence.md", source, 5);
    let snapshot = candidate
        .uml()
        .syntax
        .document(document(&candidate, "sequence.md"))
        .unwrap();
    let unsupported = snapshot
        .syntax()
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == waml::uml::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm
        })
        .count();
    assert_eq!(unsupported, 3);

    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "sequence.md"),
        )
        .unwrap();
    assert!(action.changes[0].edits.is_empty());
}

#[test]
fn noncanonical_claimed_families_match_exact_canonical_bytes() {
    let fixtures = [
        (
            "class.md",
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n## Relationships\n\n- depends [Target](target.md)\n\n## Attributes\n\n- + id: OrderId {1}\n",
            "---\ntype: uml.Class\ntitle: C\n---\n\n# C\n\n## Attributes\n- + id: OrderId {1}\n\n## Relationships\n- depends [Target](./target.md)\n",
        ),
        (
            "enum.md",
            "---\ntype:   uml.Enum\ntitle:   E\nstereotype: [ valueObject,entity ]\n---\n# Wrong\n\n## Values\n\n- Open\n- Closed\n",
            "---\ntype: uml.Enum\ntitle: E\nstereotype: [valueObject, entity]\n---\n\n# Wrong\n\n## Values\n- Open\n- Closed\n",
        ),
        (
            "object.md",
            "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n# O\n\n## Slots\n\n- state: \"Open\"\n",
            "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n\n# O\n\n## Slots\n- state: \"Open\"\n",
        ),
        (
            "diagram.md",
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\n- Users with FRAME and LARGE margins\n\n## Members\n\n- [User](user.md)\n",
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n\n# D\n\n## Members\n- [User](user.md)\n\n## Layout\n- Users with frame, large margin\n",
        ),
        (
            "activity.md",
            "---\ntype: uml.Activity\ntitle: A\n---\n# A\n\n## Nodes\n\n### Start\n- transitions to Done\n\n### Done\n",
            "---\ntype: uml.Activity\ntitle: A\n---\n\n# A\n\n## Nodes\n\n### Start\n- transitions to Done\n\n### Done\n",
        ),
        (
            "state.md",
            "---\ntype: uml.StateMachine\ntitle: S\n---\n# S\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
            "---\ntype: uml.StateMachine\ntitle: S\n---\n\n# S\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.Sequence\ntitle: Q\n---\n# Q\n\n## Messages\n\n- Buyer  calls Order `submit()`  as submission\n\n## Lifelines\n\n- [Buyer](./buyer.md)\n- [Order](./order.md)\n",
            "---\ntype: uml.Sequence\ntitle: Q\n---\n\n# Q\n\n## Lifelines\n- [Buyer](./buyer.md)\n- [Order](./order.md)\n\n## Messages\n- Buyer calls Order `submit()` as submission\n",
        ),
    ];
    for (path, source, expected) in fixtures {
        let candidate = prepared(path, source, 40);
        assert!(
            candidate
                .uml()
                .syntax
                .document(document(&candidate, path))
                .is_some(),
            "{path} was not claimed"
        );
        assert!(
            candidate
                .uml()
                .syntax
                .document(document(&candidate, path))
                .unwrap()
                .syntax()
                .diagnostics()
                .is_empty(),
            "{path} is not valid for the active parser"
        );
        let formatted = apply(
            &candidate,
            Formatter
                .format(
                    ActionContext::from_prepared(&candidate).unwrap(),
                    document(&candidate, path),
                )
                .unwrap(),
        );
        let actual = formatted
            .document(&BundlePath::parse(path).unwrap())
            .unwrap()
            .text();
        assert_eq!(actual, expected, "{path}: exact canonical bytes");
        let reparsed = prepared(path, actual, 41);
        assert!(
            reparsed
                .uml()
                .syntax
                .document(document(&reparsed, path))
                .unwrap()
                .syntax()
                .diagnostics()
                .is_empty(),
            "{path}: canonical output did not reparse"
        );
        assert!(
            Formatter
                .format(
                    ActionContext::from_prepared(&reparsed).unwrap(),
                    document(&reparsed, path),
                )
                .unwrap()
                .changes[0]
                .edits
                .is_empty(),
            "{path}: formatter is not idempotent"
        );
    }
}

#[test]
fn tolerated_bracket_multiplicity_formats_to_canonical_braces() {
    let source = "---\ntype: uml.Class\n---\n# C\n\n## Attributes\n- count: Number [0..42]\n";
    let candidate = prepared("class.md", source, 50);
    let formatted = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, "class.md"),
            )
            .unwrap(),
    );
    let actual = formatted
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text();
    assert!(actual.contains("- count: Number {0..42}\n"), "{actual}");
    let reparsed = prepared("class.md", actual, 51);
    assert!(reparsed
        .uml()
        .syntax
        .document(document(&reparsed, "class.md"))
        .is_some());
    assert!(Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed, "class.md")
        )
        .unwrap()
        .changes[0]
        .edits
        .is_empty());
}

#[test]
fn formatter_moves_owned_sections_without_covering_unknown_markdown() {
    let source = "---\ntype: uml.Class\n---\n# C\n\n## Relationships\n\n- depends [T](./t.md)\n\n## Operations\n\nRaw 😀 bytes stay.   \n\n## Attributes\n\n- id: String\n";
    let raw = "## Operations\n\nRaw 😀 bytes stay.   \n";
    let raw_start = source.find(raw).unwrap();
    let raw_end = raw_start + raw.len();
    let candidate = prepared("class.md", source, 60);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "class.md"),
        )
        .unwrap();
    for edit in action.changes[0].edits.iter() {
        assert!(
            edit.range.end().to_usize() <= raw_start || raw_end <= edit.range.start().to_usize(),
            "edit {:?} covers protected raw range {raw_start}..{raw_end}",
            edit.range
        );
    }
    let formatted = apply(&candidate, action);
    let actual = formatted
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text();
    assert!(actual.contains(raw));
    assert!(actual.find("## Attributes").unwrap() < actual.find("## Relationships").unwrap());
    assert!(actual.find("## Relationships").unwrap() < actual.find("## Operations").unwrap());
}

#[test]
fn formatter_preserves_unowned_claimed_body_while_formatting_both_sides() {
    let source = "---\ntype:   uml.Class\n---\n# C\n\nBody 😀 stays byte exact.   \n\n## Values\n\n- Ready\n";
    let raw = "Body 😀 stays byte exact.   ";
    let raw_start = source.find(raw).unwrap();
    let raw_end = raw_start + raw.len();
    let candidate = prepared("class.md", source, 61);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "class.md"),
        )
        .unwrap();
    assert!(action.changes[0].edits.iter().all(|edit| {
        edit.range.end().to_usize() <= raw_start || raw_end <= edit.range.start().to_usize()
    }));
    let actual = apply(&candidate, action)
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text()
        .to_owned();
    assert!(actual.contains(raw));
    assert!(actual.contains("type: uml.Class"));
    assert!(actual.contains("## Values\n- Ready"));
}

/// The formatter rewrites frontmatter, so it must speak the same YAML 1.2 core
/// dialect as the reader. A quoted string that looks like a `null`, a number in
/// a non-decimal spelling, or an infinity must survive a format round trip as a
/// string, and nested structure must keep its indentation.
#[test]
fn formatter_keeps_frontmatter_scalars_in_the_readers_dialect() {
    let source = concat!(
        "---\n",
        "type: uml.Class\n",
        "title: \"null\"\n",
        "tilde: \"~\"\n",
        "inf: \".inf\"\n",
        "nan: \".nan\"\n",
        "hex: \"0x10\"\n",
        "octal: \"0o17\"\n",
        "no: NO\n",
        "owner:\n",
        "  name: Ada\n",
        "  team: \"12\"\n",
        "tags: [a, b]\n",
        "---\n",
        "# C\n",
        "\n",
        "## Values\n",
        "- Ready\n",
    );
    let candidate = prepared("class.md", source, 90);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "class.md"),
        )
        .unwrap();
    let formatted = apply(&candidate, action);
    let actual = formatted
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text()
        .to_owned();
    for expected in [
        "title: \"null\"\n",
        "tilde: \"~\"\n",
        "inf: \".inf\"\n",
        "nan: \".nan\"\n",
        "hex: \"0x10\"\n",
        "octal: \"0o17\"\n",
        "no: NO\n",
        "owner:\n  name: Ada\n  team: \"12\"\n",
        "tags: [a, b]\n",
    ] {
        assert!(
            actual.contains(expected),
            "missing {expected:?} in {actual:?}"
        );
    }

    let reparsed = prepared("class.md", &actual, 91);
    let second = Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed, "class.md"),
        )
        .unwrap();
    assert!(second.changes[0].edits.is_empty(), "{second:?}");
}

#[test]
fn formatter_preserves_multiline_crlf_unicode_body_between_owned_regions() {
    let body = "Body 😀 stays.  \r\n\r\n- raw café item  \r\n> quoted `code`\r\n";
    let source = format!(
        "---\r\ntype:   uml.Class\r\n---\r\n# C\r\n\r\n{body}\r\n## Values\r\n\r\n- Ready\r\n"
    );
    let candidate = prepared("class.md", &source, 80);
    let action = Formatter
        .format(
            ActionContext::from_prepared(&candidate).unwrap(),
            document(&candidate, "class.md"),
        )
        .unwrap();
    let body_start = source.find(body).unwrap();
    let body_end = body_start + body.len();
    assert!(action.changes[0].edits.iter().all(|edit| {
        edit.range.end().to_usize() <= body_start || body_end <= edit.range.start().to_usize()
    }));
    let formatted = apply(&candidate, action);
    let actual = formatted
        .document(&BundlePath::parse("class.md").unwrap())
        .unwrap()
        .text();
    assert!(actual.contains(body), "{actual:?}");
    assert!(actual.starts_with("---\ntype: uml.Class\n---\n\n# C\n\n"));
    assert!(actual.ends_with("\n## Values\n- Ready\n"));

    let reparsed = prepared("class.md", actual, 81);
    let second = Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed, "class.md"),
        )
        .unwrap();
    assert!(second.changes[0].edits.is_empty(), "{second:?}");
}
