//! One path-confinement module backing every bundle-path *writer* (readers
//! accept a broader set; see the read/write asymmetry inventory in the
//! `waml-cli-logic-seam` plan, Task 11).
//!
//! The four call sites this module unifies do NOT share one confinement
//! rule -- they split into two symlink policies, and a naive "one shared
//! resolver" would silently change one of them:
//!
//! - [`SymlinkPolicy::RefuseAny`] -- categorical refusal: any symlinked
//!   parent or target is rejected, *even one that resolves inside the
//!   root*. This is the index writer's policy (`waml-cli`'s
//!   `resolve_index_target`), which treats "someone linked into my index
//!   tree" as inherently suspicious.
//! - [`SymlinkPolicy::FollowWithinRoot`] -- follow symlinks and reject only
//!   escapes. This is the policy for wire input (`serve`), the transactional
//!   bundle writer (`host::persist`), and the editor's native save.
//!
//! Both policies share one syntactic pre-check ([`check_rel`]) and report
//! through one typed error ([`ConfineError`]); each caller maps that error
//! to its own existing error contract (an `io::ErrorKind`, a `String`, an
//! HTTP status) -- this module never picks one itself.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Whether [`check_rel`]'s Windows reserved-device-name rule applies.
///
/// `Reject` is the rule for every writer that saves *document* content: a
/// path a Windows host cannot represent must fail at the point of mutation,
/// on every platform, rather than diverge per OS.
///
/// `Allow` exists for one caller -- `waml-cli`'s index writer -- whose
/// accepted set predates this module and must not shrink: it only ever writes
/// `index.md` basenames, so a device name can appear only as an enclosing
/// *directory* segment (`con/index.md`), which is an ordinary directory on
/// Linux. Rejecting it would make `waml index --write` fail the whole run on a
/// bundle that has always been legal there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceNamePolicy {
    Reject,
    Allow,
}

/// Syntactic validity, independent of the filesystem: applies to every
/// *writer* of a bundle-relative path. Rejects: empty, a NUL byte, an
/// absolute path (`/` or `\` leading), a UNC path, a drive prefix (`C:`),
/// an empty segment, a `.`/`..` segment, an interior `:` in any segment (an
/// NTFS alternate-data-stream write, e.g. `a:b.md`, mirroring
/// `BundlePath::parse`'s rationale), and a Windows reserved device name --
/// checked on every platform, since a document might be shared to a Windows
/// machine later.
pub fn check_rel(rel: &str) -> Result<(), ConfineError> {
    check_rel_with(rel, DeviceNamePolicy::Reject)
}

/// [`check_rel`] with an explicit [`DeviceNamePolicy`]; see that enum for the
/// single caller that opts out of the reserved-device-name rule.
pub fn check_rel_with(rel: &str, device_names: DeviceNamePolicy) -> Result<(), ConfineError> {
    if rel.is_empty() {
        return Err(ConfineError::Syntactic("path is empty".to_string()));
    }
    if rel.contains('\0') {
        return Err(ConfineError::Syntactic(format!(
            "path contains a NUL byte: {rel:?}"
        )));
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(ConfineError::Syntactic(format!(
            "path is absolute: {rel:?}"
        )));
    }
    if rel.starts_with("\\\\") {
        return Err(ConfineError::Syntactic(format!(
            "path is a UNC path: {rel:?}"
        )));
    }
    // Drive prefix, e.g. `C:` or `C:\x`.
    let mut chars = rel.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next()) {
        if letter.is_ascii_alphabetic() {
            return Err(ConfineError::Syntactic(format!(
                "path has a drive prefix: {rel:?}"
            )));
        }
    }

    for segment in rel.split(['/', '\\']) {
        if segment.is_empty() {
            return Err(ConfineError::Syntactic(format!(
                "path has an empty segment: {rel:?}"
            )));
        }
        if segment == "." || segment == ".." {
            return Err(ConfineError::Syntactic(format!(
                "path traverses via {segment:?}: {rel:?}"
            )));
        }
        if segment.contains(':') {
            return Err(ConfineError::Syntactic(format!(
                "path segment contains a colon: {segment:?}"
            )));
        }
        if device_names == DeviceNamePolicy::Reject && is_reserved_device_name(segment) {
            return Err(ConfineError::Syntactic(format!(
                "path names a reserved device: {segment:?}"
            )));
        }
    }

    Ok(())
}

/// Windows reserved device names are reserved with or without an extension,
/// and case-insensitively.
fn is_reserved_device_name(segment: &str) -> bool {
    let stem = match segment.split_once('.') {
        Some((stem, _ext)) => stem,
        None => segment,
    };
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            upper.as_str(),
            "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
        )
        || matches!(
            upper.as_str(),
            "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
        )
}

/// How a resolver treats a symlink encountered while walking down to a
/// bundle-relative path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkPolicy {
    /// Refuse any symlinked parent or target, even one that resolves inside
    /// the root.
    RefuseAny,
    /// Follow symlinks; reject only a resolution that escapes the root.
    FollowWithinRoot,
}

/// A confinement failure. Callers map each variant to their own error
/// contract -- an `io::ErrorKind`, a `String`, an HTTP status -- this module
/// never picks one itself.
#[derive(Debug)]
pub enum ConfineError {
    /// `check_rel` rejected the path before any filesystem access.
    Syntactic(String),
    /// A symlinked parent or target was refused under [`SymlinkPolicy::RefuseAny`].
    SymlinkRefused(PathBuf),
    /// The resolved path escapes the root.
    EscapesRoot(PathBuf),
    /// A path component that must be a directory is not one.
    NotADirectory(PathBuf),
    /// A path component that must be a file is not one.
    NotAFile(PathBuf),
    /// An underlying I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for ConfineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntactic(message) => write!(formatter, "{message}"),
            Self::SymlinkRefused(path) => {
                write!(formatter, "symlinked path refused: {}", path.display())
            }
            Self::EscapesRoot(path) => {
                write!(formatter, "path escapes root: {}", path.display())
            }
            Self::NotADirectory(path) => {
                write!(formatter, "not a directory: {}", path.display())
            }
            Self::NotAFile(path) => write!(formatter, "not a file: {}", path.display()),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConfineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ConfineError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Resolve `rel` (a bundle-relative path) under `root`, per `policy`.
///
/// Runs [`check_rel`] first, then walks the filesystem: `RefuseAny` checks
/// every intermediate component for a symlink and refuses it outright, even
/// one that resolves inside the root; `FollowWithinRoot` finds the nearest
/// existing ancestor, canonicalizes it, and requires the canonicalized
/// result (with the remaining, not-yet-existing components appended) to
/// stay under the canonicalized root.
///
/// `create_parents` only has an effect under `RefuseAny`, mirroring the
/// index writer's create-as-you-descend behaviour (kept inside the resolver
/// to avoid a check/create race); `FollowWithinRoot` callers create
/// directories themselves, after confinement is established.
pub fn resolve_under(
    root: &Path,
    rel: &str,
    policy: SymlinkPolicy,
    create_parents: bool,
) -> Result<PathBuf, ConfineError> {
    resolve_under_with(root, rel, policy, create_parents, DeviceNamePolicy::Reject)
}

/// [`resolve_under`] with an explicit [`DeviceNamePolicy`]; see that enum for
/// the single caller that opts out of the reserved-device-name rule.
pub fn resolve_under_with(
    root: &Path,
    rel: &str,
    policy: SymlinkPolicy,
    create_parents: bool,
    device_names: DeviceNamePolicy,
) -> Result<PathBuf, ConfineError> {
    check_rel_with(rel, device_names)?;
    let canonical_root = fs::canonicalize(root)?;
    match policy {
        SymlinkPolicy::RefuseAny => resolve_refuse_any(&canonical_root, rel, create_parents),
        SymlinkPolicy::FollowWithinRoot => resolve_follow_within_root(&canonical_root, rel),
    }
}

fn resolve_refuse_any(
    canonical_root: &Path,
    rel: &str,
    create_parents: bool,
) -> Result<PathBuf, ConfineError> {
    let mut components = Vec::new();
    for component in Path::new(rel).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => components.push(component),
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(ConfineError::Syntactic(format!("invalid path: {rel}")));
            }
        }
    }
    let Some(filename) = components.pop() else {
        return Err(ConfineError::Syntactic("path is empty".to_string()));
    };

    let mut current = canonical_root.to_path_buf();
    for component in components {
        let candidate = current.join(component);
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(ConfineError::SymlinkRefused(candidate));
                }
                if !metadata.is_dir() {
                    return Err(ConfineError::NotADirectory(candidate));
                }
                let resolved = fs::canonicalize(&candidate)?;
                if !resolved.starts_with(canonical_root) {
                    return Err(ConfineError::EscapesRoot(candidate));
                }
                current = resolved;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_parents => {
                fs::create_dir(&candidate)?;
                let resolved = fs::canonicalize(&candidate)?;
                if !resolved.starts_with(canonical_root) {
                    return Err(ConfineError::EscapesRoot(candidate));
                }
                current = resolved;
            }
            Err(error) => return Err(ConfineError::Io(error)),
        }
    }

    let target = current.join(filename);
    if !target.starts_with(canonical_root) {
        return Err(ConfineError::EscapesRoot(target));
    }
    match fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(ConfineError::SymlinkRefused(target));
            }
            let resolved = fs::canonicalize(&target)?;
            if !resolved.starts_with(canonical_root) {
                return Err(ConfineError::EscapesRoot(target));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ConfineError::Io(error)),
    }
    Ok(target)
}

fn resolve_follow_within_root(canonical_root: &Path, rel: &str) -> Result<PathBuf, ConfineError> {
    let candidate = canonical_root.join(rel);

    let existing_ancestor = nearest_existing_ancestor(&candidate)?;
    let canonical_ancestor = fs::canonicalize(&existing_ancestor)?;

    if canonical_ancestor != canonical_root && !canonical_ancestor.starts_with(canonical_root) {
        return Err(ConfineError::EscapesRoot(candidate));
    }

    let remainder = candidate.strip_prefix(&existing_ancestor).map_err(|_| {
        ConfineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path has an invalid parent: {rel:?}"),
        ))
    })?;
    // `existing_ancestor` is the target itself when the target already
    // exists (the common update case): `remainder` is then empty, and on a
    // Windows verbatim (`\\?\`) path, joining an empty component still
    // appends a raw trailing separator to the string -- invisible to `Path`
    // equality (which is component-wise) but fatal to real syscalls on that
    // string (observed: `ERROR_DIRECTORY`/`ERROR_INVALID_NAME` from
    // `canonicalize`/`create_dir` downstream). Skip the join entirely rather
    // than join an empty path.
    let resolved = if remainder.as_os_str().is_empty() {
        canonical_ancestor
    } else {
        canonical_ancestor.join(remainder)
    };

    if resolved != canonical_root && !resolved.starts_with(canonical_root) {
        return Err(ConfineError::EscapesRoot(candidate));
    }

    Ok(resolved)
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, ConfineError> {
    let mut candidate = path;
    loop {
        match fs::symlink_metadata(candidate) {
            Ok(_) => return Ok(candidate.to_path_buf()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                candidate = candidate.parent().ok_or_else(|| {
                    ConfineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("path has no existing ancestor: {path:?}"),
                    ))
                })?;
            }
            Err(error) => return Err(ConfineError::Io(error)),
        }
    }
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
                "waml-host-confine-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn check_rel_rejects_every_hostile_shape() {
        for rel in [
            "../x",
            "a/../../x",
            "/abs",
            "C:\\x",
            "\\\\unc\\x",
            "a\0b",
            "",
            ".",
            "con.md",
            "COM1.md",
            "lpt3.txt",
            "nul",
            "ab:c.md",
            "nested/ab:c.md",
        ] {
            assert!(check_rel(rel).is_err(), "expected {rel:?} to be rejected");
        }
    }

    #[test]
    fn check_rel_accepts_ordinary_and_lookalike_names() {
        for rel in [
            "a.md",
            "dir/a.md",
            "a/b/c.md",
            "console.md",
            "com10.md",
            "abc.md",
            "nested/abc.md",
        ] {
            assert!(check_rel(rel).is_ok(), "expected {rel:?} to be accepted");
        }
    }

    /// `DeviceNamePolicy::Allow` drops the reserved-device-name rule and
    /// nothing else; see the enum's docs for the one caller that needs it
    /// (`waml-cli`'s index writer, whose accepted set predates this module).
    #[test]
    fn allowing_device_names_drops_only_the_device_name_rule() {
        for rel in ["con.md", "con/index.md", "COM1.md", "nested/nul/index.md"] {
            assert!(check_rel(rel).is_err(), "expected {rel:?} to be rejected");
            assert!(
                check_rel_with(rel, DeviceNamePolicy::Allow).is_ok(),
                "expected {rel:?} to be accepted when device names are allowed"
            );
        }
        for rel in ["../x", "/abs", "ab:c.md", "a\0b", ""] {
            assert!(
                check_rel_with(rel, DeviceNamePolicy::Allow).is_err(),
                "expected {rel:?} to stay rejected when device names are allowed"
            );
        }
    }

    #[test]
    fn follow_within_root_stays_under_root() {
        let temp = TempDir::new();
        let root = temp.path();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/a.md"), "x").unwrap();

        let resolved =
            resolve_under(root, "sub/a.md", SymlinkPolicy::FollowWithinRoot, false).unwrap();
        assert_eq!(resolved, root.canonicalize().unwrap().join("sub/a.md"));

        // Not-yet-created file under an existing directory still resolves.
        let new_file =
            resolve_under(root, "sub/new.md", SymlinkPolicy::FollowWithinRoot, false).unwrap();
        assert_eq!(new_file, root.canonicalize().unwrap().join("sub/new.md"));

        assert!(matches!(
            resolve_under(root, "../escape.md", SymlinkPolicy::FollowWithinRoot, false),
            Err(ConfineError::Syntactic(_))
        ));
    }

    #[test]
    fn follow_within_root_follows_an_in_root_symlink() {
        let temp = TempDir::new();
        let root = temp.path();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        let link = root.join("linked");
        if !make_dir_link(&real, &link) {
            eprintln!("skipping: this environment cannot create directory links");
            return;
        }

        let resolved =
            resolve_under(root, "linked/a.md", SymlinkPolicy::FollowWithinRoot, false).unwrap();
        assert_eq!(resolved, root.canonicalize().unwrap().join("real/a.md"));
    }

    #[test]
    fn follow_within_root_rejects_an_out_of_root_symlink() {
        let temp = TempDir::new();
        let outside = TempDir::new();
        let root = temp.path();
        let link = root.join("linked");
        if !make_dir_link(outside.path(), &link) {
            eprintln!("skipping: this environment cannot create directory links");
            return;
        }

        let error = resolve_under(
            root,
            "linked/secret.md",
            SymlinkPolicy::FollowWithinRoot,
            false,
        )
        .unwrap_err();
        assert!(matches!(error, ConfineError::EscapesRoot(_)));
    }

    #[test]
    fn refuse_any_rejects_an_in_root_symlink() {
        let temp = TempDir::new();
        let root = temp.path();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        let link = root.join("linked");
        if !make_dir_link(&real, &link) {
            eprintln!("skipping: this environment cannot create directory links");
            return;
        }

        let error =
            resolve_under(root, "linked/a.md", SymlinkPolicy::RefuseAny, false).unwrap_err();
        assert!(matches!(error, ConfineError::SymlinkRefused(_)));
    }

    #[test]
    fn refuse_any_rejects_an_out_of_root_symlink() {
        let temp = TempDir::new();
        let outside = TempDir::new();
        let root = temp.path();
        let link = root.join("linked");
        if !make_dir_link(outside.path(), &link) {
            eprintln!("skipping: this environment cannot create directory links");
            return;
        }

        let error =
            resolve_under(root, "linked/a.md", SymlinkPolicy::RefuseAny, false).unwrap_err();
        assert!(matches!(error, ConfineError::SymlinkRefused(_)));
    }

    #[test]
    fn refuse_any_creates_missing_parents_when_asked() {
        let temp = TempDir::new();
        let root = temp.path();

        let target =
            resolve_under(root, "nested/dir/a.md", SymlinkPolicy::RefuseAny, true).unwrap();
        assert!(root.join("nested/dir").is_dir());
        assert_eq!(target, root.canonicalize().unwrap().join("nested/dir/a.md"));
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
