//! Export-time search-index asset builders. These sit beside the primitives
//! they constrain (`asset::{encode, decode, bundle_hash}`) so the export
//! boundary rule -- "hash the pairs a *decode* of the shipped envelope
//! yields" -- and its mirror (the editor's boot-time decode) are one rule,
//! reachable by every frontend, not a CLI-only concern.

use crate::source::SourceBundle;

/// Build the export-time search-index asset for `files` (spec: `waml export
/// site`'s static bundle also ships its index). The index is built from
/// exactly the pairs `files` holds -- the same pairs that go into
/// `bundle.waml` -- so search on the exported site can never see more than
/// the export shipped, and `bundle_hash(files)` is what a boot-time decode
/// must reproduce to accept the asset.
pub fn build_search_index_asset(files: &[(String, String)]) -> Result<Vec<u8>, String> {
    let prepared = crate::validate::prepare(files)?;
    let fields = super::extract::extract_bundle(prepared.source(), prepared.okf(), prepared.uml());
    let index = super::MemSearchIndex::build(fields);
    let hash = super::asset::bundle_hash(files);
    Ok(super::asset::encode(&index, hash).into_bytes())
}

/// The index asset for an already-encoded bundle `envelope`, built and hashed
/// over the pairs a DECODE of that envelope yields.
///
/// The export ships the envelope, and the site's boot path hashes the bundle
/// it decoded from it (`SourceBundle::to_pairs`), so hashing the pre-encode
/// pairs instead is only accidentally right: `bundle_hash` covers paths and
/// their order, and the envelope normalizes paths on the way out. A
/// disagreement is invisible at export time and silent at boot (the asset is
/// simply dropped and the index rebuilt locally), so the two sides are
/// derived from the same bytes here rather than trusted to match.
pub fn build_search_index_asset_for_envelope(envelope: &str) -> Result<Vec<u8>, String> {
    let parts = crate::bundle_envelope::split_bundle(envelope)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "encoded bundle is not a WAML bundle envelope".to_string())?;
    let bundle = SourceBundle::try_from_pairs(parts).map_err(|error| error.to_string())?;
    build_search_index_asset(&bundle.to_pairs())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The export ships two artefacts that must agree across the envelope:
    /// `bundle.waml` and its index asset. The wasm boot path hashes the
    /// bundle it DECODED, so hashing the pre-encode files ships an asset the
    /// boot silently rejects (`decode(..).ok()` -> a local rebuild), and the
    /// whole export-time index is inert with no failing signal. Nothing else
    /// crosses that boundary in a test.
    #[test]
    fn the_exported_assets_hash_is_the_one_a_boot_decode_computes() {
        // A path the envelope normalizes on the way out (`\` -> `/`): the
        // hash covers paths, so the two sides only agree if the export
        // hashes what the envelope actually carries.
        let files = vec![(
            "sales\\order.md".to_string(),
            "---\ntype: uml.Class\n---\n# Order\n\nAbout payments.\n".to_string(),
        )];
        let envelope = crate::bundle_envelope::encode_bundle_envelope(&files).unwrap();

        let bytes = build_search_index_asset_for_envelope(&envelope).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        // Exactly what `App`'s boot does with the fetched bundle bytes.
        let parts = crate::bundle_envelope::split_bundle(&envelope)
            .unwrap()
            .unwrap();
        let booted = crate::source::SourceBundle::try_from_pairs(parts).unwrap();
        let expected = super::super::asset::bundle_hash(&booted.to_pairs());

        let index = super::super::asset::decode(&text, expected)
            .expect("the shipped asset must decode against the booted bundle's hash");
        use super::super::SearchIndex;
        assert!(!index
            .query("payments", &super::super::QueryScope::default())
            .is_empty());
    }

    #[test]
    fn build_search_index_asset_carries_the_bundle_hash_and_finds_known_terms() {
        let files = vec![(
            "order.md".to_string(),
            "---\ntype: uml.Class\n---\n# Order\n\nAbout payments.\n".to_string(),
        )];

        let bytes = build_search_index_asset(&files).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let hash = super::super::asset::bundle_hash(&files);
        assert!(
            text.starts_with(&format!(
                "waml-search-index v{} {hash}",
                super::super::asset::FORMAT_VERSION
            )),
            "{text}"
        );

        let index = super::super::asset::decode(&text, hash).unwrap();
        use super::super::SearchIndex;
        assert!(!index
            .query("payments", &super::super::QueryScope::default())
            .is_empty());

        // A different bundle's hash must not decode this asset (spec: the
        // export boundary is hash-checked, not merely versioned).
        let other = vec![("order.md".to_string(), "# Order\n".to_string())];
        let other_hash = super::super::asset::bundle_hash(&other);
        assert!(super::super::asset::decode(&text, other_hash).is_err());
    }
}
