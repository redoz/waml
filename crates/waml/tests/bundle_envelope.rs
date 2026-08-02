use waml::bundle_envelope::{encode_bundle_envelope_with, split_bundle, BundleEnvelopeError};

const A: &str = "0000000000000000000000000000000a";
const B: &str = "0000000000000000000000000000000b";

fn marker(nonce: &str, path: &str, eol: &str) -> String {
    format!("<!-- waml/1 part {nonce} {path} -->{eol}")
}

#[test]
fn decodes_one_lf_part_and_two_crlf_parts() {
    let one = format!("{}# One\n", marker(A, "shop/order.md", "\n"));
    assert_eq!(
        split_bundle(&one).unwrap(),
        Some(vec![("shop/order.md".into(), "# One\n".into())])
    );

    let two = format!(
        "{}first{}second",
        marker(A, "shop/one.md", "\r\n"),
        marker(A, "shop/two.md", "\r\n")
    );
    assert_eq!(
        split_bundle(&two).unwrap(),
        Some(vec![
            ("shop/one.md".into(), "first".into()),
            ("shop/two.md".into(), "second".into()),
        ])
    );
}

#[test]
fn plain_markdown_never_loses_its_prefix() {
    for text in [
        "# Before\n<!-- something.md -->\n# After\n",
        "```md\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\n```\n",
        "preamble\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\nbody",
        "<!-- old/path.md -->\nbody",
    ] {
        assert_eq!(split_bundle(text).unwrap(), None, "{text:?}");
    }
}

#[test]
fn different_nonce_marker_is_body_but_matching_marker_splits_at_any_offset() {
    let text = format!(
        "{}left{}kept{}right",
        marker(A, "left.md", "\n"),
        marker(B, "other.md", "\n"),
        marker(A, "right.md", "\n"),
    );
    assert_eq!(
        split_bundle(&text).unwrap(),
        Some(vec![
            (
                "left.md".into(),
                format!("left{}kept", marker(B, "other.md", "\n"))
            ),
            ("right.md".into(), "right".into()),
        ])
    );
}

#[test]
fn byte_zero_waml_sentinel_reports_structured_errors() {
    assert!(matches!(
        split_bundle("<!-- waml/2 part 0000000000000000000000000000000a x.md -->\n"),
        Err(BundleEnvelopeError::UnsupportedVersion { version, offset: 0 }) if version == "2"
    ));
    assert!(matches!(
        split_bundle("<!-- waml/1 part ABC x.md -->\n"),
        Err(BundleEnvelopeError::InvalidNonce { offset: 17 })
    ));
    assert!(matches!(
        split_bundle("<!-- waml/1 part 0000000000000000000000000000000a x.md -->"),
        Err(BundleEnvelopeError::MalformedFirstMarker { offset: 0 })
    ));
    let malformed_later = format!(
        "{}body<!-- waml/1 part {}broken -->\n",
        marker(A, "x.md", "\n"),
        A
    );
    assert!(matches!(
        split_bundle(&malformed_later),
        Err(BundleEnvelopeError::MalformedPartMarker { .. })
    ));
}

#[test]
fn path_codec_accepts_required_escapes_and_rejects_bad_paths() {
    let text = format!(
        "{}a{}b{}c{}d",
        marker(A, "shop/special%20order.md", "\n"),
        marker(A, "shop/%E6%B3%A8%E6%96%87.md", "\n"),
        marker(A, "shop/percent%25.md", "\n"),
        marker(A, "shop/%3Clt%3E.md", "\n"),
    );
    let paths: Vec<_> = split_bundle(&text)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        paths,
        [
            "shop/special order.md",
            "shop/注文.md",
            "shop/percent%.md",
            "shop/<lt>.md",
        ]
    );

    assert!(matches!(
        split_bundle(&marker(A, "bad%2.md", "\n")),
        Err(BundleEnvelopeError::InvalidPercentEscape { .. })
    ));
    assert!(matches!(
        split_bundle(&marker(A, "bad%FF.md", "\n")),
        Err(BundleEnvelopeError::InvalidPathUtf8 { .. })
    ));
    assert!(matches!(
        split_bundle(&marker(A, "../bad.md", "\n")),
        Err(BundleEnvelopeError::InvalidBundlePath { .. })
    ));
}

#[test]
fn duplicate_paths_are_rejected_after_normalization() {
    let text = format!(
        "{}a{}b",
        marker(A, "shop/order.md", "\n"),
        marker(A, "shop%2Forder.md", "\n"),
    );
    assert!(matches!(
        split_bundle(&text),
        Err(BundleEnvelopeError::DuplicatePath { ref path, .. }) if path == "shop/order.md"
    ));
}

#[test]
fn encoder_preserves_empty_and_unterminated_bodies_and_retries_collision() {
    let first_prefix = "<!-- waml/1 part 0000000000000000000000000000000a";
    let parts = vec![
        ("empty.md".to_owned(), String::new()),
        (
            "shop/space order.md".to_owned(),
            format!("{first_prefix} authored"),
        ),
        (
            "shop/注文%.md".to_owned(),
            "tail-without-newline".to_owned(),
        ),
    ];
    let mut nonces = [Some(10_u128), Some(11_u128)].into_iter();
    let encoded = encode_bundle_envelope_with(&parts, || nonces.next().flatten()).unwrap();
    assert!(encoded.starts_with(&marker(B, "empty.md", "\n")));
    assert!(encoded.contains("shop/space%20order.md"));
    assert!(encoded.contains("shop/%E6%B3%A8%E6%96%87%25.md"));
    assert_eq!(split_bundle(&encoded).unwrap(), Some(parts));
}

#[test]
fn encoder_rejects_invalid_input_and_exhausted_nonce_source() {
    assert_eq!(
        encode_bundle_envelope_with(&[], || Some(1)).unwrap_err(),
        BundleEnvelopeError::EmptyBundle
    );
    assert!(matches!(
        encode_bundle_envelope_with(&[("bad.txt".into(), String::new())], || Some(1)),
        Err(BundleEnvelopeError::InvalidBundlePath { .. })
    ));
    assert!(matches!(
        encode_bundle_envelope_with(
            &[
                ("same.md".into(), String::new()),
                ("same.md".into(), String::new())
            ],
            || Some(1)
        ),
        Err(BundleEnvelopeError::DuplicatePath { .. })
    ));
    assert_eq!(
        encode_bundle_envelope_with(
            &[(
                "x.md".into(),
                "<!-- waml/1 part 00000000000000000000000000000001".into()
            )],
            || None
        )
        .unwrap_err(),
        BundleEnvelopeError::NonceSelectionExhausted { attempts: 0 }
    );
}
