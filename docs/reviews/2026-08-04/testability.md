# Testability review — 2026-08-04

Dimension: Testability (full evaluation).
Files examined: 16 (charter, MAP, workspace Cargo.toml, ci.yml, waml-editor lib.rs / tests/README.md / config.rs / editor_session.rs / inspector_panel.rs / browser_boot.rs, waml share.rs, golden.rs, waml-cli serve/mod.rs / lsp_e2e.rs, scripts/*.test.mjs listing, vscode src listing) plus tokensave test-risk graph (3674 functions, 34% with a direct test).

---

### [T-1] Fuzz targets exist but nothing ever runs them
Severity: high
File: C:\dev\waml\Cargo.toml:4; C:\dev\waml\fuzz\fuzz_targets\
Evidence: `exclude = ["fuzz"]` in the workspace manifest; `.github/workflows/ci.yml` contains no `cargo fuzz` step. Four targets (`parse_write`, `syntax_edits`, `outer_mapping`, `uml_islands`) plus seed corpora sit in `fuzz/`.
Why it's wrong: The riskiest code in the repo (incremental reparse, the pulldown seam — four hardening commits in the last week alone) has purpose-built fuzz targets that are dead weight: no CI job, no scheduled run, no documented cadence. The class of bug they exist for (proptest already caught one shipped reparse bug) is exactly what regresses silently.
Suggested fix: Add a scheduled (cron) CI job that runs each target for a bounded time (`cargo fuzz run <t> -- -max_total_time=300`) against the checked-in seeds, or at minimum a `cargo check`-level build of the fuzz workspace in ci.yml so the targets can't bit-rot.
Confidence: CONFIRMED

### [T-2] PR CI runs 1 of 5 web-artifact script test files
Severity: high
File: C:\dev\waml\.github\workflows\ci.yml:90
Evidence: `run: node --test scripts/inject-runtime-shell.test.mjs` — while `scripts/` contains five `.test.mjs` files (`package-web-artifact`, `prune-web-fonts`, `export-site-browser`, `verify-web-artifact`, `inject-runtime-shell`). Only pages.yml runs `node --test "scripts/*.test.mjs"`, i.e. at deploy time, after merge.
Why it's wrong: Regressions in the verify/package/prune/export-site scripts pass PR review green and only fail when the Pages deploy runs — and MEMORY records exactly this failure mode already (`verify-web-artifact.mjs` exists because cargo-makepad once shipped a dead deploy while exiting zero). Four-fifths of the script suite is not a gate.
Suggested fix: Change the ci.yml step to `node --test "scripts/*.test.mjs"` (the tests are pure node:test, no wasm build needed — pages.yml already runs them standalone).
Confidence: CONFIRMED

### [T-3] High-complexity behaviour trapped in widget draw/handle paths with no possible test
Severity: high
File: C:\dev\waml\crates\waml-editor\src\inspector_panel.rs:583; crates\waml-editor\src\canvas\class\widget.rs:1213
Evidence: `fn draw_walk(&mut self, cx: &mut Cx2d, ...)` in inspector_panel is cyclomatic-complexity 55 (tokensave), mixing state decisions into the draw: `let collapsed = self.proj.is_none(); ... let show_body = self.show_picker && !collapsed;` plus turtle-balancing subtleties the comment itself calls out ("leaves the turtle begun-but-never-ended... every later draw then silently aborts"). `apply_interaction_effects` (complexity 10, fan-in 13) similarly lives inside the widget. tests/README.md:73-115 documents that a headless render check is structurally impossible (fork's headless backend doesn't compile on Windows; widgets are bin-private).
Why it's wrong: The turtle-balance bug quoted above already shipped once (MEMORY: dock chrome regression). Visibility/collapse/show_body decisions are pure functions of widget state and could be unit-tested headlessly, but as written every one of them can only be verified by the interactive run — the gate asserts nothing about them.
Suggested fix: Extract the per-draw decisions (collapsed/show_body/margin/slot-width policy, interaction-effect resolution) into pure `#[cfg(test)]`-reachable functions taking plain state, leaving `draw_walk` as a thin applier — the pattern editor_session/scene/tree already follow.
Confidence: CONFIRMED

### [T-4] Editor is integration-test-proof by construction: lib.rs exports 2 modules
Severity: medium
File: C:\dev\waml\crates\waml-editor\src\lib.rs:1-2
Evidence: The entire file is `pub mod editor_history;` / `pub mod view_history;`. Everything else (~80 modules) is private to `main.rs`'s tree; `tests/` holds only 5 files, all constrained to those two modules plus filesystem-level checks.
Why it's wrong: A new contributor fixing a bug in, say, `doc_tabs.rs` or `scene.rs` cannot write a test without editing the module under fix to add an in-module `#[cfg(test)]` block — and cannot compose behaviour across modules (session + scene + tabs) at all. The 1000+ in-module tests mitigate but do not remove this: cross-module editor behaviour has no test seam.
Suggested fix: Promote the headless module set (editor_session, scene, documents, load, config, tree) into lib.rs (or a `waml-editor-core` crate), keeping only makepad-touching widgets bin-private.
Confidence: CONFIRMED

### [T-5] tests/README.md — the documented test strategy — is stale in two load-bearing ways
Severity: medium
File: C:\dev\waml\crates\waml-editor\tests\README.md:3-5, 227-234
Evidence: Line 3: "`waml-editor` is a **binary-only** crate (no `lib.rs`) ... There is no `--lib` target." — but `src/lib.rs` exists and `tests/` has 5 integration files. Lines 227-234 document a "pre-existing `waml-syntax` incremental property defect" (`randomized_full_and_incremental_snapshots_agree`) and bless a `--skip`-filtered workspace run; that defect was fixed in commit `10f66dc9` ("reject block windows over trailing end-of-file whitespace"), an ancestor of HEAD.
Why it's wrong: The strategy document tells contributors the workspace gate is expected-red and shows them how to skip two property tests — the exact tests that catch the crate's hardest bug class. A contributor following it will skip green tests, or worse, treat a *new* red as the documented one.
Suggested fix: Delete the known-red paragraph and the skip incantation; fix the "no lib.rs" claim to describe the two exported modules and the 5 integration tests.
Confidence: CONFIRMED

### [T-6] Verification-of-record mutates committed shared fixtures
Severity: medium
File: C:\dev\waml\crates\waml-editor\tests\fixtures\mini\ (orders-diagram.md); tests\README.md:20-31
Evidence: The README designates interactive runs against `tests/fixtures/mini` as "the **verification of record**", and the editor's drag/place flow writes layout back to the loaded bundle. Two plans carry standing warnings about the resulting dirt: "Preserve ... the existing modification to `crates/waml-editor/tests/fixtures/mini/orders-diagram.md`" (docs/superpowers/plans/2026-07-27-first-class-okf-documents.md:34) and again in 2026-08-02-waml-sequence-language-completeness.md:24. MEMORY: the fixture is "often left dirty".
Why it's wrong: A shared fixture that verification routinely modifies means every reviewer starts from an unclean baseline, `git status` noise masks real changes, and plan baselines have already been drafted against the dirty state (a recorded incident). Fixture hygiene requires the test run to leave shared fixtures untouched.
Suggested fix: Have run.ps1 (and the documented workflows) stage the fixture to a temp/target directory before launch — the markdown-integration flow in the same README already does exactly this (`target/task9-native-fixture`); make it the universal rule.
Confidence: CONFIRMED

### [T-7] `waml serve`: test-friendly seam built, nothing behind it
Severity: medium
File: C:\dev\waml\crates\waml-cli\src\serve\mod.rs:8-31
Evidence: "Everything `run` needs, decoupled from clap so tests can build one." then `pub fn run(args: ServeArgs) -> i32 { eprintln!("waml serve: not implemented yet ..."); 2 }`. axum/rand/subtle are already dependencies.
Why it's wrong: The injectable-args seam exists only to justify parse tests of a command that always fails; meanwhile three server deps sit in the build untested and unexercised. The struct's `#[allow(dead_code)]` fields are frozen against tests that don't exist yet — shape locked before behaviour, which is the inverted order.
Suggested fix: Either land the server with its e2e test (the lsp_e2e.rs pattern is right there) or gate the command and deps behind a feature until then.
Confidence: CONFIRMED

### [T-8] vscode extension.ts is entirely untested; only serverPath.ts is covered
Severity: medium
File: C:\dev\waml\editors\vscode\src\extension.ts
Evidence: `editors/vscode/src/` contains exactly `extension.ts`, `serverPath.ts`, `serverPath.test.ts`. The vitest suite (14 tests) exercises server-path resolution only; activation, LanguageClient construction, spawn-failure handling have zero coverage.
Why it's wrong: The extension is one of two shipping frontends of the LSP; its untested half is precisely the effectful part (process spawn, activation events) where a regression means a dead extension with a green CI. At minimum the client options / server options assembly is pure and extractable for test.
Suggested fix: Extract the LanguageClient option-building into a pure function with a vitest suite; mock `vscode` for an activation smoke test (standard `@vscode/test-electron` or module-mock approach).
Confidence: CONFIRMED

### [T-9] editor_session.rs is a 3,417-line god object — headless but only testable whole
Severity: low
File: C:\dev\waml\crates\waml-editor\src\editor_session.rs:29-40
Evidence: `pub struct EditorSession { current: Arc<EditorSessionSnapshot>, history: EditorHistory }` where the snapshot bundles revision, two SourceBundles, markdown snapshots, and semantic-analysis state; imports are pure waml/waml-syntax (no makepad) — good — but its 44 in-module tests each construct a full session and drive it end-to-end (e.g. `ordered_mixed_batch_commits_once_with_one_final_projection`, :2839).
Why it's wrong: The pure/effectful separation is done right; the granularity is not. Every new session behaviour costs a whole-session fixture, and failures localise poorly. This is friction, not absence, of testability.
Suggested fix: Split the batching/projection and semantic-completion install logic into their own modules with narrow inputs; keep session tests for the composition.
Confidence: CONFIRMED

### [T-10] Direct unit coverage of core parser internals is thin; safety net is integration-shaped
Severity: low
File: C:\dev\waml\crates\waml\src\uml\syntax\parser.rs; crates\waml\src\uml\analysis.rs
Evidence: tokensave test-risk: workspace direct-test coverage 34% (1267/3674 functions); the top-risk untested list is dominated by uml parser/analysis internals (`token` fan-in 122, `validate_declared_semantics` complexity 68, `declared_projection` complexity 67 — all `has_test: false`) and `markdown/inline.rs parse_inlines` (complexity 62).
Why it's wrong: These are covered indirectly (uml_*_syntax.rs ×5, conformance suites, proptests), so this is not a gap in behaviour coverage — but a failure inside `validate_declared_semantics` localises to a fixture diff, not a unit assertion. For 3.5k–4.7k-line files that raises debugging cost, not shipping risk.
Suggested fix: No mass unit-test campaign; when next touching these functions, add targeted unit tests for the branch being changed (charter: "would a fixture have caught this class of bug" — here mostly yes).
Confidence: PLAUSIBLE (indirect-coverage extent inferred from suite names, not a coverage run)

---

## Not findings

- `config.rs` disk seam is genuinely injectable: `load_from`/`store_to` take a `&Path`, tests run on temp dirs, `~/.waml` is resolved only in thin wrappers (config.rs:35-76); MRU logic is pure over `Vec<Recent>`.
- `share.rs` malformed-input handling is properly tested: bad base64, corrupt deflate, truncated payload, plain fragments all asserted non-panicking (share.rs:258-278).
- `browser_boot::select_browser_boot` is a pure `(search, hash) -> Result` function with host-side tests for every precedence branch (browser_boot.rs:40, :171-199); only the actual fetch is wasm-only.
- Golden tests are human-reviewable, not opaque: `golden.rs` fingerprints diagnostics and declared fields into sorted labelled strings (`"{label}:incomplete({expected:?})@{range}"`), and asserts lossless shell round-trips against the fixture text itself — nothing to blindly re-bless.
- Headless/frontend split holds at the manifest level: `waml`/`waml-syntax` have no makepad dependency; model, parse, and solve all test without window/GPU/filesystem.
- Property tests exist where the charter demands them: `waml-syntax/tests/properties.rs`, `src/incremental/properties.rs`, `waml/tests/parser_platform_properties.rs`, `bundle_envelope_properties.rs` — and the incremental proptest has already caught a shipped bug.
- `lsp_e2e.rs` waits are deadline-based (`Instant::now() + 20s`, `recv_timeout`), not sleep-polling; no order dependence found in the e2e suites (cli_e2e uses a SystemTime-unique temp dir per test).
- The GUI-limit rule is honoured in writing: tests/README.md states explicitly that interactive runs are the verification of record and documents (with build evidence) why a headless render check is impossible on this fork/platform.
- CI matrix deliberately includes Windows because the parser is path/line-based (ci.yml:22-24) — the right reliability call for this codebase.
- `waml-markdown-editor` is the best-tested frontend code: 14 suites, fixtures and goldens directories, widget-parity and unicode/IME tests.
- Scan-seam and public-surface guard tests (`tests/scan_seam.rs`, `tests/public_surface.rs`) pin the two contracts most likely to drift silently.
