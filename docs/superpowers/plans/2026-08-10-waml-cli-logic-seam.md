# waml-cli Logic Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the business logic stranded in bin-only `waml-cli` into the `waml` library family, so every rule about WAML documents — diagnostics remapping, fmt planning, index regeneration policy, the export search-index boundary, transactional bundle persistence, path confinement, and the `/api/documents` wire shape — has exactly one implementation reachable by every frontend (CLI, native editor, web, LSP, serve) and by cross-crate tests.

**Architecture:** Twelve tasks in strict risk order. Tasks 1, 4–9 are behaviour-preserving code motion: functions move verbatim with their unit tests, `waml-cli` keeps thin call sites, and `crates/waml-cli/tests/cli_e2e.rs` passes byte-identical at every boundary. Task 2 is a small, early semantic alignment (a path-validity rule the CLI and editor spell differently). Task 3 adds the wasm-boundary guard the later `host` tasks rely on. Task 10 is a wire-preserving dependency change (shared `DocumentWrite` type). Tasks 11–12 are the two explicitly NOT-pure-motion unifications: one confinement module replacing four sibling implementations under two distinct symlink policies, then the editor's save adopting the shared journaled transaction. Each of those two carries its own failure-mode tests.

This plan is grounded in the 2026-08-10 audit of `crates/waml-cli` (findings D1–D8), amended after an independent adversarial review. Every `file:line` below was re-verified against origin/main @ 97330f98, the base of this branch.

**Tech Stack:** Rust 2024 workspace. No new external dependencies — in particular, tests that move into `waml` must not pull `tempfile` in (Task 11 Step 1 specifies the rewrite). One new CI step (Task 3); no new workflow file. No process spawn added to any test.

## Global Constraints

- Every task ends green on the full gate: `cargo test --workspace`, then in `editors/vscode`: `pnpm test`, `pnpm lint`, `pnpm build`. From Task 3 onward, every task that touches `crates/waml`, `crates/waml-ops-dto`, or `crates/waml-editor` additionally runs the wasm boundary check: `cargo check -p waml --features serde --target wasm32-unknown-unknown && cargo check -p waml-ops-dto --target wasm32-unknown-unknown`. Commit only on a green gate.
- Tasks 1 and 4–9 are behaviour-preserving moves: no signature changes, no error-type redesigns (kept `std::io::Result` and `Result<_, String>` shapes stay), existing unit tests move with the code they cover, and `crates/waml-cli/tests/cli_e2e.rs`, `tests/lsp_e2e.rs`, `tests/serve_e2e.rs` pass untouched at every task boundary.
- Tasks 2, 3, 10, 11, 12 are NOT pure motion and are individually marked with what changes and which new tests pin it. Do not fold extra "improvements" into them.
- Every filesystem-touching module added to `waml::host` MUST be gated `#[cfg(not(target_arch = "wasm32"))]` at its `mod` declaration, mirroring `ingest` (`crates/waml/src/host/mod.rs:5-6`). `std::fs` *compiles* on `wasm32-unknown-unknown` and fails only at runtime, so an ungated module ships silent dead transaction code into the web binary — the cfg gate is the enforcement, and Task 3's compile check is the regression net around it (its limits are stated in Task 3).
- Named tests that must stay green throughout, beyond the gate: the `write_back` fault-injection suite (`crates/waml-cli/src/io.rs:1192` `late_write_failure_restores_updates_and_removes_new_artifacts`, `:1216` `late_delete_failure_restores_prior_write_and_delete`, `:1252`/`:1300` the two Windows read-only rollback tests, `:1351` `rollback_failure_retains_a_reported_recovery_journal`, `:1407` `committed_write_reports_success_with_a_warning_when_cleanup_fails`, `:1442` `successful_transaction_adds_updates_and_deletes_as_one_set`, `:1141` `write_back_rejects_ntfs_alternate_data_stream_paths`, `:1157`/`:1173` the case-sensitivity pair); the `cli_e2e.rs` index suite (`:564`, `:592`, `:637`, `:674`, `:697`, `:719`); the `cli_e2e.rs` fmt and rollback tests (`:249`, `:270`, `:163`, `:299`); the serve write-path tests in `crates/waml-cli/src/serve/state.rs` and `serve/routes.rs`; and the editor save suite in `crates/waml-editor/src/native_save.rs`.
- Rendering does NOT move. `render_human`, `render_json`, `DiagDto`, `severity_str`, `sorted`, `check_exit_code`, `render_diff`, `diff_lines` (`crates/waml-cli/src/commands.rs:10-129, 162-168`) are terminal presentation and process exit-code policy; the LSP renders via `to_lsp_diagnostic` (`crates/waml-cli/src/lsp/map.rs:201`), the editor via widgets, serve via serde. The 2026-08-04 harness design's claim that "the editor and the LSP want the same rendering" is wrong today; do not carry it forward. (See Open Questions for the one rendering-adjacent decision left to a human.)
- Deferred by design, do not implement here: the docs-conformance harness itself (downstream work this plan unblocks), a `waml-lsp` crate split, `EditorSession`/`LspAnalysisState` unification, the LSP's `logical_path` keying (`crates/waml-cli/src/lsp/bundle.rs:310-332`) adopting shared helpers, per-write content preconditions in `persist` (see Task 12's TOCTOU decision), and a load-time diagnostic for device-named paths (see Task 11's asymmetry inventory). See "Deferred, with reason".
- Worktree discipline: this plan file is authored on `plan/waml-cli-logic-seam`; implementation happens via the normal implement-plan flow, never by editing the live main checkout directly.
- Commit messages: conventional commits, real body explaining why, wrapped at 72 columns. No co-author or AI-attribution trailer.

---

### Task 1: Merge `prepare` and `diagnostics` into `waml::validate`

Closes audit finding **D1**, the one byte-for-byte duplicated rule: `commands::diagnostics` (`crates/waml-cli/src/commands.rs:303-319`) and the tail of `validate::validate` (`crates/waml/src/validate.rs:43-54`) are the same loop — clone `candidate.uml().diagnostics`, remap `diagnostic.file` through a display map. After this task there is exactly one copy, and the 3-line `try_from_pairs → prepare_candidate` adapter that CLI, serve, and LSP each respell has one shared home.

**Files:**
- Modify: `crates/waml/src/validate.rs`
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/main.rs` (call sites at `:373`, `:380`, `:893`, `:962`, `:1103`)
- Modify: `crates/waml-cli/src/serve/state.rs:69`

**Interfaces:**
- Produces: `waml::validate::prepare(files: &[(String, String)]) -> Result<PreparedCandidate, String>` and `waml::validate::diagnostics(candidate: &PreparedCandidate, display_paths: &BTreeMap<String, String>) -> Vec<Diagnostic>` — exact current bodies from `commands.rs:183-186` and `:303-319`.
- Consumed later by Tasks 5, 6, 7 (their moved functions call these).

- [ ] **Step 1: Move the two functions**

Move `prepare` and `diagnostics` verbatim from `commands.rs` into `crates/waml/src/validate.rs` as `pub fn`. They use only `waml`-internal types (`PreparedCandidate`, `SourceBundle`, `Diagnostic`) — no new dependencies.

- [ ] **Step 2: Route `validate::validate` through the shared helper**

Rewrite the tail of `validate::validate` (`validate.rs:43-54`) to call the moved `diagnostics(&candidate, &display_paths)`. Behaviour must be identical: same ordering, same remap. Leave `validate_from_source` (`validate.rs:14-18`) alone — its `.unwrap_or_default()` false-green is a known defect owned by a different project; this plan routes around it, never through it.

- [ ] **Step 3: Update call sites**

`main.rs`: `commands::prepare` → `waml::validate::prepare` (4 sites), `commands::diagnostics` → `waml::validate::diagnostics` (1 site). `serve/state.rs:69`: same. `commands.rs`: `plan_fmt` (`:322`, `:381`) and `plan_indexes` (`:246`) call the `waml::validate` versions; `build_search_index_asset` (`:195`) likewise. Delete the moved items from `commands.rs`.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm test && pnpm lint && pnpm build`.
Expected: all green; `cli_e2e.rs` output byte-identical (diagnostic ordering and display paths unchanged).

- [ ] **Step 5: Commit**

Commit: `refactor(waml): merge CLI prepare/diagnostics into waml::validate`

---

### Task 2: Align `native_save`'s path-validity rule with `BundlePath` (colon segments)

**NOT pure motion — a small, deliberate semantic alignment.** Audit finding: `crates/waml-cli/src/io.rs:621-640` (`validate_relative`) rejects `:` in any path segment on every platform (NTFS alternate-data-stream hazard: `a:b.md` writes an invisible stream, not a file), while the editor's `crates/waml-editor/src/native_save.rs:167-179` (`validate_relative_path`) accepts any `Component::Normal` — so a segment like `ab:c.md` passes the editor's check and fails the CLI's.

**Decision (made here, not left to the implementer): the CLI rule is correct, and it is already the model-level rule.** `BundlePath::parse` (`crates/waml/src/source.rs:40-42`) rejects any colon, with the same ADS rationale in its comment — so every document in a `SourceBundle` is colon-free and the divergence is currently *latent*, not reachable through the save path. It must still be closed: `native_save`'s validation exists precisely as defense-in-depth against a future path source that bypasses `BundlePath`, and a defense layer that silently disagrees with the layer above it is worse than none — the next reader assumes one of them is authoritative. Cross-platform identical behaviour ("a bundle authored on one OS behaves the same on another") is the project's own stated rule at `io.rs:625-627`.

**Files:**
- Modify: `crates/waml-editor/src/native_save.rs` (`validate_relative_path`, `:167-179`, plus its test module)

**What changes:** `validate_relative_path` additionally rejects any component whose name contains `:`, mirroring `io.rs:621-640`. Nothing else. The serve-side syntactic check (`serve/paths.rs::is_safe_rel`, which also accepts interior colons today) is aligned later in Task 11, where the whole family is unioned — do not touch it here.

- [ ] **Step 1: Tighten the check**

Extend the `components().any(...)` predicate in `validate_relative_path` to also reject `Component::Normal` names containing `':'`, with a comment citing the ADS rationale and `BundlePath::parse` as the authoritative upper layer.

- [ ] **Step 2: Pin it with tests**

Add unit tests in `native_save.rs`'s test module calling `validate_relative_path` directly (it takes a `&Path`, so the colon path is constructible even though `BundlePath` would refuse it): `ab:c.md` and `nested/ab:c.md` are rejected; `abc.md` and `nested/abc.md` still pass. Confirm the existing CLI-side pin (`io.rs:1141` `write_back_rejects_ntfs_alternate_data_stream_paths`) still passes untouched.

- [ ] **Step 3: Run the full gate**

Run: `cargo test --workspace`, then in `editors/vscode`: `pnpm test && pnpm lint && pnpm build`.
Expected: all green; the editor save suite in `native_save.rs` green with the two new tests.

- [ ] **Step 4: Commit**

Commit: `fix(editor): reject colon segments in save paths, matching BundlePath`

---

### Task 3: Add a wasm32 compile check to CI

**NOT pure motion — new CI coverage, sequenced early so Tasks 9–12 can run it.** Today no workflow compiles anything for wasm before merge: `.github/workflows/ci.yml` has no wasm target at all, and the only wasm build is the deploy-time `cargo makepad wasm build -p waml-editor` in `pages.yml` (`:127`) — after the gate, on main. This plan is about to add two filesystem modules to `waml::host` (Tasks 9 and 11); the web binary must not regress while that happens.

**Scope and honesty about teeth.** The check is `cargo check` on `waml` and `waml-ops-dto` for `wasm32-unknown-unknown` — deliberately NOT `waml-editor`, which requires the pinned cargo-makepad fork toolchain and its own flags (`pages.yml:127`); that is a deploy build, not a per-PR check, and the crates this plan modifies are `waml` and `waml-ops-dto`. Two limits, stated so nobody over-trusts the guard:
1. This check **cannot detect a missing `#[cfg(not(target_arch = "wasm32"))]` gate on fs-only code** — `std::fs` compiles on wasm32 and fails only at runtime. The gate itself (mandated in Global Constraints and in Tasks 9 and 11) is the enforcement; this check is the net around it.
2. What it DOES catch, pre-merge instead of at Pages deploy: any dependency, feature-unification, or `#[cfg]`-arithmetic change in `waml`/`waml-ops-dto` that stops compiling for the web target — the failure class that today surfaces only on main.

**Files:**
- Modify: `.github/workflows/ci.yml` (one new step in the existing test job, or a small parallel job — implementer's choice; no new workflow file)

- [ ] **Step 1: Add the CI step**

`rustup target add wasm32-unknown-unknown`, then:
`cargo check -p waml --features serde --target wasm32-unknown-unknown && cargo check -p waml-ops-dto --target wasm32-unknown-unknown`
(`--features serde` because that is how the editor consumes `waml`; also run once without features if cheap, to catch feature-gated cfg mistakes in both directions.)

- [ ] **Step 2: Verify locally and in CI**

Run the same commands locally (this Windows host has rustup; add the target if missing). Push the branch and confirm the new leg is green on the runner. Expected: green with zero source changes — this task changes CI only.

- [ ] **Step 3: Commit**

Commit: `ci: compile-check waml and waml-ops-dto for wasm32`

---

### Task 4: Move envelope expansion and bundle emitters to `waml::bundle_envelope`

Pure motion. `expand_text` (`crates/waml-cli/src/io.rs:14-24`) is a thin wrapper over `split_bundle`, which already lives in `waml::bundle_envelope`; `render_bundle_json` (`commands.rs:131`), `is_ts_export_name` (`:138`), and `render_bundle_ts` (`:148`) are artifact-format emission that belongs beside the codec (the TS-injection guard on `is_ts_export_name` is a real rule with a load-bearing doc comment — keep it).

**Files:**
- Modify: `crates/waml/Cargo.toml`
- Modify: `crates/waml/src/bundle_envelope.rs`
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/io.rs` (drop local `expand_text`, import)
- Modify: `crates/waml-cli/src/main.rs` (`:1001`, `:1015-1016`)

**Interfaces:**
- Produces: `waml::bundle_envelope::{expand_text, render_bundle_json, render_bundle_ts, is_ts_export_name}` with exact current signatures, including `expand_text`'s `std::io::Result` (kept by the behaviour-preserving rule; do not redesign the error type).

- [ ] **Step 1: Wire `serde_json` into `waml`'s serde feature**

`waml` today has `serde_json` only as a dev-dependency (`crates/waml/Cargo.toml`). Add `serde_json = { workspace = true, optional = true }` under `[dependencies]` and extend the feature to `serde = ["dep:serde", "dep:serde_json"]`. `waml-cli` already depends on `waml = { features = ["serde"] }` (`crates/waml-cli/Cargo.toml:21`), so no CLI manifest change.

- [ ] **Step 2: Move the code**

Move the four functions with their doc comments. Gate `render_bundle_json` and `render_bundle_ts` behind `#[cfg(feature = "serde")]` (both use `serde_json` for escaping); `is_ts_export_name` and `expand_text` are unconditional.

- [ ] **Step 3: Move the tests**

From `commands.rs`: `render_bundle_json_round_trips` (`:704`), `render_bundle_ts_emits_valid_export_with_escaped_content` (`:712`), `ts_export_names_admit_identifiers_and_nothing_else` (`:722`). From `io.rs`: `expands_v1_envelope_into_docs` (`:1069`), `plain_and_legacy_text_use_the_physical_display_path` (`:1081`), `malformed_envelope_includes_the_physical_input_name` (`:1091`), `stray_comment_doc_is_one_doc` (`:1107`). JSON/TS tests behind `#[cfg(feature = "serde")]`.

- [ ] **Step 4: Update call sites and run the full gate**

`main.rs:1001,1015-1016` and `io.rs:119` (inside `read_analysis_bundle`) and `io.rs:99` switch to `waml::bundle_envelope::…`. Gate: `cargo test --workspace` plus the vscode trio, plus Task 3's wasm check (this task touches `waml`'s feature graph — the check exists precisely for this). Additionally run `cargo test -p waml --features serde` so the gated tests execute somewhere explicitly. Expected: all green; `cli_e2e.rs` byte-identical (`mutation_stdout_is_an_authoritative_v1_envelope` `:482`, `multi_document_fmt_stdout_is_a_v1_envelope` `:512`).

- [ ] **Step 5: Commit**

Commit: `refactor(waml): move envelope expansion and bundle emitters to bundle_envelope`

---

### Task 5: Move `plan_fmt` and `FmtResult` to a new `waml::fmt`

Pure motion. Depends on Task 1 (`plan_fmt` calls `waml::validate::{prepare, diagnostics}`). `plan_fmt` (`crates/waml-cli/src/commands.rs:321-440`) encodes model rules any frontend offering "format document" needs or it deletes user text: skip errored files byte-for-byte (`:402-405`), skip files with prose between the H1 and the first `##` (`:407-431` — a data-loss guard with a regression test), pass index docs through verbatim (`:397-400`).

**Files:**
- Create: `crates/waml/src/fmt.rs`
- Modify: `crates/waml/src/lib.rs` (`pub mod fmt;`)
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/main.rs:422`

**Interfaces:**
- Produces: `waml::fmt::{FmtResult, plan_fmt}` — exact current signatures (`plan_fmt(files: &[(String, String)]) -> Result<Vec<FmtResult>, String>`, `FmtResult { path, formatted, changed, skipped }` from `commands.rs:170-175`). All imports (`SyntaxChangeBatch`, `ActionContext`, `Formatter`, `EditContext`, `BundlePath`) are `waml`-internal.

- [ ] **Step 1: Move code and tests**

Move `FmtResult` and `plan_fmt` verbatim into `crates/waml/src/fmt.rs`. Move their tests: `plan_fmt_leaves_index_docs_untouched` (`commands.rs:557`), `plan_fmt_still_skips_error_files_byte_for_byte` (`:638`), `formats_a_clean_file_and_detects_change` (`:655`), `skips_a_file_with_errors` (`:670`), `skips_a_file_with_pre_section_prose_instead_of_dropping_it` (`:682`). Module doc: fmt planning shared by the CLI today and by any future editor/LSP format command.

- [ ] **Step 2: Update the call site and run the full gate**

`main.rs:422` becomes `waml::fmt::plan_fmt`; `Fmt` handling otherwise untouched. Gate: `cargo test --workspace` plus vscode trio plus the wasm check. Expected: all green; `cli_e2e.rs` fmt tests (`:249` `fmt_stdout_preserves_generic_okf_exactly`, `:270` `fmt_canonical_output_is_idempotent`) byte-identical.

- [ ] **Step 3: Commit**

Commit: `refactor(waml): move fmt planning into waml::fmt`

---

### Task 6: Move `plan_indexes` and `IndexChange` to `waml::index_md`; one `is_index_basename` predicate

Pure motion plus one mechanical deduplication (behaviour-identical). Depends on Task 1. Closes audit finding **D8** and centralises the index-survival policy next to `reindex_source`. Today the policy is split across crates: `reindex_source` decides survival from the package forest (`crates/waml/src/index_md.rs:191-199`) and `plan_indexes` re-filters with a different rule — directories that transitively hold a non-index document, plus root (`commands.rs:253-268`) — plus case-collision detection (`:235-241`) and case-preserving write-back mapping. The "basename is `index.md`, case-insensitive" test is spelled 8 times across `commands.rs` (`:226,:235,:256,:267,:276,:400`), `io.rs:230`, and `index_md.rs:193-197`.

**Sequencing consequence, stated up front:** after this task `crates/waml-cli/src/io.rs` imports `IndexChange` from `waml::index_md` — `write_indexes` (`io.rs:305-339`) and its `use crate::commands::IndexChange` (`io.rs:11`) rewire, and the `write_indexes_*` tests' `crate::commands::IndexChange` references (`io.rs:1493`, `:1512`, `:1527`, `:1549`, `:1573`) follow.

**Files:**
- Modify: `crates/waml/src/index_md.rs`
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/io.rs` (`:11`, `:230`, test refs)
- Modify: `crates/waml-cli/src/main.rs` (`:544`, `:554-555`)

**Interfaces:**
- Produces: `waml::index_md::{IndexChange, plan_indexes, is_index_basename}`. `plan_indexes(files: &[(String, String)]) -> Result<Vec<IndexChange>, String>` and `IndexChange::{Upsert{path,rendered}, Remove{path}}` keep exact current shapes (`commands.rs:177-181`, `:221-301`). New `pub fn is_index_basename(path: &str) -> bool` extracted from the existing spellings — pure predicate, no behaviour change.

- [ ] **Step 1: Move code and tests**

Move `IndexChange` and `plan_indexes` verbatim into `crates/waml/src/index_md.rs` (it already holds `reindex_source`, which `plan_indexes` calls at `commands.rs:249`). Move tests `plan_indexes_keeps_deep_ancestors_and_removes_an_orphan_index` (`commands.rs:514`) and `plan_indexes_rejects_case_colliding_index_paths` (`:544`).

- [ ] **Step 2: Extract the predicate**

Add `is_index_basename` and use it at every former spelling: the moved `plan_indexes` internals, `reindex_source`'s retain (`index_md.rs:193-197`), and `io.rs:230` inside `resolve_index_target`. Behaviour-identical: same `eq_ignore_ascii_case("index.md")` on the final path segment.

- [ ] **Step 3: Rewire and run the full gate**

`main.rs:544,554-555` and `io.rs` import from `waml::index_md`. Gate: `cargo test --workspace` plus vscode trio plus the wasm check. Expected: all green; the `cli_e2e.rs` index suite byte-identical — specifically `:564`, `:592`, `:637` (ordering), `:674` (stale replacement preserves non-index bytes), `:697` (mixed-case fixpoint), `:719` (orphan removal).

- [ ] **Step 4: Commit**

Commit: `refactor(waml): move index planning into waml::index_md`

---

### Task 7: Move the export search-index builders to `waml::search`

Pure motion. Depends on Task 1. Closes audit finding **D5**'s structural half: `build_search_index_asset` (`crates/waml-cli/src/commands.rs:194-201`) and `build_search_index_asset_for_envelope` (`:213-219`) encode the export boundary rule "hash the pairs a *decode* of the shipped envelope yields", whose mirror lives in the editor's boot (`crates/waml-editor/src/app.rs:1269` hashes decoded pairs; `:1212` silently drops the asset on mismatch). Agreement is currently pinned only by the CLI-side test at `commands.rs:454`. Moving both builders next to `waml::search::asset::{encode, decode, bundle_hash}` puts the rule and its mirror in one crate, reachable by a future in-browser exporter.

**Files:**
- Modify: `crates/waml/src/search/` (new `export.rs` submodule, or extend `asset.rs` — implementer's choice; keep the doc comments, they carry the boundary rationale)
- Modify: `crates/waml/src/lib.rs` / `search/mod.rs` as needed
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/main.rs:1067`

**Interfaces:**
- Produces: `waml::search::export::{build_search_index_asset, build_search_index_asset_for_envelope}` (or `search::asset::…`), exact current signatures returning `Result<Vec<u8>, String>`.

- [ ] **Step 1: Move code and tests**

Move both functions verbatim; they consume `waml::validate::prepare` (Task 1), `search::extract::extract_bundle`, `MemSearchIndex::build`, `search::asset::{bundle_hash, encode}`, and `bundle_envelope::split_bundle` — all in-crate after Tasks 1 and 4. Move tests `the_exported_assets_hash_is_the_one_a_boot_decode_computes` (`commands.rs:454`) and `build_search_index_asset_carries_the_bundle_hash_and_finds_known_terms` (`:483`).

- [ ] **Step 2: Rewire and run the full gate**

`main.rs:1067` (`run_export_site`) imports the moved function. Gate: `cargo test --workspace` plus vscode trio plus the wasm check (these functions are pure of `std::fs` and must stay wasm-compilable — an in-browser exporter is the point of the move). Expected: all green; `commands.rs` is now empty of everything except rendering (which stays per Global Constraints) — if only rendering remains, leave the file in place rather than renaming it in this task.

- [ ] **Step 3: Commit**

Commit: `refactor(waml): move export search-index builders into waml::search`

---

### Task 8: Extract shared ingest triage and rooted keying into `waml::host::ingest`

Behaviour-preserving motion touching two crates. Closes audit finding **D6** and the CLI/editor half of **D7**. The 20-line error-triage policy — `LinkSkipped` is reported and the walk continues; the first other error is fatal — is duplicated verbatim between `crates/waml-cli/src/io.rs:42-63` (`collect_md_with`) and `crates/waml-editor/src/load.rs:66-98` (`read_bundle_with`). The "key documents relative to the bundle root, `\` → `/`" identity rule is spelled in `io.rs:69-71` (`path_key`) + `:110-128` and `load.rs:88-94`.

**Files:**
- Modify: `crates/waml/src/host/ingest.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-editor/src/load.rs`

**Interfaces:**
- Produces in `waml::host::ingest` (already gated `#[cfg(not(target_arch = "wasm32"))]` at `host/mod.rs:5-6` — no cfg change needed): a triage helper, e.g. `pub fn triage(errors: Vec<IngestError>, report_skipped_link: &mut dyn FnMut(&IngestError)) -> Result<(), IngestError>` (skipped links to the sink, first other error returned), and a keying helper, e.g. `pub fn rooted_key(root: &Path, file: &Path) -> String` (strip prefix where possible, normalize `\` to `/`). Exact names are the implementer's; the contract is that both callers become thin.
- Consumes: nothing new; both helpers are extractions of existing behaviour.

- [ ] **Step 1: Extract the triage helper**

Lift the shared loop into `host::ingest`; `io.rs::collect_md_with` and `load.rs::read_bundle_with` call it. Their differing fatal-error wrappers (`io::Error::other` vs `LoadError::Io`) stay at the call sites — that is presentation, not policy.

- [ ] **Step 2: Extract the keying helper**

Lift the strip-prefix + backslash normalization used by `io.rs::read_analysis_bundle`/`read_physical_bundle` and `load.rs:88-94`. Do NOT touch `bundle_root` (`io.rs:139-155`, multi-path common-ancestor logic — CLI-only invocation semantics, stays), `display_paths` (CLI courtesy, stays), or the LSP's `logical_path` (`lsp/bundle.rs:310-332` — deferred, see below).

- [ ] **Step 3: Run the full gate**

Gate: `cargo test --workspace` plus vscode trio plus the wasm check. Named pins: `io.rs` rooted-read tests (`:1006` `rooted_read_keys_relative_to_the_bundle_dir`, `:1025` skip-editor-state, `:1041` path-independence, `:1051` common-ancestor, `:1062` single-file basename, `:859` `collect_md_survives_a_skipped_link`), and `load.rs` tests (`:139` sorted pairs, `:166` dot-directories, `:200` skipped-link survival). All byte-identical behaviour.

- [ ] **Step 4: Commit**

Commit: `refactor(waml): share ingest triage and rooted keying in host::ingest`

---

### Task 9: Move the `write_back` transaction into `waml::host::persist`

Pure motion — the largest, and deliberately motion-only; convergence with the editor's save is Task 12. The staged/journaled/rollback transaction (`crates/waml-cli/src/io.rs:361-814`: `write_back`, `FsOps`, `write_back_with_ops`, `JournalEntry`, `StagedWrite`/`StagedDelete`, `ensure_regular_file`, `create_staging_directory`, `validate_relative`, `validate_target`, `create_missing_parents`, `rollback`, `clear_deletion_blocking_permissions`, `rollback_error`) is self-contained: fs access is already abstracted behind `FsOps` for fault injection, and nothing in it touches clap, stdin, or CLI presentation. It has **four** in-binary call sites — `main.rs:474` (the `fmt` write path), `main.rs:818` (`run_batch`), `serve/state.rs:115` (`apply_ops`), `serve/state.rs:204` (`apply_documents`) — which is the "second caller" the 2026-08-04 design was waiting for, twice over; the editor is the fifth, in Task 12.

**Files:**
- Create: `crates/waml/src/host/persist.rs`
- Modify: `crates/waml/src/host/mod.rs` (`#[cfg(not(target_arch = "wasm32"))] pub mod persist;` — mirroring `ingest` at `host/mod.rs:5-6`; this gate is mandatory, see Global Constraints)
- Modify: `crates/waml/Cargo.toml` (only if the `test-support` feature needs declaring on new items — the feature already exists)
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/src/main.rs` (`:474`, `:818` — import rewire; the 3-arg signature is unchanged)
- Modify: `crates/waml-cli/src/serve/state.rs` (`:115`, `:204` — import rewire)

**Interfaces:**
- Produces: `waml::host::persist::write_back(root, old, new) -> std::io::Result<Vec<String>>` — exact current signature and message strings (the returned "wrote/deleted {path}" lines and the staging-cleanup warning text are asserted by tests and printed by the CLI; do not reword).
- **Fault-injection seam (decided here, consumed by Task 12).** A `pub use` of a private item is a compile error (E0365 — `pub use` cannot widen visibility), so a plain feature-gated re-export of private items is not an option. The seam is built from two in-tree precedents instead:
  - `FsOps` becomes **unconditionally `#[doc(hidden)] pub`**, matching the existing `host` convention (`crates/waml/src/host/mod.rs:42-73`, where `add_document`/`replace_document`/`remove_document` are exactly this). This is forced, not chosen: `waml-editor`'s tests must `impl FsOps` on their fault injectors, and an external crate cannot implement a non-`pub` trait — no wrapper can route around that.
  - `write_back_with_ops` stays **private**; the injectable entry point is exposed via the `waml::analysis::test_support` pattern (`crates/waml/src/analysis.rs:794-796`): `#[cfg(feature = "test-support")] #[doc(hidden)] pub mod test_support` containing a **newly defined** `pub fn write_back_with_ops(...)` that delegates to the private parent-module function — legal, because a child module may *call* its parent's private items; it only cannot *re-export* them.

  Stated plainly, the tension this leaves: the earlier "crate-private by default" posture is weakened for the trait — any dependent can now see and implement `FsOps` regardless of features (doc-hidden, but public). What remains gated is what matters: the ability to *inject* an implementation into a transaction exists only under `test-support`. Inside `waml`, the moved fault-injection suite keeps using the private function directly, no feature needed. Outside, `waml-editor` already enables `waml = { features = ["serde", "test-support"] }` in its dev-dependencies (`crates/waml-editor/Cargo.toml:60`), and that dev-deps-only route is proven in-tree: `crates/waml-editor/src/editor_session/tests.rs:1571` already consumes `waml::analysis::test_support::PreparationProbe` through it — no manifest change needed.

- [ ] **Step 1: Move code and the whole fault-injection suite**

Move everything listed above verbatim, including all tests named in Global Constraints (`io.rs:816-1623`'s transaction subset, including the Windows-only read-only and junction tests and their `#[cfg(windows)]` helpers). Mark `FsOps` `#[doc(hidden)] pub` and add the gated `test_support` module with its delegating wrapper, per the Interfaces paragraph. `io.rs` keeps `write_indexes`/`resolve_index_target` (they move conceptually with confinement in Task 11) and re-exports nothing — call sites import `waml::host::persist` directly.

- [ ] **Step 2: Rewire the four call sites**

`main.rs:474` (fmt), `main.rs:818` (run_batch), `serve/state.rs:115`, `serve/state.rs:204`. All are import-path changes only — the 3-argument signature is untouched in this task.

- [ ] **Step 3: Run the full gate on both this host and CI**

Gate: `cargo test --workspace` plus vscode trio plus the wasm check (`persist` is the first module this plan adds behind the `host` cfg gate — the check must stay green with the gate in place). The moved suite includes Windows-specific behaviour (read-only displacement, junctions); this host is Windows — after the local gate, push the branch and confirm the Linux CI leg is green before calling the task done. Named pins: the full fault-injection suite, `cli_e2e.rs:163` and `:299` (rollback observed through the binary), serve write tests.

- [ ] **Step 4: Commit**

Commit: `refactor(waml): move the transactional bundle write into host::persist`

---

### Task 10: Share the `/api/documents` wire type via `waml-ops-dto`

**NOT pure motion — a wire-preserving dependency change.** Closes audit finding **D4**. `DocumentWrite` (`crates/waml-cli/src/serve/state.rs:216-221`, serde) and the editor's *private duplicate serde structs* — `DocumentWriteWire`/`DocumentsRequestWire` (`crates/waml-editor/src/api_save.rs:22-34`, serialized at `:63`) — are two spellings of one wire shape with no shared type; the doc comment on `DocumentWriteWire` (`api_save.rs:17-20`) and the test at `state.rs:295-318` both say outright that literal JSON is the only pin. (An earlier draft of this plan called the editor side "hand-built JSON"; it is not — it is serde on a private duplicate type, which makes this a textbook D4 duplication and lowers the wire-byte risk: both sides already serialize the same field names via serde.) The precedent is exact: `OpDto` was moved to `waml-ops-dto` "so the WASM bindings can reuse it" (`crates/waml-cli/src/ops_dto.rs:1-4`).

**What changes:** `DocumentWrite` (and, if the implementer judges it cheap, the `DocumentsRequest`/`DocumentsResponse` envelope shapes from `serve/routes.rs:219-228`) moves to `waml-ops-dto`; `waml-editor` gains a dependency on `waml-ops-dto` (it has none today — verified against `crates/waml-editor/Cargo.toml`) and `api_save.rs` deletes its private wire structs in favour of the shared type. **The bytes on the wire must not change.**

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `crates/waml-cli/src/serve/state.rs`, `serve/routes.rs`
- Modify: `crates/waml-editor/Cargo.toml`, `crates/waml-editor/src/api_save.rs`

- [ ] **Step 1: Move the type**

`DocumentWrite { path, baseline: Option<String>, desired }` moves to `waml-ops-dto` with its serde derives and doc comments (the "baseline: None means new file; the server's bundle is the authority" contract text is load-bearing). Serve imports it.

- [ ] **Step 2: Editor deletes its duplicate and uses the shared type**

`documents_request` (`api_save.rs:42`) constructs `{ revision, writes: Vec<DocumentWrite> }` from the shared type; `DocumentWriteWire` and `DocumentsRequestWire` are deleted. Keep the existing `api_save.rs` tests asserting the serialized body byte-for-byte, and keep the literal-JSON round-trip test at `state.rs:295-318` — it flips from "the only pin" to a regression check on the shared type's serde output, which is exactly what a wire test should be.

- [ ] **Step 3: Run the full gate**

Gate: `cargo test --workspace` plus vscode trio plus the wasm check (`waml-ops-dto` is wasm-consumed and `waml-editor` builds for wasm32 — the check now covers the crate this task grows). Named pins: `state.rs` `the_editor_wire_shape_round_trips`, `routes.rs` `a_documents_post_mutates_the_disk` (`routes.rs:417`), `api_save.rs` request-body tests (`:128`, `:157`, `:180`), `serve_e2e.rs` untouched.

- [ ] **Step 4: Commit**

Commit: `refactor(ops-dto): share the /api/documents wire shape`

---

### Task 11: Unify path confinement into one `waml::host` module

**NOT pure motion — this is the design work of the plan.** Closes audit finding **D3**. Depends on Task 9 (`validate_target` now lives in `persist`).

**The corrected premise.** The four confinement implementations do NOT share one rule; they split into two symlink policies, and any "single shared resolver" would silently change one of them:
- **Categorical refusal** — `resolve_index_target` (`crates/waml-cli/src/io.rs:209-303`) rejects a symlinked parent (`:241-246`) or symlinked target (`:285-290`) with `PermissionDenied` (`escaped_index_path`, `:205-207`) **even when the link resolves inside the root**.
- **Follow-and-contain** — the other three follow symlinks and reject only escapes: `safe_join` (`serve/paths.rs:75-107`, errors as `String` → `ApplyFailure::Confinement` → 422 via `routes.rs:253-281`), `native_save::resolved_target` (`native_save.rs:204-235`, `InvalidInput`), `persist`'s `validate_target` (formerly `io.rs:642-666`, `InvalidInput`).

Three error contracts are pinned by existing tests: `io.rs:1556` asserts `PermissionDenied`; the `native_save` suite asserts `InvalidInput`; serve asserts 422 (`routes.rs:492`). And one test gap makes the policy split invisible today: the `write_indexes` symlink tests (`io.rs:1539`, `:1561`) use only out-of-root link targets, so a resolver that quietly switched `resolve_index_target` to follow-and-contain would pass every existing test while loosening the index writer.

**Design.** One module, two explicit policies, one typed error; every caller keeps its exact current acceptance set and error contract:

- `waml::host::confine` (gated `#[cfg(not(target_arch = "wasm32"))]` at `host/mod.rs`, like `ingest` and `persist` — mandatory):
  - `pub fn check_rel(rel: &str) -> Result<(), ConfineError>` — the **syntactic union** (applies to writers only; see the asymmetry inventory): empty, NUL, absolute (`/` or `\`), UNC, drive prefix, empty segment, `.`/`..`, interior `:` (Task 2's rationale), Windows reserved device names (`paths.rs:53-68`'s table, checked on every platform).
  - `pub enum SymlinkPolicy { RefuseAny, FollowWithinRoot }`
  - `pub fn resolve_under(root: &Path, rel: &str, policy: SymlinkPolicy, create_parents: bool) -> Result<PathBuf, ConfineError>` — `RefuseAny` walks components checking `symlink_metadata` at each step exactly as `resolve_index_target` does today (including its create-as-you-descend behaviour when `create_parents`, `io.rs:262-272` — creation stays inside the resolver to avoid a check/create race); `FollowWithinRoot` is the nearest-existing-ancestor + canonicalize + containment walk `safe_join`/`resolved_target`/`validate_target` share, with `nearest_existing_ancestor` (duplicated verbatim today at `paths.rs:109-125` and `native_save.rs:237-253`) becoming one private function.
  - `pub enum ConfineError { Syntactic(String), SymlinkRefused(PathBuf), EscapesRoot(PathBuf), NotADirectory(PathBuf), NotAFile(PathBuf), Io(std::io::Error) }` — callers map to their own contracts; `confine` never picks an `io::ErrorKind` or an HTTP status.

**Per-caller before/after (state these in the commit body; each row pinned by a named test):**

| Caller | Policy | Accepted set: change | Error contract: unchanged |
| --- | --- | --- | --- |
| `io.rs::resolve_index_target` (index writer) | `RefuseAny` | **Unchanged** — categorical refusal preserved, including in-root links | `SymlinkRefused` → `PermissionDenied`; `Syntactic`/`EscapesRoot`/`NotADirectory` → `InvalidInput` (pins `io.rs:1488-1581`, esp. `:1556`) |
| `serve/paths.rs::safe_join` (wire input) | `FollowWithinRoot` | Newly rejects interior-`:` at the confinement layer (today such paths die later at `SourceBundle::try_from_pairs`/`BundlePath::parse` as `ApplyFailure::Invalid` — same 422, different error body). Device names were ALREADY rejected by `is_safe_rel` (`paths.rs:42-44`); no change there | Everything → `String` → `ApplyFailure::Confinement` → 422 (pins `routes.rs:492`) |
| `persist` (`validate_relative`+`validate_target`) | `FollowWithinRoot` | Newly rejects reserved device names (`con.md` etc. — today they pass validation and hit platform-dependent fs behaviour) | `InvalidInput`, and the syntactic mapping keeps the exact message text "bundle path must be relative and traversal-free: …" so the moved test at (former) `io.rs:1147-1151` stays untouched |
| `native_save::resolved_target` | `FollowWithinRoot` | Newly rejects reserved device names (colon already rejected by Task 2) | `InvalidInput` (pins the existing `native_save` suite) |

**Read/write asymmetry, inventoried and accepted.** A bundle containing `con.md` (or any reserved device name) still LOADS everywhere — `ingest` and `BundlePath::parse` have no device-name check — but after this task it can never be SAVED by any writer, and because every writer is transactional, one offending path fails the entire save/fmt/serve-write rather than just that file. Accepted deliberately: failing loudly with the offending path named is strictly better than writing to a console device on Windows or producing a platform-divergent bundle; and the failure is at the point of mutation, where the author can rename. The clean fix for the asymmetry — a load-time validation diagnostic for device-named paths, so `waml check` flags what save will refuse — is real work in the validate layer and is named in "Deferred, with reason", not smuggled in here.

**Files:**
- Create: `crates/waml/src/host/confine.rs`
- Modify: `crates/waml/src/host/mod.rs` (cfg-gated `pub mod confine;`)
- Modify: `crates/waml/src/host/persist.rs`
- Modify: `crates/waml-cli/src/serve/paths.rs` (thin wrapper or delete-and-rewire of `serve/state.rs:112`, `:149`)
- Modify: `crates/waml-cli/src/io.rs` (`resolve_index_target` consumes `resolve_under(RefuseAny)`; keeps its index-only rules — the `is_index_basename` gate and the not-a-file screens)
- Modify: `crates/waml-editor/src/native_save.rs` (`resolved_target` + `nearest_existing_ancestor` replaced by `resolve_under(FollowWithinRoot)`)

- [ ] **Step 1: Build `confine` with a policy-aware test matrix**

Tests live in `waml` and must NOT use `tempfile` (`waml`'s dev-deps are `serde_json` and `proptest` only — `crates/waml/Cargo.toml`; this plan adds no dependency). Port the `safe_join`/`is_safe_rel` tests (`serve/paths.rs:127-199`, which use `tempfile::tempdir()` at `:166`, `:183`) onto the manual `TempDir` guard pattern the workspace already uses (`io.rs:821-840` is the template: process-id + atomic-counter temp path, `remove_dir_all` on drop). The matrix: for each hostile path (`../x`, `a/../../x`, `/abs`, `C:\x`, `\\unc\x`, `a\0b`, empty, `.`, `con.md`, `COM1.md`, `lpt3.txt`, `nul`, `ab:c.md`), `check_rel` rejects — one table, one rule; negative cases `console.md`, `com10.md` accepted (`paths.rs:133`'s existing negatives). Symlink cases per policy: an in-root link is **rejected** under `RefuseAny` and **followed** under `FollowWithinRoot`; an out-of-root link is rejected under both.

- [ ] **Step 2: Close the in-root-symlink test gap FIRST**

Before rewiring anything, add `write_indexes_rejects_an_in_root_symlinked_parent` to `io.rs`'s suite: a symlink inside the root pointing at a sibling directory inside the root, `write_indexes` refuses with `PermissionDenied`. This pins the categorical policy the existing out-of-root tests (`io.rs:1539`, `:1561`) cannot distinguish, so the rewire in Step 3 is done against a test that would catch the loosening. (Follow the suite's existing convention of skipping gracefully when the host cannot create links, `io.rs:1543-1545`.)

- [ ] **Step 3: Rewire the four callers, one at a time, keeping each suite green**

Order: `persist` (fault-injection suite + message-text pins stay green), then `serve/paths.rs` (its tests become thin-wrapper tests or move to `confine`), then `io.rs::resolve_index_target` (full `write_indexes_*` suite including Step 2's new test), then `native_save` (save suite + Task 2's colon tests). Add one new named test per newly-rejecting caller: `persist` rejects `con.md` with `InvalidInput`; `native_save` rejects `con.md`; serve's confinement rejects `ab:c.md` with 422 at the routes layer.

- [ ] **Step 4: Run the full gate, both OSes**

Gate: `cargo test --workspace` plus vscode trio plus the wasm check locally; push and confirm the Linux CI leg (symlink tests take real symlink/junction paths on each OS; the Windows-only cases ran locally). Named pins: everything in Step 3 plus `routes.rs:492` `an_escaping_documents_path_is_422_not_500`.

- [ ] **Step 5: Commit**

Commit: `refactor(waml): unify bundle path confinement in host::confine`

---

### Task 12: Editor save adopts `host::persist` behind a deletion policy

**NOT pure motion — the D2 unification, with failure-mode tests.** Depends on Tasks 9 and 11. Today the editor's `save_bundle_atomic` (`crates/waml-editor/src/native_save.rs:10-109`) and the shared transaction differ in guarantees: the editor refuses deletions outright (`:28-35`), writes per-file with no journal, and preserves clean files' bytes untouched (`:60-77`); `persist::write_back` deletes files dropped from the bundle, journals every step, and rolls back on failure. "Native and CLI persistence have different transaction guarantees" is a standing P1; this task closes it in the conservative direction.

**Decision 1 — deletions: the editor keeps refusing them.** `persist` grows an explicit policy — e.g. `DeletePolicy::{Refuse, Transact}` (names are the implementer's) — where `Refuse` fails before staging if any old-bundle path is absent from the new bundle, with the editor's current message ("removing bundle files is not supported by atomic save").

**Decision 2 — entry points: the existing 3-arg `write_back` signature does not change.** All four existing callers (`main.rs:474`, `:818`, `serve/state.rs:115`, `:204`) keep calling `write_back(root, old, new)`, which is defined as `Transact`. A second public entry, `write_back_with_policy(root, old, new, policy)`, carries the policy; the editor calls that. Internally one private full-parameter function takes `(root, old, new, policy, ops)`; Task 9's gated `test_support` module grows a second delegating wrapper for it (same pattern: a newly defined `pub fn` calling the private parent-module function — never a `pub use`), so both the `persist` suite and the editor-side tests inject faults through the same seam, against the unconditionally-public `#[doc(hidden)] FsOps` trait. **No waml-cli file changes in this task.**

**Decision 3 — TOCTOU: the per-write interleaved disk re-check is retired; the window widens from per-write to per-transaction, and that is accepted.** Today `save_bundle_atomic` runs `disk_state` (`native_save.rs:266-293`) three times: at plan time, after directory creation (`:88-95`), and immediately before each individual write (`:97-104`) — real content-level conflict detection (`save_conflict` → `ErrorKind::AlreadyExists`, `:295-300`), with an in-code admission that the residual race "cannot be eliminated" (`:97-101`). Under this design the editor keeps plan-time `disk_state` AND re-runs the full `disk_state` screen once more immediately before the single `persist` call; what disappears is the per-write interleaving, so an external edit landing *during* the transaction is no longer detected. Why this is acceptable: (a) the window was already nonzero and acknowledged; it changes from "between the last per-file check and that file's rename" to "between the pre-transaction screen and that file's rename" — bounded by transaction duration on a local disk; (b) in exchange, a mid-save failure now rolls the whole save back instead of leaving a half-written bundle, which is the strictly worse and strictly more common failure today; (c) the honest fix for the narrower race — per-write content preconditions (compare-and-swap) inside `persist` — would serve the CLI and serve too and is named in "Deferred, with reason" as the follow-up if the wider window proves harmful. The pre-transaction conflict screen is pinned by a new named test (Step 3), which no existing test does (they assert only `error.kind()`).

**What the editor keeps doing itself (unchanged):** baseline indexing, external-edit tolerance for *clean* files (a clean file changed on disk by another program is never rewritten — the editor computes its dirty set before calling `persist` and hands over only dirty pairs), and alias detection — the last possibly redundant with `persist`'s duplicate-target reservation (former `io.rs:428-449`, including the Windows/macOS case-fold matching `native_save.rs:194-197`); delete the editor copy only if a test proves the shared one covers the same cases.

**Files:**
- Modify: `crates/waml/src/host/persist.rs` (delete policy + `write_back_with_policy` + extended `test_support`)
- Modify: `crates/waml-editor/src/native_save.rs` (`save_bundle_atomic` becomes planning + one `write_back_with_policy(..., Refuse)` call; `save_ticket_atomic` unchanged)

- [ ] **Step 1: Add the delete policy to `persist`**

`Refuse` fails the transaction before staging with the editor's current message; `Transact` is today's behaviour and the 3-arg `write_back` delegates to it. Pin `Refuse` with a `persist` unit test; confirm by inspection (and by the untouched suite) that no existing caller's behaviour changed.

- [ ] **Step 2: Rebuild the editor save on the shared transaction**

Planning phase unchanged (validate, alias-check, `disk_state` plan pass, dirty-set computation). Then: `create_dir_all` for planned parents stays editor-side (as today, `:82-84`), one more `disk_state` screen over the pending set (preserving the current `:88-95` re-check, now as the final pre-transaction screen), then a single `write_back_with_policy(root, planned_old, planned_new, Refuse)` call replaces the per-write loop (`:97-107`). `resolved_target`, `nearest_existing_ancestor`, and `write_atomic`/`replace_file` become dead editor-side and are deleted (confinement moved in Task 11; atomic replacement is `persist`'s).

- [ ] **Step 3: Failure-mode tests on the editor path**

New tests in `native_save.rs`, injecting through `waml::host::persist::test_support` (available because `waml-editor` dev-deps already enable `test-support`, `crates/waml-editor/Cargo.toml:60`):
- a mid-save rename failure on the second of two dirty files rolls the first back to its baseline bytes;
- a rollback failure reports the retained recovery journal path in the error;
- an external edit to a *clean* file survives a save untouched (existing behaviour, now pinned against the new engine);
- an external edit to a *dirty* file made before the save call is refused as a save conflict with `ErrorKind::AlreadyExists` and nothing written (pins the pre-transaction screen of Decision 3);
- deletions still refused (existing test, now exercising `DeletePolicy::Refuse`).
Existing save tests stay green.

- [ ] **Step 4: Run the full gate, both OSes**

Gate: `cargo test --workspace` plus vscode trio plus the wasm check locally; push and confirm Linux CI. Named pins: the whole `native_save.rs` suite plus the new failure-mode tests; the `persist` fault-injection suite; `cli_e2e.rs:163`, `:299`.

- [ ] **Step 5: Commit**

Commit: `feat(editor): journaled atomic save via host::persist`

---

## Spec Coverage

Mapping to the 2026-08-10 audit findings, verdict rows, and the adversarial-review amendments. Nothing is silently dropped; rows not covered by a task are in "Deferred, with reason" or "Open Questions".

| Audit finding / MOVE row / review amendment | Task |
| --- | --- |
| D1 — diagnostics remap duplicated (`commands.rs:303` ≡ `validate.rs:43`) | Task 1 |
| D2 — two transactional persistence implementations | Task 9 (motion), Task 12 (unification) |
| D3 — confinement family under two symlink policies, `nearest_existing_ancestor` copied verbatim | Task 11 |
| D4 — `DocumentWrite` wire shape duplicated as private serde structs | Task 10 |
| D5 — export search-index hash boundary mirrored in editor boot | Task 7 (structural half; the boot-side mirror is unchanged by design — the rule now lives beside the primitives it constrains) |
| D6 — ingest error-triage duplicated CLI/editor | Task 8 |
| D7 — bundle keying rule, three spellings | Task 8 (CLI + editor); LSP `logical_path` deferred |
| D8 — `index.md` basename rule, 8 sites | Task 6 |
| `:`-in-path divergence (`io.rs:621` vs `native_save.rs:167`) | Task 2 (corrected: latent, not reachable — `BundlePath::parse` at `source.rs:40-42` already rejects colons; aligned as defense-in-depth) |
| Review B — wasm boundary: cfg gates + compile check | Task 3 (CI check), gates mandated in Global Constraints and Tasks 9, 11 |
| Review A — fault-injection seam across crates (E0365-safe: doc-hidden pub trait + gated delegating wrappers) | Task 9 Interfaces, consumed in Task 12 Step 3 |
| Review C — confinement redesign (two policies, three error contracts, in-root-symlink gap, read/write asymmetry) | Task 11 |
| Review D — third/fourth `write_back` call sites | Task 9 Files + Step 2 |
| Review E — entry-point signatures for the delete policy | Task 12 Decision 2 |
| Review F — editor wire structs are serde, not hand-built | Task 10 (description corrected) |
| Review G — no `tempfile` in `waml` | Task 11 Step 1 |
| Review H — TOCTOU window decision | Task 12 Decision 3 |
| MOVE `write_back` family → `waml::host` | Task 9 |
| MOVE `prepare`/`diagnostics` → `waml::validate` | Task 1 |
| MOVE `DocumentWrite` → `waml-ops-dto` | Task 10 |
| MOVE `plan_fmt` + `FmtResult` → `waml::fmt` | Task 5 |
| MOVE `plan_indexes` + `IndexChange` → `waml::index_md` | Task 6 |
| MOVE confinement → shared module | Task 11 |
| MOVE `build_search_index_asset*` → `waml::search` | Task 7 |
| MOVE ingest triage → `waml::host::ingest` | Task 8 |
| SPLIT `path_key`/rooting → `waml::host` (keying half) | Task 8; `bundle_root`, `display_paths`, stdin handling stay CLI per the audit's seam |
| MOVE (low) `expand_text`, `render_bundle_ts/json`, `is_ts_export_name` → `waml::bundle_envelope` | Task 4 |
| STAYS rows (rendering, exit codes, clap tree, `read_ndjson`, serve guard/routes/ui, `ServeState` policy, LSP modules, `site.rs`, `web_artifact.rs`, `build.rs`) | No task, by design — see Global Constraints and "Deferred" |

Downstream consequence, not a task here: after Tasks 1, 5, 6 the dropped docs-conformance harness (`crates/waml/tests/docs_conformance.rs` per the 2026-08-04 design) becomes writable as ordinary `waml` integration tests — `plan_fmt`, `plan_indexes`, and `validate::diagnostics` are all importable cross-crate.

## Deferred, with reason

- **The docs-conformance harness.** This plan unblocks it; building it is the 2026-08-04 plan's Part 2, to be re-planned against post-seam reality (its file:line map has rotted once already).
- **Per-write content preconditions in `persist` (compare-and-swap writes).** The honest fix for the transaction-wide TOCTOU window Task 12 accepts. It would serve the CLI and serve as well as the editor, and it changes `persist`'s public contract — its own project, taken up only if the wider window proves harmful in practice.
- **A load-time diagnostic for device-named paths.** Task 11 creates a deliberate read/write asymmetry (`con.md` loads, can never save). Closing it belongs in the validate layer — a warning diagnostic so `waml check` flags what save will refuse — not in a confinement module.
- **A `waml-lsp` crate split.** The LSP's only frontend is `waml lsp --stdio`; `map.rs`/`query.rs` are LSP-typed through and through. Splitting the crate would let `lsp_e2e.rs` stop hand-framing JSON-RPC, but that is test ergonomics, not a duplicated rule — no frontend is waiting on it.
- **LSP `logical_path` adopting the shared keying rule** (`lsp/bundle.rs:310-332`). It adds an `__external__/` mangling scheme the other two spellings lack; folding it in means designing where out-of-root documents key, which is LSP session semantics, not motion. Revisit after Task 8 settles the shared helper's shape.
- **`EditorSession`/`LspAnalysisState` unification.** The overlay state machine in `lsp/bundle.rs` is conceptually the editor session's problem, but unifying them is a design project with its own concurrency invariants (generations, CAS, publication ordering) — not code motion.
- **`render_diff` and the renderers.** Stay in the CLI per the audit; the 2026-08-04 doc's `waml::report` module is dropped deliberately.
- **`site.rs::is_safe_relative_path` and `build.rs::validate_path`.** Fifth and sixth syntactic siblings of D3, but they validate *our own build artifact's manifest*, not user paths, and `build.rs` cannot depend on the library it builds. Left as documented siblings (`site.rs:199-207` already states the relationship).
- **A wasm compile check for `waml-editor` itself.** Requires the pinned cargo-makepad fork toolchain (`pages.yml:127`); per-PR cost and toolchain coupling make it a deploy-pipeline concern, not a gate step. Task 3's `waml`/`waml-ops-dto` check covers the crates this plan modifies.

## Open Questions (for a human, not tasks)

1. **Two JSON diagnostic shapes from one binary.** `waml check --format json` emits `DiagDto` (`commands.rs:10-19`: severity/code/message/file/line/span) while `GET /api/diagnostics` emits serde-derived `waml::diagnostic::Diagnostic` (`serve/state.rs:68-70`). Divergence by design or by accident? If these should converge, that is a wire-contract decision with external consumers on both sides — decide before anyone "fixes" it in passing.
2. **Should the editor ever support deletions in save?** Task 12 deliberately preserves the refusal (`native_save.rs:28-35`) via a policy flag. If deletion support is wanted (e.g. for delete-node-with-file flows), it becomes a one-line policy change *after* Task 12 — but the product decision is not this plan's to make.
