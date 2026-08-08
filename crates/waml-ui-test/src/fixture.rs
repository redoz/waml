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
    require_empty_reserved_run_root(&ownership_root, run_root)?;
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

pub(crate) fn reserve_run_root(
    workspace_root: &Path,
    run_id: &str,
    test_slug: &str,
) -> io::Result<(PathBuf, PathBuf)> {
    let ownership_root = ownership_root(workspace_root);
    fs::create_dir_all(&ownership_root)?;
    let ownership_metadata = fs::symlink_metadata(&ownership_root)?;
    reject_link_or_reparse(&ownership_root, &ownership_metadata)?;
    if !ownership_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "run ownership root is not a directory: {}",
                ownership_root.display()
            ),
        ));
    }

    let allocation_root = ownership_root.join(run_id);
    fs::create_dir(&allocation_root)?;
    let run_root = allocation_root.join(test_slug);
    if let Err(error) = fs::create_dir(&run_root) {
        return Err(match fs::remove_dir(&allocation_root) {
            Ok(()) => error,
            Err(cleanup_error) => io::Error::new(
                error.kind(),
                format!(
                    "{error}; failed to remove reservation {}: {cleanup_error}",
                    allocation_root.display()
                ),
            ),
        });
    }
    Ok((allocation_root, run_root))
}

fn require_empty_reserved_run_root(ownership_root: &Path, run_root: &Path) -> io::Result<()> {
    if !validate_cleanup_candidate(ownership_root, run_root)? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("reserved run root does not exist: {}", run_root.display()),
        ));
    }
    if fs::read_dir(run_root)?.next().transpose()?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("reserved run root is not empty: {}", run_root.display()),
        ));
    }
    Ok(())
}

pub(crate) fn cleanup_run(
    ownership_root: &Path,
    candidate: &Path,
    succeeded: bool,
) -> io::Result<()> {
    let candidate_exists = validate_cleanup_candidate(ownership_root, candidate)?;
    if !succeeded {
        return Ok(());
    }
    if !candidate_exists {
        return Ok(());
    }
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
                "path is a symbolic link or reparse point: {}",
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

fn validate_cleanup_candidate(ownership_root: &Path, candidate: &Path) -> io::Result<bool> {
    require_owned_path(ownership_root, candidate)?;

    let ownership_metadata = fs::symlink_metadata(ownership_root)?;
    reject_link_or_reparse(ownership_root, &ownership_metadata)?;
    if !ownership_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ownership root is not a directory: {}",
                ownership_root.display()
            ),
        ));
    }

    let mut nearest_existing = ownership_root.to_path_buf();
    let mut candidate_exists = true;
    let relative = candidate.strip_prefix(ownership_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "path is not below ownership root {}: {}",
                ownership_root.display(),
                candidate.display()
            ),
        )
    })?;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "cleanup path has an unsafe component: {}",
                    candidate.display()
                ),
            ));
        };
        nearest_existing.push(component);
        match fs::symlink_metadata(&nearest_existing) {
            Ok(metadata) => {
                reject_link_or_reparse(&nearest_existing, &metadata)?;
                if !metadata.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "cleanup path component is not a directory: {}",
                            nearest_existing.display()
                        ),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                nearest_existing.pop();
                candidate_exists = false;
                break;
            }
            Err(error) => return Err(error),
        }
    }

    let canonical_ownership_root = fs::canonicalize(ownership_root)?;
    let canonical_nearest_existing = fs::canonicalize(&nearest_existing)?;
    if !canonical_nearest_existing.starts_with(&canonical_ownership_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "cleanup path resolves outside ownership root {}: {}",
                canonical_ownership_root.display(),
                canonical_nearest_existing.display()
            ),
        ));
    }

    Ok(candidate_exists)
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
        workspace: crate::config::WorkspaceBinding {
            root: crate::config::WorkspaceRootFingerprint {
                title: "Mini",
                value: "/",
            },
            ready_diagram: crate::DiagramName::ORDERS,
        },
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
        fs::create_dir_all(&run_root).unwrap();

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
        fs::create_dir_all(&run_root).unwrap();
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
        fs::create_dir_all(&run_root).unwrap();
        create_file_link(&source.join("orders.md"), &source.join("linked.md")).unwrap();

        let error = stage_fixture(temp.path(), MINI, &run_root).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("symbolic link or reparse point"));
    }

    #[test]
    fn stage_fixture_refuses_a_nonempty_reserved_run_root() {
        let (temp, _, run_root) = synthetic_workspace();
        fs::create_dir_all(&run_root).unwrap();
        fs::write(run_root.join("failure.txt"), "existing evidence").unwrap();

        let error = stage_fixture(temp.path(), MINI, &run_root).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read_to_string(run_root.join("failure.txt")).unwrap(),
            "existing evidence"
        );
        assert!(!run_root.join("workspace").exists());
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
    fn failed_cleanup_refuses_a_linked_run_target() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let actual_run = ownership_root.join("actual-run");
        let linked_run = ownership_root.join("linked-run");
        fs::create_dir_all(&actual_run).unwrap();
        create_directory_link(&actual_run, &linked_run).unwrap();

        let error = cleanup_run(&ownership_root, &linked_run, false).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(actual_run.is_dir());
    }

    #[test]
    fn failed_cleanup_refuses_a_linked_ancestor() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let outside_parent = temp.path().join("outside-parent");
        let linked_parent = ownership_root.join("linked-parent");
        let candidate = linked_parent.join("run");
        fs::create_dir_all(outside_parent.join("run")).unwrap();
        fs::create_dir_all(&ownership_root).unwrap();
        create_directory_link(&outside_parent, &linked_parent).unwrap();

        let error = cleanup_run(&ownership_root, &candidate, false).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(outside_parent.join("run").is_dir());
    }

    #[test]
    fn successful_cleanup_refuses_a_dangling_link_target() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let missing_target = ownership_root.join("missing-target");
        let dangling_run = ownership_root.join("dangling-run");
        fs::create_dir_all(&ownership_root).unwrap();
        create_directory_link(&missing_target, &dangling_run).unwrap();

        let error = cleanup_run(&ownership_root, &dangling_run, true).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fs::symlink_metadata(&dangling_run).is_ok());
    }

    #[test]
    fn successful_cleanup_validates_the_nearest_existing_parent() {
        let temp = tempfile::tempdir().unwrap();
        let ownership_root = temp.path().join("target").join("waml-ui-test");
        let outside_parent = temp.path().join("outside-parent");
        let linked_parent = ownership_root.join("linked-parent");
        let missing_candidate = linked_parent.join("missing-run");
        fs::create_dir_all(&outside_parent).unwrap();
        fs::create_dir_all(&ownership_root).unwrap();
        create_directory_link(&outside_parent, &linked_parent).unwrap();

        let error = cleanup_run(&ownership_root, &missing_candidate, true).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!outside_parent.join("missing-run").exists());
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

    #[cfg(unix)]
    fn create_directory_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(source, link)
    }

    #[cfg(windows)]
    fn create_directory_link(source: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(source, link)
    }
}
