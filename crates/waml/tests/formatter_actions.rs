use waml::{
    action::SyntaxChangeBatch,
    analysis::{prepare_candidate, PreparedCandidate},
    edit::{EditBatch, EditContext},
    source::{BundlePath, DocumentId, SourceBundle},
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
            "---\ntype: uml.ClassDiagram\ntitle: Orders\nprofile: uml-domain\n---\n\n# Orders\n\n## Members\n- [Café Order](./class.md)\n- [Status](./enum.md)\n\n## Layout\n- class right of enum\n",
        ),
        (
            "activity.md",
            include_str!("fixtures/parser-platform/activity.md"),
            "---\ntype: uml.ActivityDiagram\ntitle: Fulfil\n---\n\n# Fulfil\n\n## Nodes\n\n### Start\n- transitions to Pack\n\n### Pack\n- transitions to Done\n\n### Done\n",
        ),
        (
            "state-machine.md",
            include_str!("fixtures/parser-platform/state-machine.md"),
            "---\ntype: uml.StateMachineDiagram\ntitle: Order State\n---\n\n# Order State\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.SequenceDiagram\ntitle: Checkout\n---\n# Checkout\n\n## Lifelines\n\n- [Buyer](./object.md)\n- [Order](./class.md)\n\n## Messages\n\n- Buyer calls Order `submit()` as submission\n",
            "---\ntype: uml.SequenceDiagram\ntitle: Checkout\n---\n\n# Checkout\n\n## Lifelines\n- [Buyer](./object.md)\n- [Order](./class.md)\n\n## Messages\n- Buyer calls Order `submit()` as submission\n",
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
fn formatter_uses_inline_trace_for_one_target_and_lines_for_many() {
    let source = "---\ntype: uml.StateMachineDiagram\ntitle: Sign In\n---\n# Sign In\n\n## Nodes\n\n### SignedOut\n- on `password` transitions to SignedIn\n  traces [AUTH-PASSWORD](requirements.md#auth-password)\n- on `oidc` transitions to SignedIn traces [AUTH-OIDC-004](sign-in.md#auth-oidc-004) traces [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)\n\n### SignedIn\n";
    let expected = "---\ntype: uml.StateMachineDiagram\ntitle: Sign In\n---\n\n# Sign In\n\n## Nodes\n\n### SignedOut\n- on `password` transitions to SignedIn traces [AUTH-PASSWORD](./requirements.md#auth-password)\n- on `oidc` transitions to SignedIn\n  traces [AUTH-OIDC-004](./sign-in.md#auth-oidc-004)\n  traces [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)\n\n### SignedIn\n";
    let candidate = prepared("sign-in.md", source, 21);

    let formatted = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, "sign-in.md"),
            )
            .unwrap(),
    );

    assert_eq!(
        formatted
            .document(&BundlePath::parse("sign-in.md").unwrap())
            .unwrap()
            .text(),
        expected
    );
}

#[test]
fn formatter_does_not_rewrite_orphan_or_recovery_traces() {
    for source in [
        "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### A\n  traces [Orphan](#orphan)\n- transitions to B\n### B\n",
        "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### A\n- transitions to B traces [Broken](\n### B\n",
        "---\ntype: uml.ActivityDiagram\n---\n# Flow\n\n## Nodes\n### A\n- transitions to B\n\n  traces [Orphan](#orphan)\n### B\n",
    ] {
        let candidate = prepared("flow.md", source, 22);
        let action = Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, "flow.md"),
            )
            .unwrap();

        assert!(action.changes[0].edits.is_empty(), "{source}");
    }
}

#[test]
fn removed_sequence_spellings_are_not_rewritten_as_canonical_messages() {
    let source = "---\ntype: uml.SequenceDiagram\n---\n# Sequence\n\n## Messages\n- a sends b: `old`\n- a replies b: `old`\n- a calls b: `old`\n";
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
            "---\ntype: uml.ClassDiagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n\n- Users with FRAME and LARGE margins\n\n## Members\n\n- [User](user.md)\n",
            "---\ntype: uml.ClassDiagram\ntitle: D\nprofile: uml-domain\n---\n\n# D\n\n## Members\n- [User](user.md)\n\n## Layout\n- Users with frame, large margin\n",
        ),
        (
            "activity.md",
            "---\ntype: uml.ActivityDiagram\ntitle: A\n---\n# A\n\n## Nodes\n\n### Start\n- transitions to Done\n\n### Done\n",
            "---\ntype: uml.ActivityDiagram\ntitle: A\n---\n\n# A\n\n## Nodes\n\n### Start\n- transitions to Done\n\n### Done\n",
        ),
        (
            "state.md",
            "---\ntype: uml.StateMachineDiagram\ntitle: S\n---\n# S\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
            "---\ntype: uml.StateMachineDiagram\ntitle: S\n---\n\n# S\n\n## Nodes\n\n### Draft\n- on `submit` transitions to Submitted\n\n### Submitted\n",
        ),
        (
            "sequence.md",
            "---\ntype: uml.SequenceDiagram\ntitle: Q\n---\n# Q\n\n## Messages\n\n- Buyer  calls Order `submit()`  as submission\n\n## Lifelines\n\n- [Buyer](./buyer.md)\n- [Order](./order.md)\n",
            "---\ntype: uml.SequenceDiagram\ntitle: Q\n---\n\n# Q\n\n## Lifelines\n- [Buyer](./buyer.md)\n- [Order](./order.md)\n\n## Messages\n- Buyer calls Order `submit()` as submission\n",
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
fn adjacent_unknown_sections_keep_exact_bytes_and_format_idempotently() {
    let source = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\n",
        "Complete an order without losing cart state.  \n",
        "\n",
        "- Keep the selected items.\n",
        "- Confirm available inventory.\n",
        "\n",
        "## Scenarios\n",
        "Customer starts checkout.\n",
        "\n",
        "1. System validates the cart.\n",
        "2. System confirms the order.\n",
    );
    let expected = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\n",
        "Complete an order without losing cart state.  \n",
        "\n",
        "- Keep the selected items.\n",
        "- Confirm available inventory.\n",
        "\n",
        "## Scenarios\n",
        "Customer starts checkout.\n",
        "\n",
        "1. System validates the cart.\n",
        "2. System confirms the order.\n",
    );

    let candidate = prepared("use-case.md", source, 62);
    let first = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, "use-case.md"),
            )
            .unwrap(),
    );
    let first_text = first
        .document(&BundlePath::parse("use-case.md").unwrap())
        .unwrap()
        .text();
    let reparsed = prepared("use-case.md", first_text, 63);
    let second_action = Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed, "use-case.md"),
        )
        .unwrap();
    let second = apply(&reparsed, second_action.clone());
    let second_text = second
        .document(&BundlePath::parse("use-case.md").unwrap())
        .unwrap()
        .text();

    assert_eq!(
        second_text, first_text,
        "second formatting pass changed bytes"
    );
    assert_eq!(first_text, expected, "exact canonical bytes");
    assert!(second_action.changes[0].edits.is_empty());
    assert!(first_text.contains(
        "## Owning goal\nComplete an order without losing cart state.  \n\n- Keep the selected items.\n- Confirm available inventory.\n"
    ));
    assert!(first_text.contains(
        "## Scenarios\nCustomer starts checkout.\n\n1. System validates the cart.\n2. System confirms the order.\n"
    ));
    assert!(
        first_text.find("## Relationships").unwrap() < first_text.find("## Owning goal").unwrap()
    );
    assert!(first_text.find("## Owning goal").unwrap() < first_text.find("## Scenarios").unwrap());
}

fn assert_passthrough_separator_is_stable(path: &str, source: &str, expected: &str, revision: u64) {
    let candidate = prepared(path, source, revision);
    let first = apply(
        &candidate,
        Formatter
            .format(
                ActionContext::from_prepared(&candidate).unwrap(),
                document(&candidate, path),
            )
            .unwrap(),
    );
    let first_text = first
        .document(&BundlePath::parse(path).unwrap())
        .unwrap()
        .text();
    assert_eq!(first_text, expected, "{path}: exact first-pass bytes");

    let reparsed = prepared(path, first_text, revision + 1);
    let second_action = Formatter
        .format(
            ActionContext::from_prepared(&reparsed).unwrap(),
            document(&reparsed, path),
        )
        .unwrap();
    assert!(
        second_action.changes[0].edits.is_empty(),
        "{path}: second pass produced edits"
    );
    let second = apply(&reparsed, second_action);
    assert_eq!(
        second
            .document(&BundlePath::parse(path).unwrap())
            .unwrap()
            .text(),
        expected,
        "{path}: second-pass bytes"
    );
}

#[test]
fn crlf_passthrough_blank_line_is_not_extended() {
    let source = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\r\n",
        "Keep the raw CRLF bytes.\r\n",
        "\r\n",
        "## Scenarios\n",
        "- Complete checkout.\n",
    );
    let expected = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\r\n",
        "Keep the raw CRLF bytes.\r\n",
        "\r\n",
        "## Scenarios\n",
        "- Complete checkout.\n",
    );

    assert_passthrough_separator_is_stable("crlf-use-case.md", source, expected, 64);
}

#[test]
fn whitespace_only_passthrough_blank_line_is_not_extended() {
    let source = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\n",
        "Keep spaces and tabs on the blank line.\n",
        " \t \n",
        "## Scenarios\n",
        "- Complete checkout.\n",
    );
    let expected = concat!(
        "---\n",
        "type: uml.UseCase\n",
        "---\n",
        "\n",
        "# Complete checkout\n",
        "\n",
        "## Relationships\n",
        "- includes [Checkout](./checkout.md)\n",
        "\n",
        "## Owning goal\n",
        "Keep spaces and tabs on the blank line.\n",
        " \t \n",
        "## Scenarios\n",
        "- Complete checkout.\n",
    );

    assert_passthrough_separator_is_stable("whitespace-use-case.md", source, expected, 66);
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
