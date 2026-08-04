//! A single, hardened bundle-ingest walker shared by every non-wasm host: the
//! LSP, the CLI, and the editor. Classifies entries via `fs::symlink_metadata`
//! (never `Path::is_dir`, which follows symlinks and NTFS junctions), tracks
//! visited directories when links are followed so a cycle terminates instead
//! of hanging or overflowing the stack, and reports every skip-for-cause as a
//! structured [`IngestError`] instead of silently dropping content.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Options controlling how [`ingest_markdown`] walks the filesystem.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// Follow symlinks and NTFS junctions into their targets. When `false`
    /// (the default), a *discovered* link is not recursed into or read; a
    /// skipped link whose target is a directory or a `.md` file is recorded
    /// as an [`IngestErrorKind::LinkSkipped`] error. A link passed directly
    /// as a root is still followed, since that is an explicit request rather
    /// than an incidental discovery.
    pub follow_links: bool,
    /// Skip dot-directories (`.git`, `.waml`, ...) discovered *during* the
    /// walk. A dot-directory passed directly as a root is still descended
    /// into, since that is an explicit request rather than an incidental
    /// discovery.
    pub skip_dot_dirs: bool,
    /// Reject files larger than this many bytes before reading them.
    pub max_file_bytes: Option<u64>,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            follow_links: false,
            skip_dot_dirs: true,
            max_file_bytes: None,
        }
    }
}

/// The cause of one entry being excluded from the ingested set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestErrorKind {
    /// A directory could not be listed.
    ReadDir,
    /// A file could not be read.
    ReadFile,
    /// A file's bytes were not valid UTF-8.
    NotUtf8,
    /// Following links led back to a directory already on the current walk
    /// path.
    LinkCycle,
    /// A link to a directory or a `.md` file was skipped because
    /// `follow_links` is disabled, so content behind it was not ingested.
    LinkSkipped,
    /// A link's resolved target lies outside every ingest root.
    EscapesRoot,
    /// A file exceeded `max_file_bytes`.
    TooLarge,
    /// An entry's metadata could not be read.
    Metadata,
}

/// One entry excluded from the ingested set, with the reason why.
#[derive(Debug)]
pub struct IngestError {
    pub path: PathBuf,
    pub kind: IngestErrorKind,
    /// The underlying OS error message, when the kind carries one.
    pub detail: Option<String>,
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self.kind {
            IngestErrorKind::ReadDir => "could not read directory",
            IngestErrorKind::ReadFile => "could not read file",
            IngestErrorKind::NotUtf8 => "not valid UTF-8",
            IngestErrorKind::LinkCycle => "link cycle detected",
            IngestErrorKind::LinkSkipped => "link skipped (follow_links is disabled)",
            IngestErrorKind::EscapesRoot => "link target escapes the ingest root",
            IngestErrorKind::TooLarge => "file exceeds the maximum size",
            IngestErrorKind::Metadata => "could not read metadata",
        };
        match &self.detail {
            Some(detail) => write!(formatter, "{}: {reason}: {detail}", self.path.display()),
            None => write!(formatter, "{}: {reason}", self.path.display()),
        }
    }
}

/// The result of one [`ingest_markdown`] call: every `.md` file that was
/// successfully read, and every entry that was excluded, with a cause.
pub struct Ingested {
    /// `.md` files read successfully, sorted by path.
    pub files: Vec<(PathBuf, String)>,
    pub errors: Vec<IngestError>,
}

/// Recursively collect `.md` files under `roots`, applying `options`.
///
/// Nothing is ever silently dropped: every skip-for-cause lands in
/// [`Ingested::errors`].
pub fn ingest_markdown(roots: &[PathBuf], options: &IngestOptions) -> Ingested {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    let canonical_roots: Vec<PathBuf> = roots
        .iter()
        .filter_map(|root| fs::canonicalize(root).ok())
        .collect();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut walk_path: Vec<String> = Vec::new();

    for root in roots {
        walk(
            root,
            true,
            options,
            &canonical_roots,
            &mut visited,
            &mut walk_path,
            &mut files,
            &mut errors,
        );
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ingested { files, errors }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    path: &Path,
    is_root: bool,
    options: &IngestOptions,
    canonical_roots: &[PathBuf],
    visited: &mut BTreeSet<String>,
    walk_path: &mut Vec<String>,
    files: &mut Vec<(PathBuf, String)>,
    errors: &mut Vec<IngestError>,
) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::Metadata,
                detail: Some(error.to_string()),
            });
            return;
        }
    };

    // `is_symlink` covers symlinks on every platform and NTFS junctions
    // (mount points) on Windows, but deliberately NOT other reparse tags:
    // a OneDrive/cloud-placeholder or dedup reparse point on an ordinary
    // file is not a link and must still be read as a regular file.
    let is_link = metadata.file_type().is_symlink();
    let mut entered_directory = false;
    if is_link {
        // A link passed directly as a root is an explicit request, not an
        // incidental discovery — follow it even when `follow_links` is
        // disabled, mirroring the dot-dir root exemption above.
        if !options.follow_links && !is_root {
            // A link whose target would otherwise have been ingested — a
            // directory or a `.md` file — is recorded, never silently
            // dropped. A link to anything else was never ingestible, so
            // skipping it loses nothing and is not an error.
            let target_would_be_ingested = match fs::metadata(path) {
                Ok(target) => {
                    target.is_dir()
                        || (target.is_file()
                            && path.extension().and_then(|extension| extension.to_str())
                                == Some("md"))
                }
                Err(_) => false,
            };
            if target_would_be_ingested {
                errors.push(IngestError {
                    path: path.to_path_buf(),
                    kind: IngestErrorKind::LinkSkipped,
                    detail: None,
                });
            }
            return;
        }
        let Ok(canonical) = fs::canonicalize(path) else {
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::Metadata,
                detail: None,
            });
            return;
        };
        if !canonical_roots
            .iter()
            .any(|root| canonical == *root || canonical.starts_with(root))
        {
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::EscapesRoot,
                detail: None,
            });
            return;
        }
        let key = canonical.to_string_lossy().into_owned();
        if walk_path.contains(&key) {
            // The link leads back to a directory already on the current
            // walk path: a true cycle.
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::LinkCycle,
                detail: None,
            });
            return;
        }
        if !visited.insert(key.clone()) {
            // A second, distinct link to a target already ingested earlier
            // in the walk (a diamond, not a cycle): dedup, not an error.
            return;
        }
        if path.is_dir() {
            walk_path.push(key);
            entered_directory = true;
        }
    } else if metadata.is_dir() && options.follow_links {
        // When links can be followed, a directory reached WITHOUT a link must
        // be tracked too: a link back to a regular-directory ancestor must
        // fire the cycle check on first encounter, not after re-ingesting the
        // whole ancestor tree, and a link to a directory already walked must
        // dedup instead of producing duplicate (path, text) entries.
        if let Ok(canonical) = fs::canonicalize(path) {
            let key = canonical.to_string_lossy().into_owned();
            if !visited.insert(key.clone()) {
                return;
            }
            walk_path.push(key);
            entered_directory = true;
        }
    }

    if metadata.is_dir() || (is_link && path.is_dir()) {
        if is_root || !options.skip_dot_dirs || !is_dot_dir(path) {
            match fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => walk(
                                &entry.path(),
                                false,
                                options,
                                canonical_roots,
                                visited,
                                walk_path,
                                files,
                                errors,
                            ),
                            Err(error) => errors.push(IngestError {
                                path: path.to_path_buf(),
                                kind: IngestErrorKind::ReadDir,
                                detail: Some(error.to_string()),
                            }),
                        }
                    }
                }
                Err(error) => errors.push(IngestError {
                    path: path.to_path_buf(),
                    kind: IngestErrorKind::ReadDir,
                    detail: Some(error.to_string()),
                }),
            }
        }
        if entered_directory {
            walk_path.pop();
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
        return;
    }

    if let Some(max_bytes) = options.max_file_bytes {
        if metadata.len() > max_bytes {
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::TooLarge,
                detail: None,
            });
            return;
        }
    }

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            errors.push(IngestError {
                path: path.to_path_buf(),
                kind: IngestErrorKind::ReadFile,
                detail: Some(error.to_string()),
            });
            return;
        }
    };
    match String::from_utf8(bytes) {
        Ok(text) => files.push((path.to_path_buf(), text)),
        Err(_) => errors.push(IngestError {
            path: path.to_path_buf(),
            kind: IngestErrorKind::NotUtf8,
            detail: None,
        }),
    }
}

fn is_dot_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
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
                "waml-ingest-{}-{}",
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
    fn output_is_sorted_by_path() {
        let temp = TempDir::new();
        fs::write(temp.0.join("b.md"), "b").unwrap();
        fs::write(temp.0.join("a.md"), "a").unwrap();
        fs::create_dir(temp.0.join("nested")).unwrap();
        fs::write(temp.0.join("nested/c.md"), "c").unwrap();

        let result = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let paths: Vec<_> = result.files.iter().map(|(path, _)| path.clone()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted, "output must be deterministically sorted");
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn discovered_dot_dir_is_skipped_but_explicit_dot_root_is_walked() {
        let temp = TempDir::new();
        fs::write(temp.0.join("order.md"), "order").unwrap();
        fs::create_dir(temp.0.join(".waml")).unwrap();
        fs::write(temp.0.join(".waml/README.md"), "hidden").unwrap();

        let result = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].0.file_name().unwrap(), "order.md");

        let explicit = ingest_markdown(&[temp.0.join(".waml")], &IngestOptions::default());
        assert!(explicit.errors.is_empty(), "{:?}", explicit.errors);
        assert_eq!(explicit.files.len(), 1);
        assert_eq!(explicit.files[0].0.file_name().unwrap(), "README.md");
    }

    #[test]
    fn unreadable_directory_is_reported_not_silently_dropped() {
        let temp = TempDir::new();
        let missing = temp.0.join("does-not-exist");

        let result = ingest_markdown(std::slice::from_ref(&missing), &IngestOptions::default());
        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].path, missing);
        assert_eq!(result.errors[0].kind, IngestErrorKind::Metadata);
    }

    #[test]
    fn non_utf8_file_is_reported_not_silently_dropped() {
        let temp = TempDir::new();
        let bad = temp.0.join("bad.md");
        fs::write(&bad, [0xff, 0xfe, 0x00, 0x01]).unwrap();

        let result = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());
        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].kind, IngestErrorKind::NotUtf8);
    }
}
