use std::collections::BTreeSet;

use proptest::prelude::*;
use waml::bundle_envelope::{encode_bundle_envelope_with, split_bundle};

fn valid_path() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Za-z0-9 _~%<>é注]{1,12}", 1..4)
        .prop_map(|segments| format!("{}.md", segments.join("/")))
}

fn valid_bundle() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((valid_path(), any::<String>()), 1..8).prop_filter_map(
        "bundle paths must be unique",
        |pairs| {
            let mut seen = BTreeSet::new();
            pairs
                .iter()
                .all(|(path, _)| seen.insert(path.replace('\\', "/")))
                .then_some(pairs)
        },
    )
}

fn non_sentinel_markdown() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>().prop_map(|mut text| {
            if text.starts_with("<!-- waml/") {
                text.insert(0, 'x');
            }
            text
        }),
        any::<String>().prop_map(|body| format!(
            "authored prefix\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\n{body}"
        )),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn arbitrary_valid_bundles_round_trip(parts in valid_bundle(), nonce in any::<u128>()) {
        let mut candidate = nonce;
        let encoded = encode_bundle_envelope_with(&parts, || {
            let current = candidate;
            candidate = candidate.wrapping_add(1);
            Some(current)
        }).unwrap();
        prop_assert_eq!(split_bundle(&encoded).unwrap(), Some(parts));
    }

    #[test]
    fn arbitrary_non_sentinel_markdown_is_never_split(text in non_sentinel_markdown()) {
        let original = text.clone();
        prop_assert_eq!(split_bundle(&text).unwrap(), None);
        prop_assert_eq!(text, original);
    }
}
