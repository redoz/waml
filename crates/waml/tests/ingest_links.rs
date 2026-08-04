//! Platform-gated integration tests for `waml::host::ingest::ingest_markdown`:
//! link cycles, root-escape, and cross-platform failure determinism. These
//! exercise real filesystem symlinks/junctions, so they live in `tests/`
//! rather than as unit tests inside `host/ingest.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use waml::host::ingest::{ingest_markdown, IngestErrorKind, IngestOptions};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "waml-ingest-links-{}-{}",
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

#[cfg(unix)]
#[test]
fn unix_symlink_cycle_terminates_and_is_reported_when_followed() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("order.md"), "# Order\n").unwrap();
    // A symlink back to the root creates a cycle when followed.
    symlink(&root, root.join("loop")).unwrap();

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    // The link is reachable via one legitimate path (root/loop/order.md) before
    // the walk revisits the canonical root a second time and stops -- so the
    // walk terminates deterministically and reports the cycle, without
    // recursing forever or overflowing the stack. It does not promise
    // deduplication of content reachable through more than one real path.
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.kind == IngestErrorKind::LinkCycle),
        "cycle must be reported, not silently dropped: {:?}",
        result.errors
    );
    assert!(
        !result.files.is_empty(),
        "the real file must still be reachable and ingested: {:?}",
        result.files
    );
}

#[cfg(unix)]
#[test]
fn unix_skipped_directory_symlink_is_recorded_by_default() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("order.md"), "# Order\n").unwrap();
    symlink(&root, root.join("loop")).unwrap();

    // Default options do not follow links, but a link that would otherwise
    // have been ingested (a directory or a `.md` file) must be recorded,
    // never silently dropped.
    let result = ingest_markdown(std::slice::from_ref(&root), &IngestOptions::default());

    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(result.errors[0].kind, IngestErrorKind::LinkSkipped);
    assert_eq!(result.errors[0].path, root.join("loop"));
    assert_eq!(result.files.len(), 1);
}

#[cfg(unix)]
#[test]
fn unix_skipped_markdown_file_symlink_is_recorded_by_default() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("order.md"), "# Order\n").unwrap();
    symlink(root.join("order.md"), root.join("alias.md")).unwrap();
    // A link to a non-markdown file would never have been ingested, so
    // skipping it is not a loss and is not recorded.
    fs::write(root.join("notes.txt"), "notes").unwrap();
    symlink(root.join("notes.txt"), root.join("notes-alias.txt")).unwrap();

    let result = ingest_markdown(std::slice::from_ref(&root), &IngestOptions::default());

    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(result.errors[0].kind, IngestErrorKind::LinkSkipped);
    assert_eq!(result.errors[0].path, root.join("alias.md"));
    assert_eq!(result.files.len(), 1);
}

#[cfg(unix)]
#[test]
fn unix_two_symlinks_to_one_shared_directory_are_not_a_cycle() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("shared")).unwrap();
    fs::write(root.join("shared/target.md"), "# Target\n").unwrap();
    // A diamond: two distinct links to the same shared directory. Neither is
    // on its own walk path, so neither is a cycle; the second visit is an
    // already-ingested dedup skip, not an error.
    symlink(root.join("shared"), root.join("link1")).unwrap();
    symlink(root.join("shared"), root.join("link2")).unwrap();

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    assert!(
        !result
            .errors
            .iter()
            .any(|error| error.kind == IngestErrorKind::LinkCycle),
        "a diamond is not a cycle: {:?}",
        result.errors
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        result
            .files
            .iter()
            .any(|(path, _)| path.file_name().unwrap() == "target.md"),
        "the shared target must be ingested: {:?}",
        result.files
    );
}

#[cfg(unix)]
#[test]
fn unix_symlink_escaping_the_root_is_reported_and_not_ingested() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let root = temp.0.join("root");
    let outside = temp.0.join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret.md"), "# Secret\n").unwrap();
    symlink(&outside, root.join("escape")).unwrap();

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    assert!(
        result
            .errors
            .iter()
            .any(|error| error.kind == IngestErrorKind::EscapesRoot),
        "escape must be reported: {:?}",
        result.errors
    );
    assert!(
        result.files.is_empty(),
        "content outside the root must never be ingested: {:?}",
        result.files
    );
}

/// Best-effort junction creation via `mklink /J` (no admin required, unlike
/// symlinks on Windows). Returns `None` and prints a notice when the
/// environment cannot create one, so the test degrades gracefully rather
/// than failing on a locked-down CI runner.
#[cfg(windows)]
fn try_make_junction(link: &std::path::Path, target: &std::path::Path) -> bool {
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .status();
    matches!(status, Ok(status) if status.success())
}

#[cfg(windows)]
#[test]
fn windows_junction_cycle_terminates_and_is_reported_when_followed() {
    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("order.md"), "# Order\n").unwrap();
    let link = root.join("loop");

    if !try_make_junction(&link, &root) {
        eprintln!("skipping: this environment cannot create NTFS junctions (mklink /J failed)");
        return;
    }

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    // See the unix cycle test above: the walk terminates and reports the
    // cycle, without promising dedup of content reachable via more than one
    // real path.
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.kind == IngestErrorKind::LinkCycle),
        "cycle must be reported, not silently dropped: {:?}",
        result.errors
    );
    assert!(
        !result.files.is_empty(),
        "the real file must still be reachable and ingested: {:?}",
        result.files
    );
}

#[cfg(windows)]
#[test]
fn windows_skipped_junction_is_recorded_by_default() {
    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("order.md"), "# Order\n").unwrap();
    let link = root.join("loop");

    if !try_make_junction(&link, &root) {
        eprintln!("skipping: this environment cannot create NTFS junctions (mklink /J failed)");
        return;
    }

    // Default options do not follow links, but a link that would otherwise
    // have been ingested (a directory or a `.md` file) must be recorded,
    // never silently dropped.
    let result = ingest_markdown(std::slice::from_ref(&root), &IngestOptions::default());

    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(result.errors[0].kind, IngestErrorKind::LinkSkipped);
    assert_eq!(result.errors[0].path, link);
    assert_eq!(result.files.len(), 1);
}

#[cfg(windows)]
#[test]
fn windows_two_junctions_to_one_shared_directory_are_not_a_cycle() {
    let temp = TempDir::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("shared")).unwrap();
    fs::write(root.join("shared/target.md"), "# Target\n").unwrap();

    if !try_make_junction(&root.join("link1"), &root.join("shared"))
        || !try_make_junction(&root.join("link2"), &root.join("shared"))
    {
        eprintln!("skipping: this environment cannot create NTFS junctions (mklink /J failed)");
        return;
    }

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    // A diamond: two distinct links to the same shared directory. Neither is
    // on its own walk path, so neither is a cycle; the second visit is an
    // already-ingested dedup skip, not an error.
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        result
            .files
            .iter()
            .any(|(path, _)| path.file_name().unwrap() == "target.md"),
        "the shared target must be ingested: {:?}",
        result.files
    );
}

#[cfg(windows)]
#[test]
fn windows_junction_escaping_the_root_is_reported_and_not_ingested() {
    let temp = TempDir::new();
    let root = temp.0.join("root");
    let outside = temp.0.join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret.md"), "# Secret\n").unwrap();
    let link = root.join("escape");

    if !try_make_junction(&link, &outside) {
        eprintln!("skipping: this environment cannot create NTFS junctions (mklink /J failed)");
        return;
    }

    let options = IngestOptions {
        follow_links: true,
        ..IngestOptions::default()
    };
    let result = ingest_markdown(std::slice::from_ref(&root), &options);

    assert!(
        result
            .errors
            .iter()
            .any(|error| error.kind == IngestErrorKind::EscapesRoot),
        "escape must be reported: {:?}",
        result.errors
    );
    assert!(
        result.files.is_empty(),
        "content outside the root must never be ingested: {:?}",
        result.files
    );
}

#[cfg(unix)]
#[test]
fn unix_unreadable_file_produces_a_structured_error() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new();
    let locked = temp.0.join("locked.md");
    fs::write(&locked, "# Locked\n").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let result = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());

    // Restore permissions before the TempDir drop tries to remove the tree.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();

    assert!(result.files.is_empty(), "{:?}", result.files);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].kind, IngestErrorKind::ReadFile);
}

#[test]
fn unreadable_and_non_utf8_files_produce_structured_errors_deterministically() {
    let temp = TempDir::new();
    fs::write(temp.0.join("good.md"), "# Good\n").unwrap();
    fs::write(temp.0.join("bad.md"), [0xff, 0xfe, 0x00, 0x01]).unwrap();

    // Run the walk twice: ordering of both files and errors must be
    // deterministic across runs, not an accident of directory-entry order.
    let first = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());
    let second = ingest_markdown(std::slice::from_ref(&temp.0), &IngestOptions::default());

    assert_eq!(first.files.len(), 1);
    assert_eq!(first.files[0].0.file_name().unwrap(), "good.md");
    assert_eq!(first.errors.len(), 1);
    assert_eq!(first.errors[0].kind, IngestErrorKind::NotUtf8);

    let first_paths: Vec<_> = first.files.iter().map(|(path, _)| path.clone()).collect();
    let second_paths: Vec<_> = second.files.iter().map(|(path, _)| path.clone()).collect();
    assert_eq!(first_paths, second_paths, "file ordering must be stable");

    let first_error_paths: Vec<_> = first
        .errors
        .iter()
        .map(|error| error.path.clone())
        .collect();
    let second_error_paths: Vec<_> = second
        .errors
        .iter()
        .map(|error| error.path.clone())
        .collect();
    assert_eq!(
        first_error_paths, second_error_paths,
        "error ordering must be stable"
    );
}
