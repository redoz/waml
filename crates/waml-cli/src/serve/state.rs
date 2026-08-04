//! Serve state: the semantic core the routes call into, no HTTP.
//!
//! Owns the loaded bundle's prepared analysis and the revision counter the
//! spec's read table and write routes share. Read half only (Task 3); the
//! two write surfaces (`apply_ops`, `apply_documents`) land in Tasks 4-5.

use std::path::{Path, PathBuf};

use waml::analysis::{prepare_candidate, PreparedCandidate, PreviousAnalyses};
use waml::bundle_envelope::encode_bundle_envelope;
use waml::diagnostic::Diagnostic;
use waml::edit::{EditBatch, EditContext};
use waml::source::SourceBundle;

use crate::commands;
use crate::io;
use crate::ops_dto::{to_batch, OpDto};
use crate::serve::paths;

/// The loaded bundle plus its current prepared analysis and revision.
///
/// `prepare_candidate` runs once at `load`; reads reuse the stored analysis
/// rather than re-parsing per request. Writes (Tasks 4-5) swap in a freshly
/// validated `PreparedCandidate` and bump `revision`.
pub struct ServeState {
    root: PathBuf,
    prepared: PreparedCandidate,
}

impl ServeState {
    /// Read the directory rooted at `root` (bundle-relative paths, exactly
    /// as `run_batch` does) and prepare it once at revision 0.
    pub fn load(root: &Path) -> Result<Self, String> {
        let pairs = io::read_bundle_rooted(std::slice::from_ref(&root.to_path_buf()), false)
            .map_err(|e| e.to_string())?;
        let source = SourceBundle::try_from_pairs(pairs).map_err(|e| e.to_string())?;
        let prepared = prepare_candidate(source, None, 0).map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.to_path_buf(),
            prepared,
        })
    }

    #[allow(dead_code)] // consumed by Task 7 (ui.rs's api-only degradation message)
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn revision(&self) -> u64 {
        self.prepared.revision()
    }

    /// The bundle envelope body for `GET /api/bundle` — the same codec
    /// `waml <cmd> --stdout` uses, so the editor's `decode_boot_bundle`
    /// stays a shared decoder.
    pub fn bundle_envelope(&self) -> Result<String, String> {
        encode_bundle_envelope(&self.prepared.source().to_pairs()).map_err(|e| e.to_string())
    }

    /// The `uml::Projection` for `GET /api/model`, ready to serialize.
    pub fn model(&self) -> &waml::uml::Projection {
        &self.prepared.uml().projection
    }

    /// The validation diagnostics for `GET /api/diagnostics` — the same
    /// shapes `waml check` renders, no display-path remapping (serve
    /// speaks bundle-relative paths directly).
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        commands::diagnostics(&self.prepared, &std::collections::BTreeMap::new())
    }

    /// Apply a batch of ops (`POST /api/ops`): revision-checked, all or
    /// nothing, through the exact funnel `run_batch` uses — `to_batch` →
    /// `Batch::lower` under `EditContext` → `prepare_candidate` revalidation.
    /// A candidate that fails revalidation writes nothing. On success every
    /// touched path is confined under `root` before writing, and only the
    /// `(path, markdown)` pairs whose text changed are returned.
    pub fn apply_ops(
        &mut self,
        at: u64,
        dtos: &[OpDto],
    ) -> Result<Vec<(String, String)>, ApplyFailure> {
        let current = self.revision();
        if at != current {
            return Err(ApplyFailure::Stale { current });
        }

        let batch = to_batch(dtos).map_err(ApplyFailure::Edit)?;
        let changed = batch
            .lower(EditContext {
                source: self.prepared.source(),
                okf_analysis: self.prepared.okf(),
                session_revision: self.prepared.revision(),
                uml: self.prepared.uml(),
            })
            .map_err(|error| ApplyFailure::Edit(error.to_string()))?;

        let validated = prepare_candidate(
            changed,
            Some(PreviousAnalyses {
                okf: self.prepared.okf(),
                uml: self.prepared.uml(),
            }),
            current + 1,
        )
        .map_err(|error| ApplyFailure::Invalid(error.to_string()))?;

        let old_pairs = self.prepared.source().to_pairs();
        let new_pairs = validated.source().to_pairs();

        for (path, _) in &new_pairs {
            paths::safe_join(&self.root, path).map_err(ApplyFailure::Confinement)?;
        }

        io::write_back(&self.root, &old_pairs, &new_pairs).map_err(|error| {
            ApplyFailure::Io(format!("could not write bundle changes: {error}"))
        })?;

        let old_map: std::collections::BTreeMap<&str, &str> = old_pairs
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        let changed_pairs: Vec<(String, String)> = new_pairs
            .iter()
            .filter(|(path, text)| old_map.get(path.as_str()) != Some(&text.as_str()))
            .cloned()
            .collect();

        self.prepared = validated;
        Ok(changed_pairs)
    }

    /// Apply a set of baseline-guarded document writes (`POST
    /// /api/documents`): revision-checked, each write's `baseline` checked
    /// against the in-memory document (a mismatch is a conflict, not a
    /// merge), candidate revalidated before anything lands. `baseline: None`
    /// means "this is a new file".
    pub fn apply_documents(
        &mut self,
        at: u64,
        writes: &[DocumentWrite],
    ) -> Result<(), ApplyFailure> {
        let current = self.revision();
        if at != current {
            return Err(ApplyFailure::Stale { current });
        }

        for write in writes {
            paths::safe_join(&self.root, &write.path).map_err(ApplyFailure::Confinement)?;
        }

        let old_pairs = self.prepared.source().to_pairs();
        let old_map: std::collections::BTreeMap<&str, &str> = old_pairs
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();

        for write in writes {
            let existing = old_map.get(write.path.as_str());
            let matches = match (&write.baseline, existing) {
                (None, None) => true,
                (Some(baseline), Some(text)) => baseline == text,
                _ => false,
            };
            if !matches {
                return Err(ApplyFailure::Stale { current });
            }
        }

        let mut new_map: std::collections::BTreeMap<String, String> = old_pairs
            .iter()
            .map(|(p, c)| (p.clone(), c.clone()))
            .collect();
        for write in writes {
            new_map.insert(write.path.clone(), write.desired.clone());
        }

        // A request that leaves every document as it already is (including an
        // empty writes array) is a no-op: land nothing and keep the revision,
        // so other clients are not forced through the 409/reload path.
        let unchanged = new_map.len() == old_map.len()
            && new_map
                .iter()
                .all(|(path, text)| old_map.get(path.as_str()) == Some(&text.as_str()));
        if unchanged {
            return Ok(());
        }

        let new_pairs: Vec<(String, String)> = new_map.into_iter().collect();

        let candidate_source = SourceBundle::try_from_pairs(new_pairs.clone())
            .map_err(|error| ApplyFailure::Invalid(error.to_string()))?;
        let validated = prepare_candidate(
            candidate_source,
            Some(PreviousAnalyses {
                okf: self.prepared.okf(),
                uml: self.prepared.uml(),
            }),
            current + 1,
        )
        .map_err(|error| ApplyFailure::Invalid(error.to_string()))?;

        let new_pairs = validated.source().to_pairs();
        io::write_back(&self.root, &old_pairs, &new_pairs).map_err(|error| {
            ApplyFailure::Io(format!("could not write bundle changes: {error}"))
        })?;

        self.prepared = validated;
        Ok(())
    }
}

/// One document write in a `POST /api/documents` request. `baseline: None`
/// means the caller believes this is a new file; the server's bundle is the
/// authority for the check either way.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentWrite {
    pub path: String,
    pub baseline: Option<String>,
    pub desired: String,
}

/// Why an `apply_ops`/`apply_documents` call did not land.
#[derive(Debug)]
pub enum ApplyFailure {
    /// The caller's `at` revision is behind the server's current one.
    Stale { current: u64 },
    /// The op batch itself could not be built or lowered (bad op index in
    /// the message, matching the CLI's `run apply` error text).
    Edit(String),
    /// The candidate lowered but failed revalidation — nothing was written.
    Invalid(String),
    /// A path in the request escapes the bundle root — rejected client
    /// input, nothing was written.
    Confinement(String),
    /// A filesystem failure while writing the result.
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("order.md"),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn load_reads_the_directory_and_starts_at_revision_zero() {
        let dir = fixture();
        let state = ServeState::load(dir.path()).unwrap();
        assert_eq!(state.revision(), 0);
        assert_eq!(state.root(), dir.path());
    }

    #[test]
    fn the_model_projection_equals_preparing_the_bundle_directly() {
        let dir = fixture();
        let state = ServeState::load(dir.path()).unwrap();
        let from_state = serde_json::to_value(state.model()).unwrap();

        let pairs =
            io::read_bundle_rooted(std::slice::from_ref(&dir.path().to_path_buf()), false).unwrap();
        let source = SourceBundle::try_from_pairs(pairs).unwrap();
        let prepared = prepare_candidate(source, None, 0).unwrap();
        let direct = serde_json::to_value(&prepared.uml().projection).unwrap();

        assert_eq!(from_state, direct);
    }

    #[test]
    fn the_envelope_round_trips_to_the_same_bundle() {
        let dir = fixture();
        let state = ServeState::load(dir.path()).unwrap();
        let envelope = state.bundle_envelope().unwrap();

        let split = waml::bundle_envelope::split_bundle(&envelope)
            .unwrap()
            .expect("envelope should be recognized");
        let reproduced = SourceBundle::try_from_pairs(split).unwrap();
        assert_eq!(reproduced.to_pairs(), state.prepared.source().to_pairs());
    }

    /// `waml-cli` and `waml-editor` share no dependency edge, so their two
    /// `DocumentWrite`/`documents_request` spellings can only be pinned to one
    /// wire shape by literal JSON like this -- the shape
    /// `crates/waml-editor/src/api_save.rs`'s `documents_request` builds.
    #[test]
    fn the_editor_wire_shape_round_trips() {
        let json = r##"{
            "revision": 3,
            "writes": [
                { "path": "order.md", "baseline": "# Order\n", "desired": "# Order (renamed)\n" },
                { "path": "new.md", "baseline": null, "desired": "# New\n" }
            ]
        }"##;
        #[derive(serde::Deserialize)]
        struct Request {
            revision: u64,
            writes: Vec<DocumentWrite>,
        }
        let parsed: Request = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.revision, 3);
        assert_eq!(parsed.writes.len(), 2);
        assert_eq!(parsed.writes[0].path, "order.md");
        assert_eq!(parsed.writes[0].baseline.as_deref(), Some("# Order\n"));
        assert_eq!(parsed.writes[0].desired, "# Order (renamed)\n");
        assert_eq!(parsed.writes[1].path, "new.md");
        assert_eq!(parsed.writes[1].baseline, None);
        assert_eq!(parsed.writes[1].desired, "# New\n");
    }

    fn attr_add(node: &str, name: &str, ty: &str) -> OpDto {
        OpDto::AttrAdd {
            v: 1,
            node: node.to_string(),
            name: name.to_string(),
            ty: ty.to_string(),
            mult: None,
            vis: None,
        }
    }

    #[test]
    fn apply_bumps_the_revision_and_returns_only_changed_files() {
        let dir = fixture();
        std::fs::write(dir.path().join("other.md"), "# Other\n").unwrap();
        let other_before = std::fs::read(dir.path().join("other.md")).unwrap();

        let mut state = ServeState::load(dir.path()).unwrap();
        let changed = state
            .apply_ops(0, &[attr_add("order", "total", "Money")])
            .unwrap();

        assert_eq!(state.revision(), 1);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0, "order.md");
        assert!(changed[0].1.contains("- total: Money"));

        let other_after = std::fs::read(dir.path().join("other.md")).unwrap();
        assert_eq!(other_before, other_after);
    }

    #[test]
    fn an_unformatted_op_result_lands_canonically_formatted() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();
        state
            .apply_ops(0, &[attr_add("order", "total", "Money")])
            .unwrap();

        let written = std::fs::read_to_string(dir.path().join("order.md")).unwrap();
        let reparsed = SourceBundle::try_from_pairs([("order.md", written.as_str())]).unwrap();
        let canonical = prepare_candidate(reparsed, None, 0).unwrap();
        assert_eq!(canonical.source().documents()[0].text(), written);
    }

    #[test]
    fn a_failing_op_writes_nothing_and_names_its_index() {
        let dir = fixture();
        let before = std::fs::read(dir.path().join("order.md")).unwrap();
        let mut state = ServeState::load(dir.path()).unwrap();

        let ops = [
            attr_add("order", "total", "Money"),
            attr_add("missing-node", "x", "String"),
        ];
        let error = match state.apply_ops(0, &ops) {
            Err(ApplyFailure::Edit(message)) => message,
            other => panic!("expected an Edit failure naming the index, got {other:?}"),
        };
        assert!(error.contains('1'), "error should name index 1: {error}");

        assert_eq!(state.revision(), 0);
        let after = std::fs::read(dir.path().join("order.md")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn a_stale_revision_is_rejected_without_writing() {
        let dir = fixture();
        let before = std::fs::read(dir.path().join("order.md")).unwrap();
        let mut state = ServeState::load(dir.path()).unwrap();

        let error = match state.apply_ops(5, &[attr_add("order", "total", "Money")]) {
            Err(ApplyFailure::Stale { current }) => current,
            other => panic!("expected a Stale failure, got {other:?}"),
        };
        assert_eq!(error, 0);
        assert_eq!(state.revision(), 0);
        let after = std::fs::read(dir.path().join("order.md")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn a_directory_that_fails_to_prepare_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("order.md"),
            "<!-- waml/9 part 0000000000000000000000000000000a x.md -->\n",
        )
        .unwrap();
        let err = match ServeState::load(dir.path()) {
            Err(err) => err,
            Ok(_) => panic!("expected load to fail on a malformed envelope"),
        };
        assert!(!err.is_empty());
    }

    #[test]
    fn a_document_write_lands_byte_exactly() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();
        let baseline = std::fs::read_to_string(dir.path().join("order.md")).unwrap();
        let desired = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\nUnformatted   text\n";

        state
            .apply_documents(
                0,
                &[DocumentWrite {
                    path: "order.md".into(),
                    baseline: Some(baseline),
                    desired: desired.to_string(),
                }],
            )
            .unwrap();

        let written = std::fs::read_to_string(dir.path().join("order.md")).unwrap();
        assert_eq!(written, desired);
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn a_write_whose_candidate_fails_validation_writes_nothing() {
        let dir = fixture();
        let before = std::fs::read(dir.path().join("order.md")).unwrap();
        let mut state = ServeState::load(dir.path()).unwrap();

        let error = match state.apply_documents(
            0,
            &[
                DocumentWrite {
                    path: "a\\b.md".into(),
                    baseline: None,
                    desired: "# One\n".to_string(),
                },
                DocumentWrite {
                    path: "a/b.md".into(),
                    baseline: None,
                    desired: "# Two\n".to_string(),
                },
            ],
        ) {
            Err(ApplyFailure::Invalid(message)) => message,
            other => panic!("expected an Invalid failure, got {other:?}"),
        };
        assert!(!error.is_empty());

        assert_eq!(state.revision(), 0);
        let after = std::fs::read(dir.path().join("order.md")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn a_baseline_mismatch_is_a_conflict() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();

        let current = match state.apply_documents(
            0,
            &[DocumentWrite {
                path: "order.md".into(),
                baseline: Some("stale text that does not match".to_string()),
                desired: "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
            }],
        ) {
            Err(ApplyFailure::Stale { current }) => current,
            other => panic!("expected a Stale failure, got {other:?}"),
        };
        assert_eq!(current, 0);
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn a_new_file_write_with_no_baseline_is_accepted() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();
        let desired = "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n";

        state
            .apply_documents(
                0,
                &[DocumentWrite {
                    path: "customer.md".into(),
                    baseline: None,
                    desired: desired.to_string(),
                }],
            )
            .unwrap();

        assert_eq!(state.revision(), 1);
        let written = std::fs::read_to_string(dir.path().join("customer.md")).unwrap();
        assert_eq!(written, desired);
    }

    #[test]
    fn a_confined_path_is_enforced_on_documents_too() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();

        let error = match state.apply_documents(
            0,
            &[DocumentWrite {
                path: "../x.md".into(),
                baseline: None,
                desired: "# Escape\n".to_string(),
            }],
        ) {
            Err(ApplyFailure::Confinement(message)) => message,
            other => panic!("expected a Confinement failure, got {other:?}"),
        };
        assert!(!error.is_empty());
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn a_noop_documents_post_does_not_bump_the_revision() {
        let dir = fixture();
        let mut state = ServeState::load(dir.path()).unwrap();
        let baseline = std::fs::read_to_string(dir.path().join("order.md")).unwrap();

        // An empty writes array is a no-op.
        state.apply_documents(0, &[]).unwrap();
        assert_eq!(state.revision(), 0);

        // A write whose desired text equals the server's text is a no-op too:
        // bumping the revision here would 409 every other client for nothing.
        state
            .apply_documents(
                0,
                &[DocumentWrite {
                    path: "order.md".into(),
                    baseline: Some(baseline.clone()),
                    desired: baseline,
                }],
            )
            .unwrap();
        assert_eq!(state.revision(), 0);
    }
}
