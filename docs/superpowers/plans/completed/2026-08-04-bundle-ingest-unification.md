# Bundle Ingest Unification

The workspace has three independent recursive markdown-bundle walkers, all of which classify entries via `path.is_dir()` (which follows symlinks and NTFS junctions) and none of which track visited directories: the CLI's `collect_md` (`crates/waml-cli/src/io.rs:30-47`, fail-fast IO errors, skips dot-directories, sorts), the editor's `collect` (`crates/waml-editor/src/load.rs:53-75`, entry point `read_bundle` at line 38, same shape), and the LSP's `read_disk_documents` (`crates/waml-cli/src/lsp/bundle.rs:338-358`, called from `crates/waml-cli/src/lsp/server.rs:103`) — the worst of the three: `entries.flatten()` plus `if let Ok(text)` silently drop unreadable directories, unreadable files, and non-UTF-8 content, and it does not skip dot-directories, so `.git/` and `.waml/` leak into LSP analysis. A junction or symlink cycle hangs or stack-overflows all three, and a junction escaping the bundle root is silently ingested. The write path (`write_back` in `io.rs`) already screens symlinks (`ensure_regular_file`, `validate_target`); the read path has no screening at all. This plan introduces one shared, hardened ingest walker in the headless `waml` crate — which must not depend on the editor or on makepad — and migrates all three call sites onto it, then deletes the private walkers. The new module is gated `#[cfg(not(target_arch = "wasm32"))]`, following the precedent in `crates/waml/src/bundle_envelope.rs` (lines 10, 349, 370).

**Verification:** `cargo test --workspace`

### Task 1: Add `waml::host::ingest` with a hardened shared walker

**Files:**
- `crates/waml/src/host.rs` → move to `crates/waml/src/host/mod.rs`
- `crates/waml/src/host/ingest.rs` (new)

**Steps:**
1. Convert `host.rs` into `host/mod.rs` unchanged, add `#[cfg(not(target_arch = "wasm32"))] pub mod ingest;` (cfg precedent: `crates/waml/src/bundle_envelope.rs:10`). No editor/makepad dependency may be introduced — std-only.
2. Define the API in `host/ingest.rs`:
   - `pub struct IngestOptions { pub follow_links: bool /* default false */, pub skip_dot_dirs: bool /* default true */, pub max_file_bytes: Option<u64> }` with `Default`. Preserve the `io.rs` nuance (see `crates/waml-cli/src/io.rs:28-29`): a dot-directory named *directly as a root* is still descended into; only dot-directories *discovered during the walk* are skipped.
   - `pub enum IngestErrorKind { ReadDir, ReadFile, NotUtf8, LinkCycle, EscapesRoot, TooLarge, Metadata }`
   - `pub struct IngestError { pub path: PathBuf, pub kind: IngestErrorKind }` (plus the underlying `io::Error` message where one exists).
   - `pub struct Ingested { pub files: Vec<(PathBuf, String)>, pub errors: Vec<IngestError> }` — `files` sorted by path.
   - `pub fn ingest_markdown(roots: &[PathBuf], options: &IngestOptions) -> Ingested`. Nothing is ever silently dropped: every skip-for-cause lands in `errors`.
3. Mechanics:
   - Classify entries via `fs::symlink_metadata`, never `path.is_dir()`.
   - When `follow_links` is false, treat links as `EscapesRoot`-free skips recorded only when they would otherwise be ingested; on Windows detect junctions via `std::os::windows::fs::MetadataExt::file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT` (NTFS junctions are **not** `is_symlink()` in std), cfg-gated; on Unix use `metadata.is_symlink()`.
   - When `follow_links` is true, maintain a visited-directory set keyed on `fs::canonicalize` string form (pragmatic identity; `file_index` is not stable) to record `LinkCycle` instead of recursing forever, and record `EscapesRoot` when a canonicalized target leaves the canonicalized root.
   - Enforce `max_file_bytes` before reading (`TooLarge`), read bytes and convert with explicit UTF-8 validation (`NotUtf8`), record `ReadDir`/`ReadFile`/`Metadata` failures with their paths.

**Tests (unit, in `host/ingest.rs`):** deterministic sorted output; dot-directory skipped during walk; explicitly-named dot root descended; unreadable-file and read-dir failures accumulate as structured errors rather than aborting or vanishing.

Commit: green unit, `cargo test --workspace`.

### Task 2: Migrate the LSP to `ingest_markdown`

**Files:**
- `crates/waml-cli/src/lsp/bundle.rs`
- `crates/waml-cli/src/lsp/server.rs`

**Steps:**
1. Replace the body of `read_disk_documents` (`crates/waml-cli/src/lsp/bundle.rs:338-358`) with a thin adapter over `waml::host::ingest::ingest_markdown` (default options), applying `normalize_physical` to returned paths and returning `(files, errors)` or logging the errors before returning files.
2. In the caller (`crates/waml-cli/src/lsp/server.rs:103`), surface each `IngestError` via LSP log messages (`window/logMessage`) so nothing is silently dropped anymore.
3. Behavior change to note in the commit message: dot-directories (`.git/`, `.waml/`) are now skipped by the LSP walk, and unreadable/non-UTF-8 files are reported instead of vanishing.

**Tests:** update/extend the existing tests in `crates/waml-cli/src/lsp/bundle.rs` `mod tests` — dot-directory now excluded; a non-UTF-8 `.md` produces a logged error, not silence.

Commit: green unit.

### Task 3: Migrate CLI `collect_md`

**Files:**
- `crates/waml-cli/src/io.rs`

**Steps:**
1. Keep the public signature `pub fn collect_md(&[PathBuf]) -> std::io::Result<Vec<PathBuf>>`. Reimplement over `ingest_markdown`; the CLI stays fail-fast — map the *first* `IngestError` to `std::io::Error`, preserving the offending path in the message. Return sorted paths (drop the contents the ingester read, or add a paths-only mode if trivially cheap — do not complicate the API for it).
2. Leave `write_back`, `ensure_regular_file`, `validate_target` untouched.

**Tests:** existing CLI tests keep passing; add one asserting the fail-fast error message names the path.

Commit: green unit.

### Task 4: Migrate editor `read_bundle`

**Files:**
- `crates/waml-editor/src/load.rs`

**Steps:**
1. Reimplement `read_bundle` (`crates/waml-editor/src/load.rs:38-43`) over `ingest_markdown` with the single root: strip the root prefix from each returned path, normalize to forward slashes (as `collect` did at lines 66-70), and build the `SourceBundle`.
2. Map any `IngestError` to `LoadError::Io` (fail-fast, matching current behavior), preserving the path.
3. Delete the private `collect` walker.

**Tests:** existing editor load tests keep passing; add a dot-directory-skip assertion if none exists.

Commit: green unit.

### Task 5: Platform-gated integration tests and walker deletion sweep

**Files:**
- `crates/waml/tests/` (new integration test file, e.g. `crates/waml/tests/ingest_links.rs`)
- `crates/waml-cli/src/io.rs`, `crates/waml-cli/src/lsp/bundle.rs`, `crates/waml-editor/src/load.rs` (cleanup)

**Steps:**
1. `#[cfg(unix)]` tests: a symlink cycle inside the root yields `LinkCycle` (with `follow_links: true`) or a clean skip (default), and terminates; a symlink escaping the root yields `EscapesRoot` and its target is not ingested.
2. `#[cfg(windows)]` tests: create a junction via `cmd /C mklink /J` (no admin required); if `mklink` fails in the environment, skip gracefully (early-return with a printed notice) rather than fail. Assert the junction cycle terminates and the escape is reported.
3. Cross-platform tests: unreadable file and non-UTF-8 `.md` produce structured errors; output ordering is deterministic across runs.
4. Confirm all three private walkers are gone; run `rg read_dir` across the workspace and verify no stray recursive ingestion remains (the `write_back` machinery in `crates/waml-cli/src/io.rs` legitimately stays).

Commit: green unit.
