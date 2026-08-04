# Config rename close-out: dismiss the false "replace-existing" P2

The issues.md P2 "Repeated configuration writes are not replace-existing
portable" (issues.md ~lines 180-198) claims `std::fs::rename` on Windows lacks
replace-existing semantics, so theme/recents/pins would stop persisting after
the first write. Investigation verdict: **false**. `std::fs::rename` on Windows
calls `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` (newer Rust first tries
`FILE_RENAME_FLAG_POSIX_SEMANTICS | REPLACE_IF_EXISTS`), so an existing
destination is replaced. `store_to` at `crates/waml-editor/src/config.rs:78-84`
writes the temp file in the same directory as the destination, so a
cross-volume rename failure is impossible, and theme/recents demonstrably
persist across sessions in daily Windows use. The only residual is a transient
rename failure when the destination is briefly held open without
`FILE_SHARE_DELETE` (antivirus/indexer), which callers already log and swallow.
This plan adds a regression test locking in second-write-wins behavior and
removes the issue with an evidence trail.

Verification: `cargo test -p waml-editor config`

### Task 1: Add regression test `store_to_twice_second_value_wins`

Files:
- `crates/waml-editor/src/config.rs` (tests mod, near
  `store_to_then_load_from_round_trips`)

Steps:
1. In the existing `mod tests`, add `#[test] fn store_to_twice_second_value_wins()`
   following the local idiom (`TempDir::new()`, `EDITOR_FILE`, `rec` helper):
   - Build config A (`ThemeMode::Light`, recents `[rec("/a", 1)]`) and
     `store_to(tmp.path(), EDITOR_FILE, &a).unwrap()`.
   - Build config B with a different theme (`ThemeMode::Dark`) and different
     recents (`[rec("/b", 2), rec("/c", 3)]`) and store it the same way.
   - `load_from(tmp.path(), EDITOR_FILE)` and assert it equals B.
   - Assert the temp file is gone:
     `assert!(!tmp.path().join(format!("{EDITOR_FILE}.tmp")).exists());`
2. No `cfg(windows)` gate — the behavior must hold on every platform, and the
   test proves the Windows claim wherever CI runs on Windows.

Verify: `cargo test -p waml-editor config` — new test passes alongside the
existing config tests.

### Task 2: Remove the dismissed P2 from issues.md

Files:
- `issues.md` (repo root)

Steps:
1. Delete the entire section
   `## P2 — Repeated configuration writes are not replace-existing portable`
   (heading through its Recommendation paragraph, up to but not including
   `## P2 — Bundle ingestion has three filesystem authorities and follows links`).
2. Add a bullet to the `Removed in the 2026-08-04 reconciliation:` list near the
   top, e.g.:
   - Config rename not replace-existing on Windows: dismissed as false —
     `std::fs::rename` uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` (and
     POSIX-semantics rename on newer Windows), the temp file is written in the
     destination directory so cross-volume failure cannot occur, and the new
     `store_to_twice_second_value_wins` test locks in second-write-wins.
     Residual transient sharing-violation failures are already logged and
     swallowed by callers.

Verify: `cargo test -p waml-editor config` still green; grep issues.md for
`replace-existing` — only the reconciliation bullet and unrelated line ~143
remain.
