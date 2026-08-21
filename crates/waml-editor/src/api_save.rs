//! Pure helpers for `POST /api/documents`, the `waml serve` write route the
//! editor's API save backend transports through (see `app/workspace.rs`).
//!
//! Diffing a `SaveTicket` into the wire shape, and parsing the response
//! bodies the route answers with, are both host-testable; only the
//! `cx.http_request` call that sends the built body is wasm-only.

use std::collections::BTreeMap;
use waml_ops_dto::{ConflictResponse, DocumentWrite, DocumentsRequest, RevisionResponse};

use crate::editor_session::SaveTicket;

// Everything below is consumed only by `app/workspace.rs`'s wasm-only API
// save backend; it is headless-testable and pinned in-file, but a native
// build never calls it above the wasm `cfg` seam.

/// Build the `POST /api/documents` request body for `ticket` at `revision`,
/// diffing its persisted source against its live source the same way
/// `native_save::save_bundle_atomic` plans a native write: a document whose
/// text still equals its baseline is skipped, and a path absent from the
/// baseline bundle is a new file (`baseline: null`).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn documents_request(ticket: &SaveTicket, revision: u64) -> Result<String, String> {
    let baseline = &ticket.snapshot.persisted_source;
    let current = &ticket.snapshot.source;
    let mut baseline_by_path: BTreeMap<&str, &str> = BTreeMap::new();
    for document in baseline.documents() {
        baseline_by_path.insert(document.path().as_str(), document.text());
    }
    let mut writes = Vec::new();
    for document in current.documents() {
        let path = document.path().as_str();
        let desired = document.text();
        let baseline_text = baseline_by_path.get(path).copied();
        if baseline_text == Some(desired) {
            continue;
        }
        writes.push(DocumentWrite {
            path: path.to_string(),
            baseline: baseline_text.map(str::to_string),
            desired: desired.to_string(),
        });
    }
    serde_json::to_string(&DocumentsRequest { revision, writes })
        .map_err(|error| format!("failed to encode the documents request: {error}"))
}

/// Parse the `{ "revision": u64 }` body of a successful `POST /api/documents`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn parse_save_response(body: &[u8]) -> Result<u64, String> {
    let parsed: RevisionResponse = serde_json::from_slice(body)
        .map_err(|error| format!("could not read the save response: {error}"))?;
    Ok(parsed.revision)
}

/// Parse the `{ "error": ..., "current": u64 }` body of a 409 conflict.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn parse_conflict(body: &[u8]) -> Result<u64, String> {
    let parsed: ConflictResponse = serde_json::from_slice(body)
        .map_err(|error| format!("could not read the conflict response: {error}"))?;
    Ok(parsed.current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use waml::edit::ExactSourceEdit;
    use waml::source::SourceBundle;
    use waml_syntax::{TextChange, TextRange, TextSize};

    fn dirty_session_ticket(original: &str, edited: &str) -> SaveTicket {
        let path = waml::source::BundlePath::parse("order.md").unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", original)]).unwrap())
            .unwrap();
        let before = session.snapshot();
        let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
        let syntax = before.markdown_snapshot(document).unwrap();
        session
            .apply(ExactSourceEdit {
                document,
                base_revision: syntax.revision(),
                changes: Arc::from([TextChange {
                    old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                    replacement: Arc::from(edited),
                }]),
                expected_text: waml_syntax::SourceText::new(edited.to_string()).unwrap(),
            })
            .unwrap();
        session.save_ticket().unwrap()
    }

    #[test]
    fn a_dirty_document_becomes_a_baseline_guarded_write() {
        let original = "# Order\n";
        let edited = "# Order (renamed)\n";
        let ticket = dirty_session_ticket(original, edited);

        let body = documents_request(&ticket, 3).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(parsed["revision"], 3);
        let writes = parsed["writes"].as_array().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0]["path"], "order.md");
        assert_eq!(writes[0]["baseline"], original);
        assert_eq!(writes[0]["desired"], edited);
    }

    #[test]
    fn a_new_document_has_no_baseline() {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", "# Order\n")]).unwrap())
            .unwrap();
        // Simulate a persisted bundle that predates `order.md` by swapping the
        // snapshot's own `persisted_source` back to a bundle without it; the
        // live `source` (what `replace` just installed) keeps the new file.
        let snapshot = session.snapshot();
        let ticket = SaveTicket {
            revision: session.revision(),
            history_state: session.history_state(),
            snapshot: Arc::new(snapshot.with_persisted_source(Arc::new(
                SourceBundle::try_from_pairs(std::iter::empty::<(&str, &str)>()).unwrap(),
            ))),
        };

        let body = documents_request(&ticket, 1).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

        let writes = parsed["writes"].as_array().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0]["path"], "order.md");
        assert!(writes[0]["baseline"].is_null());
        assert_eq!(writes[0]["desired"], "# Order\n");
    }

    #[test]
    fn a_clean_ticket_produces_no_writes() {
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", "# Order\n")]).unwrap())
            .unwrap();
        let snapshot = session.snapshot();
        let ticket = SaveTicket {
            revision: session.revision(),
            history_state: session.history_state(),
            snapshot,
        };

        let body = documents_request(&ticket, 0).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert!(parsed["writes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn conflict_and_error_responses_parse() {
        let ok_body = br#"{"revision": 7}"#;
        assert_eq!(parse_save_response(ok_body), Ok(7));

        let conflict_body = br#"{"error": "stale revision", "current": 9}"#;
        assert_eq!(parse_conflict(conflict_body), Ok(9));

        assert!(parse_save_response(b"not json").is_err());
        assert!(parse_conflict(b"not json").is_err());
    }
}
