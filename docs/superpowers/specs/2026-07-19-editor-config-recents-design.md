# Editor Config + Recents Infrastructure

## Goal

Give the `waml-editor` makepad app a persistent config store, and use it to hold a
**recent-projects** (MRU) list. This is the foundation slice for a VS-style
**start window** (shown when the app launches with no directory argument) — but
this spec covers **only the config/recents infrastructure**, not the launcher UI.
The dialogs are a separate follow-up slice.

**Why now:** the app today hard-errors when launched without an OKF directory
(`cli.rs:29` → `usage: waml-editor <okf-dir>`), leaving a blank window. The
eventual launcher needs somewhere to read/write a recent list; that store is
this slice.

## Scope

**In scope:**
- A `config` module in `waml-editor` that resolves a config directory, and
  atomically loads/stores versioned JSON files.
- An `EditorConfig` payload holding a recents list, with MRU / dedup / cap /
  prune semantics.
- Wiring the existing "open a directory" startup path to record a recent entry.

**Out of scope (deferred to later slices):**
- The start-window UI (recent list pane + New/Open actions).
- The New-project template picker (Empty / Domain / Use-case / Activity /
  Sequence).
- The `rfd` folder picker (a UI concern — lands with the dialogs slice).
- Saving / materializing in-memory scratch projects to disk.
- Pinning recents.
- Sharing config with `waml-cli` (the seam is designed for it; the crate is not
  extracted here).

## Context

`waml-editor` is a native makepad binary. It already reads OKF directories from
disk with plain `std::fs` (`load.rs::read_bundle` / `load_model`), so native
filesystem access is a given — there is no sandbox constraint. `handle_startup`
(`app.rs:344`) parses argv, loads the model, and shows a root name in the
`pkg_name` label (`app.rs:362-369`): the model's `path` when non-empty, else
`"bundle"`.

The CLI (`waml-cli`) is stateless — every subcommand takes an explicit `--dir`.
It has no recents or persistent config today, so nothing is shared with it now.
The store is structured so a `waml-config` crate can be lifted out later without
reworking call sites, but that extraction is not part of this slice.

## Design

### Storage layout

- **Root:** `~/.waml/` — a dotfolder in the user home directory, via
  `dirs::home_dir()`. Windows: `%USERPROFILE%\.waml\`. Both the editor and a
  future CLI point at this same root.
- **Per-consumer files, not a shared blob.** The editor owns `~/.waml/editor.json`.
  A future CLI would own `~/.waml/cli.json`. Because each write is an atomic
  whole-file rewrite, separate files mean two independently-running binaries
  cannot clobber each other's state.
- **Format:** JSON via the existing `serde_json` dependency (no new serde dep).
  Every file carries a top-level `version: u32` for future migration.

### New dependency

- `dirs` — used only for `home_dir()`. Cross-platform, small. (Chosen over
  `directories`, which is heavier and oriented around per-app config dirs we do
  not want, since we want one shared `~/.waml/` root.)
- `rfd` is **not** added in this slice — it is a UI dialog concern for the
  launcher slice.

### Module: `crates/waml-editor/src/config.rs`

Split into a generic seam (extractable later) and the editor payload. The seam's
disk functions are **directory-injectable**: the public wrappers resolve
`~/.waml/`, while dir-taking inner variants let tests run against a temp dir and
never touch the real home directory.

**Generic seam** (the part a future `waml-config` crate would hold):

```rust
/// `~/.waml`, or `None` when the home directory cannot be resolved.
fn waml_dir() -> Option<PathBuf>;

/// Load a versioned JSON file from `dir/<file>`.
/// Missing file -> `T::default()`.
/// Malformed JSON -> back up the bad file to `<file>.bak`, then return `T::default()`.
/// Never panics.
fn load_from<T: DeserializeOwned + Default>(dir: &Path, file: &str) -> T;

/// Atomically write `val` to `dir/<file>` (write temp in `dir`, then rename over
/// the target). Creates `dir` if absent. Returns the io error on failure.
fn store_to<T: Serialize>(dir: &Path, file: &str, val: &T) -> io::Result<()>;

/// Public wrappers: resolve `~/.waml/` then delegate. When home is unresolvable,
/// `load` returns `T::default()` and `store` returns an io error the caller logs.
fn load<T: DeserializeOwned + Default>(file: &str) -> T;
fn store<T: Serialize>(file: &str, val: &T) -> io::Result<()>;
```

**Editor payload:**

```rust
#[derive(Serialize, Deserialize, Default)]
struct EditorConfig {
    version: u32,          // current schema version
    recents: Vec<Recent>,  // MRU order, front = most recent
}

#[derive(Serialize, Deserialize, Clone)]
struct Recent {
    path: PathBuf,   // the OKF directory
    title: String,   // display name (see below)
    opened_at: u64,  // unix seconds, last time opened
}
```

**Public recents API** used by the app:

```rust
/// Load `editor.json`, drop entries whose `path` no longer exists on disk,
/// and return the surviving list in MRU order. (Dead recents self-heal on read.)
fn recents() -> Vec<Recent>;

/// Record an open: add or promote `path` to the front (MRU), refresh
/// `opened_at`, cap the list, and persist. Best-effort — logs on write failure.
fn push_recent(path: &Path, title: &str);
```

The list transforms `push_recent` and `recents` rely on are factored as **pure
functions over `Vec<Recent>`** (add-or-promote + cap; prune-missing), so the
MRU / dedup / cap / prune logic is unit-testable without any filesystem.

### Recents semantics

- **`title`** = the OKF model's root name — the same value already shown in the
  `pkg_name` label (`app.rs:362`): `model.path` when non-empty, else the
  directory's file-name basename as a fallback.
- **Dedup key** = the **canonicalized** path (`std::fs::canonicalize`), so the
  same directory reached by different spellings collapses to one entry. If
  canonicalization fails (e.g. path vanished between select and record), fall
  back to the path as given.
- **MRU:** `push_recent` moves an existing entry to the front and refreshes its
  `opened_at`; a new entry is inserted at the front.
- **Cap:** 10. Entries past the cap are dropped (oldest first).
- **Prune on read:** `recents()` drops entries whose `path` no longer exists.
  Pruning is applied to the returned list; it does not itself rewrite the file
  (the next `push_recent` persists the pruned state).
- **No pinning** in this slice.

### Wiring into startup

`handle_startup` (`app.rs`) already resolves `args.dir` and loads the model. After
a successful load, compute the display title (reusing the existing `root_name`
logic) and call `config::push_recent(&args.dir, title)`. This means launching the
app on a directory — the only entry path that exists today — records a recent, so
the store is populated and testable before the launcher UI exists.

The no-argument case still logs its usage error and returns for now; the launcher
slice replaces that branch with the start window.

## Error handling

- No home directory → `waml_dir()` returns `None`; `load` returns default,
  `store` returns an error that callers log and swallow. The app runs without
  persistence rather than failing.
- Corrupt `editor.json` → backed up to `editor.json.bak`, treated as empty. The
  user loses their recents list but the app starts clean.
- `store` failures (disk full, permissions) are logged via makepad `log!` and
  ignored — recording a recent must never block opening a project.

## Testing

Unit tests in `config.rs`.

Pure list functions (no filesystem):
- add-or-promote on an empty list inserts at front with correct `title`/`opened_at`.
- add-or-promote on an existing path promotes to front and refreshes `opened_at`,
  without duplicating.
- Cap: an 11th distinct entry drops the oldest.
- Dedup collapses two spellings of the same canonical path.
- Prune drops entries whose path does not exist; keeps those that do.

Disk seam, via `load_from` / `store_to` against a temp dir (never the real home):
- `load_from` of a missing file returns default.
- `load_from` of malformed JSON writes a `.bak` and returns default.
- `store_to` then `load_from` round-trips an `EditorConfig`.

## Follow-up slices

1. **Start window UI** — VS-style two-pane: recent list (left, from `recents()`)
   + actions (right): New project, Open project.
2. **New-project template picker** — Empty / Domain / Use-case / Activity /
   Sequence, seeding an in-memory model + oplog.
3. **Open project…** — `rfd` cross-platform folder picker, then `push_recent`.
4. **Save / materialize** — write an in-memory scratch project (oplog) to disk.
