//! Project-local editor state, stored under `<project>/.waml/`.
//!
//! ```text
//! <project>/.waml/settings.json   versioned UI state for THIS project
//! <project>/.waml/README.md       written once, explaining the directory
//! ```
//!
//! This is the project-scoped counterpart to the global `~/.waml/editor.json`
//! that [`crate::config`] owns: theme and recents are properties of the user,
//! dock column widths are properties of the project being looked at. The two
//! stores share a disk seam -- [`crate::config::load_from`] /
//! [`crate::config::store_to`] give us the atomic temp-write plus rename, the
//! corrupt-file-to-`.bak` rescue, and the never-panics posture for free, and
//! being directory-injectable they let these tests run against a temp dir.
//!
//! Error posture, per the dock-splitters design: a missing or unreadable
//! `settings.json` yields defaults, malformed JSON is moved aside to
//! `settings.json.bak` and defaults are used, and a failed write returns its io
//! error for the caller to log and swallow. Losing a panel width must never cost
//! a user an edit.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::inspector_panel::INSPECTOR_W;
use crate::tree_panel::PROJECT_TREE_W;

/// Directory holding this project's editor state, relative to the project root.
pub(crate) const PROJECT_DIR: &str = ".waml";
/// Settings file inside [`PROJECT_DIR`].
const SETTINGS_FILE: &str = "settings.json";
/// Explanatory readme inside [`PROJECT_DIR`]; written once, never overwritten.
const README_FILE: &str = "README.md";
/// Current `settings.json` schema version.
const SETTINGS_VERSION: u32 = 1;

/// Prose dropped beside `settings.json` the first time it is written, for the
/// benefit of whoever finds this directory in a diff and wonders what it is.
const README_TEXT: &str = "\
# `.waml/`

This directory holds editor state for **this project**, written by the WAML
editor. Nothing in here is part of your model -- the model is the markdown in
the project itself, and the editor will load it exactly the same whether this
directory exists or not. Deleting `.waml/` costs you nothing but the layout
you had set up.

`settings.json` is where that layout lives: things like how wide you dragged
the model tree and the inspector columns. It is versioned, so a newer editor
can read a file an older one wrote.

## Should I commit this?

Most people will want to add `.waml/` to their `.gitignore`. These are your
personal window proportions, and they will churn in every diff and conflict
with everyone else's while telling no one anything about the model.

The editor deliberately does **not** write a `.gitignore` entry for you. If a
team wants to agree on a shared starting layout for a project -- a wide tree
for a deeply nested model, say -- committing this file is a perfectly
reasonable thing to do, and that should be a decision someone made on purpose
rather than one the editor made quietly on their behalf.
";

/// Persisted widths of the two dock columns, in logical pixels.
///
/// Stored **unclamped**: a width wider than today's viewport allows is clamped
/// for display at layout time, so widening the window restores the size the user
/// actually asked for. Defaults are the compiled-in column widths, referenced
/// rather than re-hardcoded so the two cannot drift apart.
#[cfg_attr(not(test), allow(dead_code))] // The shell wiring lands separately.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub(crate) struct DockWidths {
    /// Width of the left (model tree) column.
    pub tree_w: f64,
    /// Width of the right (inspector) column.
    pub inspector_w: f64,
}

impl Default for DockWidths {
    fn default() -> Self {
        Self {
            tree_w: PROJECT_TREE_W,
            inspector_w: INSPECTOR_W,
        }
    }
}

/// Contents of `<project>/.waml/settings.json`.
///
/// Every field carries `#[serde(default)]` -- exactly as `EditorConfig` does --
/// so a file written by an older editor, or one hand-edited down to `{}`, still
/// loads with the missing pieces filled in from defaults.
#[cfg_attr(not(test), allow(dead_code))] // The shell wiring lands separately.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct ProjectSettings {
    /// Schema version of this file.
    #[serde(default)]
    pub version: u32,
    /// Dock column widths; absent in a hand-trimmed file -> defaults.
    #[serde(default)]
    pub dock: DockWidths,
}

/// `<project>/.waml`.
fn project_dir(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_DIR)
}

/// Load `<project>/.waml/settings.json`.
///
/// Missing or unreadable -> defaults. Malformed JSON -> the bad file is renamed
/// to `settings.json.bak` and defaults are returned. Never panics, never errors:
/// a project whose settings cannot be read is simply a project at default
/// widths.
#[cfg_attr(not(test), allow(dead_code))] // The shell wiring lands separately.
pub(crate) fn load(project_root: &Path) -> ProjectSettings {
    crate::config::load_from(&project_dir(project_root), SETTINGS_FILE)
}

/// Store `settings` to `<project>/.waml/settings.json`, creating the directory
/// if needed, and write [`README_FILE`] alongside it the first time.
///
/// The written file is always stamped with the current [`SETTINGS_VERSION`],
/// mirroring how `config.rs` stamps `editor.json` -- the caller supplies state,
/// not a schema version.
///
/// Returns the io error on failure for the caller to log and swallow -- failing
/// to persist a column width must never surface as a dialog or block an edit.
/// The readme is best-effort and never overwritten: a user may have edited it,
/// and it is explanatory prose, not state we own after the first write.
#[cfg_attr(not(test), allow(dead_code))] // The shell wiring lands separately.
pub(crate) fn store(project_root: &Path, settings: &ProjectSettings) -> io::Result<()> {
    let dir = project_dir(project_root);
    let stamped = ProjectSettings {
        version: SETTINGS_VERSION,
        ..settings.clone()
    };
    crate::config::store_to(&dir, SETTINGS_FILE, &stamped)?;
    write_readme_once(&dir);
    Ok(())
}

/// Write `README.md` into `dir` unless it already exists. Silent on failure --
/// the settings write already succeeded and that is the part that matters.
fn write_readme_once(dir: &Path) {
    let path = dir.join(README_FILE);
    if !path.exists() {
        let _ = std::fs::write(&path, README_TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn settings(tree_w: f64, inspector_w: f64) -> ProjectSettings {
        ProjectSettings {
            version: SETTINGS_VERSION,
            dock: DockWidths {
                tree_w,
                inspector_w,
            },
        }
    }

    #[test]
    fn project_settings_defaults_track_the_column_constants() {
        let d = DockWidths::default();
        assert_eq!(d.tree_w, PROJECT_TREE_W);
        assert_eq!(d.inspector_w, INSPECTOR_W);
    }

    #[test]
    fn project_settings_round_trips_store_then_load() {
        let tmp = TempDir::new();
        let want = settings(410.0, 250.0);
        store(tmp.path(), &want).unwrap();
        assert_eq!(load(tmp.path()), want);
    }

    #[test]
    fn project_settings_store_stamps_the_current_version() {
        let tmp = TempDir::new();
        let mut unstamped = settings(300.0, 300.0);
        unstamped.version = 0;
        store(tmp.path(), &unstamped).unwrap();
        assert_eq!(load(tmp.path()).version, SETTINGS_VERSION);
    }

    #[test]
    fn project_settings_missing_file_yields_defaults() {
        let tmp = TempDir::new();
        assert_eq!(load(tmp.path()), ProjectSettings::default());
        assert_eq!(load(tmp.path()).dock, DockWidths::default());
    }

    #[test]
    fn project_settings_absent_dock_field_yields_default_widths() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SETTINGS_FILE), br#"{"version":1}"#).unwrap();
        assert_eq!(load(tmp.path()).dock, DockWidths::default());
    }

    #[test]
    fn project_settings_corrupt_json_backs_up_and_yields_defaults() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SETTINGS_FILE);
        std::fs::write(&path, b"{ not valid json ]").unwrap();

        assert_eq!(load(tmp.path()), ProjectSettings::default());
        assert!(!path.exists(), "corrupt file moved aside");
        assert!(
            dir.join("settings.json.bak").exists(),
            "corrupt file preserved to .bak"
        );
    }

    #[test]
    fn project_settings_writes_readme_on_first_store() {
        let tmp = TempDir::new();
        store(tmp.path(), &settings(300.0, 300.0)).unwrap();
        let readme = project_dir(tmp.path()).join(README_FILE);
        let text = std::fs::read_to_string(&readme).unwrap();
        assert!(text.contains(".gitignore"), "readme explains gitignoring");
        // The editor must not gitignore the directory on the user's behalf.
        assert!(!project_dir(tmp.path()).join(".gitignore").exists());
    }

    #[test]
    fn project_settings_never_overwrites_an_edited_readme() {
        let tmp = TempDir::new();
        store(tmp.path(), &settings(300.0, 300.0)).unwrap();
        let readme = project_dir(tmp.path()).join(README_FILE);
        std::fs::write(&readme, "SENTINEL: a user edited this").unwrap();

        store(tmp.path(), &settings(305.0, 315.0)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&readme).unwrap(),
            "SENTINEL: a user edited this",
            "a second store must not clobber the user's readme"
        );
        // ...while the settings themselves still updated.
        assert_eq!(load(tmp.path()).dock.tree_w, 305.0);
    }

    /// Minimal temp dir, mirroring `config.rs`'s: the repo has no temp-dir
    /// dev-dependency, so make a unique subdir under the system temp dir and
    /// remove it on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "waml-editor-proj-{}-{}-{}",
                std::process::id(),
                nanos,
                n
            ));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
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
}
