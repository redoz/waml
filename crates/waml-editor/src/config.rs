//! Persistent editor config + a recent-projects (MRU) store.
//!
//! Two layers live here:
//!
//! * A **generic disk seam** (`waml_dir` / `load_from` / `store_to` and the
//!   `load` / `store` wrappers) that a future `waml-config` crate could lift out
//!   unchanged. The disk functions are directory-injectable so tests run against
//!   a temp dir and never touch the real `~/.waml`.
//! * The **editor payload** (`EditorConfig` / `Recent`) plus the recents API the
//!   app calls (`recents` / `push_recent`). The MRU / dedup / cap / prune logic
//!   is factored into pure functions over `Vec<Recent>` so it unit-tests without
//!   any filesystem.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use makepad_widgets::log;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Config file the editor owns, under `~/.waml/`.
const EDITOR_FILE: &str = "editor.json";
/// Current `editor.json` schema version.
const EDITOR_VERSION: u32 = 1;
/// Most-recent-projects retained; older entries fall off the back.
const RECENTS_CAP: usize = 10;

// ---------------------------------------------------------------------------
// Generic disk seam (extractable later as `waml-config`).
// ---------------------------------------------------------------------------

/// `~/.waml`, or `None` when the home directory cannot be resolved.
fn waml_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".waml"))
}

/// Load a versioned JSON file from `dir/<file>`.
///
/// Missing (or unreadable) file -> `T::default()`. Malformed JSON -> back the bad
/// file up to `<file>.bak`, then return `T::default()`. Never panics.
fn load_from<T: DeserializeOwned + Default>(dir: &Path, file: &str) -> T {
    let path = dir.join(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return T::default(),
    };
    match serde_json::from_slice(&bytes) {
        Ok(val) => val,
        Err(_) => {
            // Preserve the corrupt file for forensics, then start clean.
            let _ = std::fs::rename(&path, dir.join(format!("{file}.bak")));
            T::default()
        }
    }
}

/// Atomically write `val` to `dir/<file>`: write a temp file in the same dir,
/// then rename it over the target so a crash mid-write cannot leave a half file.
/// Creates `dir` if absent. Returns the io error on failure.
fn store_to<T: Serialize>(dir: &Path, file: &str, val: &T) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(val).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{file}.tmp"));
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, dir.join(file))
}

/// Resolve `~/.waml/` and load `file`; `T::default()` when home is unresolvable.
fn load<T: DeserializeOwned + Default>(file: &str) -> T {
    match waml_dir() {
        Some(dir) => load_from(&dir, file),
        None => T::default(),
    }
}

/// Resolve `~/.waml/` and store `val` to `file`; an io error when home is
/// unresolvable (the caller logs and swallows it).
fn store<T: Serialize>(file: &str, val: &T) -> io::Result<()> {
    let dir = waml_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))?;
    store_to(&dir, file, val)
}

// ---------------------------------------------------------------------------
// Editor payload.
// ---------------------------------------------------------------------------

/// The editor UI theme. Exactly two modes -- see `theme_atlas.rs`.
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Light (Atlas) -- the default.
    #[default]
    Light,
    /// Dark (Atlas).
    Dark,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
struct EditorConfig {
    /// Current schema version.
    version: u32,
    /// Recent projects, MRU order (front = most recent).
    recents: Vec<Recent>,
    /// Chosen UI theme; absent in older files -> `ThemeMode::default()`.
    #[serde(default)]
    theme: ThemeMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Recent {
    /// The OKF directory.
    path: PathBuf,
    /// Display name (the model's root name; see `push_recent` caller).
    title: String,
    /// Unix seconds, last time opened.
    opened_at: u64,
}

impl Recent {
    /// The OKF directory this recent points at.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Display name (the model's root name, recorded at open time).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Unix seconds of the last time this project was opened.
    pub fn opened_at(&self) -> u64 {
        self.opened_at
    }
}

/// Seconds since the Unix epoch (0 if the clock somehow predates it).
fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Dedup key for a recent: the canonicalized path, so the same directory reached
/// by different spellings collapses to one entry. Falls back to the path as
/// given when canonicalization fails (e.g. it vanished between select and record).
fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Add or promote `path` to the front of `recents` (MRU), refreshing its
/// `opened_at`, then cap the list at `RECENTS_CAP` (dropping oldest first).
fn add_or_promote(
    mut recents: Vec<Recent>,
    path: &Path,
    title: &str,
    opened_at: u64,
) -> Vec<Recent> {
    let key = canonical_key(path);
    recents.retain(|r| canonical_key(&r.path) != key);
    recents.insert(
        0,
        Recent { path: path.to_path_buf(), title: title.to_string(), opened_at },
    );
    recents.truncate(RECENTS_CAP);
    recents
}

/// Drop entries whose `path` no longer exists on disk, preserving MRU order.
fn prune_missing(recents: Vec<Recent>) -> Vec<Recent> {
    recents.into_iter().filter(|r| r.path.exists()).collect()
}

// ---------------------------------------------------------------------------
// Public recents API.
// ---------------------------------------------------------------------------

/// Load `editor.json`, drop entries whose `path` no longer exists (dead recents
/// self-heal on read), and return the survivors in MRU order. Pruning is applied
/// to the returned list only; the next `push_recent` persists the pruned state.
pub fn recents() -> Vec<Recent> {
    let config: EditorConfig = load(EDITOR_FILE);
    prune_missing(config.recents)
}

/// Load `editor.json` and return the persisted UI theme (`Light` when the file
/// is missing, malformed, or predates the `theme` field).
pub fn theme() -> ThemeMode {
    let config: EditorConfig = load(EDITOR_FILE);
    config.theme
}

/// Persist `mode` as the chosen UI theme, preserving the rest of the config.
/// Best-effort -- a write failure is logged and swallowed.
pub fn set_theme(mode: ThemeMode) {
    let mut config: EditorConfig = load(EDITOR_FILE);
    config.version = EDITOR_VERSION;
    config.theme = mode;
    if let Err(e) = store(EDITOR_FILE, &config) {
        log!("waml-editor: failed to persist theme {:?}: {e}", mode);
    }
}

/// Flip the persisted theme (`Light` <-> `Dark`) and return the new mode. Drives
/// the in-app toggle: the caller persists via this, then `cx.request_live_edit()`
/// so `App::script_mod` re-runs and repoints `mod.atlas` at the new block.
pub fn toggle_theme() -> ThemeMode {
    let next = match theme() {
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    };
    set_theme(next);
    next
}

/// Record an open: add or promote `path` to the front (MRU), refresh its
/// `opened_at`, cap the list, and persist. Best-effort — a write failure is
/// logged and swallowed so recording a recent never blocks opening a project.
pub fn push_recent(path: &Path, title: &str) {
    let mut config: EditorConfig = load(EDITOR_FILE);
    config.version = EDITOR_VERSION;
    config.recents = add_or_promote(config.recents, path, title, now_unix());
    if let Err(e) = store(EDITOR_FILE, &config) {
        log!("waml-editor: failed to persist recent {:?}: {e}", path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn rec(path: &str, opened_at: u64) -> Recent {
        Recent { path: PathBuf::from(path), title: format!("t:{path}"), opened_at }
    }

    // ---- pure list functions (no filesystem) ----

    #[test]
    fn add_or_promote_on_empty_inserts_at_front() {
        let out = add_or_promote(Vec::new(), Path::new("/a"), "Alpha", 100);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, PathBuf::from("/a"));
        assert_eq!(out[0].title, "Alpha");
        assert_eq!(out[0].opened_at, 100);
    }

    #[test]
    fn add_or_promote_existing_promotes_and_refreshes() {
        let start = vec![rec("/a", 1), rec("/b", 2), rec("/c", 3)];
        let out = add_or_promote(start, Path::new("/c"), "C-updated", 99);
        assert_eq!(out.len(), 3, "no duplicate for an existing path");
        assert_eq!(out[0].path, PathBuf::from("/c"));
        assert_eq!(out[0].opened_at, 99, "opened_at refreshed");
        assert_eq!(out[0].title, "C-updated");
        assert_eq!(out[1].path, PathBuf::from("/a"));
        assert_eq!(out[2].path, PathBuf::from("/b"));
    }

    #[test]
    fn cap_drops_the_oldest_entry() {
        let mut list = Vec::new();
        for i in 0..RECENTS_CAP {
            list = add_or_promote(list, Path::new(&format!("/p{i}")), "t", i as u64);
        }
        assert_eq!(list.len(), RECENTS_CAP);
        // An 11th distinct entry evicts the oldest (/p0).
        list = add_or_promote(list, Path::new("/p-new"), "t", 999);
        assert_eq!(list.len(), RECENTS_CAP);
        assert_eq!(list[0].path, PathBuf::from("/p-new"));
        assert!(!list.iter().any(|r| r.path == Path::new("/p0")), "oldest dropped");
    }

    #[test]
    fn dedup_collapses_two_spellings_of_same_path() {
        let tmp = TempDir::new();
        // Same real directory reached two ways: as-is, and via a redundant ".".
        let a = tmp.path().to_path_buf();
        let b = tmp.path().join(".");
        let list = add_or_promote(Vec::new(), &a, "A", 1);
        let list = add_or_promote(list, &b, "B", 2);
        assert_eq!(list.len(), 1, "two spellings collapse to one entry");
        assert_eq!(list[0].title, "B", "the later open wins");
    }

    #[test]
    fn prune_drops_missing_keeps_existing() {
        let tmp = TempDir::new();
        let here = tmp.path().to_path_buf();
        let list = vec![
            Recent { path: here.clone(), title: "here".into(), opened_at: 1 },
            Recent { path: tmp.path().join("gone"), title: "gone".into(), opened_at: 2 },
        ];
        let out = prune_missing(list);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, here);
    }

    #[test]
    fn recent_getters_return_stored_fields() {
        let r = Recent { path: PathBuf::from("/proj"), title: "Proj".into(), opened_at: 5 };
        assert_eq!(r.path(), Path::new("/proj"));
        assert_eq!(r.title(), "Proj");
    }

    // ---- disk seam, against a temp dir (never the real home) ----

    #[test]
    fn load_from_missing_file_returns_default() {
        let tmp = TempDir::new();
        let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(cfg, EditorConfig::default());
    }

    #[test]
    fn load_from_malformed_json_backs_up_and_returns_default() {
        let tmp = TempDir::new();
        let path = tmp.path().join(EDITOR_FILE);
        std::fs::write(&path, b"{ not valid json ]").unwrap();
        let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(cfg, EditorConfig::default());
        assert!(!path.exists(), "corrupt file moved aside");
        assert!(tmp.path().join("editor.json.bak").exists(), "backup written");
    }

    #[test]
    fn store_to_then_load_from_round_trips() {
        let tmp = TempDir::new();
        let cfg = EditorConfig {
            version: EDITOR_VERSION,
            recents: vec![rec("/x", 7), rec("/y", 8)],
            theme: ThemeMode::Dark,
        };
        store_to(tmp.path(), EDITOR_FILE, &cfg).unwrap();
        let back: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(back, cfg);
    }

    #[test]
    fn theme_defaults_light_and_round_trips_dark() {
        let tmp = TempDir::new();
        // Missing field / missing file -> Light.
        let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(cfg.theme, ThemeMode::Light);
        // Dark survives a store/load round-trip.
        let cfg = EditorConfig { version: EDITOR_VERSION, recents: Vec::new(), theme: ThemeMode::Dark };
        store_to(tmp.path(), EDITOR_FILE, &cfg).unwrap();
        let back: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(back.theme, ThemeMode::Dark);
    }

    #[test]
    fn theme_field_absent_in_old_file_loads_light() {
        let tmp = TempDir::new();
        // A v1 file written before the `theme` field existed.
        std::fs::write(
            tmp.path().join(EDITOR_FILE),
            br#"{"version":1,"recents":[]}"#,
        )
        .unwrap();
        let cfg: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
        assert_eq!(cfg.theme, ThemeMode::Light, "absent theme field -> default Light");
    }

    /// Minimal temp dir: the repo has no temp-dir dev-dependency, so we make a
    /// unique subdir under the system temp dir and remove it on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let n = N.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("waml-editor-cfg-{}-{}-{}", std::process::id(), nanos, n));
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
