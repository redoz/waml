//! The staged/journaled/rollback transaction that writes a bundle's changes
//! back to disk. Every write and delete is staged into a per-transaction
//! directory first; only once every staged entry is ready are the real
//! targets swapped in via atomic renames, with a journal recording each swap
//! so a mid-transaction failure can be rolled back to the prior on-disk
//! state. Filesystem access is abstracted behind [`FsOps`] so the fault paths
//! (a rename failing partway through, cleanup failing after a successful
//! commit) can be exercised without depending on real I/O failures.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::confine::{self, ConfineError, SymlinkPolicy};

/// Whether a transaction may delete entries present in `old` but absent from
/// `new`. `Transact` (the default, via [`write_back`]) deletes them as part
/// of the same transaction. `Refuse` fails the whole transaction before any
/// staging happens if `old` contains a path `new` does not -- the guarantee
/// `waml-editor`'s atomic save relies on, with its existing refusal message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletePolicy {
    Refuse,
    Transact,
}

/// Write only changed/added entries; delete entries dropped from the bundle.
/// Returns a human list of what happened.
pub fn write_back(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
) -> std::io::Result<Vec<String>> {
    write_back_with_policy(root, old, new, DeletePolicy::Transact)
}

/// Like [`write_back`], but with an explicit [`DeletePolicy`]. See
/// `DeletePolicy::Refuse` for the guarantee `waml-editor`'s atomic save
/// relies on.
pub fn write_back_with_policy(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
    policy: DeletePolicy,
) -> std::io::Result<Vec<String>> {
    write_back_with_ops(root, old, new, policy, &RealFs)
}

/// Fault-injection seam for an in-tree frontend that supplies its own
/// [`FsOps`] -- today the `waml` CLI binary, whose `WAML_CLI_TEST_FAIL_*`
/// end-to-end backdoor lives in `waml-cli`'s `io::write_back` and is gated
/// there on `debug_assertions`.
///
/// That backdoor deliberately does NOT live in this library. `write_back*` is
/// also the native editor's save path and `serve`'s write path, and both are
/// routinely built with debug assertions on (`run.ps1`), so an env var
/// exported for a CLI test would otherwise silently sabotage an editor save.
/// A library takes its fault injection as an argument; it does not read the
/// process environment.
///
/// The `test_support` module below carries the same seam for consumers of the
/// `test-support` feature (`waml-editor`'s dev-dependency); this one is
/// unconditional because `waml-cli`'s *binary* needs it, and a binary cannot
/// rely on dev-dependency feature unification to compile.
#[doc(hidden)]
pub fn write_back_injecting_ops(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
    policy: DeletePolicy,
    ops: &impl FsOps,
) -> std::io::Result<Vec<String>> {
    write_back_with_ops(root, old, new, policy, ops)
}

#[doc(hidden)]
pub trait FsOps {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;

    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
        fs::remove_dir_all(path)
    }
}

struct RealFs;

impl FsOps for RealFs {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }
}

enum JournalEntry {
    Updated { target: PathBuf, backup: PathBuf },
    Added { target: PathBuf },
    Deleted { target: PathBuf, backup: PathBuf },
}

struct StagedWrite {
    logical: String,
    target: PathBuf,
    desired: PathBuf,
    backup: PathBuf,
    existed: bool,
}

struct StagedDelete {
    logical: String,
    target: PathBuf,
    backup: PathBuf,
}

fn write_back_with_ops(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
    policy: DeletePolicy,
    ops: &impl FsOps,
) -> std::io::Result<Vec<String>> {
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    if policy == DeletePolicy::Refuse && om.keys().any(|logical| !nm.contains_key(logical)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "removing bundle files is not supported by atomic save",
        ));
    }
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bundle root is not a directory: {}", root.display()),
        ));
    }
    let staging = create_staging_directory(&root)?;
    let staged = (|| {
        let mut writes = Vec::new();
        let mut deletes = Vec::new();
        let mut nonce = 0usize;
        let mut touched = Vec::new();
        let mut targets = std::collections::BTreeSet::new();

        let mut reserve = |logical: &str| -> std::io::Result<PathBuf> {
            let target =
                confine::resolve_under(&root, logical, SymlinkPolicy::FollowWithinRoot, false)
                    .map_err(|error| confine_error_to_io(logical, error))?;
            // `FollowWithinRoot` dereferences the *final* component too, so an
            // existing in-root file symlink resolves to its link target and
            // would be silently overwritten (or deleted) there. This writer
            // never followed a leaf link: the target was the raw
            // `root.join(rel)`, and the `symlink_metadata(..).is_file()` screen
            // below refused a symlink with "bundle target is not a file". Keep
            // refusing -- `serve`'s `/api/documents` reaches this with a
            // client-supplied path. Parent components are still followed, as
            // they always were (`symlink_metadata` resolves everything but the
            // last component).
            let unresolved = root.join(logical);
            if fs::symlink_metadata(&unresolved)
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("bundle target is not a file: {}", unresolved.display()),
                ));
            }
            // Two logical paths that collide on one filesystem entry would
            // make the transaction overwrite its own work. Fold case only
            // where the platform's filesystems are conventionally
            // case-insensitive (Windows, macOS); on Linux `A.md` and `a.md`
            // are genuinely distinct files and must both be writable.
            #[cfg(any(windows, target_os = "macos"))]
            let key = target.to_string_lossy().to_lowercase();
            #[cfg(not(any(windows, target_os = "macos")))]
            let key = target.to_string_lossy().into_owned();
            if !targets.insert(key) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("duplicate filesystem target: {logical}"),
                ));
            }
            Ok(target)
        };

        for (logical, contents) in &nm {
            if om.get(logical) == Some(contents) {
                continue;
            }
            let target = reserve(logical)?;
            let metadata = match fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.is_file() => Some(metadata),
                Ok(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("bundle target is not a file: {}", target.display()),
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            let desired = staging.join(format!("desired-{nonce}"));
            let backup = staging.join(format!("backup-{nonce}"));
            nonce += 1;
            fs::write(&desired, contents.as_bytes())?;
            if let Some(metadata) = &metadata {
                fs::set_permissions(&desired, metadata.permissions())?;
            }
            writes.push(StagedWrite {
                logical: (*logical).to_owned(),
                target,
                desired,
                backup,
                existed: metadata.is_some(),
            });
            touched.push(format!("wrote {logical}"));
        }
        for logical in om.keys() {
            if nm.contains_key(logical) {
                continue;
            }
            let target = reserve(logical)?;
            let metadata = fs::symlink_metadata(&target)?;
            if !metadata.is_file() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("bundle target is not a file: {}", target.display()),
                ));
            }
            deletes.push(StagedDelete {
                logical: (*logical).to_owned(),
                target,
                backup: staging.join(format!("backup-{nonce}")),
            });
            nonce += 1;
            touched.push(format!("deleted {logical}"));
        }
        Ok((writes, deletes, touched))
    })();
    let (writes, deletes, touched) = match staged {
        Ok(staged) => staged,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    let mut journal = Vec::new();
    let mut created_directories = Vec::new();
    let result = (|| {
        for write in writes {
            create_missing_parents(&root, &write.target, &mut created_directories)?;
            if write.existed {
                // Re-screen at commit: the staging pass checked this target,
                // but a concurrent writer may have swapped it for a symlink or
                // directory since (S-4 TOCTOU). Renames never follow symlinks,
                // so the residual race after this check cannot escape the
                // root; it can only displace whatever now sits at the target.
                ensure_regular_file(&write.target)?;
                ops.rename(&write.target, &write.backup)?;
                journal.push(JournalEntry::Updated {
                    target: write.target.clone(),
                    backup: write.backup,
                });
            }
            if let Err(error) = ops.rename(&write.desired, &write.target) {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("failed to write {}: {error}", write.logical),
                ));
            }
            if !write.existed {
                journal.push(JournalEntry::Added {
                    target: write.target,
                });
            }
        }
        for delete in deletes {
            // Same commit-time re-screen as updates (S-4).
            ensure_regular_file(&delete.target)?;
            if let Err(error) = ops.rename(&delete.target, &delete.backup) {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("failed to delete {}: {error}", delete.logical),
                ));
            }
            journal.push(JournalEntry::Deleted {
                target: delete.target,
                backup: delete.backup,
            });
        }
        Ok(())
    })();

    if let Err(error) = result {
        let rollback = rollback(&journal, &created_directories, &staging, ops);
        return match rollback {
            Ok(()) => match fs::remove_dir_all(&staging) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(std::io::Error::new(
                    error.kind(),
                    format!(
                        "{error}; rollback restored targets but cleanup failed: {cleanup}; \
                         recovery journal retained at {}",
                        staging.display()
                    ),
                )),
            },
            Err(rollback) => Err(std::io::Error::new(
                error.kind(),
                format!(
                    "{error}; rollback failed: {rollback}; recovery journal retained at {}",
                    staging.display()
                ),
            )),
        };
    }
    // Every target is committed at this point; failing to sweep up the
    // staging directory must not be reported as a failed write. Surface it as
    // a warning alongside the successful entries instead.
    let mut touched = touched;
    if let Err(cleanup) = ops.remove_dir_all(&staging) {
        touched.push(format!(
            "warning: all changes committed, but removing the staging directory {} failed: {cleanup}",
            staging.display()
        ));
    }
    Ok(touched)
}

fn ensure_regular_file(target: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(target)?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bundle target is not a file: {}", target.display()),
        ))
    }
}

fn create_staging_directory(root: &Path) -> std::io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
        let staging = root.join(format!(".waml-cli-{}-{nonce}.txn", std::process::id()));
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

/// Map a [`ConfineError`] from `confine::resolve_under` to `persist`'s
/// existing `InvalidInput` contract. The syntactic message text is a named
/// pin (`write_back_rejects_ntfs_alternate_data_stream_paths` asserts it
/// verbatim) -- kept exactly, regardless of which syntactic rule inside
/// `check_rel` actually fired, since that test predates the shared checker
/// and pins persist's own wording, not `confine`'s.
fn confine_error_to_io(logical: &str, error: ConfineError) -> std::io::Error {
    match error {
        ConfineError::Syntactic(_) => std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bundle path must be relative and traversal-free: {logical}"),
        ),
        ConfineError::SymlinkRefused(target) | ConfineError::EscapesRoot(target) => {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("bundle path resolves outside root: {}", target.display()),
            )
        }
        ConfineError::NotADirectory(target) => std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "bundle path parent is not a directory: {}",
                target.display()
            ),
        ),
        ConfineError::NotAFile(target) => std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bundle target is not a file: {}", target.display()),
        ),
        ConfineError::Io(io_error) => io_error,
    }
}

fn create_missing_parents(
    root: &Path,
    target: &Path,
    created: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    let parent = target.parent().expect("validated target has a parent");
    let mut missing = Vec::new();
    let mut cursor = parent;
    while cursor != root && !cursor.exists() {
        missing.push(cursor.to_path_buf());
        cursor = cursor.parent().expect("target is within root");
    }
    for directory in missing.into_iter().rev() {
        fs::create_dir(&directory)?;
        created.push(directory);
    }
    Ok(())
}

fn rollback(
    journal: &[JournalEntry],
    created: &[PathBuf],
    staging: &Path,
    ops: &impl FsOps,
) -> std::io::Result<()> {
    let mut trash = Vec::new();
    for (index, entry) in journal.iter().enumerate().rev() {
        match entry {
            JournalEntry::Updated { target, backup } => {
                let displaced = staging.join(format!("rollback-{index}"));
                let displaced = match ops.rename(target, &displaced) {
                    Ok(()) => Some(displaced),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => {
                        return Err(rollback_error(
                            "displace updated target",
                            target,
                            &displaced,
                            error,
                        ));
                    }
                };
                if let Err(restore) = ops.rename(backup, target) {
                    let Some(displaced) = displaced.as_ref() else {
                        return Err(rollback_error("restore backup", backup, target, restore));
                    };
                    let reinstall = ops.rename(displaced, target);
                    return Err(match reinstall {
                        Ok(()) => rollback_error("restore backup", backup, target, restore),
                        Err(reinstall) => std::io::Error::new(
                            restore.kind(),
                            format!(
                                "{}; reinstalling displaced target also failed: {}",
                                rollback_error("restore backup", backup, target, restore),
                                rollback_error(
                                    "reinstall displaced target",
                                    displaced,
                                    target,
                                    reinstall
                                )
                            ),
                        ),
                    });
                }
                if let Some(displaced) = displaced {
                    trash.push(displaced);
                }
            }
            JournalEntry::Added { target } => {
                let displaced = staging.join(format!("rollback-{index}"));
                ops.rename(target, &displaced).map_err(|error| {
                    rollback_error("displace added target", target, &displaced, error)
                })?;
                trash.push(displaced);
            }
            JournalEntry::Deleted { target, backup } => {
                ops.rename(backup, target).map_err(|error| {
                    rollback_error("restore deleted target", backup, target, error)
                })?;
            }
        }
    }
    for directory in created.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!(
                        "failed to remove transaction-created directory {}: {error}",
                        directory.display()
                    ),
                ));
            }
        }
    }
    for displaced in trash {
        let metadata = fs::metadata(&displaced).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "failed to inspect displaced rollback target {}: {error}",
                    displaced.display()
                ),
            )
        })?;
        clear_deletion_blocking_permissions(&displaced, &metadata)?;
        fs::remove_file(&displaced).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "failed to remove displaced rollback target {}: {error}",
                    displaced.display()
                ),
            )
        })?;
    }
    Ok(())
}

#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn clear_deletion_blocking_permissions(
    path: &Path,
    metadata: &fs::Metadata,
) -> std::io::Result<()> {
    if metadata.permissions().readonly() {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!(
                    "failed to clear deletion-blocking permissions on {}: {error}",
                    path.display()
                ),
            )
        })?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn clear_deletion_blocking_permissions(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> std::io::Result<()> {
    Ok(())
}

fn rollback_error(action: &str, from: &Path, to: &Path, error: std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        format!(
            "failed to {action} {} -> {}: {error}",
            from.display(),
            to.display()
        ),
    )
}

/// Fault-injection seam for out-of-tree fault injectors (e.g. `waml-editor`'s
/// save-path tests). `write_back_with_ops` itself stays private -- a plain
/// re-export of a private item is a compile error (E0365) -- so this module
/// defines its own `pub fn` that simply delegates to the private parent
/// function, exactly like `waml::analysis::test_support::PreparationProbe`.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub fn write_back_with_ops(
        root: &Path,
        old: &[(String, String)],
        new: &[(String, String)],
        ops: &impl FsOps,
    ) -> std::io::Result<Vec<String>> {
        super::write_back_with_ops(root, old, new, DeletePolicy::Transact, ops)
    }

    pub fn write_back_with_ops_and_policy(
        root: &Path,
        old: &[(String, String)],
        new: &[(String, String)],
        policy: DeletePolicy,
        ops: &impl FsOps,
    ) -> std::io::Result<Vec<String>> {
        super::write_back_with_ops(root, old, new, policy, ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "waml-host-persist-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FailRename {
        at: usize,
        calls: AtomicUsize,
    }

    struct FailRenames {
        at: &'static [usize],
        calls: AtomicUsize,
    }

    impl FsOps for FailRenames {
        fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            if self.at.contains(&call) {
                Err(std::io::Error::other(format!(
                    "injected rename failure at call {call}"
                )))
            } else {
                fs::rename(from, to)
            }
        }
    }

    #[cfg(windows)]
    struct RequireRollbackDisplacement {
        readonly_target: PathBuf,
        calls: AtomicUsize,
        displaced: std::sync::atomic::AtomicBool,
    }

    #[cfg(windows)]
    impl RequireRollbackDisplacement {
        fn new(readonly_target: PathBuf) -> Self {
            Self {
                readonly_target: readonly_target.canonicalize().unwrap(),
                calls: AtomicUsize::new(0),
                displaced: std::sync::atomic::AtomicBool::new(false),
            }
        }

        fn displaced(&self) -> bool {
            self.displaced.load(Ordering::Relaxed)
        }
    }

    #[cfg(windows)]
    impl FsOps for RequireRollbackDisplacement {
        fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            if call == 3 {
                assert!(
                    fs::metadata(&self.readonly_target)?
                        .permissions()
                        .readonly(),
                    "installed rollback target must be genuinely read-only"
                );
                return Err(std::io::Error::other("injected later rename failure"));
            }
            if from == self.readonly_target {
                self.displaced.store(true, Ordering::Relaxed);
            }
            if to == self.readonly_target && call > 3 && !self.displaced.load(Ordering::Relaxed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "read-only rollback target was not displaced",
                ));
            }
            fs::rename(from, to)
        }
    }

    impl FailRename {
        fn new(at: usize) -> Self {
            Self {
                at,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl FsOps for FailRename {
        fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            if call == self.at {
                Err(std::io::Error::other(format!(
                    "injected rename failure at call {call}"
                )))
            } else {
                fs::rename(from, to)
            }
        }
    }

    fn directory_entries(path: &Path) -> Vec<String> {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }

    #[cfg(windows)]
    fn windows_file_attributes(path: &Path) -> u32 {
        use std::os::windows::fs::MetadataExt;
        fs::metadata(path).unwrap().file_attributes()
    }

    #[cfg(windows)]
    fn set_readonly(path: &Path, readonly: bool) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(readonly);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn write_back_rejects_ntfs_alternate_data_stream_paths() {
        let temp = TempDir::new();
        for logical in ["a:b.md", "nested/a:b.md"] {
            let new = vec![(logical.to_owned(), "streamed".to_owned())];
            let error = write_back(&temp.0, &[], &new).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput, "{logical}");
            assert!(
                error.to_string().contains("relative and traversal-free"),
                "{error}"
            );
        }
        assert_eq!(directory_entries(&temp.0), Vec::<String>::new());
    }

    /// A document named after a Windows device is an ordinary file, and this
    /// transaction writes it like any other. `confine::check_rel` deliberately
    /// carries no device-name table -- see its docs for the probe showing
    /// `con.md` is written, read back and renamed onto correctly, and that the
    /// one case which misbehaves (`nul/` as a directory) is reported by the
    /// filesystem.
    #[test]
    fn write_back_writes_a_document_named_after_a_reserved_device() {
        let temp = TempDir::new();
        for logical in ["con.md", "nested/con.md"] {
            let new = vec![(logical.to_owned(), "streamed".to_owned())];
            write_back(&temp.0, &[], &new).unwrap();
            assert_eq!(
                std::fs::read_to_string(temp.0.join(logical)).unwrap(),
                "streamed",
                "{logical}"
            );
        }
    }

    /// The rule that replaced the device table: Win32 strips a trailing dot or
    /// space from every segment, so `dir./x.md` and `dir/x.md` name the same
    /// file. A transaction holding both would silently lose one, and no layer
    /// below reports it -- so this writer refuses the shape before staging.
    #[test]
    fn write_back_rejects_a_segment_windows_would_silently_strip() {
        let temp = TempDir::new();
        for logical in ["dir./x.md", "dir /x.md", "trailing.md."] {
            let new = vec![(logical.to_owned(), "streamed".to_owned())];
            let error = write_back(&temp.0, &[], &new).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput, "{logical}");
        }
        assert_eq!(directory_entries(&temp.0), Vec::<String>::new());
    }

    /// `confine::resolve_under(.., FollowWithinRoot, ..)` dereferences the
    /// *final* component as well, but this writer must not: before the shared
    /// resolver, the target was the raw `root.join(rel)` and an existing file
    /// symlink failed the `symlink_metadata(..).is_file()` screen with "bundle
    /// target is not a file". Following it instead would let a bundle path
    /// overwrite whatever the link points at -- reachable from `serve`'s
    /// `/api/documents` with a client-supplied path.
    #[test]
    fn write_back_refuses_to_write_through_an_in_root_file_symlink() {
        let temp = TempDir::new();
        let real = temp.0.join("real.md");
        fs::write(&real, "original").unwrap();
        if !make_file_link(&real, &temp.0.join("link.md")) {
            eprintln!("skipping: this environment cannot create file links");
            return;
        }

        let new = vec![("link.md".to_owned(), "overwritten".to_owned())];
        let error = write_back(&temp.0, &[], &new).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("bundle target is not a file"),
            "{error}"
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "original");
    }

    /// The same refusal on the delete side: dropping a bundle path whose
    /// on-disk entry is a symlink must not delete the link's target.
    #[test]
    fn write_back_refuses_to_delete_through_an_in_root_file_symlink() {
        let temp = TempDir::new();
        let real = temp.0.join("real.md");
        fs::write(&real, "original").unwrap();
        if !make_file_link(&real, &temp.0.join("link.md")) {
            eprintln!("skipping: this environment cannot create file links");
            return;
        }

        let old = vec![("link.md".to_owned(), "original".to_owned())];
        let error = write_back(&temp.0, &old, &[]).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("bundle target is not a file"),
            "{error}"
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "original");
    }

    #[cfg(windows)]
    fn make_file_link(target: &Path, link: &Path) -> bool {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => true,
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied
                    || error.kind() == std::io::ErrorKind::Unsupported
                    || error.raw_os_error() == Some(1314) =>
            {
                false
            }
            Err(error) => panic!("failed to create file link: {error}"),
        }
    }

    #[cfg(unix)]
    fn make_file_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).unwrap();
        true
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn case_differing_targets_are_rejected_on_case_insensitive_platforms() {
        let temp = TempDir::new();
        let new = vec![
            ("Order.md".to_owned(), "upper".to_owned()),
            ("order.md".to_owned(), "lower".to_owned()),
        ];
        let error = write_back(&temp.0, &[], &new).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            error.to_string().contains("duplicate filesystem target"),
            "{error}"
        );
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    #[test]
    fn case_differing_targets_are_distinct_files_on_case_sensitive_platforms() {
        let temp = TempDir::new();
        let new = vec![
            ("Order.md".to_owned(), "upper".to_owned()),
            ("order.md".to_owned(), "lower".to_owned()),
        ];
        let touched = write_back(&temp.0, &[], &new).unwrap();
        assert_eq!(touched, ["wrote Order.md", "wrote order.md"]);
        assert_eq!(
            fs::read_to_string(temp.0.join("Order.md")).unwrap(),
            "upper"
        );
        assert_eq!(
            fs::read_to_string(temp.0.join("order.md")).unwrap(),
            "lower"
        );
    }

    #[test]
    fn late_write_failure_restores_updates_and_removes_new_artifacts() {
        let temp = TempDir::new();
        fs::write(temp.0.join("a.md"), "before").unwrap();
        let old = vec![("a.md".to_owned(), "before".to_owned())];
        let new = vec![
            ("a.md".to_owned(), "after".to_owned()),
            ("b/new.md".to_owned(), "new".to_owned()),
            ("z/new.md".to_owned(), "cannot write".to_owned()),
        ];

        let error = write_back_with_ops(
            &temp.0,
            &old,
            &new,
            DeletePolicy::Transact,
            &FailRename::new(4),
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            error.to_string(),
            "failed to write z/new.md: injected rename failure at call 4"
        );
        assert_eq!(fs::read_to_string(temp.0.join("a.md")).unwrap(), "before");
        assert!(!temp.0.join("b").exists());
        assert!(!temp.0.join("z").exists());
        assert_eq!(directory_entries(&temp.0), ["a.md"]);
    }

    #[test]
    fn late_delete_failure_restores_prior_write_and_delete() {
        let temp = TempDir::new();
        fs::write(temp.0.join("a.md"), "before").unwrap();
        fs::write(temp.0.join("m-delete.md"), "keep me").unwrap();
        fs::write(temp.0.join("z-delete.md"), "keep me too").unwrap();
        let old = vec![
            ("a.md".to_owned(), "before".to_owned()),
            ("m-delete.md".to_owned(), "keep me".to_owned()),
            ("z-delete.md".to_owned(), "claimed file".to_owned()),
        ];
        let new = vec![("a.md".to_owned(), "after".to_owned())];

        let error = write_back_with_ops(
            &temp.0,
            &old,
            &new,
            DeletePolicy::Transact,
            &FailRename::new(4),
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            error.to_string(),
            "failed to delete z-delete.md: injected rename failure at call 4"
        );
        assert_eq!(fs::read_to_string(temp.0.join("a.md")).unwrap(), "before");
        assert_eq!(
            fs::read_to_string(temp.0.join("m-delete.md")).unwrap(),
            "keep me"
        );
        assert_eq!(
            fs::read_to_string(temp.0.join("z-delete.md")).unwrap(),
            "keep me too"
        );
        assert_eq!(
            directory_entries(&temp.0),
            ["a.md", "m-delete.md", "z-delete.md"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn readonly_update_is_restored_after_later_write_failure() {
        let temp = TempDir::new();
        let readonly = temp.0.join("a-readonly.md");
        let unrelated = temp.0.join("unrelated.md");
        fs::write(&readonly, "original readonly bytes").unwrap();
        fs::write(&unrelated, "unrelated bytes").unwrap();
        set_readonly(&readonly, true);
        let original_attributes = windows_file_attributes(&readonly);
        let old = vec![
            (
                "a-readonly.md".to_owned(),
                "original readonly bytes".to_owned(),
            ),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
        ];
        let new = vec![
            ("a-readonly.md".to_owned(), "replacement".to_owned()),
            ("z-later.md".to_owned(), "later write".to_owned()),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
        ];

        let fault = RequireRollbackDisplacement::new(readonly.clone());
        let error =
            write_back_with_ops(&temp.0, &old, &new, DeletePolicy::Transact, &fault).unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to write z-later.md: injected later rename failure"
        );
        assert!(
            fault.displaced(),
            "rollback must displace the installed read-only target through the checked boundary"
        );
        assert_eq!(
            fs::read_to_string(&readonly).unwrap(),
            "original readonly bytes"
        );
        assert_eq!(windows_file_attributes(&readonly), original_attributes);
        assert!(fs::metadata(&readonly).unwrap().permissions().readonly());
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "unrelated bytes");
        assert!(!temp.0.join("z-later.md").exists());
        assert_eq!(
            directory_entries(&temp.0),
            ["a-readonly.md", "unrelated.md"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn readonly_update_is_restored_after_later_delete_failure() {
        let temp = TempDir::new();
        let readonly = temp.0.join("a-readonly.md");
        let deleted = temp.0.join("z-delete.md");
        let unrelated = temp.0.join("unrelated.md");
        fs::write(&readonly, "original readonly bytes").unwrap();
        fs::write(&deleted, "delete original").unwrap();
        fs::write(&unrelated, "unrelated bytes").unwrap();
        set_readonly(&readonly, true);
        let readonly_attributes = windows_file_attributes(&readonly);
        let deleted_attributes = windows_file_attributes(&deleted);
        let old = vec![
            (
                "a-readonly.md".to_owned(),
                "original readonly bytes".to_owned(),
            ),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
            ("z-delete.md".to_owned(), "delete original".to_owned()),
        ];
        let new = vec![
            ("a-readonly.md".to_owned(), "replacement".to_owned()),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
        ];

        let fault = RequireRollbackDisplacement::new(readonly.clone());
        let error =
            write_back_with_ops(&temp.0, &old, &new, DeletePolicy::Transact, &fault).unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to delete z-delete.md: injected later rename failure"
        );
        assert!(
            fault.displaced(),
            "rollback must displace the installed read-only target through the checked boundary"
        );
        assert_eq!(
            fs::read_to_string(&readonly).unwrap(),
            "original readonly bytes"
        );
        assert_eq!(windows_file_attributes(&readonly), readonly_attributes);
        assert!(fs::metadata(&readonly).unwrap().permissions().readonly());
        assert_eq!(fs::read_to_string(&deleted).unwrap(), "delete original");
        assert_eq!(windows_file_attributes(&deleted), deleted_attributes);
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "unrelated bytes");
        assert_eq!(
            directory_entries(&temp.0),
            ["a-readonly.md", "unrelated.md", "z-delete.md"]
        );
    }

    #[test]
    fn rollback_failure_retains_a_reported_recovery_journal() {
        let temp = TempDir::new();
        let updated = temp.0.join("a.md");
        let unrelated = temp.0.join("unrelated.md");
        fs::write(&updated, "original bytes").unwrap();
        fs::write(&unrelated, "unrelated bytes").unwrap();
        let old = vec![
            ("a.md".to_owned(), "original bytes".to_owned()),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
        ];
        let new = vec![
            ("a.md".to_owned(), "replacement".to_owned()),
            ("unrelated.md".to_owned(), "unrelated bytes".to_owned()),
            ("z-later.md".to_owned(), "later".to_owned()),
        ];

        let error = write_back_with_ops(
            &temp.0,
            &old,
            &new,
            DeletePolicy::Transact,
            &FailRenames {
                at: &[3, 4],
                calls: AtomicUsize::new(0),
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains(
            "failed to write z-later.md: injected rename failure at call 3; rollback failed:"
        ));
        assert!(error.to_string().contains("recovery journal retained at "));
        assert_eq!(fs::read_to_string(&updated).unwrap(), "replacement");
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "unrelated bytes");
        let journal = fs::read_dir(&temp.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".waml-cli-"))
            })
            .expect("failed rollback must retain its recovery journal");
        let backup = fs::read_dir(&journal)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("backup-"))
            })
            .expect("recovery journal must retain the original backup");
        assert_eq!(fs::read_to_string(backup).unwrap(), "original bytes");
    }

    /// Every target is committed by the time the staging directory is swept
    /// up; a cleanup failure must surface as a warning, never as a failed
    /// write.
    #[test]
    fn committed_write_reports_success_with_a_warning_when_cleanup_fails() {
        struct FailCleanup;

        impl FsOps for FailCleanup {
            fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
                fs::rename(from, to)
            }

            fn remove_dir_all(&self, _path: &Path) -> std::io::Result<()> {
                Err(std::io::Error::other("injected cleanup failure"))
            }
        }

        let temp = TempDir::new();
        let target = temp.0.join("a.md");
        fs::write(&target, "before").unwrap();
        let old = vec![("a.md".to_owned(), "before".to_owned())];
        let new = vec![("a.md".to_owned(), "after".to_owned())];

        let touched =
            write_back_with_ops(&temp.0, &old, &new, DeletePolicy::Transact, &FailCleanup).unwrap();

        assert_eq!(touched.len(), 2, "{touched:?}");
        assert_eq!(touched[0], "wrote a.md");
        assert!(
            touched[1].starts_with("warning: all changes committed"),
            "{touched:?}"
        );
        assert!(
            touched[1].contains("injected cleanup failure"),
            "{touched:?}"
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "after");
    }

    /// `DeletePolicy::Refuse` (Task 12 of the waml-cli-logic-seam plan) fails
    /// before any staging directory is created, with the exact message
    /// `waml-editor`'s atomic save relied on before it adopted this
    /// transaction.
    #[test]
    fn refuse_policy_rejects_a_dropped_path_before_staging() {
        let temp = TempDir::new();
        fs::write(temp.0.join("a.md"), "before").unwrap();
        let old = vec![("a.md".to_owned(), "before".to_owned())];
        let new: Vec<(String, String)> = vec![];

        let error = write_back_with_policy(&temp.0, &old, &new, DeletePolicy::Refuse).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "removing bundle files is not supported by atomic save"
        );
        assert_eq!(fs::read_to_string(temp.0.join("a.md")).unwrap(), "before");
        assert_eq!(directory_entries(&temp.0), ["a.md"]);
    }

    /// `DeletePolicy::Refuse` still allows plain updates/adds -- only a path
    /// dropped from `new` is refused.
    #[test]
    fn refuse_policy_allows_updates_and_adds() {
        let temp = TempDir::new();
        fs::write(temp.0.join("a.md"), "before").unwrap();
        let old = vec![("a.md".to_owned(), "before".to_owned())];
        let new = vec![
            ("a.md".to_owned(), "after".to_owned()),
            ("b.md".to_owned(), "new".to_owned()),
        ];

        let touched = write_back_with_policy(&temp.0, &old, &new, DeletePolicy::Refuse).unwrap();

        assert_eq!(touched, ["wrote a.md", "wrote b.md"]);
        assert_eq!(fs::read_to_string(temp.0.join("a.md")).unwrap(), "after");
        assert_eq!(fs::read_to_string(temp.0.join("b.md")).unwrap(), "new");
    }

    #[test]
    fn successful_transaction_adds_updates_and_deletes_as_one_set() {
        let temp = TempDir::new();
        fs::write(temp.0.join("update.md"), "before").unwrap();
        fs::write(temp.0.join("delete.md"), "remove").unwrap();
        let permissions = fs::metadata(temp.0.join("update.md"))
            .unwrap()
            .permissions();
        let old = vec![
            ("delete.md".to_owned(), "remove".to_owned()),
            ("update.md".to_owned(), "before".to_owned()),
        ];
        let new = vec![
            ("nested/add.md".to_owned(), "added".to_owned()),
            ("update.md".to_owned(), "after".to_owned()),
        ];

        let touched = write_back(&temp.0, &old, &new).unwrap();

        assert_eq!(
            touched,
            [
                "wrote nested/add.md",
                "wrote update.md",
                "deleted delete.md"
            ]
        );
        assert_eq!(
            fs::read_to_string(temp.0.join("nested/add.md")).unwrap(),
            "added"
        );
        assert_eq!(
            fs::read_to_string(temp.0.join("update.md")).unwrap(),
            "after"
        );
        assert_eq!(
            fs::metadata(temp.0.join("update.md"))
                .unwrap()
                .permissions()
                .readonly(),
            permissions.readonly()
        );
        assert!(!temp.0.join("delete.md").exists());
        assert_eq!(directory_entries(&temp.0), ["nested", "update.md"]);
    }
}
