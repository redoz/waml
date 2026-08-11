use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use waml::bundle_envelope::expand_text;
use waml::host::ingest::{ingest_markdown, rooted_key, triage, IngestError, IngestOptions};
use waml::index_md::IndexChange;

/// Recursively collect `.md` files under the given files/directories.
///
/// Dot-directories are skipped during the walk: they hold tool state, not model
/// content. The editor writes `<project>/.waml/README.md` on open, and reading
/// that back as a document would silently add a phantom node to the model.
/// A dot-directory named directly on the command line is still descended into,
/// since that is an explicit request rather than an incidental discovery.
pub fn collect_md(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    collect_md_with(paths, &mut |skipped| eprintln!("warning: {skipped}"))
}

/// `collect_md` with an injectable sink for skipped-link reports. A
/// non-followed link is the walker's clean-skip default, not a failure: the
/// pre-unification walkers never made one link abort the collection. But the
/// old walkers *followed* links, so a bundle that relied on one would now
/// silently lose documents -- every skip is surfaced through `report_skipped_link`.
fn collect_md_with(
    paths: &[PathBuf],
    report_skipped_link: &mut dyn FnMut(&IngestError),
) -> std::io::Result<Vec<PathBuf>> {
    let ingested = ingest_markdown(paths, &IngestOptions::default());
    triage(ingested.errors, report_skipped_link).map_err(ingest_error_to_io)?;
    Ok(ingested
        .files
        .into_iter()
        .map(|(path, _text)| path)
        .collect())
}

fn ingest_error_to_io(error: IngestError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

fn path_key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Read a bundle with every document keyed by its path *relative to the bundle
/// root* rather than as typed on the command line.
///
/// The distinction matters because a bundle's paths are its OKF ids
/// ([`waml::okf::id_of`]). Diagnostics-facing commands must echo the path the
/// user typed, but a shared bundle is a model: keying it off the invocation
/// path would make the same directory encode to a different model depending on
/// where it was shared from, would disagree with the editor (which reads paths
/// relative to the model directory), and would stamp the author's directory
/// layout into every link.
pub fn read_bundle_rooted(
    paths: &[PathBuf],
    stdin: bool,
) -> std::io::Result<Vec<(String, String)>> {
    Ok(read_analysis_bundle(paths, stdin)?.files)
}

pub struct InvocationBundle {
    pub files: Vec<(String, String)>,
    pub display_paths: BTreeMap<String, String>,
}

pub fn read_analysis_bundle(paths: &[PathBuf], stdin: bool) -> std::io::Result<InvocationBundle> {
    if stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        let files = expand_text("stdin.md", &buf)?;
        let display_paths = files
            .iter()
            .map(|(path, _)| (path.clone(), "stdin".to_owned()))
            .collect();
        return Ok(InvocationBundle {
            files,
            display_paths,
        });
    }
    let files = collect_md(paths)?;
    let root = bundle_root(paths, &files);
    let mut out = Vec::new();
    let mut display_paths = BTreeMap::new();
    for file in &files {
        let text = fs::read_to_string(file)?;
        let key = match &root {
            Some(root) => rooted_key(root, file),
            None => path_key(file),
        };
        let expanded = expand_text(&key, &text)?;
        let display = file.to_string_lossy().into_owned();
        for (path, _) in &expanded {
            display_paths.insert(path.clone(), display.clone());
        }
        out.extend(expanded);
    }
    // Match the editor's `read_bundle`: sorted keys make the encoding
    // deterministic, so the same model always yields the same link.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(InvocationBundle {
        files: out,
        display_paths,
    })
}

/// The directory paths should be made relative to: the directory itself when a
/// single one was given (the common case, and what the editor does), otherwise
/// the deepest directory containing every collected file. `None` when there is
/// no shared ancestor, in which case paths are left as-is.
fn bundle_root(paths: &[PathBuf], files: &[PathBuf]) -> Option<PathBuf> {
    if let [only] = paths {
        if only.is_dir() {
            return Some(only.clone());
        }
    }
    let mut root = files.first()?.parent()?.to_path_buf();
    for file in files.iter().skip(1) {
        let parent = file.parent()?;
        while !parent.starts_with(&root) {
            if !root.pop() {
                return None;
            }
        }
    }
    Some(root)
}

/// `fmt`: each physical `.md` file is a single document (no blob splitting).
pub fn read_files(paths: &[PathBuf]) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for file in collect_md(paths)? {
        let text = fs::read_to_string(&file)?;
        out.push((path_key(&file), text));
    }
    Ok(out)
}

pub struct PhysicalBundle {
    pub root: PathBuf,
    pub files: Vec<(String, String)>,
    pub display_paths: BTreeMap<String, String>,
}

/// Read physical Markdown files as one invocation-local bundle while retaining
/// the root needed to map validated bundle paths back to filesystem paths.
pub fn read_physical_bundle(paths: &[PathBuf]) -> std::io::Result<PhysicalBundle> {
    let physical = collect_md(paths)?;
    let root = bundle_root(paths, &physical)
        .or_else(|| {
            physical
                .first()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        })
        .unwrap_or_default();
    let mut files = Vec::with_capacity(physical.len());
    let mut display_paths = BTreeMap::new();
    for path in physical {
        let logical = rooted_key(&root, &path);
        display_paths.insert(logical.clone(), path.to_string_lossy().into_owned());
        files.push((logical, fs::read_to_string(path)?));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(PhysicalBundle {
        root,
        files,
        display_paths,
    })
}

fn invalid_index_path(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn escaped_index_path(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message.into())
}

fn resolve_index_target(
    root: &Path,
    relative: &str,
    create_parents: bool,
) -> std::io::Result<PathBuf> {
    let canonical_root = fs::canonicalize(root)?;
    let mut components = Vec::new();
    for component in Path::new(relative).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => components.push(component),
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(invalid_index_path(format!(
                    "invalid index path: {relative}"
                )));
            }
        }
    }
    let Some(filename) = components.pop() else {
        return Err(invalid_index_path("index path is empty"));
    };
    if !waml::index_md::is_index_basename(&filename.to_string_lossy()) {
        return Err(invalid_index_path(format!(
            "not an index document: {relative}"
        )));
    }

    let mut current = canonical_root.clone();
    for component in components {
        let candidate = current.join(component);
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(escaped_index_path(format!(
                        "symlinked index parent: {}",
                        candidate.display()
                    )));
                }
                if !metadata.is_dir() {
                    return Err(invalid_index_path(format!(
                        "index parent is not a directory: {}",
                        candidate.display()
                    )));
                }
                let resolved = fs::canonicalize(&candidate)?;
                if !resolved.starts_with(&canonical_root) {
                    return Err(escaped_index_path(format!(
                        "index path escapes root: {}",
                        candidate.display()
                    )));
                }
                current = resolved;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_parents => {
                fs::create_dir(&candidate)?;
                let resolved = fs::canonicalize(&candidate)?;
                if !resolved.starts_with(&canonical_root) {
                    return Err(escaped_index_path(format!(
                        "index path escapes root: {}",
                        candidate.display()
                    )));
                }
                current = resolved;
            }
            Err(error) => return Err(error),
        }
    }

    let target = current.join(filename);
    if !target.starts_with(&canonical_root) {
        return Err(invalid_index_path(format!(
            "index path escapes root: {relative}"
        )));
    }
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(escaped_index_path(format!(
                    "symlinked index target: {}",
                    target.display()
                )));
            }
            let resolved = fs::canonicalize(&target)?;
            if !resolved.starts_with(&canonical_root) {
                return Err(escaped_index_path(format!(
                    "index path escapes root: {}",
                    target.display()
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(target)
}

pub fn write_indexes(root: &Path, changes: &[IndexChange]) -> std::io::Result<()> {
    for change in changes {
        match change {
            IndexChange::Upsert { path, rendered } => {
                let target = resolve_index_target(root, path, true)?;
                if let Ok(metadata) = fs::symlink_metadata(&target) {
                    if !metadata.file_type().is_file() {
                        return Err(invalid_index_path(format!(
                            "index target is not a file: {}",
                            target.display()
                        )));
                    }
                }
                fs::write(target, rendered.as_bytes())?;
            }
            IndexChange::Remove { path } => {
                let target = resolve_index_target(root, path, false)?;
                match fs::symlink_metadata(&target) {
                    Ok(metadata) => {
                        if !metadata.file_type().is_file() {
                            return Err(invalid_index_path(format!(
                                "index target is not a file: {}",
                                target.display()
                            )));
                        }
                        fs::remove_file(target)?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(())
}

/// Read an NDJSON op-log: `(line_number, trimmed_line)` per non-blank line.
/// `src == "-"` reads stdin.
pub fn read_ndjson(src: &str) -> std::io::Result<Vec<(usize, String)>> {
    let text = if src == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        fs::read_to_string(src)?
    };
    Ok(text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .filter(|(_, l)| !l.is_empty())
        .collect())
}

/// Write only changed/added entries; delete entries dropped from the bundle.
/// Returns a human list of what happened.
pub fn write_back(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
) -> std::io::Result<Vec<String>> {
    #[cfg(debug_assertions)]
    if let Some(ops) = DebugFaultFs::from_env()? {
        return write_back_with_ops(root, old, new, &ops);
    }
    write_back_with_ops(root, old, new, &RealFs)
}

trait FsOps {
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

#[cfg(debug_assertions)]
struct DebugFaultFs {
    fail_rename_at: Option<u64>,
    rename_calls: AtomicU64,
    fail_cleanup: bool,
}

#[cfg(debug_assertions)]
impl DebugFaultFs {
    fn from_env() -> std::io::Result<Option<Self>> {
        let fail_rename_at = std::env::var("WAML_CLI_TEST_FAIL_RENAME_AT")
            .ok()
            .map(|value| {
                value.parse::<u64>().map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid WAML_CLI_TEST_FAIL_RENAME_AT: {error}"),
                    )
                })
            })
            .transpose()?;
        let fail_cleanup = std::env::var_os("WAML_CLI_TEST_FAIL_CLEANUP").is_some();
        Ok((fail_rename_at.is_some() || fail_cleanup).then(|| Self {
            fail_rename_at,
            rename_calls: AtomicU64::new(0),
            fail_cleanup,
        }))
    }
}

#[cfg(debug_assertions)]
impl FsOps for DebugFaultFs {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        let call = self.rename_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_rename_at == Some(call) {
            return Err(std::io::Error::other(format!(
                "injected rename failure at call {call}"
            )));
        }
        fs::rename(from, to)
    }

    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()> {
        if self.fail_cleanup {
            return Err(std::io::Error::other("injected cleanup failure"));
        }
        fs::remove_dir_all(path)
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
    ops: &impl FsOps,
) -> std::io::Result<Vec<String>> {
    let om: BTreeMap<&str, &str> = old.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let nm: BTreeMap<&str, &str> = new.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
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
            let relative = Path::new(logical);
            validate_relative(relative)?;
            let target = root.join(relative);
            validate_target(&root, &target)?;
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

fn validate_relative(path: &Path) -> std::io::Result<()> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| match component {
            // A colon inside a name is an NTFS alternate-data-stream write
            // (`a:b.md`), not a file the user can see; reject it everywhere
            // so bundles behave identically across platforms.
            Component::Normal(name) => name.to_string_lossy().contains(':'),
            _ => true,
        })
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "bundle path must be relative and traversal-free: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn validate_target(root: &Path, target: &Path) -> std::io::Result<()> {
    let mut ancestor = target.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "bundle path has no parent",
        )
    })?;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "bundle path has no existing ancestor",
            )
        })?;
    }
    let canonical = ancestor.canonicalize()?;
    if canonical == root || canonical.starts_with(root) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bundle path resolves outside root: {}", target.display()),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "waml-cli-io-{}-{}",
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

    #[test]
    fn collect_md_fail_fast_error_names_the_offending_path() {
        let temp = TempDir::new();
        let missing = temp.0.join("does-not-exist.md");

        let error = collect_md(std::slice::from_ref(&missing)).unwrap_err();
        assert!(
            error.to_string().contains(&missing.display().to_string()),
            "error should name the offending path: {error}"
        );
    }

    /// One link anywhere under a collected directory must not make the whole
    /// collection fail: a non-followed link is a clean skip, not a fatal
    /// error. The skip must be *surfaced*, not silent -- the pre-unification
    /// walker followed links, so a bundle relying on one loses documents here.
    #[test]
    fn collect_md_survives_a_skipped_link() {
        let temp = TempDir::new();
        fs::create_dir(temp.0.join("real")).unwrap();
        fs::write(temp.0.join("order.md"), "# Order\n").unwrap();
        fs::write(temp.0.join("real/nested.md"), "# Nested\n").unwrap();
        if !make_dir_link(&temp.0.join("linked"), &temp.0.join("real")) {
            eprintln!("skipping: this environment cannot create directory links");
            return;
        }

        let mut skipped = Vec::new();
        let files = collect_md_with(std::slice::from_ref(&temp.0), &mut |error| {
            skipped.push(error.to_string())
        })
        .expect("a skipped link must not fail the collection");
        assert_eq!(skipped.len(), 1, "the skipped link must be reported");
        assert!(
            skipped[0].contains("linked"),
            "the report must name the skipped link: {}",
            skipped[0]
        );
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["order.md", "nested.md"]);
    }

    #[cfg(unix)]
    fn make_dir_link(link: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn make_dir_link(link: &Path, target: &Path) -> bool {
        // NTFS junction via `mklink /J`: needs no admin rights, unlike symlinks.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status();
        matches!(status, Ok(status) if status.success())
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

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../waml-editor/tests/fixtures")
            .join(name)
    }

    /// A bundle's paths are its OKF ids, so a shared model must key off the
    /// bundle root -- exactly like the editor's `load::read_bundle` -- and not
    /// off whatever prefix happened to be typed at the shell.
    #[test]
    fn rooted_read_keys_relative_to_the_bundle_dir() {
        let bundle = read_bundle_rooted(&[fixture("mini")], false).unwrap();
        let keys: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            keys,
            [
                "customer.md",
                "index.md",
                "order.md",
                "orders-diagram.md",
                "payment-gateway.md"
            ]
        );
    }

    /// `.waml/` is editor state, not model content. The editor drops a README
    /// there on open, and picking it up would add a phantom document to every
    /// bundle read from a project that has ever been opened in the editor.
    #[test]
    fn rooted_read_skips_editor_state_directories() {
        let temp = TempDir::new();
        fs::write(temp.0.join("order.md"), "# Order\n").unwrap();
        fs::create_dir_all(temp.0.join(".waml")).unwrap();
        fs::write(temp.0.join(".waml/README.md"), "# Not your model\n").unwrap();
        fs::write(temp.0.join(".waml/editor.json"), "{}").unwrap();

        let bundle = read_bundle_rooted(std::slice::from_ref(&temp.0), false).unwrap();
        let keys: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(keys, ["order.md"]);
    }

    /// The same model shared from a different working directory must encode
    /// identically; otherwise one model has as many identities as it has
    /// callers, and links leak the author's directory layout.
    #[test]
    fn rooted_read_is_independent_of_the_path_typed() {
        let absolute = read_bundle_rooted(&[fixture("mini")], false).unwrap();
        let indirect =
            read_bundle_rooted(&[fixture("groups").parent().unwrap().join("mini")], false).unwrap();
        assert_eq!(absolute, indirect);
    }

    /// Given loose files rather than a directory, the root is the deepest
    /// directory containing all of them.
    #[test]
    fn rooted_read_uses_the_common_ancestor_of_loose_files() {
        let dir = fixture("mini");
        let bundle =
            read_bundle_rooted(&[dir.join("order.md"), dir.join("customer.md")], false).unwrap();
        let keys: Vec<&str> = bundle.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(keys, ["customer.md", "order.md"]);
    }

    /// One-shot analysis requires validated bundle-relative paths even when a
    /// single physical file was typed.
    #[test]
    fn rooted_read_of_one_file_uses_its_basename() {
        let dir = fixture("mini");
        let bundle = read_bundle_rooted(&[dir.join("order.md")], false).unwrap();
        assert_eq!(bundle[0].0, "order.md");
    }

    #[test]
    fn read_ndjson_skips_blanks_and_numbers_lines() {
        // write a temp file
        let dir = std::env::temp_dir().join(format!("waml_ndjson_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ops.ndjson");
        fs::write(&f, "{\"op\":\"a\"}\n\n{\"op\":\"b\"}\n").unwrap();
        let lines = read_ndjson(f.to_str().unwrap()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, 1);
        assert_eq!(lines[1].0, 3, "blank line 2 skipped, numbering preserved");
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

        let error = write_back_with_ops(&temp.0, &old, &new, &FailRename::new(4)).unwrap_err();

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

        let error = write_back_with_ops(&temp.0, &old, &new, &FailRename::new(4)).unwrap_err();

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
        let error = write_back_with_ops(&temp.0, &old, &new, &fault).unwrap_err();

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
        let error = write_back_with_ops(&temp.0, &old, &new, &fault).unwrap_err();

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

        let touched = write_back_with_ops(&temp.0, &old, &new, &FailCleanup).unwrap();

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

    #[test]
    fn write_indexes_rejects_traversal_before_creating_a_file() {
        let temp = TempDir::new();
        let outside = temp.0.parent().unwrap().join("outside-index.md");
        let error = write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Upsert {
                path: "../outside-index.md/index.md".into(),
                rendered: "bad".into(),
            }],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.exists());
    }

    #[test]
    fn write_indexes_rejects_an_absolute_target_before_removal() {
        let temp = TempDir::new();
        let outside_dir = TempDir::new();
        let outside = outside_dir.0.join("index.md");
        fs::write(&outside, "keep").unwrap();
        let error = write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Remove {
                path: outside.to_string_lossy().into_owned(),
            }],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");
    }

    #[test]
    fn write_indexes_rejects_a_non_index_basename_before_writing() {
        let temp = TempDir::new();
        let error = write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Upsert {
                path: "nested/not-index.md".into(),
                rendered: "bad".into(),
            }],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.0.join("nested").exists());
    }

    #[test]
    fn write_indexes_rejects_a_symlinked_parent_before_writing() {
        let temp = TempDir::new();
        let outside = TempDir::new();
        let link = temp.0.join("linked");
        if create_dir_symlink(&outside.0, &link).is_err() {
            return;
        }

        let error = write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Upsert {
                path: "linked/index.md".into(),
                rendered: "bad".into(),
            }],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!outside.0.join("index.md").exists());
    }

    #[test]
    fn write_indexes_rejects_a_symlinked_index_before_removal() {
        let temp = TempDir::new();
        let outside = TempDir::new();
        let target = outside.0.join("index.md");
        fs::write(&target, "keep").unwrap();
        let link = temp.0.join("index.md");
        if create_file_symlink(&target, &link).is_err() {
            return;
        }

        let error = write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Remove {
                path: "index.md".into(),
            }],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(target).unwrap(), "keep");
    }

    #[cfg(windows)]
    fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(source, target)
    }

    #[cfg(not(windows))]
    fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, target)
    }

    #[cfg(windows)]
    fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(source, target)
    }

    #[cfg(not(windows))]
    fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(source, target)
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
}
