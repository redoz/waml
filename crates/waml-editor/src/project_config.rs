//! Project-local VIEW configuration: `<project>/.waml/project.json`.
//!
//! ```text
//! <project>/.waml/project.json   how this project wants its views resolved
//! <project>/.waml/editor.json    how one person has their panels sized
//! ```
//!
//! The two are deliberately separate files with separate owners. A depth cap is
//! a property of the model -- any waml tool that grows a chain runner reads the
//! same one, and a team may well want it committed. A column width is a
//! property of somebody's desk, churns in every diff, and belongs in the file
//! most people gitignore. Putting them together meant the editor owned a
//! setting the CLI had every right to read, and the CLI could not read it
//! without depending on the editor.
//!
//! So the SHAPE lives in the core crate ([`waml::view::chain::ProjectConfig`],
//! beside the [`waml::view::chain::ChainLimits`] it produces) and only the file
//! read lives here, reusing [`crate::config::load_from`] /
//! [`crate::config::store_to`] for the atomic temp-write plus rename, the
//! corrupt-file-to-`.bak` rescue, and the never-panics posture -- the same seam
//! [`crate::project_settings`] uses.
//!
//! **Not migrated from the old `settings.json`.** `max_view_depth` used to sit
//! in the editor's own file; a project that had one hand-set gets the runner's
//! default until it is set again here. The alternative was reading a legacy
//! editor file to seed a non-editor one, which re-creates the exact tangle this
//! split exists to undo.

use std::io;
use std::path::{Path, PathBuf};

use waml::view::chain::ProjectConfig;

use crate::project_settings::PROJECT_DIR;

/// View-config file inside [`PROJECT_DIR`].
const PROJECT_FILE: &str = "project.json";

/// `<project>/.waml`.
fn project_dir(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_DIR)
}

/// Load `<project>/.waml/project.json`.
///
/// Missing or unreadable -> defaults. Malformed JSON -> the bad file is renamed
/// to `project.json.bak` and defaults are returned. Never panics, never errors:
/// a project whose config cannot be read is a project at the runner's default
/// bounds, which is what it had before anyone wrote the file.
pub(crate) fn load(project_root: &Path) -> ProjectConfig {
    crate::config::load_from(&project_dir(project_root), PROJECT_FILE)
}

/// Store `config` to `<project>/.waml/project.json`, creating the directory if
/// needed, stamped with the current [`ProjectConfig::VERSION`].
///
/// No caller in the editor yet: nothing in the UI edits a depth cap, so the
/// file is hand-written today. It exists so that whatever grows one -- a
/// project-properties panel here, `waml config` in the CLI -- writes through
/// the same seam it reads through, rather than minting a second writer.
#[allow(dead_code)]
pub(crate) fn store(project_root: &Path, config: &ProjectConfig) -> io::Result<()> {
    let stamped = ProjectConfig {
        version: ProjectConfig::VERSION,
        ..config.clone()
    };
    crate::config::store_to(&project_dir(project_root), PROJECT_FILE, &stamped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_settings::tests::TempDir;

    #[test]
    fn a_missing_file_yields_the_runners_default_bounds() {
        let tmp = TempDir::new();
        assert_eq!(load(tmp.path()), ProjectConfig::default());
        assert_eq!(
            load(tmp.path()).chain_limits(),
            waml::view::chain::ChainLimits::default(),
        );
    }

    #[test]
    fn max_view_depth_round_trips_and_reaches_the_runner() {
        let tmp = TempDir::new();
        let want = ProjectConfig {
            version: ProjectConfig::VERSION,
            max_view_depth: Some(7),
        };
        store(tmp.path(), &want).unwrap();
        assert_eq!(load(tmp.path()), want);
        assert_eq!(load(tmp.path()).chain_limits().max_depth, 7);
    }

    #[test]
    fn store_stamps_the_current_version() {
        let tmp = TempDir::new();
        let unstamped = ProjectConfig {
            version: 0,
            max_view_depth: Some(3),
        };
        store(tmp.path(), &unstamped).unwrap();
        assert_eq!(load(tmp.path()).version, ProjectConfig::VERSION);
    }

    /// A hand-trimmed file must not cost the reader their bounds.
    #[test]
    fn an_absent_depth_field_yields_the_default() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PROJECT_FILE), br#"{"version":1}"#).unwrap();
        assert_eq!(load(tmp.path()).max_view_depth, None);
        assert_eq!(load(tmp.path()).chain_limits().max_depth, 20);
    }

    #[test]
    fn corrupt_json_backs_up_and_yields_defaults() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(PROJECT_FILE), b"{ not valid json ]").unwrap();

        assert_eq!(load(tmp.path()), ProjectConfig::default());
        assert!(!dir.join(PROJECT_FILE).exists(), "corrupt file moved aside");
        assert!(dir.join("project.json.bak").exists());
    }

    /// The editor's own file must not be able to reach the runner: a depth cap
    /// left behind in `editor.json` (or the pre-split `settings.json` it was
    /// renamed from) is inert, not a second source of bounds.
    #[test]
    fn a_depth_left_in_the_editor_file_never_reaches_the_runner() {
        let tmp = TempDir::new();
        let dir = project_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("editor.json"),
            br#"{"version":1,"dock":{"tree_w":410.0,"inspector_w":250.0},"max_view_depth":3}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("settings.json"),
            br#"{"version":1,"max_view_depth":5}"#,
        )
        .unwrap();

        assert_eq!(load(tmp.path()).chain_limits().max_depth, 20);
    }
}
