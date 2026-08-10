//! Export-time index asset: a deterministic, dependency-free, line-oriented
//! text encoding of a [`MemSearchIndex`]'s source documents, so a static
//! site export can ship its search index alongside `bundle.waml` and the
//! wasm boot path can load it instead of rebuilding locally.
//!
//! The encoding serializes [`DocumentFields`] (the extracted field entries
//! `MemSearchIndex::build` indexes from), never raw markdown -- decoding
//! feeds the documents straight back through `MemSearchIndex::build`, which
//! is a pure, deterministic function, so encode -> decode always reproduces
//! the exact same query results. The asset is hash-checked against the
//! bundle bytes it was built from (spec boundary rule): a stale or
//! mismatched asset is rejected rather than silently served.
//!
//! Format (`FORMAT_VERSION` 1):
//! ```text
//! waml-search-index v1 <fnv1a-of-bundle-bytes>
//! <document count>
//! D\t<path>\t<has concept 0/1>\t<concept id>\t<title>\t<has kind 0/1>\t<kind>\t<entry count>
//! E\t<group code>\t<text>\tT\t<start>\t<end>\t<line>
//! E\t<group code>\t<text>\tM\t<key>
//! ```
//! Text fields are backslash-escaped (`\\`, `\n`, `\r`, `\t`) so embedded
//! newlines and tabs in prose never break the line-oriented grammar.

use super::extract::{DocumentFields, FieldEntry};
use super::index::MemSearchIndex;
use super::{FieldGroup, HitTarget};

/// Suffix appended to a bundle's file name to name its index asset.
pub const SEARCH_INDEX_SUFFIX: &str = ".search-index";

/// The encoding version this module reads and writes. Bumped whenever the
/// line grammar below changes incompatibly.
pub const FORMAT_VERSION: u32 = 1;

const MAGIC: &str = "waml-search-index";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// `"bundle.waml"` -> `"bundle.waml.search-index"`. Total for any bundle
/// name; the index must track whatever `?bundle=<name>` resolved to, never a
/// fixed name.
pub fn index_file_name(bundle_file: &str) -> String {
    format!("{bundle_file}{SEARCH_INDEX_SUFFIX}")
}

/// Why a decode failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetError {
    /// The asset's `FORMAT_VERSION` does not match what this build reads.
    Version,
    /// The asset is not well-formed (truncated, malformed, or its embedded
    /// hash does not match `expected_bundle_hash`).
    Corrupt,
}

/// FNV-1a over `paths+bytes`: `path`, a NUL separator, `content`, a NUL
/// separator, for every pair in order. Any change to any file's path or
/// bytes changes the hash, so an asset built from a different bundle is
/// always detected.
pub fn bundle_hash(pairs: &[(String, String)]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for (path, content) in pairs {
        hash = fnv1a(path.as_bytes(), hash);
        hash = fnv1a(&[0], hash);
        hash = fnv1a(content.as_bytes(), hash);
        hash = fnv1a(&[0], hash);
    }
    hash
}

fn fnv1a(bytes: &[u8], mut hash: u64) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Encode `index`'s source documents as the line-oriented asset text,
/// stamping `bundle_hash` (from [`bundle_hash`]) into the header line.
pub fn encode(index: &MemSearchIndex, bundle_hash: u64) -> String {
    let docs = index.documents();
    let mut out = String::new();
    out.push_str(MAGIC);
    out.push_str(&format!(" v{FORMAT_VERSION} {bundle_hash}\n"));
    out.push_str(&docs.len().to_string());
    out.push('\n');
    for doc in docs {
        out.push_str("D\t");
        out.push_str(&escape(&doc.path));
        out.push('\t');
        push_option(&mut out, doc.concept_id.as_deref());
        out.push('\t');
        out.push_str(&escape(&doc.title));
        out.push('\t');
        push_option(&mut out, doc.kind.as_deref());
        out.push('\t');
        out.push_str(&doc.entries.len().to_string());
        out.push('\n');
        for entry in &doc.entries {
            encode_entry(&mut out, entry);
        }
    }
    out
}

fn push_option(out: &mut String, value: Option<&str>) {
    match value {
        Some(text) => {
            out.push('1');
            out.push('\t');
            out.push_str(&escape(text));
        }
        None => {
            out.push('0');
            out.push('\t');
        }
    }
}

fn encode_entry(out: &mut String, entry: &FieldEntry) {
    out.push_str("E\t");
    out.push_str(&group_code(entry.group).to_string());
    out.push('\t');
    out.push_str(&escape(&entry.text));
    match &entry.target {
        HitTarget::TextSpan { start, end, line } => {
            out.push_str("\tT\t");
            out.push_str(&start.to_string());
            out.push('\t');
            out.push_str(&end.to_string());
            out.push('\t');
            out.push_str(&line.to_string());
        }
        HitTarget::ModelElement { key } => {
            out.push_str("\tM\t");
            out.push_str(&escape(key));
        }
    }
    out.push('\n');
}

/// Decode `text` into a fresh [`MemSearchIndex`], rejecting a wrong version
/// or a hash that does not match `expected_bundle_hash`. Never panics on
/// truncated or malformed input -- every field read is checked.
pub fn decode(text: &str, expected_bundle_hash: u64) -> Result<MemSearchIndex, AssetError> {
    let mut lines = text.lines();
    let header = lines.next().ok_or(AssetError::Corrupt)?;
    let mut header_fields = header.split(' ');
    if header_fields.next() != Some(MAGIC) {
        return Err(AssetError::Corrupt);
    }
    let version_token = header_fields.next().ok_or(AssetError::Corrupt)?;
    let version_text = version_token.strip_prefix('v').ok_or(AssetError::Corrupt)?;
    let version: u32 = version_text.parse().map_err(|_| AssetError::Corrupt)?;
    if version != FORMAT_VERSION {
        return Err(AssetError::Version);
    }
    let hash_token = header_fields.next().ok_or(AssetError::Corrupt)?;
    if header_fields.next().is_some() {
        return Err(AssetError::Corrupt);
    }
    let hash: u64 = hash_token.parse().map_err(|_| AssetError::Corrupt)?;
    if hash != expected_bundle_hash {
        return Err(AssetError::Corrupt);
    }

    let doc_count: usize = lines
        .next()
        .ok_or(AssetError::Corrupt)?
        .parse()
        .map_err(|_| AssetError::Corrupt)?;

    let mut docs = Vec::with_capacity(doc_count);
    for _ in 0..doc_count {
        docs.push(decode_document(&mut lines)?);
    }

    Ok(MemSearchIndex::build(docs))
}

fn decode_document<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
) -> Result<DocumentFields, AssetError> {
    let line = lines.next().ok_or(AssetError::Corrupt)?;
    let mut fields = line.split('\t');
    if fields.next() != Some("D") {
        return Err(AssetError::Corrupt);
    }
    let path = unescape(fields.next().ok_or(AssetError::Corrupt)?)?;
    let concept_id = pull_option(&mut fields)?;
    let title = unescape(fields.next().ok_or(AssetError::Corrupt)?)?;
    let kind = pull_option(&mut fields)?;
    let entry_count: usize = fields
        .next()
        .ok_or(AssetError::Corrupt)?
        .parse()
        .map_err(|_| AssetError::Corrupt)?;
    if fields.next().is_some() {
        return Err(AssetError::Corrupt);
    }

    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        entries.push(decode_entry(lines)?);
    }

    Ok(DocumentFields {
        path,
        concept_id,
        title,
        kind,
        entries,
    })
}

fn pull_option<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
) -> Result<Option<String>, AssetError> {
    let flag = fields.next().ok_or(AssetError::Corrupt)?;
    let value = fields.next().ok_or(AssetError::Corrupt)?;
    match flag {
        "0" => Ok(None),
        "1" => Ok(Some(unescape(value)?)),
        _ => Err(AssetError::Corrupt),
    }
}

fn decode_entry<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Result<FieldEntry, AssetError> {
    let line = lines.next().ok_or(AssetError::Corrupt)?;
    let mut fields = line.split('\t');
    if fields.next() != Some("E") {
        return Err(AssetError::Corrupt);
    }
    let code: u8 = fields
        .next()
        .ok_or(AssetError::Corrupt)?
        .parse()
        .map_err(|_| AssetError::Corrupt)?;
    let group = group_from_code(code)?;
    let text = unescape(fields.next().ok_or(AssetError::Corrupt)?)?;
    let kind = fields.next().ok_or(AssetError::Corrupt)?;
    let target = match kind {
        "T" => {
            let start = parse_u32(fields.next())?;
            let end = parse_u32(fields.next())?;
            let line = parse_u32(fields.next())?;
            HitTarget::TextSpan { start, end, line }
        }
        "M" => {
            let key = unescape(fields.next().ok_or(AssetError::Corrupt)?)?;
            HitTarget::ModelElement { key }
        }
        _ => return Err(AssetError::Corrupt),
    };
    if fields.next().is_some() {
        return Err(AssetError::Corrupt);
    }
    Ok(FieldEntry {
        group,
        text,
        target,
    })
}

fn parse_u32(field: Option<&str>) -> Result<u32, AssetError> {
    field
        .ok_or(AssetError::Corrupt)?
        .parse()
        .map_err(|_| AssetError::Corrupt)
}

fn group_code(group: FieldGroup) -> u8 {
    match group {
        FieldGroup::Names => 0,
        FieldGroup::Model => 1,
        FieldGroup::Prose => 2,
        FieldGroup::Structure => 3,
    }
}

fn group_from_code(code: u8) -> Result<FieldGroup, AssetError> {
    match code {
        0 => Ok(FieldGroup::Names),
        1 => Ok(FieldGroup::Model),
        2 => Ok(FieldGroup::Prose),
        3 => Ok(FieldGroup::Structure),
        _ => Err(AssetError::Corrupt),
    }
}

/// Backslash-escape `\`, `\n`, `\r`, and `\t` so a text field can never be
/// mistaken for the tab-separated grammar around it.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn unescape(text: &str) -> Result<String, AssetError> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            _ => return Err(AssetError::Corrupt),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{QueryScope, SearchIndex};

    fn text_span(start: u32, end: u32, line: u32) -> HitTarget {
        HitTarget::TextSpan { start, end, line }
    }

    fn model(key: &str) -> HitTarget {
        HitTarget::ModelElement { key: key.into() }
    }

    fn doc(
        path: &str,
        kind: Option<&str>,
        entries: Vec<(FieldGroup, &str, HitTarget)>,
    ) -> DocumentFields {
        DocumentFields {
            path: path.to_string(),
            concept_id: Some(path.to_string()),
            title: path.to_string(),
            kind: kind.map(str::to_string),
            entries: entries
                .into_iter()
                .map(|(group, text, target)| FieldEntry {
                    group,
                    text: text.to_string(),
                    target,
                })
                .collect(),
        }
    }

    fn fixture_index() -> MemSearchIndex {
        MemSearchIndex::build(vec![
            doc(
                "payment.md",
                Some("class"),
                vec![
                    (FieldGroup::Names, "Payment", model("payment")),
                    (
                        FieldGroup::Prose,
                        "A payment holds funds.\nSecond line with a\ttab.",
                        text_span(0, 23, 1),
                    ),
                ],
            ),
            doc(
                "notes.md",
                None,
                vec![(
                    FieldGroup::Prose,
                    "See the payment capture flow for details.",
                    text_span(0, 43, 3),
                )],
            ),
        ])
    }

    fn pairs() -> Vec<(String, String)> {
        vec![
            ("payment.md".to_string(), "# Payment\n".to_string()),
            ("notes.md".to_string(), "# Notes\n".to_string()),
        ]
    }

    #[test]
    fn index_file_name_derives_the_sibling_asset_name() {
        assert_eq!(index_file_name("bundle.waml"), "bundle.waml.search-index");
        assert_eq!(index_file_name("orders.waml"), "orders.waml.search-index");
    }

    #[test]
    fn encode_decode_round_trips_query_results() {
        let index = fixture_index();
        let hash = bundle_hash(&pairs());
        let text = encode(&index, hash);

        let decoded = decode(&text, hash).expect("valid asset decodes");

        for query in ["payment", "capture", "tab"] {
            assert_eq!(
                decoded.query(query, &QueryScope::default()),
                index.query(query, &QueryScope::default()),
                "query {query:?} diverged after round-trip"
            );
        }
    }

    /// `MemSearchIndex` carries no `Debug`/`PartialEq` (spec: nothing outside
    /// `search` should structurally compare a backend), so error assertions
    /// pattern-match the `Result` rather than using `unwrap_err`/`assert_eq!`.
    fn assert_decode_err(text: &str, hash: u64, expected: AssetError) {
        match decode(text, hash) {
            Err(actual) => assert_eq!(actual, expected, "decoding {text:?}"),
            Ok(_) => panic!("expected {expected:?} decoding {text:?}"),
        }
    }

    #[test]
    fn a_wrong_version_is_rejected() {
        let index = fixture_index();
        let hash = bundle_hash(&pairs());
        let text = encode(&index, hash).replacen("v1", "v99", 1);

        assert_decode_err(&text, hash, AssetError::Version);
    }

    #[test]
    fn a_wrong_hash_is_rejected() {
        let index = fixture_index();
        let hash = bundle_hash(&pairs());
        let text = encode(&index, hash);

        assert_decode_err(&text, hash.wrapping_add(1), AssetError::Corrupt);
    }

    #[test]
    fn truncated_text_is_corrupt_never_a_panic() {
        let index = fixture_index();
        let hash = bundle_hash(&pairs());
        let text = encode(&index, hash);

        // `text.len() - 1` alone would only drop the final newline, which
        // `lines()` tolerates (a valid, still-complete last line) -- cut
        // further back so the last entry line is itself incomplete.
        for cut in [0, 5, text.len() / 2, text.len() - 10] {
            let truncated = &text[..cut.min(text.len())];
            assert_decode_err(truncated, hash, AssetError::Corrupt);
        }
        assert_decode_err("", hash, AssetError::Corrupt);
        assert_decode_err("not an index at all", hash, AssetError::Corrupt);
    }

    #[test]
    fn bundle_hash_changes_with_either_path_or_content() {
        let base = vec![("a.md".to_string(), "hello".to_string())];
        let renamed = vec![("b.md".to_string(), "hello".to_string())];
        let edited = vec![("a.md".to_string(), "hello!".to_string())];

        let base_hash = bundle_hash(&base);
        assert_ne!(base_hash, bundle_hash(&renamed));
        assert_ne!(base_hash, bundle_hash(&edited));
        assert_eq!(base_hash, bundle_hash(&base));
    }
}
