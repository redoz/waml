//! Project-local editor state, stored under `<project>/.waml/`.
//!
//! ```text
//! <project>/.waml/editor.json   versioned UI state for THIS project
//! <project>/.waml/README.md     written once, explaining the directory
//! ```
//!
//! This is the project-scoped counterpart to the global `~/.waml/editor.json`
//! that [`crate::config`] owns: theme and recents are properties of the user,
//! dock column widths are properties of the project being looked at. Same file
//! name in both scopes, deliberately -- the scope is the directory, not the
//! spelling. The two stores share a disk seam -- [`crate::config::load_from`] /
//! [`crate::config::store_to`] give us the atomic temp-write plus rename, the
//! corrupt-file-to-`.bak` rescue, and the never-panics posture for free, and
//! being directory-injectable they let these tests run against a temp dir.
//!
//! Error posture, per the dock-splitters design: a missing or unreadable
//! `editor.json` yields defaults, malformed JSON is moved aside to
//! `editor.json.bak` and defaults are used, and a failed write returns its io
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
const EDITOR_FILE: &str = "editor.json";
/// What [`EDITOR_FILE`] was called before it was renamed to match the global
/// `~/.waml/editor.json`. Read once, on [`load`], when the current name is not
/// there yet, so a project that already had a layout keeps it; the next
/// [`store`] writes the current name and this one stops being consulted.
const LEGACY_SETTINGS_FILE: &str = "settings.json";
/// Explanatory readme inside [`PROJECT_DIR`]; written once, never overwritten.
const README_FILE: &str = "README.md";
/// Current `editor.json` schema version.
const SETTINGS_VERSION: u32 = 1;

/// Prose dropped beside `editor.json` the first time it is written, for the
/// benefit of whoever finds this directory in a diff and wonders what it is.
const README_TEXT: &str = "\
# `.waml/`

This directory holds settings for **this project**, written by WAML tooling.
Nothing in here is part of your model -- the model is the markdown in the
project itself, and it will load whether this directory exists or not.

There are two files, and they belong to different people:

`editor.json` is yours. It holds the layout you set up: things like how wide
you dragged the model tree and the inspector columns.

`project.json` is the project's. It holds settings about how the model itself
is read -- currently `max_view_depth`, how far a folder view will follow its
view chain before giving up. Any WAML tool reads it, not just the editor.

Both are versioned, so newer tooling can read a file an older version wrote.

## Should I commit this?

Different answers for the two files, which is why they are two files.

Most people will want to gitignore `editor.json`. Those are your personal
window proportions: they churn in every diff and conflict with everyone
else's, while telling no one anything about the model.

`project.json` is usually worth committing. It says something true about the
model rather than about your screen, and a team is better off agreeing on it
than each discovering it separately.

The tooling deliberately does **not** write a `.gitignore` entry for you.
Committing your layout is a perfectly reasonable thing for a team to do --
a wide tree for a deeply nested model, say -- and that should be a decision
someone made on purpose rather than one made quietly on their behalf.
";

/// Persisted widths of the two dock columns, in logical pixels.
///
/// Stored **unclamped**: a width wider than today's viewport allows is clamped
/// for display at layout time, so widening the window restores the size the user
/// actually asked for. Defaults are the compiled-in column widths, referenced
/// rather than re-hardcoded so the two cannot drift apart.
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

/// Contents of `<project>/.waml/editor.json`.
///
/// ONE PERSON'S VIEW OF A PROJECT, and nothing else. How a project wants its
/// views resolved lives in `.waml/project.json`
/// ([`waml::view::chain::ProjectConfig`], read by
/// [`crate::project_config`]) -- that is a property of the model, readable by
/// any waml tool, and a thing a team may well commit; this file is a column
/// width on one person's desk.
///
/// Every field carries `#[serde(default)]` -- exactly as `EditorConfig` does --
/// so a file written by an older editor, or one hand-edited down to `{}`, still
/// loads with the missing pieces filled in from defaults.
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

/// Load `<project>/.waml/editor.json`, falling back to the pre-rename
/// [`LEGACY_SETTINGS_FILE`] when only that one is on disk.
///
/// The legacy file also carried `max_view_depth`, which now belongs to
/// `.waml/project.json`. It is NOT migrated: [`ProjectSettings`] has no such
/// field, so serde drops it, and a project that had one hand-set goes back to
/// the runner's default depth until it is set again in the new file. A clean
/// break, deliberately -- see the module docs on `project_config`.
///
/// Missing or unreadable -> defaults. Malformed JSON -> the bad file is renamed
/// to `editor.json.bak` and defaults are returned. Never panics, never errors:
/// a project whose settings cannot be read is simply a project at default
/// widths.
pub(crate) fn load(project_root: &Path) -> ProjectSettings {
    let dir = project_dir(project_root);
    // The current name wins whenever it exists, INCLUDING when it is corrupt:
    // falling through to the legacy file on a parse failure would resurrect a
    // layout the user has since changed, and would hide the `.bak` rescue.
    let file = if !dir.join(EDITOR_FILE).exists() && dir.join(LEGACY_SETTINGS_FILE).exists() {
        LEGACY_SETTINGS_FILE
    } else {
        EDITOR_FILE
    };
    crate::config::load_from(&dir, file)
}

/// Store `settings` to `<project>/.waml/editor.json`, creating the directory
/// if needed, and write [`README_FILE`] alongside it the first time.
///
/// Always writes the current name; a leftover [`LEGACY_SETTINGS_FILE`] is left
/// where it is rather than deleted. It is inert once `editor.json` exists, and
/// the editor removing a file out of the user's project to tidy up after itself
/// is a worse trade than one stale file.
///
/// The written file is always stamped with the current [`SETTINGS_VERSION`],
/// mirroring how `config.rs` stamps `editor.json` -- the caller supplies state,
/// not a schema version.
///
/// Returns the io error on failure for the caller to log and swallow -- failing
/// to persist a column width must never surface as a dialog or block an edit.
/// The readme is best-effort and never overwritten: a user may have edited it,
/// and it is explanatory prose, not state we own after the first write.
pub(crate) fn store(project_root: &Path, settings: &ProjectSettings) -> io::Result<()> {
    let dir = project_dir(project_root);
    let stamped = ProjectSettings {
        version: SETTINGS_VERSION,
        ..settings.clone()
    };
    crate::config::store_to(&dir, EDITOR_FILE, &stamped)?;
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
pub(crate) mod tests {
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
        std::fs::write(dir.join(EDITOR_FILE), br#"{"version":1}"#).unwrap();
        assert_eq!(load(tmp.path()).dock, DockWidths::default());
    }

    #[test]
    fn project_settings_corrupt_json_backs_up_and_yields_defaults() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(EDITOR_FILE);
        std::fs::write(&path, b"{ not valid json ]").unwrap();

        assert_eq!(load(tmp.path()), ProjectSettings::default());
        assert!(!path.exists(), "corrupt file moved aside");
        assert!(
            dir.join("editor.json.bak").exists(),
            "corrupt file preserved to .bak"
        );
    }

    /// The file was called `settings.json` before it was renamed to match the
    /// global `~/.waml/editor.json`. A project that already had a layout must
    /// keep it rather than silently snapping back to default widths.
    #[test]
    fn project_settings_read_the_pre_rename_file_when_it_is_the_only_one() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(LEGACY_SETTINGS_FILE),
            br#"{"version":1,"dock":{"tree_w":410.0,"inspector_w":250.0},"max_view_depth":7}"#,
        )
        .unwrap();

        assert_eq!(load(tmp.path()).dock.tree_w, 410.0);

        // The next store writes the current name, and the legacy file stops
        // being consulted from then on -- including when the two disagree.
        store(tmp.path(), &settings(305.0, 315.0)).unwrap();
        assert!(dir.join(EDITOR_FILE).exists());
        assert_eq!(load(tmp.path()).dock.tree_w, 305.0);
    }

    /// The layout migrates; the view config does NOT. `max_view_depth` moved to
    /// `.waml/project.json`, and reading it back out of a legacy EDITOR file to
    /// seed a non-editor one would re-create the tangle the split undoes. The
    /// field simply is not in this struct, so serde drops it -- assert that,
    /// because "the depth quietly stopped applying" is the failure mode.
    #[test]
    fn the_pre_split_depth_cap_is_dropped_rather_than_migrated() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(LEGACY_SETTINGS_FILE),
            br#"{"version":1,"dock":{"tree_w":410.0,"inspector_w":250.0},"max_view_depth":7}"#,
        )
        .unwrap();

        // The layout survives...
        assert_eq!(load(tmp.path()).dock.tree_w, 410.0);
        // ...and the depth cap is not something this file can carry at all.
        store(tmp.path(), &load(tmp.path())).unwrap();
        let written = std::fs::read_to_string(dir.join(EDITOR_FILE)).unwrap();
        assert!(
            !written.contains("max_view_depth"),
            "the editor's file must not round-trip a view setting: {written}"
        );
    }

    /// A corrupt `editor.json` must take the `.bak` rescue, NOT fall through to
    /// a stale legacy file: resurrecting an old layout would look like the
    /// editor ignoring everything the user has done since the rename.
    #[test]
    fn a_corrupt_current_file_does_not_fall_back_to_the_legacy_one() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(LEGACY_SETTINGS_FILE),
            br#"{"version":1,"dock":{"tree_w":410.0,"inspector_w":250.0}}"#,
        )
        .unwrap();
        std::fs::write(dir.join(EDITOR_FILE), b"{ not valid json ]").unwrap();

        assert_eq!(load(tmp.path()), ProjectSettings::default());
        assert!(dir.join("editor.json.bak").exists());
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
    ///
    /// `pub(crate)` so `project_config`'s tests -- the other half of this
    /// directory -- borrow it rather than minting a third copy.
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn new() -> Self {
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

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
