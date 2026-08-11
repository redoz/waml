use serde::Serialize;
use waml::diagnostic::{Diagnostic, Severity};
use waml::source::SourceBundle;

#[derive(Serialize)]
struct DiagDto<'a> {
    severity: &'a str,
    code: &'a str,
    message: &'a str,
    file: &'a str,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<(usize, usize)>,
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn sorted(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    let mut v: Vec<&Diagnostic> = diags.iter().collect();
    v.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    v
}

pub fn render_human(diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return "No problems found.".to_string();
    }
    let mut lines = Vec::new();
    for d in sorted(diags) {
        // `span` is 0-based byte columns within the line; print 1-based
        // columns to match the 1-based `line`.
        let location = match d.span {
            Some((start, end)) => format!("{}:{}:{}-{}", d.file, d.line, start + 1, end + 1),
            None => format!("{}:{}", d.file, d.line),
        };
        lines.push(format!(
            "{location}: {}[{}]: {}",
            severity_str(d.severity),
            d.code.as_str(),
            d.message
        ));
    }
    let errors = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diags.len() - errors;
    lines.push(format!("\n{errors} error(s), {warnings} warning(s)."));
    lines.join("\n")
}

pub fn render_json(diags: &[Diagnostic]) -> String {
    let dtos: Vec<DiagDto> = sorted(diags)
        .into_iter()
        .map(|d| DiagDto {
            severity: severity_str(d.severity),
            code: d.code.as_str(),
            message: &d.message,
            file: &d.file,
            line: d.line,
            span: d.span,
        })
        .collect();
    serde_json::to_string_pretty(&dtos).unwrap_or_else(|_| "[]".to_string())
}

fn diff_lines(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut s = 0;
    while s < al.len() && s < bl.len() && al[s] == bl[s] {
        s += 1;
    }
    let (mut ea, mut eb) = (al.len(), bl.len());
    while ea > s && eb > s && al[ea - 1] == bl[eb - 1] {
        ea -= 1;
        eb -= 1;
    }
    let mut out = String::new();
    for l in &al[s..ea] {
        out.push_str(&format!("-{l}\n"));
    }
    for l in &bl[s..eb] {
        out.push_str(&format!("+{l}\n"));
    }
    out
}

/// Render a human-readable summary of changes between an old and new bundle:
/// `~ path` (changed, with unified-ish added/removed lines), `+ path (new)`,
/// `- path (deleted)`.
pub fn render_diff(old: &[(String, String)], new: &[(String, String)]) -> String {
    use std::collections::BTreeMap;
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let mut out = String::new();
    for (p, c) in &nm {
        match om.get(p) {
            Some(old_c) if old_c == c => {}
            Some(old_c) => {
                out.push_str(&format!("~ {p}\n"));
                out.push_str(&diff_lines(old_c, c));
            }
            None => {
                out.push_str(&format!("+ {p} (new)\n"));
                out.push_str(&diff_lines("", c));
            }
        }
    }
    for p in om.keys() {
        if !nm.contains_key(p) {
            out.push_str(&format!("- {p} (deleted)\n"));
        }
    }
    if out.is_empty() {
        out.push_str("no changes\n");
    }
    out
}

pub fn check_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| d.severity == Severity::Error) {
        1
    } else {
        0
    }
}

/// Build the export-time search-index asset for `files` (spec: `waml export
/// site`'s static bundle also ships its index). The index is built from
/// exactly the pairs `files` holds -- the same pairs that go into
/// `bundle.waml` -- so search on the exported site can never see more than
/// the export shipped, and `bundle_hash(files)` is what a boot-time decode
/// must reproduce to accept the asset.
pub fn build_search_index_asset(files: &[(String, String)]) -> Result<Vec<u8>, String> {
    let prepared = waml::validate::prepare(files)?;
    let fields =
        waml::search::extract::extract_bundle(prepared.source(), prepared.okf(), prepared.uml());
    let index = waml::search::MemSearchIndex::build(fields);
    let hash = waml::search::asset::bundle_hash(files);
    Ok(waml::search::asset::encode(&index, hash).into_bytes())
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
    let parts = waml::bundle_envelope::split_bundle(envelope)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "encoded bundle is not a WAML bundle envelope".to_string())?;
    let bundle = SourceBundle::try_from_pairs(parts).map_err(|error| error.to_string())?;
    build_search_index_asset(&bundle.to_pairs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::diagnostic::DiagCode;

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
        let envelope = waml::bundle_envelope::encode_bundle_envelope(&files).unwrap();

        let bytes = build_search_index_asset_for_envelope(&envelope).unwrap();
        let text = String::from_utf8(bytes).unwrap();

        // Exactly what `App`'s boot does with the fetched bundle bytes.
        let parts = waml::bundle_envelope::split_bundle(&envelope)
            .unwrap()
            .unwrap();
        let booted = waml::source::SourceBundle::try_from_pairs(parts).unwrap();
        let expected = waml::search::asset::bundle_hash(&booted.to_pairs());

        let index = waml::search::asset::decode(&text, expected)
            .expect("the shipped asset must decode against the booted bundle's hash");
        use waml::search::SearchIndex;
        assert!(!index
            .query("payments", &waml::search::QueryScope::default())
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
        let hash = waml::search::asset::bundle_hash(&files);
        assert!(
            text.starts_with(&format!(
                "waml-search-index v{} {hash}",
                waml::search::asset::FORMAT_VERSION
            )),
            "{text}"
        );

        let index = waml::search::asset::decode(&text, hash).unwrap();
        use waml::search::SearchIndex;
        assert!(!index
            .query("payments", &waml::search::QueryScope::default())
            .is_empty());

        // A different bundle's hash must not decode this asset (spec: the
        // export boundary is hash-checked, not merely versioned).
        let other = vec![("order.md".to_string(), "# Order\n".to_string())];
        let other_hash = waml::search::asset::bundle_hash(&other);
        assert!(waml::search::asset::decode(&text, other_hash).is_err());
    }

    fn sample() -> Vec<Diagnostic> {
        vec![
            Diagnostic::new(
                DiagCode::UnresolvedTarget,
                "no doc './ghost.md'",
                "a/order.md",
                8,
            ),
            Diagnostic::warn(
                DiagCode::UnknownType,
                "unknown type 'bpmn.Task'",
                "a/x.md",
                2,
            ),
        ]
    }

    #[test]
    fn human_output_has_file_line_and_code() {
        let out = render_human(&sample());
        assert!(out.contains("a/order.md:8: error[unresolved-target]: no doc './ghost.md'"));
        assert!(out.contains("a/x.md:2: warning[unknown-type]:"));
    }

    #[test]
    fn human_output_includes_the_column_span_when_present() {
        let diags = vec![
            Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 8).with_span((2, 20)),
        ];
        let out = render_human(&diags);
        assert!(
            out.contains("a.md:8:3-21: error[malformed-attribute]: bad"),
            "{out}"
        );
    }

    #[test]
    fn json_output_is_an_array_of_diagnostics() {
        let out = render_json(&sample());
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["code"], "unresolved-target");
        assert_eq!(v[0]["line"], 8);
    }

    #[test]
    fn json_output_includes_span_when_present() {
        let diags = vec![
            Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 8).with_span((2, 20)),
        ];
        let out = render_json(&diags);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["span"][0], 2);
        assert_eq!(v[0]["span"][1], 20);
    }

    #[test]
    fn exit_code_is_one_with_errors_zero_with_only_warnings() {
        assert_eq!(check_exit_code(&sample()), 1);
        let only_warn = vec![Diagnostic::warn(DiagCode::UnknownType, "w", "a.md", 1)];
        assert_eq!(check_exit_code(&only_warn), 0);
        assert_eq!(check_exit_code(&[]), 0);
    }

    #[test]
    fn render_diff_shows_added_changed_deleted() {
        let old = vec![
            ("a.md".to_string(), "x\ny\n".to_string()),
            ("gone.md".to_string(), "z\n".to_string()),
        ];
        let new = vec![
            ("a.md".to_string(), "x\nY\n".to_string()),
            ("new.md".to_string(), "q\n".to_string()),
        ];
        let d = render_diff(&old, &new);
        assert!(d.contains("a.md"));
        assert!(d.contains("-y"));
        assert!(d.contains("+Y"));
        assert!(d.contains("new.md")); // added
        assert!(d.contains("gone.md")); // deleted
    }
}
