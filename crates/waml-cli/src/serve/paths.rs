//! Bundle-path confinement: syntactic checks plus canonicalized containment.
//!
//! `site::is_safe_relative_path` stays as-is for the embedded manifest — this
//! is the stricter live-filesystem sibling for wire input (op/document paths
//! arriving over the API), so the two intentionally are not merged.
//!
//! Both functions here are thin wrappers over `waml::host::confine`, which
//! carries the actual syntactic and symlink-containment rules shared with
//! `host::persist` and the editor's native save (Task 11 of the
//! `waml-cli-logic-seam` plan). This module's job is only to translate
//! `confine`'s typed `ConfineError` into the `Result<_, String>` contract
//! `serve`'s error path expects (`ApplyFailure::Confinement` → HTTP 422).

use std::path::{Path, PathBuf};

use waml::host::confine::{self, ConfineError, SymlinkPolicy};

/// Syntactic check only: reject anything that could escape the served
/// directory, plus the trailing dot/space segments Windows silently strips,
/// on every platform (a document authored on one OS can be served from
/// another). See `confine::check_rel` for what is deliberately NOT rejected.
pub fn is_safe_rel(rel: &str) -> Result<(), String> {
    confine::check_rel(rel).map_err(|error| confine_error_to_string(rel, error))
}

/// Join `rel` onto `root` and require the result to stay under the
/// canonicalized root, resolving symlinks along the way. `rel` need not
/// exist yet: the nearest existing ancestor is canonicalized and the
/// remaining components are appended.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    is_safe_rel(rel)?;
    confine::resolve_under(root, rel, SymlinkPolicy::FollowWithinRoot, false)
        .map_err(|error| confine_error_to_string(rel, error))
}

fn confine_error_to_string(rel: &str, error: ConfineError) -> String {
    match error {
        ConfineError::Syntactic(message) => message,
        ConfineError::SymlinkRefused(_) | ConfineError::EscapesRoot(_) => {
            format!("path resolves outside the served directory: {rel:?}")
        }
        ConfineError::NotADirectory(path) => {
            format!("not a directory: {}", path.display())
        }
        ConfineError::NotAFile(path) => format!("not a file: {}", path.display()),
        ConfineError::Io(io_error) => format!("could not resolve {rel:?}: {io_error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        for rel in ["a.md", "dir/a.md", "a/b/c.md", "console.md", "com10.md"] {
            assert!(is_safe_rel(rel).is_ok(), "expected {rel:?} to be safe");
        }
    }

    #[test]
    fn rejects_traversal_and_absolutes() {
        for rel in [
            "../x",
            "a/../../x",
            "/etc/passwd",
            "C:\\x",
            "\\\\server\\share\\x",
            "",
            ".",
        ] {
            assert!(is_safe_rel(rel).is_err(), "expected {rel:?} to be rejected");
        }
    }

    #[test]
    fn rejects_a_nul_byte() {
        assert!(is_safe_rel("a\0b").is_err());
    }

    /// Windows reserved device names are no longer rejected here: they are
    /// ordinary files and directories in practice, and the filesystem reports
    /// the one case that misbehaves. See `confine::check_rel` for the probe.
    /// What replaced the table is the trailing dot/space rule, the single
    /// Windows quirk the filesystem performs silently.
    #[test]
    fn admits_device_names_and_rejects_silently_stripped_segments() {
        for rel in ["con.md", "CON", "com1", "COM1.md", "lpt3.txt", "nul"] {
            assert!(is_safe_rel(rel).is_ok(), "expected {rel:?} to be accepted");
        }
        for rel in ["dir./x.md", "dir /x.md"] {
            assert!(is_safe_rel(rel).is_err(), "expected {rel:?} to be rejected");
        }
    }

    /// New as of the shared `host::confine` module (Task 11): an interior
    /// colon is an NTFS alternate-data-stream write (`a:b.md`), not a file a
    /// client can see, so it is now rejected at the confinement layer too --
    /// previously such a path passed here and only died later at
    /// `SourceBundle::try_from_pairs`/`BundlePath::parse`, still a 422 but a
    /// different error body (`routes.rs`'s `an_invalid_documents_candidate_is_422_with_diagnostics`
    /// still covers that fallback for any path that reaches the model
    /// layer some other way).
    #[test]
    fn rejects_interior_colon_segments() {
        for rel in ["ab:c.md", "nested/ab:c.md"] {
            assert!(is_safe_rel(rel).is_err(), "expected {rel:?} to be rejected");
        }
    }

    #[test]
    fn safe_join_stays_under_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.md"), "x").unwrap();

        let resolved = safe_join(root, "sub/a.md").unwrap();
        assert_eq!(resolved, root.canonicalize().unwrap().join("sub/a.md"));

        // Not-yet-created file under an existing directory still resolves.
        let new_file = safe_join(root, "sub/new.md").unwrap();
        assert_eq!(new_file, root.canonicalize().unwrap().join("sub/new.md"));

        assert!(safe_join(root, "../escape.md").is_err());
    }

    #[test]
    fn safe_join_rejects_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.md"), "x").unwrap();

        let link = root.join("escape");
        if !make_dir_link(&outside, &link) {
            // Symlink creation denied (e.g. no privilege on this Windows
            // host) — skip gracefully, matching
            // native_save::tests::make_dir_link's convention.
            return;
        }

        assert!(safe_join(&root, "escape/secret.md").is_err());
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
