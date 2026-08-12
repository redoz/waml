use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use waml::host::confine::{self, ConfineError, SymlinkPolicy};
use waml::source::SourceBundle;

use crate::editor_session::{SaveCompletion, SaveTicket};

pub(crate) fn save_bundle_atomic(
    root: &Path,
    baseline: &SourceBundle,
    current: &SourceBundle,
) -> io::Result<()> {
    save_bundle_atomic_with_commit(root, baseline, current, commit_dirty_writes)
}

/// The real commit step, pinned to `DeletePolicy::Refuse`.
///
/// Be precise about what that buys, because it is easy to overstate: the
/// deletion refusal users actually hit is the full-bundle check at the top of
/// [`save_bundle_atomic_with_commit`], which fails before any planning. By the
/// time control reaches here, `old` has been rebuilt from the *pending* set,
/// so every one of its keys is present in `new` by construction and the
/// `Refuse` branch inside `host::persist` cannot fire from this call.
///
/// `Refuse` is passed anyway as a standing declaration of intent, not as an
/// active second check: it is what makes "the editor's save never deletes"
/// true of the shared transaction itself rather than only of this crate's
/// planning code, so any future caller that hands this function a broader
/// `old` set fails loudly instead of quietly deleting a user's file.
/// `commit_dirty_writes_refuses_a_dropped_path` pins that, since no
/// production save path can reach it.
fn commit_dirty_writes(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
) -> io::Result<Vec<String>> {
    waml::host::persist::write_back_with_policy(
        root,
        old,
        new,
        waml::host::persist::DeletePolicy::Refuse,
    )
}

/// `save_bundle_atomic`'s planning logic, parameterized over the commit step
/// so tests can inject filesystem faults into the real `host::persist`
/// transaction through `waml::host::persist::test_support` (Task 12 of the
/// waml-cli-logic-seam plan, Decision 2's fault-injection seam) without
/// duplicating this planning logic.
fn save_bundle_atomic_with_commit(
    root: &Path,
    baseline: &SourceBundle,
    current: &SourceBundle,
    commit: impl FnOnce(&Path, &[(String, String)], &[(String, String)]) -> io::Result<Vec<String>>,
) -> io::Result<()> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("opened bundle root is not a directory: {root:?}"),
        ));
    }

    let baseline_by_path = index_bundle(baseline, "baseline")?;
    let current_by_path = index_bundle(current, "current")?;
    for document in baseline.documents() {
        let relative = Path::new(document.path().as_str());
        if !current_by_path.contains_key(&bundle_path_key(relative)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "removing bundle files is not supported by atomic save: {:?}",
                    relative
                ),
            ));
        }
    }
    let mut resolved_targets = BTreeSet::new();
    let mut validated_targets = Vec::with_capacity(current.len());
    for document in current.documents() {
        let relative = document.path().as_str();
        let target =
            confine::resolve_under(&root, relative, SymlinkPolicy::FollowWithinRoot, false)
                .map_err(|error| confine_error_to_io(relative, error))?;
        let parent = target
            .parent()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "bundle path has no parent")
            })?
            .to_path_buf();
        if !resolved_targets.insert(filesystem_path_key(&target)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bundle contains paths resolving to the same target: {target:?}"),
            ));
        }
        validated_targets.push((target, parent, relative.to_owned()));
    }

    let mut planned = Vec::new();
    // Plan every dirty target using only reads. Clean paths participate in
    // containment and alias validation above, but their contents are not read or
    // rewritten, so an unrelated external edit survives this save.
    for (document, (target, parent, relative_owned)) in
        current.documents().iter().zip(validated_targets)
    {
        let relative = Path::new(document.path().as_str());
        let desired = document.text();
        let baseline = baseline_by_path.get(&bundle_path_key(relative)).copied();
        if baseline == Some(desired) {
            continue;
        }
        if disk_state(&root, &target, baseline, desired)? == DiskState::NeedsWrite {
            planned.push(PlannedWrite {
                relative: relative_owned,
                target,
                parent,
                baseline,
                desired,
            });
        }
    }

    // All lexical, containment, alias, and conflict checks above complete before
    // the first directory or file mutation. Operational failures (permissions,
    // full disks) can still occur while materializing a previously missing path.
    for write in &planned {
        fs::create_dir_all(&write.parent)?;
    }

    // Directory creation can expose a concurrent link swap, so re-check the
    // entire pending set before the first file replacement.
    let mut pending = Vec::with_capacity(planned.len());
    for mut write in planned {
        // Adopt the re-resolved target: if a parent was swapped for an in-root
        // symlink since planning, the path this save will actually touch is the
        // resolved one, so screening the stale planning-pass target would read a
        // different file than the transaction writes.
        write.target = recheck_confinement(&root, &write.relative)?;
        if disk_state(&root, &write.target, write.baseline, write.desired)? == DiskState::NeedsWrite
        {
            pending.push(write);
        }
    }

    // The disk_state re-screen above is the last read before mutation; what
    // used to be a third, per-write re-check immediately before each
    // individual replacement is retired in favour of one journaled
    // transaction (Task 12 of the waml-cli-logic-seam plan, Decision 3). The
    // window this widens -- an external edit landing after the screen above
    // but before this transaction's rename -- is bounded by the transaction's
    // duration and is accepted in exchange for atomic all-or-nothing commit:
    // a mid-save failure now rolls every write back instead of leaving a
    // half-written bundle. `write_back_with_policy`'s own target resolution
    // (inside its staging pass, right before any rename) still re-confines
    // every path through `confine::resolve_under`, so a concurrent link swap
    // is still caught -- only the *content* re-check is what disappears.
    if !pending.is_empty() {
        let mut old_pairs = Vec::with_capacity(pending.len());
        let mut new_pairs = Vec::with_capacity(pending.len());
        for write in &pending {
            if let Some(baseline) = write.baseline {
                old_pairs.push((write.relative.clone(), baseline.to_owned()));
            }
            new_pairs.push((write.relative.clone(), write.desired.to_owned()));
        }
        commit(&root, &old_pairs, &new_pairs)?;
    }

    Ok(())
}

/// Re-resolve `relative` under `root` after directory creation, returning the
/// path the save will actually touch. Callers must use the returned target
/// rather than one resolved at plan time: an in-root link swap between the two
/// passes changes where the write lands.
fn recheck_confinement(root: &Path, relative: &str) -> io::Result<PathBuf> {
    confine::resolve_under(root, relative, SymlinkPolicy::FollowWithinRoot, false)
        .map_err(|error| confine_error_to_io(relative, error))
}

pub(crate) fn save_ticket_atomic(root: &Path, ticket: &SaveTicket) -> io::Result<SaveCompletion> {
    if ticket.snapshot.dirty_revision != Some(ticket.revision)
        || ticket.snapshot.revision != ticket.revision
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to save a clean or mismatched editor ticket",
        ));
    }
    let result = save_bundle_atomic(
        root,
        &ticket.snapshot.persisted_source,
        &ticket.snapshot.source,
    )
    .map_err(|error| error.to_string());
    Ok(SaveCompletion {
        revision: ticket.revision,
        history_state: ticket.history_state,
        result,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiskState {
    AlreadyDesired,
    NeedsWrite,
}

struct PlannedWrite<'a> {
    relative: String,
    target: PathBuf,
    parent: PathBuf,
    baseline: Option<&'a str>,
    desired: &'a str,
}

fn index_bundle<'a>(
    bundle: &'a SourceBundle,
    bundle_name: &str,
) -> io::Result<BTreeMap<String, &'a str>> {
    let mut indexed = BTreeMap::new();
    for document in bundle.documents() {
        let relative = document.path().as_str();
        // The one syntactic rule, shared with `resolve_under` below and with
        // every other bundle-path writer (Task 11 of the waml-cli-logic-seam
        // plan). This screen exists because `index_bundle` also sees the
        // *baseline* bundle, which `resolve_under` never resolves; keeping a
        // second, local spelling of the rule here would be a defense layer
        // that silently disagrees with the layer above it.
        confine::check_rel(relative).map_err(|error| confine_error_to_io(relative, error))?;
        if indexed
            .insert(bundle_path_key(Path::new(relative)), document.text())
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{bundle_name} bundle contains duplicate path: {relative:?}"),
            ));
        }
    }
    Ok(indexed)
}

fn bundle_path_key(path: &Path) -> String {
    let key = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    normalize_path_key(key)
}

fn filesystem_path_key(path: &Path) -> String {
    normalize_path_key(path.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn normalize_path_key(key: String) -> String {
    key.replace('\\', "/").to_lowercase()
}

#[cfg(not(windows))]
fn normalize_path_key(key: String) -> String {
    key
}

/// Map a [`ConfineError`] from `waml::host::confine::resolve_under` (shared
/// with `host::persist` and `serve/paths.rs`, Task 11 of the
/// `waml-cli-logic-seam` plan) to the editor's existing all-`InvalidInput`
/// save-error contract; no test here distinguishes error kinds beyond that.
fn confine_error_to_io(relative: &str, error: ConfineError) -> io::Error {
    let message = match &error {
        ConfineError::Syntactic(message) => message.clone(),
        ConfineError::SymlinkRefused(path) | ConfineError::EscapesRoot(path) => {
            format!("bundle path resolves outside opened directory: {path:?}")
        }
        ConfineError::NotADirectory(path) => {
            format!("bundle path parent is not a directory: {path:?}")
        }
        ConfineError::NotAFile(path) => format!("bundle target is not a file: {path:?}"),
        ConfineError::Io(io_error) => {
            return io::Error::new(io_error.kind(), io_error.to_string());
        }
    };
    let _ = relative;
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn ensure_within_root(root: &Path, canonical: &Path, original: &Path) -> io::Result<()> {
    if canonical == root || canonical.starts_with(root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bundle path resolves outside opened directory: {original:?}"),
        ))
    }
}

fn disk_state(
    root: &Path,
    target: &Path,
    baseline: Option<&str>,
    desired: &str,
) -> io::Result<DiskState> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            let canonical_target = target.canonicalize()?;
            ensure_within_root(root, &canonical_target, target)?;
            let disk = fs::read(target)?;
            if disk == desired.as_bytes() {
                Ok(DiskState::AlreadyDesired)
            } else if baseline.is_some_and(|baseline| disk == baseline.as_bytes()) {
                Ok(DiskState::NeedsWrite)
            } else {
                Err(save_conflict(target, "changed on disk since it was loaded"))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if baseline.is_none() {
                Ok(DiskState::NeedsWrite)
            } else {
                Err(save_conflict(target, "was removed after it was loaded"))
            }
        }
        Err(error) => Err(error),
    }
}

fn save_conflict(target: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("save conflict for {target:?}: {reason}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        commit_dirty_writes, recheck_confinement, save_bundle_atomic as save_source_bundle_atomic,
        save_bundle_atomic_with_commit, save_ticket_atomic,
    };
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicU32, AtomicUsize, Ordering},
        Arc,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use waml::host::persist::{test_support, DeletePolicy, FsOps};
    use waml::source::SourceBundle;
    use waml_syntax::{SourceText, TextChange, TextRange, TextSize};

    fn save_bundle_atomic(
        root: &Path,
        baseline: &[(String, String)],
        current: &[(String, String)],
    ) -> std::io::Result<()> {
        let invalid_source = |error: waml::source::SourceError| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
        };
        let baseline =
            SourceBundle::try_from_pairs(baseline.iter().cloned()).map_err(invalid_source)?;
        let current =
            SourceBundle::try_from_pairs(current.iter().cloned()).map_err(invalid_source)?;
        save_source_bundle_atomic(root, &baseline, &current)
    }

    /// Runs the real planning logic of `save_bundle_atomic` but routes the
    /// commit step through `waml::host::persist::test_support`, injecting
    /// `ops` into the same `host::persist` transaction the production save
    /// path uses (Task 12 of the waml-cli-logic-seam plan, Decision 2's
    /// fault-injection seam) instead of duplicating the planning logic.
    fn save_bundle_atomic_with_ops(
        root: &Path,
        baseline: &[(String, String)],
        current: &[(String, String)],
        ops: &impl FsOps,
    ) -> std::io::Result<()> {
        let invalid_source = |error: waml::source::SourceError| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
        };
        let baseline =
            SourceBundle::try_from_pairs(baseline.iter().cloned()).map_err(invalid_source)?;
        let current =
            SourceBundle::try_from_pairs(current.iter().cloned()).map_err(invalid_source)?;
        save_bundle_atomic_with_commit(root, &baseline, &current, |root, old, new| {
            test_support::write_back_with_ops_and_policy(root, old, new, DeletePolicy::Refuse, ops)
        })
    }

    struct FailRenamesAt {
        at: &'static [usize],
        calls: AtomicUsize,
    }

    impl FsOps for FailRenamesAt {
        fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            if self.at.contains(&call) {
                Err(std::io::Error::other(format!(
                    "injected rename failure at call {call}"
                )))
            } else {
                std::fs::rename(from, to)
            }
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "waml-native-save-{}-{nanos}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// New as of the shared `host::confine` module (Task 11 of the
    /// waml-cli-logic-seam plan): before, `con.md` passed this validation and
    /// hit whatever platform-dependent behaviour writing to a Windows
    /// console device produces. It now fails loudly, before any mutation.
    #[test]
    fn rejects_a_reserved_device_name() {
        let temp = TempDir::new();

        let error =
            save_bundle_atomic(temp.path(), &[], &[("con.md".into(), "new".into())]).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("con.md").exists());
    }

    #[test]
    fn existing_file_is_replaced_with_bundle_contents() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "old").unwrap();

        save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "old".into())],
            &[("diagram.md".into(), "new authored display".into())],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(target).unwrap(),
            "new authored display"
        );
    }

    #[test]
    fn nested_bundle_paths_are_created() {
        let temp = TempDir::new();

        save_bundle_atomic(
            temp.path(),
            &[],
            &[("nested/diagram.md".into(), "authored".into())],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(temp.path().join("nested/diagram.md")).unwrap(),
            "authored"
        );
    }

    /// `index_bundle` screens *both* bundles, while `resolve_under` only sees
    /// the current one, so the local screen must be the same rule -- not a
    /// weaker second spelling of it. These shapes (reserved device name, NUL
    /// byte) survive `BundlePath::parse` and were accepted by the retired
    /// local checker while `confine::check_rel` rejects them.
    #[test]
    fn index_bundle_applies_the_shared_syntactic_rule() {
        use super::index_bundle;

        for path in ["con.md", "nested/nul.md", "a\0b.md"] {
            let bundle =
                SourceBundle::try_from_pairs([(path.to_string(), "authored".to_string())]).unwrap();
            let error = index_bundle(&bundle, "current").unwrap_err();
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::InvalidInput,
                "expected {path:?} to be rejected"
            );
        }

        let accepted =
            SourceBundle::try_from_pairs([("nested/abc.md".to_string(), "authored".to_string())])
                .unwrap();
        assert!(index_bundle(&accepted, "current").is_ok());
    }

    #[test]
    fn parent_traversal_is_rejected_without_writing_outside_root() {
        let temp = TempDir::new();
        let escape_name = format!("waml-native-save-escape-{}.md", std::process::id());
        let outside = temp.path().parent().unwrap().join(&escape_name);
        let _ = std::fs::remove_file(&outside);

        let error = save_bundle_atomic(
            temp.path(),
            &[],
            &[(format!("../{escape_name}"), "escape".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.exists());
    }

    #[test]
    fn external_edit_to_unchanged_file_survives_dirty_save() {
        let temp = TempDir::new();
        let target = temp.path().join("unchanged.md");
        std::fs::write(&target, "loaded").unwrap();
        std::fs::write(temp.path().join("dirty.md"), "before").unwrap();
        let baseline = vec![
            ("unchanged.md".into(), "loaded".into()),
            ("dirty.md".into(), "before".into()),
        ];
        std::fs::write(&target, "external edit").unwrap();

        save_bundle_atomic(
            temp.path(),
            &baseline,
            &[
                ("unchanged.md".into(), "loaded".into()),
                ("dirty.md".into(), "after".into()),
            ],
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(target).unwrap(), "external edit");
        assert_eq!(
            std::fs::read_to_string(temp.path().join("dirty.md")).unwrap(),
            "after"
        );
    }

    #[test]
    fn external_edit_to_dirty_file_is_a_conflict_and_is_not_overwritten() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "loaded").unwrap();
        std::fs::write(&target, "external edit").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "loaded".into())],
            &[("diagram.md".into(), "editor edit".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "external edit");
    }

    #[test]
    fn already_desired_file_makes_failed_batch_retry_idempotent() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "editor edit").unwrap();
        let identity_before = file_identity(&target);
        std::thread::sleep(std::time::Duration::from_millis(20));

        save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "loaded".into())],
            &[("diagram.md".into(), "editor edit".into())],
        )
        .unwrap();

        assert_eq!(file_identity(&target), identity_before);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "editor edit");
    }

    #[test]
    fn duplicate_current_paths_are_rejected_before_writing() {
        let temp = TempDir::new();

        let error = save_bundle_atomic(
            temp.path(),
            &[],
            &[
                ("duplicate.md".into(), "first".into()),
                ("duplicate.md".into(), "second".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("duplicate.md").exists());
    }

    #[test]
    fn duplicate_baseline_paths_are_rejected_before_writing() {
        let temp = TempDir::new();

        let error = save_bundle_atomic(
            temp.path(),
            &[
                ("duplicate.md".into(), "first".into()),
                ("duplicate.md".into(), "second".into()),
            ],
            &[("safe/new.md".into(), "new".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("safe").exists());
    }

    #[test]
    fn clean_session_has_no_save_ticket() {
        let temp = TempDir::new();
        let session = crate::editor_session::EditorSession::default();

        assert!(session.save_ticket().is_none());
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[test]
    fn invalid_source_ticket_is_saved_byte_exactly() {
        let temp = TempDir::new();
        let path = waml::source::BundlePath::parse("order.md").unwrap();
        let original = "# Order\r\n";
        let invalid = "# Order\r\n[unterminated **\u{2028}";
        std::fs::write(temp.path().join("order.md"), original.as_bytes()).unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", original)]).unwrap())
            .unwrap();
        let before = session.snapshot();
        let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
        let syntax = before.markdown_snapshot(document).unwrap();
        session
            .apply(waml::edit::ExactSourceEdit {
                document,
                base_revision: syntax.revision(),
                changes: Arc::from([TextChange {
                    old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                    replacement: Arc::from(invalid),
                }]),
                expected_text: SourceText::new(invalid.to_string()).unwrap(),
            })
            .unwrap();

        let ticket = session.save_ticket().unwrap();
        let completion = save_ticket_atomic(temp.path(), &ticket).unwrap();

        assert_eq!(completion.revision, ticket.revision);
        assert_eq!(completion.history_state, ticket.history_state);
        assert_eq!(completion.result, Ok(()));
        assert_eq!(
            std::fs::read(temp.path().join("order.md")).unwrap(),
            invalid.as_bytes()
        );
    }

    /// The user-facing refusal of a dropped path is
    /// `removed_bundle_path_is_rejected_before_earlier_mutation` below, which
    /// never reaches the transaction. This pins the other half of the
    /// guarantee: the commit step itself hands `host::persist`
    /// `DeletePolicy::Refuse`, so the shared transaction would refuse rather
    /// than delete if it were ever handed an `old` set with a dropped key.
    /// Swapping the policy for `Transact` (or calling the 3-arg `write_back`)
    /// deletes `gone.md` here instead of erroring.
    #[test]
    fn commit_dirty_writes_refuses_a_dropped_path() {
        let temp = TempDir::new();
        std::fs::write(temp.path().join("gone.md"), "loaded").unwrap();

        let error = commit_dirty_writes(
            temp.path(),
            &[("gone.md".into(), "loaded".into())],
            &[("kept.md".into(), "new".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "removing bundle files is not supported by atomic save"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("gone.md")).unwrap(),
            "loaded"
        );
        assert!(!temp.path().join("kept.md").exists());
    }

    #[test]
    fn removed_bundle_path_is_rejected_before_earlier_mutation() {
        let temp = TempDir::new();
        let removed = temp.path().join("removed.md");
        std::fs::write(&removed, "loaded").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("removed.md".into(), "loaded".into())],
            &[("safe/new.md".into(), "new".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("safe").exists());
        assert_eq!(std::fs::read_to_string(removed).unwrap(), "loaded");
    }

    #[test]
    fn later_conflict_does_not_create_earlier_safe_parent() {
        let temp = TempDir::new();
        std::fs::write(temp.path().join("conflict.md"), "external edit").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("conflict.md".into(), "loaded".into())],
            &[
                ("safe/new.md".into(), "new".into()),
                ("conflict.md".into(), "editor edit".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!temp.path().join("safe").exists());
        assert_eq!(
            std::fs::read_to_string(temp.path().join("conflict.md")).unwrap(),
            "external edit"
        );
    }

    #[test]
    fn linked_parent_escape_is_rejected_before_creating_missing_descendants() {
        let root = TempDir::new();
        let outside = TempDir::new();
        if !make_dir_link(outside.path(), &root.path().join("linked")) {
            return;
        }

        let error = save_bundle_atomic(
            root.path(),
            &[],
            &[("linked/missing/diagram.md".into(), "escape".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.path().join("missing").exists());
    }

    #[test]
    fn later_unsafe_target_does_not_create_earlier_safe_parent() {
        let root = TempDir::new();
        let outside = TempDir::new();
        if !make_dir_link(outside.path(), &root.path().join("linked")) {
            return;
        }

        let error = save_bundle_atomic(
            root.path(),
            &[],
            &[
                ("safe/new.md".into(), "new".into()),
                ("linked/missing/diagram.md".into(), "escape".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!root.path().join("safe").exists());
        assert!(!outside.path().join("missing").exists());
    }

    #[test]
    fn the_pre_transaction_recheck_returns_the_resolved_target() {
        let root = TempDir::new();
        let real = root.path().join("real");
        std::fs::create_dir(&real).unwrap();
        if !make_dir_link(&real, &root.path().join("linked")) {
            return;
        }

        // The conflict screen runs against this path, so it must be the file the
        // transaction will actually replace -- resolved through the in-root link,
        // not the lexical join the planning pass started from.
        let resolved = recheck_confinement(root.path(), "linked/diagram.md").unwrap();

        assert_eq!(
            resolved,
            std::fs::canonicalize(&real).unwrap().join("diagram.md")
        );
    }

    #[test]
    fn clean_and_dirty_aliases_are_rejected_before_mutating_shared_target() {
        let root = TempDir::new();
        let real = root.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let target = real.join("diagram.md");
        std::fs::write(&target, "loaded").unwrap();
        if !make_dir_link(&real, &root.path().join("linked")) {
            return;
        }

        let baseline = vec![
            ("linked/diagram.md".into(), "loaded".into()),
            ("real/diagram.md".into(), "loaded".into()),
        ];
        let current = vec![
            ("linked/diagram.md".into(), "editor edit".into()),
            ("real/diagram.md".into(), "loaded".into()),
        ];

        let error = save_bundle_atomic(root.path(), &baseline, &current).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "loaded");
    }

    /// Task 12 of the waml-cli-logic-seam plan: `save_bundle_atomic` no
    /// longer writes file-by-file -- it commits every dirty file through one
    /// `host::persist` transaction. A failure partway through that
    /// transaction must roll back everything already written, not just leave
    /// a half-saved bundle.
    #[test]
    fn mid_save_rename_failure_on_second_dirty_file_rolls_first_back_to_baseline() {
        let temp = TempDir::new();
        std::fs::write(temp.path().join("a.md"), "before-a").unwrap();
        std::fs::write(temp.path().join("b.md"), "before-b").unwrap();
        let baseline = vec![
            ("a.md".into(), "before-a".into()),
            ("b.md".into(), "before-b".into()),
        ];
        let current = vec![
            ("a.md".into(), "after-a".into()),
            ("b.md".into(), "after-b".into()),
        ];

        // Call 1-2 write a.md (backup rename, then content rename); call 3 is
        // the first rename touching b.md (its backup rename) -- failing there
        // means b.md is never touched at all, and only a.md's completed write
        // needs rolling back.
        let fault = FailRenamesAt {
            at: &[3],
            calls: AtomicUsize::new(0),
        };
        let error =
            save_bundle_atomic_with_ops(temp.path(), &baseline, &current, &fault).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("injected rename failure at call 3"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("a.md")).unwrap(),
            "before-a"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("b.md")).unwrap(),
            "before-b"
        );
    }

    /// Mirrors `host::persist`'s own
    /// `rollback_failure_retains_a_reported_recovery_journal` test, but
    /// through `waml-editor`'s real save path, to prove the fault-injection
    /// seam (Decision 2) actually wires into the transaction `save_bundle_atomic`
    /// uses in production.
    #[test]
    fn rollback_failure_reports_the_retained_recovery_journal_path() {
        let temp = TempDir::new();
        std::fs::write(temp.path().join("a.md"), "before-a").unwrap();
        std::fs::write(temp.path().join("b.md"), "before-b").unwrap();
        let baseline = vec![
            ("a.md".into(), "before-a".into()),
            ("b.md".into(), "before-b".into()),
        ];
        let current = vec![
            ("a.md".into(), "after-a".into()),
            ("b.md".into(), "after-b".into()),
        ];

        // Call 4 is b.md's content rename -- failing it triggers rollback of
        // both journaled entries. Call 8 is a.md's backup-restore during that
        // rollback -- failing it too makes the rollback itself fail, which
        // must surface the recovery journal's path instead of silently
        // losing it.
        let fault = FailRenamesAt {
            at: &[4, 8],
            calls: AtomicUsize::new(0),
        };
        let error =
            save_bundle_atomic_with_ops(temp.path(), &baseline, &current, &fault).unwrap_err();

        assert!(
            error.to_string().contains(
                "failed to write b.md: injected rename failure at call 4; rollback failed:"
            ),
            "{error}"
        );
        assert!(
            error.to_string().contains("recovery journal retained at "),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("b.md")).unwrap(),
            "before-b"
        );
        let journal = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".waml-cli-"))
            })
            .expect("failed rollback must retain its recovery journal");
        assert!(journal.exists());
    }

    #[cfg(windows)]
    fn file_identity(path: &Path) -> u64 {
        use std::os::windows::fs::MetadataExt;

        std::fs::metadata(path).unwrap().creation_time()
    }

    #[cfg(unix)]
    fn file_identity(path: &Path) -> u64 {
        use std::os::unix::fs::MetadataExt;

        std::fs::metadata(path).unwrap().ino()
    }

    #[cfg(windows)]
    fn make_dir_link(target: &Path, link: &Path) -> bool {
        match std::os::windows::fs::symlink_dir(target, link) {
            Ok(()) => true,
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.kind() == std::io::ErrorKind::Unsupported
                    || error.raw_os_error() == Some(1314) =>
            {
                false
            }
            Err(error) => panic!("failed to create directory link: {error}"),
        }
    }

    #[cfg(unix)]
    fn make_dir_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).unwrap();
        true
    }
}
