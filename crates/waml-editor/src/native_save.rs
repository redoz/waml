use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn save_bundle_atomic(root: &Path, bundle: &[(String, String)]) -> io::Result<()> {
    let root = root.canonicalize()?;
    let mut targets = Vec::with_capacity(bundle.len());

    // Reject every unsafe bundle path before creating directories or files.
    for (relative, _) in bundle {
        let relative = Path::new(relative);
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bundle path must be relative and traversal-free: {relative:?}"),
            ));
        }

        let target = root.join(relative);
        let parent = target
            .parent()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "bundle path has no parent")
            })?
            .to_path_buf();
        targets.push((target, parent));
    }

    // Resolve every parent before writing so directory symlinks cannot escape the opened root.
    for (_, parent) in &targets {
        fs::create_dir_all(parent)?;
        if !parent.canonicalize()?.starts_with(&root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bundle path resolves outside opened directory: {parent:?}"),
            ));
        }
    }

    for ((_, contents), (target, parent)) in bundle.iter().zip(targets) {
        write_atomic(&parent, &target, contents.as_bytes())?;
    }

    Ok(())
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
    use super::save_bundle_atomic;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
            &[("nested/diagram.md".into(), "authored".into())],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(temp.path().join("nested/diagram.md")).unwrap(),
            "authored"
        );
    }

    #[test]
    fn parent_traversal_is_rejected_without_writing_outside_root() {
        let temp = TempDir::new();
        let escape_name = format!("waml-native-save-escape-{}.md", std::process::id());
        let outside = temp.path().parent().unwrap().join(&escape_name);
        let _ = std::fs::remove_file(&outside);

        let error = save_bundle_atomic(
            temp.path(),
            &[(format!("../{escape_name}"), "escape".into())],
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!outside.exists());
    }
}
