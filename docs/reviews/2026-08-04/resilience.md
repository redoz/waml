# Resilience review — 2026-08-04

Dimension: Resilience (full evaluation)
Files examined: 16 (share.rs, bundle_envelope.rs, analysis.rs, okf.rs, uml/analysis.rs, ops/mod.rs, solve/{mod,route,geometry}.rs, browser_boot.rs, load.rs, editor_session.rs, app/workspace.rs, config.rs, waml-cli/src/io.rs, scripts/inject-runtime-shell.mjs, plus targeted greps across the workspace)

---

### [R-1] UML analysis pipeline uses `expect` for document-derived invariants — a broken invariant is a dead session, not a diagnostic

Severity: high
File: `crates/waml/src/uml/analysis.rs:353` (also :115, :510-514, :612-616, :907-947, :1460, :1573-1586, :1784-1792)

Evidence:
```rust
syntax::parse_authoritative_island(
    document.text().clone(), structure, island.owner, island.content_range,
)
.expect("validated Markdown structure identifies its UML island")
```
and `let mut islands = documents.get(&document).unwrap()...` (:115), `crate::source::BundlePath::parse(path).expect("analyzed path is valid")` (:907), plus ~20 more production `expect`s asserting that ranges/paths derived from Markdown structure line up with catalog state.

Why it's wrong: These expects encode cross-layer invariants between the Markdown structure map (built over the pulldown-cmark seam) and the UML island parser — the exact seam that has needed four hardening commits in the last week (ef05d711, 9a29e227, 1424757b, f82d2f03). The pipeline already has a total error channel (`AnalysisError::Specialization` is used two functions away at :104-112), so a violated invariant here panics an in-process editor — losing unsaved work natively and poisoning the wasm instance — instead of failing one analysis revision. The `uml_islands` fuzz target that would exercise these is excluded from CI.

Suggested fix: Convert the invariant `expect`s reachable from `prepare_candidate` into `AnalysisError::Specialization`/`CatalogInvariant` returns; the editor already keeps the previous snapshot on `Err` (see R-not-findings), so the failure would cost one revision, not the session.

Confidence: CONFIRMED (code); reachability of the invariant break itself is PLAUSIBLE (it requires a seam bug, which has recent precedent).

### [R-2] One structurally-bad document aborts analysis of the whole bundle

Severity: medium
File: `crates/waml/src/analysis.rs:600-608, :1433, :1447`

Evidence:
```rust
pub enum AnalysisError {
    SourceTooLarge { path: BundlePath, bytes: usize },
    Shell { path: BundlePath, source: ParseError },
    ...
```
`prepare_candidate` returns `Err` for the entire `SourceBundle` when any single document fails shell ingestion (oversize, `ParseError`). `EditorSession::replace` (`editor_session.rs:360-371`) then rejects the whole load.

Why it's wrong: Ordinary syntax problems degrade correctly to per-document diagnostics, but a single document that trips a shell-level limit (e.g. one file over the `SourceText` size ceiling in an otherwise fine directory) makes the *entire project* unopenable. The charter says one bad document must not take the workspace; here it takes the open. Combined with R-3, the user is not even told which file did it.

Suggested fix: On shell-level per-document failure, quarantine the offending document (load it as an inert/unanalyzed entry with a diagnostic naming it) and open the rest of the bundle.

Confidence: CONFIRMED.

### [R-3] Failed project open is invisible to the user — the error goes only to `log!`

Severity: medium
File: `crates/waml-editor/src/app/workspace.rs:287-293` (also :310-313)

Evidence:
```rust
Err(BackingTransitionError::Load(error)) => {
    self.save_feedback.finish_save(&Ok(()));
    self.sync_save_error(cx);
    log!("{error}");
    return false;
}
```

Why it's wrong: State recovery itself is correct (the current screen stays up, nothing is wedged), but the only trace of "why did nothing happen when I picked that folder?" is a console log the native user never sees. `save_feedback` is even reset to `Ok(())`, so no UI element carries the load failure. Same pattern for the asset-root canonicalization failure at :310-313. A silent no-op on a user gesture is a stale-state trap: the user retries or assumes the app is broken.

Suggested fix: Route load/open failures into the same visible feedback channel saves use (statusbar / save_feedback), naming the path and reason.

Confidence: CONFIRMED.

### [R-4] `SystemTime::now()` call sites in editor code compile for wasm, guarded only by data emptiness

Severity: low
File: `crates/waml-editor/src/app/workspace.rs:680` (`format_opened`), `crates/waml-editor/src/config.rs:159` (`now_unix`)

Evidence:
```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(secs);
```
`format_opened` is called from `show_start_screen` (workspace.rs:600), which is not cfg-gated and runs on wasm (the Start boot source). `SystemTime::now()` itself panics on wasm32-unknown-unknown — the `unwrap_or` does not help. Today this is unreachable only because `config::recents()` is empty on wasm (`dirs::home_dir()` → `None`), so the closure never runs.

Why it's wrong: The safety argument is indirect ("the list happens to be empty on that platform"). `bundle_envelope.rs:349-374` shows the project knows this trap and cfg-gates it properly; these two sites rely on data flow instead. Any future browser-side recents/persistence lights the fuse.

Suggested fix: Mirror the `bundle_envelope` pattern: a cfg-gated `now_unix()` helper (returning 0 or a JS-time shim on wasm) used by both sites.

Confidence: CONFIRMED code path; the panic today is PLAUSIBLE-only (requires non-empty recents on wasm).

### [R-5] `write_back` success path reports failure after a fully committed write when staging cleanup fails

Severity: low
File: `crates/waml-cli/src/io.rs:409`

Evidence:
```rust
    fs::remove_dir_all(&staging)?;
    Ok(touched)
```

Why it's wrong: All target renames have committed by this point; if only the staging-dir removal fails (AV lock on Windows is a realistic cause), the caller gets `Err` and cannot distinguish "nothing happened" from "everything happened, a temp dir is left behind". The rollback-path errors (:389-407) carefully spell out this distinction; the success path does not.

Suggested fix: On cleanup failure after a committed journal, return `Ok(touched)` with a warning line appended (or an error message that states the writes committed), matching the precision of the rollback messages.

Confidence: CONFIRMED.

### [R-6] Fuzz targets covering the highest-panic-risk surfaces never run in CI

Severity: low
File: `fuzz/fuzz_targets/{uml_islands,syntax_edits,outer_mapping,parse_write}.rs`; `Cargo.toml` (workspace `exclude = ["fuzz"]`), `.github/workflows/ci.yml`

Evidence: MAP §4: "fuzz targets excluded from CI"; the workspace excludes `fuzz/`, and no workflow invokes `cargo fuzz`.

Why it's wrong: The panic surfaces found in R-1 (UML island invariants) and the incremental-reparse seam are exactly what these fuzzers exist for — one already caught a shipped reparse bug (MEMORY). Resilience regressions in these invariants will only be found by users.

Suggested fix: Add a short-budget (`-max_total_time=60`-per-target) fuzz smoke job to CI, or at minimum a scheduled nightly run.

Confidence: CONFIRMED (absence verified in ci.yml/pages.yml).

---

## Not findings

- `share.rs decode_source`: exemplary hostile-input handling — decompression capped at 64 MiB (`decompress_to_vec_with_limit`), bogus counts don't pre-allocate, lying lengths/truncation/UTF-8 all return typed `ShareError`; tests cover each rejection.
- `bundle_envelope.rs` nonce: wasm clock/process-id panic explicitly cfg-gated out (:349-374) with a documented rationale.
- `EditorSession` mutation is transactional: `self.current` is only replaced after `prepare_candidate` succeeds (`editor_session.rs:360-364, :447-462`), so a failed edit/replace leaves the previous snapshot intact — good state recovery.
- `browser_boot.rs`: fetch failures produce explicit user-facing messages (`boot_fetch_error` names URL, HTTP status, and the CORS ambiguity); HTML-at-a-bundle-URL and malformed site configs are refused with reasons; precedence fully tested.
- Web boot loader (`scripts/inject-runtime-shell.mjs:381-390`): boot exceptions flip the loader to an `error` phase ("Couldn't start WAML") — not a silent blank canvas.
- Undo/view history bounded: `HISTORY_LIMIT = 1024` (`editor_history.rs:16,317`), `VIEW_HISTORY_LIMIT = 256` with `pop_front` (`view_history.rs:140`).
- Markdown layout caches evict: `table_intrinsics.retain(...)` keyed by subtree fingerprints (`layout/engine.rs:1043`); asset host maps are lease-scoped and removed on lease drop (`markdown_hosts.rs:230-234`).
- `waml/src/{ops/mod.rs, okf/ops.rs, solve/route.rs, solve/mod.rs}`: the MAP's "panic density" counts are almost entirely `#[cfg(test)]` code; non-test bodies are unwrap-free (verified by tests-start line vs. match lines).
- `write_back` failure ladder: staging dir + journal + rollback + journal-retained-on-rollback-failure is a genuinely careful design (io.rs:339-408), including symlink and non-file target refusal.
- `load.rs read_bundle`: IO and source errors are typed `LoadError`; dot-directories skipped; no panics on user directory content.
- `close_model` save failure: keeps the session, reschedules the save timer, and surfaces the error (`workspace.rs:569-575`) — no lost work, no wedge.
- `okf::project`'s `expect`s (:421-427) are only reachable from test/support callers (verified callers: model.rs test, uml.rs support), not from document content.
- Direct `begin_turtle`/`end_turtle` in production editor code is limited to the presentation harness; the past unbalanced-turtle blanking bug (dock chrome) is fixed and not re-introduced at current call sites.
