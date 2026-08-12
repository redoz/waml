use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use waml::bundle_envelope::expand_text;
use waml::host::confine::{self, ConfineError, DeviceNamePolicy, SymlinkPolicy};
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

/// The index writer's own rules on top of shared confinement: only an
/// `index.md`-basename target is acceptable (checked before any directory
/// gets created, so a non-index basename never leaves a partial directory
/// behind -- see `write_indexes_rejects_a_non_index_basename_before_writing`),
/// and confinement itself is `SymlinkPolicy::RefuseAny`: a symlinked parent
/// or target is refused categorically, even one that resolves inside the
/// root -- stricter than the wire-input/persist/native-save writers, which
/// follow in-root symlinks (`SymlinkPolicy::FollowWithinRoot`, `host::confine`).
///
/// Shared confinement also brings `confine::check_rel`, which this writer did
/// not run before -- so it is taken with `DeviceNamePolicy::Allow`, keeping
/// this caller's accepted set exactly as it was. This writer only ever writes
/// an `index.md` basename (the gate below), so a Windows reserved device name
/// could only appear as an enclosing *directory* segment (`con/index.md`),
/// which is an ordinary directory on Linux; rejecting it would fail an entire
/// `waml index --write` run on a bundle that has always been legal there,
/// which is not a change the `waml-cli-logic-seam` plan's Task 11 per-caller
/// table sanctions (it records this caller as **Unchanged**). Document
/// writers -- `persist`, the native save, `serve`'s wire input -- keep the
/// default `Reject`: they save author-visible files whose names must be
/// representable on every platform. `check_rel`'s remaining rules (interior
/// `:`, `.`/`..` segments, absolute/UNC/drive-prefixed paths) cannot reach
/// here: `IndexChange` paths come from `BundlePath`s, and `BundlePath::parse`
/// (`waml::source`) already rejects every one of those shapes.
fn resolve_index_target(
    root: &Path,
    relative: &str,
    create_parents: bool,
) -> std::io::Result<PathBuf> {
    let filename = Path::new(relative)
        .file_name()
        .ok_or_else(|| invalid_index_path("index path is empty"))?;
    if !waml::index_md::is_index_basename(&filename.to_string_lossy()) {
        return Err(invalid_index_path(format!(
            "not an index document: {relative}"
        )));
    }
    confine::resolve_under_with(
        root,
        relative,
        SymlinkPolicy::RefuseAny,
        create_parents,
        DeviceNamePolicy::Allow,
    )
    .map_err(|error| index_confine_error_to_io(relative, error))
}

fn index_confine_error_to_io(relative: &str, error: ConfineError) -> std::io::Error {
    match error {
        ConfineError::Syntactic(_) => invalid_index_path(format!("invalid index path: {relative}")),
        ConfineError::SymlinkRefused(path) => {
            escaped_index_path(format!("symlinked index path: {}", path.display()))
        }
        ConfineError::EscapesRoot(path) => {
            invalid_index_path(format!("index path escapes root: {}", path.display()))
        }
        ConfineError::NotADirectory(path) => invalid_index_path(format!(
            "index parent is not a directory: {}",
            path.display()
        )),
        ConfineError::NotAFile(path) => {
            invalid_index_path(format!("index target is not a file: {}", path.display()))
        }
        ConfineError::Io(io_error) => io_error,
    }
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

/// Every CLI-side bundle write -- `fmt --write`, `index --write`, `apply`,
/// and `serve`'s write path -- goes through here rather than calling
/// [`waml::host::persist::write_back`] directly, so that the
/// `WAML_CLI_TEST_FAIL_*` fault-injection backdoor `cli_e2e.rs` drives stays
/// scoped to *this binary*, exactly as it was before the transaction moved
/// into `waml::host::persist`.
///
/// It must not live in the shared library: `write_back` is also the native
/// editor's save path, and the editor is routinely run as a debug build
/// (`run.ps1`), so a library-side env-var backdoor would let a variable
/// exported for a CLI test silently sabotage an editor save.
pub fn write_back(
    root: &Path,
    old: &[(String, String)],
    new: &[(String, String)],
) -> std::io::Result<Vec<String>> {
    #[cfg(debug_assertions)]
    if let Some(ops) = DebugFaultFs::from_env()? {
        return waml::host::persist::write_back_injecting_ops(
            root,
            old,
            new,
            waml::host::persist::DeletePolicy::Transact,
            &ops,
        );
    }
    waml::host::persist::write_back(root, old, new)
}

/// The `WAML_CLI_TEST_FAIL_*` end-to-end fault injector: `cli_e2e.rs` cannot
/// pass an [`FsOps`](waml::host::persist::FsOps) into a spawned process, so it
/// asks for the fault through the environment instead. Debug builds only.
#[cfg(debug_assertions)]
struct DebugFaultFs {
    fail_rename_at: Option<u64>,
    rename_calls: std::sync::atomic::AtomicU64,
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
            rename_calls: std::sync::atomic::AtomicU64::new(0),
            fail_cleanup,
        }))
    }
}

#[cfg(debug_assertions)]
impl waml::host::persist::FsOps for DebugFaultFs {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        let call = self
            .rename_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
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

    /// `resolve_index_target` runs the shared `confine::check_rel`, which the
    /// pre-unification index writer did not -- with `DeviceNamePolicy::Allow`,
    /// so this writer's accepted set stays exactly what it was. A segment
    /// naming a Windows device can only be an enclosing *directory* here (the
    /// basename gate allows nothing but `index.md`), and `con/` is an ordinary
    /// directory on Linux: refusing it would fail an entire `waml index
    /// --write` run on a long-legal bundle. Resolution must therefore reach
    /// the filesystem and report the missing directory, not refuse the path.
    #[test]
    fn index_paths_accept_a_reserved_device_name_directory_segment() {
        let temp = TempDir::new();

        let error = resolve_index_target(&temp.0, "con/index.md", false).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(!temp.0.join("con").exists());
    }

    /// The same rule end to end, on the platforms where such a directory is
    /// actually creatable: `waml index --write` writes into it.
    #[cfg(unix)]
    #[test]
    fn write_indexes_writes_under_a_reserved_device_name_directory_segment() {
        let temp = TempDir::new();

        write_indexes(
            &temp.0,
            &[waml::index_md::IndexChange::Upsert {
                path: "con/index.md".into(),
                rendered: "generated".into(),
            }],
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(temp.0.join("con/index.md")).unwrap(),
            "generated"
        );
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

    /// The index writer's policy is categorical refusal
    /// (`SymlinkPolicy::RefuseAny`), not "follow and contain": a symlinked
    /// parent is refused even when it resolves *inside* the root. The two
    /// tests above only cover an out-of-root link target, which a
    /// follow-and-contain resolver would also reject -- so they cannot tell
    /// the two policies apart. This one can: if `resolve_index_target` were
    /// ever loosened to follow-and-contain, this is the test that would
    /// catch it (Task 11 of the waml-cli-logic-seam plan).
    #[test]
    fn write_indexes_rejects_an_in_root_symlinked_parent() {
        let temp = TempDir::new();
        let real = temp.0.join("real");
        fs::create_dir(&real).unwrap();
        let link = temp.0.join("linked");
        if create_dir_symlink(&real, &link).is_err() {
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
        assert!(!real.join("index.md").exists());
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
}
