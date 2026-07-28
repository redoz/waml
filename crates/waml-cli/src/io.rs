use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use waml::parse::split_bundle;

/// Turn one file's text into `(path, content)` docs: split on `<!-- path -->`
/// markers if present, otherwise a single doc keyed by `display_path`.
pub fn expand_text(display_path: &str, text: &str) -> Vec<(String, String)> {
    if text.contains("<!--") {
        let parts = split_bundle(text);
        // split_bundle returns "pasted/doc.md" for unmarked text; only trust it if markers existed.
        if parts.len() > 1
            || parts
                .first()
                .map(|(p, _)| p != "pasted/doc.md")
                .unwrap_or(false)
        {
            return parts;
        }
    }
    vec![(display_path.to_string(), text.to_string())]
}

/// Recursively collect `.md` files under the given files/directories.
pub fn collect_md(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in fs::read_dir(p)? {
                let path = entry?.path();
                out.extend(collect_md(&[path])?);
            }
        } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(p.clone());
        }
    }
    out.sort();
    Ok(out)
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
        let files = expand_text("stdin.md", &buf);
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
        let rel = root
            .as_ref()
            .and_then(|r| file.strip_prefix(r).ok())
            .unwrap_or(file);
        let expanded = expand_text(&path_key(rel), &text);
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
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let logical = path_key(relative);
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
    write_back_with_ops(root, old, new, &RealFs)
}

trait FsOps {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
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
            let key = target.to_string_lossy().to_lowercase();
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
        let rollback = rollback(&journal, &created_directories);
        let _ = fs::remove_dir_all(&staging);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback) => Err(std::io::Error::new(
                error.kind(),
                format!("{error}; rollback failed: {rollback}"),
            )),
        };
    }
    fs::remove_dir_all(&staging)?;
    Ok(touched)
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
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
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

fn rollback(journal: &[JournalEntry], created: &[PathBuf]) -> std::io::Result<()> {
    let mut first_error = None;
    for entry in journal.iter().rev() {
        let result = match entry {
            JournalEntry::Updated { target, backup } => {
                let _ = fs::remove_file(target);
                fs::rename(backup, target)
            }
            JournalEntry::Added { target } => fs::remove_file(target),
            JournalEntry::Deleted { target, backup } => fs::rename(backup, target),
        };
        if let Err(error) = result {
            first_error.get_or_insert(error);
        }
    }
    for directory in created.iter().rev() {
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
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
                "waml-cli-io-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct FailRename {
        at: usize,
        calls: AtomicUsize,
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
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("injected rename failure at call {call}"),
                ))
            } else {
                std::fs::rename(from, to)
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
    fn expands_blob_text_into_docs() {
        let blob = "<!-- a/one.md -->\n# One\n\n<!-- a/two.md -->\n# Two\n";
        let docs = expand_text("stdin", blob);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].0, "a/one.md");
    }

    #[test]
    fn plain_text_uses_its_own_path() {
        let docs = expand_text("shop/order.md", "# Order\n");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].0, "shop/order.md");
    }

    #[test]
    fn stray_comment_doc_is_one_doc() {
        // A single .md doc that happens to contain a lone, non-marker HTML
        // comment (e.g. a review note) must not be split apart: it must
        // come back as exactly one doc, keyed by its real display path,
        // with the full content — including the unresolved relationship
        // section that follows the stray comment — intact.
        let text = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n<!-- reviewed: needs follow-up -->\n\n## Relationships\n- depends [Ghost](./ghost.md)\n";
        let docs = expand_text("shop/order.md", text);
        assert_eq!(
            docs.len(),
            1,
            "a stray non-.md comment must not split the document"
        );
        assert_eq!(docs[0].0, "shop/order.md");
        assert_eq!(
            docs[0].1, text,
            "content must be kept intact, nothing discarded"
        );
    }

    #[test]
    fn read_ndjson_skips_blanks_and_numbers_lines() {
        // write a temp file
        let dir = std::env::temp_dir().join(format!("waml_ndjson_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ops.ndjson");
        std::fs::write(&f, "{\"op\":\"a\"}\n\n{\"op\":\"b\"}\n").unwrap();
        let lines = read_ndjson(f.to_str().unwrap()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, 1);
        assert_eq!(lines[1].0, 3, "blank line 2 skipped, numbering preserved");
    }

    #[test]
    fn late_write_failure_restores_updates_and_removes_new_artifacts() {
        let temp = TempDir::new();
        std::fs::write(temp.0.join("a.md"), "before").unwrap();
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
        assert_eq!(
            std::fs::read_to_string(temp.0.join("a.md")).unwrap(),
            "before"
        );
        assert!(!temp.0.join("b").exists());
        assert!(!temp.0.join("z").exists());
        assert_eq!(directory_entries(&temp.0), ["a.md"]);
    }

    #[test]
    fn late_delete_failure_restores_prior_write_and_delete() {
        let temp = TempDir::new();
        std::fs::write(temp.0.join("a.md"), "before").unwrap();
        std::fs::write(temp.0.join("m-delete.md"), "keep me").unwrap();
        std::fs::write(temp.0.join("z-delete.md"), "keep me too").unwrap();
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
        assert_eq!(
            std::fs::read_to_string(temp.0.join("a.md")).unwrap(),
            "before"
        );
        assert_eq!(
            std::fs::read_to_string(temp.0.join("m-delete.md")).unwrap(),
            "keep me"
        );
        assert_eq!(
            std::fs::read_to_string(temp.0.join("z-delete.md")).unwrap(),
            "keep me too"
        );
        assert_eq!(
            directory_entries(&temp.0),
            ["a.md", "m-delete.md", "z-delete.md"]
        );
    }

    #[test]
    fn successful_transaction_adds_updates_and_deletes_as_one_set() {
        let temp = TempDir::new();
        std::fs::write(temp.0.join("update.md"), "before").unwrap();
        std::fs::write(temp.0.join("delete.md"), "remove").unwrap();
        let permissions = std::fs::metadata(temp.0.join("update.md"))
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
            std::fs::read_to_string(temp.0.join("nested/add.md")).unwrap(),
            "added"
        );
        assert_eq!(
            std::fs::read_to_string(temp.0.join("update.md")).unwrap(),
            "after"
        );
        assert_eq!(
            std::fs::metadata(temp.0.join("update.md"))
                .unwrap()
                .permissions()
                .readonly(),
            permissions.readonly()
        );
        assert!(!temp.0.join("delete.md").exists());
        assert_eq!(directory_entries(&temp.0), ["nested", "update.md"]);
    }

    fn directory_entries(path: &Path) -> Vec<String> {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }
}
