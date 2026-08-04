//! Bundle-path confinement: syntactic checks plus canonicalized containment.
//!
//! `site::is_safe_relative_path` stays as-is for the embedded manifest — this
//! is the stricter live-filesystem sibling for wire input (op/document paths
//! arriving over the API), so the two intentionally are not merged.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Syntactic check only: reject anything that could escape the served
/// directory or name a reserved device, on every platform (a document
/// authored on one OS can be served from another).
pub fn is_safe_rel(rel: &str) -> Result<(), String> {
    if rel.is_empty() {
        return Err("path is empty".to_string());
    }
    if rel.contains('\0') {
        return Err(format!("path contains a NUL byte: {rel:?}"));
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(format!("path is absolute: {rel:?}"));
    }
    if rel.starts_with("\\\\") {
        return Err(format!("path is a UNC path: {rel:?}"));
    }
    // Drive prefix, e.g. `C:` or `C:\x`.
    let mut chars = rel.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next()) {
        if letter.is_ascii_alphabetic() {
            return Err(format!("path has a drive prefix: {rel:?}"));
        }
    }

    for segment in rel.split(['/', '\\']) {
        if segment.is_empty() {
            return Err(format!("path has an empty segment: {rel:?}"));
        }
        if segment == "." || segment == ".." {
            return Err(format!("path traverses via {segment:?}: {rel:?}"));
        }
        if is_reserved_device_name(segment) {
            return Err(format!("path names a reserved device: {segment:?}"));
        }
    }

    Ok(())
}

/// Windows reserved device names are reserved with or without an extension,
/// and case-insensitively — checked on every platform since a document might
/// be shared to a Windows machine later.
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

/// Join `rel` onto `root` and require the result to stay under the
/// canonicalized root, resolving symlinks along the way. `rel` need not
/// exist yet: the nearest existing ancestor is canonicalized and the
/// remaining components are appended, the same trick
/// `native_save::resolved_target` uses for not-yet-created files.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    is_safe_rel(rel)?;

    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("could not resolve root {root:?}: {error}"))?;
    let candidate = root.join(rel);

    let existing_ancestor = nearest_existing_ancestor(&candidate)
        .map_err(|error| format!("could not resolve {rel:?}: {error}"))?;
    let canonical_ancestor = existing_ancestor
        .canonicalize()
        .map_err(|error| format!("could not resolve {rel:?}: {error}"))?;

    if canonical_ancestor != canonical_root && !canonical_ancestor.starts_with(&canonical_root) {
        return Err(format!(
            "path resolves outside the served directory: {rel:?}"
        ));
    }

    let remainder = candidate
        .strip_prefix(&existing_ancestor)
        .map_err(|_| format!("path has an invalid parent: {rel:?}"))?;
    let resolved = canonical_ancestor.join(remainder);

    if resolved != canonical_root && !resolved.starts_with(&canonical_root) {
        return Err(format!(
            "path resolves outside the served directory: {rel:?}"
        ));
    }

    Ok(resolved)
}

fn nearest_existing_ancestor(path: &Path) -> io::Result<PathBuf> {
    let mut candidate = path;
    loop {
        match fs::symlink_metadata(candidate) {
            Ok(_) => return Ok(candidate.to_path_buf()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = candidate.parent().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("path has no existing ancestor: {path:?}"),
                    )
                })?;
            }
            Err(error) => return Err(error),
        }
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
    fn rejects_nul_and_windows_device_names() {
        for rel in [
            "a\0b", "con.md", "CON", "com1", "COM1.md", "lpt3.txt", "nul",
        ] {
            assert!(is_safe_rel(rel).is_err(), "expected {rel:?} to be rejected");
        }
        // Negative: a name that merely contains a reserved word is fine.
        assert!(is_safe_rel("console.md").is_ok());
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
