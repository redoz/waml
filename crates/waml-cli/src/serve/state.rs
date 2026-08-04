//! Serve state: the semantic core the routes call into, no HTTP.
//!
//! Owns the loaded bundle's prepared analysis and the revision counter the
//! spec's read table and write routes share. Read half only (Task 3); the
//! two write surfaces (`apply_ops`, `apply_documents`) land in Tasks 4-5.

use std::path::{Path, PathBuf};

use waml::analysis::{prepare_candidate, PreparedCandidate};
use waml::bundle_envelope::encode_bundle_envelope;
use waml::diagnostic::Diagnostic;
use waml::source::SourceBundle;

use crate::commands;
use crate::io;

/// The loaded bundle plus its current prepared analysis and revision.
///
/// `prepare_candidate` runs once at `load`; reads reuse the stored analysis
/// rather than re-parsing per request. Writes (Tasks 4-5) swap in a freshly
/// validated `PreparedCandidate` and bump `revision`.
#[allow(dead_code)] // consumed by Task 6 (routes hold this behind a Mutex)
pub struct ServeState {
    root: PathBuf,
    prepared: PreparedCandidate,
}

impl ServeState {
    /// Read the directory rooted at `root` (bundle-relative paths, exactly
    /// as `run_batch` does) and prepare it once at revision 0.
    #[allow(dead_code)] // consumed by Task 6
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

    #[allow(dead_code)] // consumed by Task 6
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[allow(dead_code)] // consumed by Task 6
    pub fn revision(&self) -> u64 {
        self.prepared.revision()
    }

    /// The bundle envelope body for `GET /api/bundle` — the same codec
    /// `waml <cmd> --stdout` uses, so the editor's `decode_boot_bundle`
    /// stays a shared decoder.
    #[allow(dead_code)] // consumed by Task 6
    pub fn bundle_envelope(&self) -> Result<String, String> {
        encode_bundle_envelope(&self.prepared.source().to_pairs()).map_err(|e| e.to_string())
    }

    /// The `uml::Projection` for `GET /api/model`, ready to serialize.
    #[allow(dead_code)] // consumed by Task 6
    pub fn model(&self) -> &waml::uml::Projection {
        &self.prepared.uml().projection
    }

    /// The validation diagnostics for `GET /api/diagnostics` — the same
    /// shapes `waml check` renders, no display-path remapping (serve
    /// speaks bundle-relative paths directly).
    #[allow(dead_code)] // consumed by Task 6
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        commands::diagnostics(&self.prepared, &std::collections::BTreeMap::new())
    }
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
}
