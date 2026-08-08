use crate::config::FixtureDescriptor;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub(crate) fn stage_fixture(
    workspace_root: &Path,
    descriptor: FixtureDescriptor,
    run_root: &Path,
) -> io::Result<PathBuf> {
    let ownership_root = ownership_root(workspace_root);
    require_owned_path(&ownership_root, run_root)?;
    if run_root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("run root already exists: {}", run_root.display()),
        ));
    }
    let relative_path = Path::new(descriptor.relative_path);
    require_safe_relative_path(relative_path)?;
    let source = workspace_root
        .join("crates")
        .join("waml-editor")
        .join(relative_path);
    let staged = run_root.join("workspace");
    copy_entry(&source, &staged)?;
    Ok(staged)
}

pub(crate) fn cleanup_run(
    ownership_root: &Path,
    candidate: &Path,
    succeeded: bool,
) -> io::Result<()> {
    require_owned_path(ownership_root, candidate)?;
    if !succeeded {
        return Ok(());
    }
    if !candidate.exists() {
        return Ok(());
    }
    reject_link_or_reparse(candidate, &fs::symlink_metadata(candidate)?)?;
    let canonical_ownership_root = fs::canonicalize(ownership_root)?;
    let canonical_candidate = fs::canonicalize(candidate)?;
    require_owned_path(&canonical_ownership_root, &canonical_candidate)?;
    fs::remove_dir_all(candidate)
}

pub(crate) fn ownership_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join("target").join("waml-ui-test")
}

pub(crate) fn resolve_workspace_root(editor_manifest_dir: &Path) -> io::Result<PathBuf> {
    if !editor_manifest_dir.join("Cargo.toml").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "waml-editor manifest is absent below {}",
                editor_manifest_dir.display()
            ),
        ));
    }
    if editor_manifest_dir
        .file_name()
        .and_then(|name| name.to_str())
        != Some("waml-editor")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "editor manifest directory must be named waml-editor",
        ));
    }
    let crates_dir = editor_manifest_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "waml-editor manifest directory has no crates parent",
        )
    })?;
    if crates_dir.file_name().and_then(|name| name.to_str()) != Some("crates") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "waml-editor manifest directory is not below crates",
        ));
    }
    let workspace_root = crates_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "crates directory has no workspace parent",
        )
    })?;
    Ok(workspace_root.to_path_buf())
}

fn copy_entry(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    reject_link_or_reparse(source, &metadata)?;
    if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        fs::copy(source, destination)?;
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "fixture entry is not a regular file or directory: {}",
            source.display()
        ),
    ))
}

fn reject_link_or_reparse(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if is_link_or_reparse(metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "fixture entry is a symbolic link or reparse point: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn require_safe_relative_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "fixture path is not a safe relative path: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn require_owned_path(ownership_root: &Path, candidate: &Path) -> io::Result<()> {
    let contains_parent = candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir));
    if contains_parent || !candidate.starts_with(ownership_root) || candidate == ownership_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "path is not an owned run below {}: {}",
                ownership_root.display(),
                candidate.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cleanup_run, resolve_workspace_root, stage_fixture};
    use crate::config::FixtureDescriptor;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const MINI: FixtureDescriptor = FixtureDescriptor {
        relative_path: "tests/fixtures/mini",
        ready_diagram: "Orders",
    };

    fn synthetic_workspace() -> (TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let source = temp
            .path()
            .join("crates")
            .join("waml-editor")
            .join("tests")
            .join("fixtures")
            .join("mini");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.md"), b"# Mini\r\n").unwrap();
        fs::write(source.join("orders.md"), b"# Orders\n\x00tail").unwrap();
        let run_root = temp
            .path()
            .join("target")
            .join("waml-ui-test")
            .join("123-1")
            .join("ui-opens-orders");
        (temp, source, run_root)
    }

    #[test]
    fn stage_fixture_copies_every_regular_file_byte_for_byte() {
        let (temp, _, run_root) = synthetic_workspace();

        let staged = stage_fixture(temp.path(), MINI, &run_root).unwrap();

        assert_eq!(fs::read(staged.join("index.md")).unwrap(), b"# Mini\r\n");
        assert_eq!(
            fs::read(staged.join("orders.md")).unwrap(),
            b"# Orders\n\x00tail"
        );
    }

    #[test]
    fn staged_fixture_changes_do_not_modify_the_catalog_source() {
        let (temp, source, run_root) = synthetic_workspace();
        let staged = stage_fixture(temp.path(), MINI, &run_root).unwrap();

        fs::write(staged.join("orders.md"), b"changed").unwrap();

        assert_eq!(
            fs::read(source.join("orders.md")).unwrap(),
            b"# Orders\n\x00tail"
        );
    }

    #[test]
    fn stage_fixture_rejects_source_symlinks_or_reparse_points() {
        let (temp, source, run_root) = synthetic_workspace();
        create_file_link(&source.join("orders.md"), &source.join("linked.md")).unwrap();

        let error = stage_fixture(temp.path(), MINI, &run_root).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symbolic link or reparse point"));
    }

    #[test]
    fn cleanup_refuses_to_remove_the_ownership_root() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        fs::create_dir_all(&ownership_root).unwrap();

        let error = cleanup_run(&ownership_root, &ownership_root, true).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(ownership_root.exists());
    }

    #[test]
    fn cleanup_refuses_a_candidate_outside_the_ownership_root() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&ownership_root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let error = cleanup_run(&ownership_root, &outside, true).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(outside.exists());
    }

    #[test]
    fn failed_cleanup_still_refuses_a_candidate_outside_the_ownership_root() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&ownership_root).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let error = cleanup_run(&ownership_root, &outside, false).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(outside.exists());
    }

    #[test]
    fn cleanup_removes_a_successful_owned_run() {
        let (temp, _, run_root) = synthetic_workspace();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        fs::create_dir_all(&run_root).unwrap();

        cleanup_run(&ownership_root, &run_root, true).unwrap();

        assert!(!run_root.exists());
    }

    #[test]
    fn cleanup_preserves_a_failed_owned_run() {
        let (temp, _, run_root) = synthetic_workspace();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        fs::create_dir_all(&run_root).unwrap();

        cleanup_run(&ownership_root, &run_root, false).unwrap();

        assert!(run_root.exists());
    }

    #[test]
    fn workspace_root_is_the_parent_of_the_editor_crates_directory() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_dir = temp.path().join("crates").join("waml-editor");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(manifest_dir.join("Cargo.toml"), "[package]").unwrap();

        let root = resolve_workspace_root(&manifest_dir).unwrap();

        assert_eq!(root, temp.path());
    }

    #[test]
    fn workspace_root_rejects_an_editor_outside_the_crates_directory() {
        let temp = tempfile::tempdir().unwrap();
        let manifest_dir = temp.path().join("waml-editor");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(manifest_dir.join("Cargo.toml"), "[package]").unwrap();

        let error = resolve_workspace_root(&manifest_dir).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    fn create_file_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn create_file_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(source, link)
    }
}
