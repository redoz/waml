use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use waml::source::SourceBundle;

use crate::editor_session::{SaveCompletion, SaveTicket};

pub(crate) fn save_bundle_atomic(
    root: &Path,
    baseline: &SourceBundle,
    current: &SourceBundle,
) -> io::Result<()> {
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("opened bundle root is not a directory: {root:?}"),
        ));
    }

    let baseline_by_path = index_bundle(baseline, "baseline")?;
    let current_by_path = index_bundle(current, "current")?;
    for document in baseline.documents() {
        let relative = Path::new(document.path().as_str());
        if !current_by_path.contains_key(&bundle_path_key(relative)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "removing bundle files is not supported by atomic save: {:?}",
                    relative
                ),
            ));
        }
    }
    let mut resolved_targets = BTreeSet::new();
    let mut validated_targets = Vec::with_capacity(current.len());
    for document in current.documents() {
        let relative = Path::new(document.path().as_str());
        let target = root.join(relative);
        let parent = target
            .parent()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "bundle path has no parent")
            })?
            .to_path_buf();
        let resolved_target = resolved_target(&root, &parent, &target)?;
        if !resolved_targets.insert(filesystem_path_key(&resolved_target)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bundle contains paths resolving to the same target: {target:?}"),
            ));
        }
        validated_targets.push((target, parent));
    }

    let mut planned = Vec::new();
    // Plan every dirty target using only reads. Clean paths participate in
    // containment and alias validation above, but their contents are not read or
    // rewritten, so an unrelated external edit survives this save.
    for (document, (target, parent)) in current.documents().iter().zip(validated_targets) {
        let relative = Path::new(document.path().as_str());
        let desired = document.text();
        let baseline = baseline_by_path.get(&bundle_path_key(relative)).copied();
        if baseline == Some(desired) {
            continue;
        }
        if disk_state(&root, &target, baseline, desired)? == DiskState::NeedsWrite {
            planned.push(PlannedWrite {
                target,
                parent,
                baseline,
                desired,
            });
        }
    }

    // All lexical, containment, alias, and conflict checks above complete before
    // the first directory or file mutation. Operational failures (permissions,
    // full disks) can still occur while materializing a previously missing path.
    for write in &planned {
        fs::create_dir_all(&write.parent)?;
    }

    // Directory creation can expose a concurrent link swap, so re-check the
    // entire pending set before the first file replacement.
    let mut pending = Vec::with_capacity(planned.len());
    for write in planned {
        let _ = resolved_target(&root, &write.parent, &write.target)?;
        if disk_state(&root, &write.target, write.baseline, write.desired)? == DiskState::NeedsWrite
        {
            pending.push(write);
        }
    }

    for write in pending {
        // This final read narrows, but cannot eliminate, the filesystem TOCTOU
        // window between validation and atomic replacement. Preventing that
        // fully requires directory-handle-relative APIs unavailable in std.
        let _ = resolved_target(&root, &write.parent, &write.target)?;
        if disk_state(&root, &write.target, write.baseline, write.desired)? == DiskState::NeedsWrite
        {
            write_atomic(&write.parent, &write.target, write.desired.as_bytes())?;
        }
    }

    Ok(())
}

pub(crate) fn save_ticket_atomic(root: &Path, ticket: &SaveTicket) -> io::Result<SaveCompletion> {
    if ticket.snapshot.dirty_revision != Some(ticket.revision)
        || ticket.snapshot.revision != ticket.revision
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to save a clean or mismatched editor ticket",
        ));
    }
    let result = save_bundle_atomic(
        root,
        &ticket.snapshot.persisted_source,
        &ticket.snapshot.source,
    )
    .map_err(|error| error.to_string());
    Ok(SaveCompletion {
        revision: ticket.revision,
        history_state: ticket.history_state,
        result,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiskState {
    AlreadyDesired,
    NeedsWrite,
}

struct PlannedWrite<'a> {
    target: PathBuf,
    parent: PathBuf,
    baseline: Option<&'a str>,
    desired: &'a str,
}

fn index_bundle<'a>(
    bundle: &'a SourceBundle,
    bundle_name: &str,
) -> io::Result<BTreeMap<String, &'a str>> {
    let mut indexed = BTreeMap::new();
    for document in bundle.documents() {
        let relative = Path::new(document.path().as_str());
        validate_relative_path(relative)?;
        if indexed
            .insert(bundle_path_key(relative), document.text())
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{bundle_name} bundle contains duplicate path: {relative:?}"),
            ));
        }
    }
    Ok(indexed)
}

fn validate_relative_path(relative: &Path) -> io::Result<()> {
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| match component {
            // A colon inside a name is an NTFS alternate-data-stream write
            // (`a:b.md`), not a file the user can see; reject it everywhere
            // so bundles behave identically across platforms. This mirrors
            // `BundlePath::parse` (the authoritative upper layer, which
            // already rejects any colon) and the CLI's `validate_relative`
            // (`waml-cli/src/io.rs`) — a defense layer that silently
            // disagreed with the layer above it would be worse than none.
            Component::Normal(name) => name.to_string_lossy().contains(':'),
            _ => true,
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bundle path must be relative and traversal-free: {relative:?}"),
        ));
    }
    Ok(())
}

fn bundle_path_key(path: &Path) -> String {
    let key = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    normalize_path_key(key)
}

fn filesystem_path_key(path: &Path) -> String {
    normalize_path_key(path.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn normalize_path_key(key: String) -> String {
    key.replace('\\', "/").to_lowercase()
}

#[cfg(not(windows))]
fn normalize_path_key(key: String) -> String {
    key
}

fn resolved_target(root: &Path, parent: &Path, target: &Path) -> io::Result<PathBuf> {
    let existing_parent = nearest_existing_ancestor(parent)?;
    let canonical_parent = existing_parent.canonicalize()?;
    ensure_within_root(root, &canonical_parent, parent)?;
    if !fs::metadata(&existing_parent)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bundle path parent is not a directory: {existing_parent:?}"),
        ));
    }

    match fs::symlink_metadata(target) {
        Ok(_) => {
            let canonical_target = target.canonicalize()?;
            ensure_within_root(root, &canonical_target, target)?;
            Ok(canonical_target)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let missing_parent = parent.strip_prefix(&existing_parent).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("bundle path has an invalid parent: {parent:?}"),
                )
            })?;
            let file_name = target.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "bundle path has no file name")
            })?;
            Ok(canonical_parent.join(missing_parent).join(file_name))
        }
        Err(error) => Err(error),
    }
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
                        format!("bundle path has no existing ancestor: {path:?}"),
                    )
                })?;
            }
            Err(error) => return Err(error),
        }
    }
}

fn ensure_within_root(root: &Path, canonical: &Path, original: &Path) -> io::Result<()> {
    if canonical == root || canonical.starts_with(root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bundle path resolves outside opened directory: {original:?}"),
        ))
    }
}

fn disk_state(
    root: &Path,
    target: &Path,
    baseline: Option<&str>,
    desired: &str,
) -> io::Result<DiskState> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            let canonical_target = target.canonicalize()?;
            ensure_within_root(root, &canonical_target, target)?;
            let disk = fs::read(target)?;
            if disk == desired.as_bytes() {
                Ok(DiskState::AlreadyDesired)
            } else if baseline.is_some_and(|baseline| disk == baseline.as_bytes()) {
                Ok(DiskState::NeedsWrite)
            } else {
                Err(save_conflict(target, "changed on disk since it was loaded"))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if baseline.is_none() {
                Ok(DiskState::NeedsWrite)
            } else {
                Err(save_conflict(target, "was removed after it was loaded"))
            }
        }
        Err(error) => Err(error),
    }
}

fn save_conflict(target: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("save conflict for {target:?}: {reason}"),
    )
}

fn write_atomic(parent: &Path, target: &Path, contents: &[u8]) -> io::Result<()> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    let (temp, mut file) = loop {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!(".waml-save-{}-{nonce}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp, target)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(windows)]
fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let existing: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let replacement: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            existing.as_ptr(),
            replacement.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{save_bundle_atomic as save_source_bundle_atomic, save_ticket_atomic};
    use std::path::{Path, PathBuf};
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use waml::source::SourceBundle;
    use waml_syntax::{SourceText, TextChange, TextRange, TextSize};

    fn save_bundle_atomic(
        root: &Path,
        baseline: &[(String, String)],
        current: &[(String, String)],
    ) -> std::io::Result<()> {
        let invalid_source = |error: waml::source::SourceError| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
        };
        let baseline =
            SourceBundle::try_from_pairs(baseline.iter().cloned()).map_err(invalid_source)?;
        let current =
            SourceBundle::try_from_pairs(current.iter().cloned()).map_err(invalid_source)?;
        save_source_bundle_atomic(root, &baseline, &current)
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "waml-native-save-{}-{nanos}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn existing_file_is_replaced_with_bundle_contents() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "old").unwrap();

        save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "old".into())],
            &[("diagram.md".into(), "new authored display".into())],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(target).unwrap(),
            "new authored display"
        );
    }

    #[test]
    fn nested_bundle_paths_are_created() {
        let temp = TempDir::new();

        save_bundle_atomic(
            temp.path(),
            &[],
            &[("nested/diagram.md".into(), "authored".into())],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(temp.path().join("nested/diagram.md")).unwrap(),
            "authored"
        );
    }

    #[test]
    fn colon_segments_are_rejected_by_validate_relative_path() {
        use super::validate_relative_path;

        assert_eq!(
            validate_relative_path(Path::new("ab:c.md"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            validate_relative_path(Path::new("nested/ab:c.md"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn colon_free_segments_still_pass_validate_relative_path() {
        use super::validate_relative_path;

        assert!(validate_relative_path(Path::new("abc.md")).is_ok());
        assert!(validate_relative_path(Path::new("nested/abc.md")).is_ok());
    }

    #[test]
    fn parent_traversal_is_rejected_without_writing_outside_root() {
        let temp = TempDir::new();
        let escape_name = format!("waml-native-save-escape-{}.md", std::process::id());
        let outside = temp.path().parent().unwrap().join(&escape_name);
        let _ = std::fs::remove_file(&outside);

        let error = save_bundle_atomic(
            temp.path(),
            &[],
            &[(format!("../{escape_name}"), "escape".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.exists());
    }

    #[test]
    fn external_edit_to_unchanged_file_survives_dirty_save() {
        let temp = TempDir::new();
        let target = temp.path().join("unchanged.md");
        std::fs::write(&target, "loaded").unwrap();
        std::fs::write(temp.path().join("dirty.md"), "before").unwrap();
        let baseline = vec![
            ("unchanged.md".into(), "loaded".into()),
            ("dirty.md".into(), "before".into()),
        ];
        std::fs::write(&target, "external edit").unwrap();

        save_bundle_atomic(
            temp.path(),
            &baseline,
            &[
                ("unchanged.md".into(), "loaded".into()),
                ("dirty.md".into(), "after".into()),
            ],
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(target).unwrap(), "external edit");
        assert_eq!(
            std::fs::read_to_string(temp.path().join("dirty.md")).unwrap(),
            "after"
        );
    }

    #[test]
    fn external_edit_to_dirty_file_is_a_conflict_and_is_not_overwritten() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "loaded").unwrap();
        std::fs::write(&target, "external edit").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "loaded".into())],
            &[("diagram.md".into(), "editor edit".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "external edit");
    }

    #[test]
    fn already_desired_file_makes_failed_batch_retry_idempotent() {
        let temp = TempDir::new();
        let target = temp.path().join("diagram.md");
        std::fs::write(&target, "editor edit").unwrap();
        let identity_before = file_identity(&target);
        std::thread::sleep(std::time::Duration::from_millis(20));

        save_bundle_atomic(
            temp.path(),
            &[("diagram.md".into(), "loaded".into())],
            &[("diagram.md".into(), "editor edit".into())],
        )
        .unwrap();

        assert_eq!(file_identity(&target), identity_before);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "editor edit");
    }

    #[test]
    fn duplicate_current_paths_are_rejected_before_writing() {
        let temp = TempDir::new();

        let error = save_bundle_atomic(
            temp.path(),
            &[],
            &[
                ("duplicate.md".into(), "first".into()),
                ("duplicate.md".into(), "second".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("duplicate.md").exists());
    }

    #[test]
    fn duplicate_baseline_paths_are_rejected_before_writing() {
        let temp = TempDir::new();

        let error = save_bundle_atomic(
            temp.path(),
            &[
                ("duplicate.md".into(), "first".into()),
                ("duplicate.md".into(), "second".into()),
            ],
            &[("safe/new.md".into(), "new".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("safe").exists());
    }

    #[test]
    fn clean_session_has_no_save_ticket() {
        let temp = TempDir::new();
        let session = crate::editor_session::EditorSession::default();

        assert!(session.save_ticket().is_none());
        assert!(std::fs::read_dir(temp.path()).unwrap().next().is_none());
    }

    #[test]
    fn invalid_source_ticket_is_saved_byte_exactly() {
        let temp = TempDir::new();
        let path = waml::source::BundlePath::parse("order.md").unwrap();
        let original = "# Order\r\n";
        let invalid = "# Order\r\n[unterminated **\u{2028}";
        std::fs::write(temp.path().join("order.md"), original.as_bytes()).unwrap();
        let mut session = crate::editor_session::EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs([("order.md", original)]).unwrap())
            .unwrap();
        let before = session.snapshot();
        let document = before.okf_analysis.catalog.id_for_path(&path).unwrap();
        let syntax = before.markdown_snapshot(document).unwrap();
        session
            .apply(waml::edit::ExactSourceEdit {
                document,
                base_revision: syntax.revision(),
                changes: Arc::from([TextChange {
                    old_range: TextRange::new(TextSize::new(0), syntax.text().len()).unwrap(),
                    replacement: Arc::from(invalid),
                }]),
                expected_text: SourceText::new(invalid.to_string()).unwrap(),
            })
            .unwrap();

        let ticket = session.save_ticket().unwrap();
        let completion = save_ticket_atomic(temp.path(), &ticket).unwrap();

        assert_eq!(completion.revision, ticket.revision);
        assert_eq!(completion.history_state, ticket.history_state);
        assert_eq!(completion.result, Ok(()));
        assert_eq!(
            std::fs::read(temp.path().join("order.md")).unwrap(),
            invalid.as_bytes()
        );
    }

    #[test]
    fn removed_bundle_path_is_rejected_before_earlier_mutation() {
        let temp = TempDir::new();
        let removed = temp.path().join("removed.md");
        std::fs::write(&removed, "loaded").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("removed.md".into(), "loaded".into())],
            &[("safe/new.md".into(), "new".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!temp.path().join("safe").exists());
        assert_eq!(std::fs::read_to_string(removed).unwrap(), "loaded");
    }

    #[test]
    fn later_conflict_does_not_create_earlier_safe_parent() {
        let temp = TempDir::new();
        std::fs::write(temp.path().join("conflict.md"), "external edit").unwrap();

        let error = save_bundle_atomic(
            temp.path(),
            &[("conflict.md".into(), "loaded".into())],
            &[
                ("safe/new.md".into(), "new".into()),
                ("conflict.md".into(), "editor edit".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(!temp.path().join("safe").exists());
        assert_eq!(
            std::fs::read_to_string(temp.path().join("conflict.md")).unwrap(),
            "external edit"
        );
    }

    #[test]
    fn linked_parent_escape_is_rejected_before_creating_missing_descendants() {
        let root = TempDir::new();
        let outside = TempDir::new();
        if !make_dir_link(outside.path(), &root.path().join("linked")) {
            return;
        }

        let error = save_bundle_atomic(
            root.path(),
            &[],
            &[("linked/missing/diagram.md".into(), "escape".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.path().join("missing").exists());
    }

    #[test]
    fn later_unsafe_target_does_not_create_earlier_safe_parent() {
        let root = TempDir::new();
        let outside = TempDir::new();
        if !make_dir_link(outside.path(), &root.path().join("linked")) {
            return;
        }

        let error = save_bundle_atomic(
            root.path(),
            &[],
            &[
                ("safe/new.md".into(), "new".into()),
                ("linked/missing/diagram.md".into(), "escape".into()),
            ],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!root.path().join("safe").exists());
        assert!(!outside.path().join("missing").exists());
    }

    #[test]
    fn clean_and_dirty_aliases_are_rejected_before_mutating_shared_target() {
        let root = TempDir::new();
        let real = root.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let target = real.join("diagram.md");
        std::fs::write(&target, "loaded").unwrap();
        if !make_dir_link(&real, &root.path().join("linked")) {
            return;
        }

        let baseline = vec![
            ("linked/diagram.md".into(), "loaded".into()),
            ("real/diagram.md".into(), "loaded".into()),
        ];
        let current = vec![
            ("linked/diagram.md".into(), "editor edit".into()),
            ("real/diagram.md".into(), "loaded".into()),
        ];

        let error = save_bundle_atomic(root.path(), &baseline, &current).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(std::fs::read_to_string(target).unwrap(), "loaded");
    }

    #[cfg(windows)]
    fn file_identity(path: &Path) -> u64 {
        use std::os::windows::fs::MetadataExt;

        std::fs::metadata(path).unwrap().creation_time()
    }

    #[cfg(unix)]
    fn file_identity(path: &Path) -> u64 {
        use std::os::unix::fs::MetadataExt;

        std::fs::metadata(path).unwrap().ino()
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
