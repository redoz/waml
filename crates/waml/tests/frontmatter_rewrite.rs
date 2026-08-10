use waml::frontmatter::replace_frontmatter_string_scalar;

#[test]
fn rewrite_changes_only_an_unquoted_scalar_before_a_comment() {
    let source = "---\n# keep this comment\ntype: Diagram # legacy\nprofile: er\n---\n# Body\n";
    let expected =
        "---\n# keep this comment\ntype: uml.ClassDiagram # legacy\nprofile: er\n---\n# Body\n";

    assert_eq!(
        replace_frontmatter_string_scalar(source, "type", "Diagram", "uml.ClassDiagram")
            .unwrap()
            .as_deref(),
        Some(expected)
    );
}

#[test]
fn rewrite_preserves_single_and_double_quote_styles() {
    for (source, expected) in [
        (
            "---\ntype: 'Diagram'\n---\n",
            "---\ntype: 'uml.UseCaseDiagram'\n---\n",
        ),
        (
            "---\ntype: \"Diagram\"\n---\n",
            "---\ntype: \"uml.UseCaseDiagram\"\n---\n",
        ),
    ] {
        assert_eq!(
            replace_frontmatter_string_scalar(source, "type", "Diagram", "uml.UseCaseDiagram")
                .unwrap()
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn rewrite_preserves_crlf_and_every_surrounding_byte() {
    let source = "---\r\ntitle: Before\r\ntype: uml.Activity # keep\r\ndescription: After\r\n---\r\n# Body\r\nText.\r\n";
    let expected = "---\r\ntitle: Before\r\ntype: uml.ActivityDiagram # keep\r\ndescription: After\r\n---\r\n# Body\r\nText.\r\n";

    assert_eq!(
        replace_frontmatter_string_scalar(source, "type", "uml.Activity", "uml.ActivityDiagram")
            .unwrap()
            .as_deref(),
        Some(expected)
    );
}

#[test]
fn rewrite_returns_none_when_the_key_or_expected_value_does_not_match() {
    let source = "---\ntype: Diagram\n---\n# Body\n";

    assert_eq!(
        replace_frontmatter_string_scalar(source, "kind", "Diagram", "uml.ClassDiagram").unwrap(),
        None
    );
    assert_eq!(
        replace_frontmatter_string_scalar(source, "type", "uml.Activity", "uml.ActivityDiagram")
            .unwrap(),
        None
    );
}

#[test]
fn rewrite_rejects_a_brace_wrapped_malformed_flow_map() {
    let source = "---\ntype: uml.Activity\nsources:\n  - { id: affected-analysis, resource analysis.rs }\n---\n";

    assert!(replace_frontmatter_string_scalar(
        source,
        "type",
        "uml.Activity",
        "uml.ActivityDiagram"
    )
    .is_err());
}
