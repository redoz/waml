# `waml serve` Implementation Plan

> **For agentic workers:** implement task-by-task; every task must land independently green under the full gate (`cargo test --workspace` plus the editors/vscode test/lint/build).

**Source spec:** `docs/superpowers/specs/2026-07-25-waml-serve-design.md` (rewritten 2026-08-04). The spec settles the decisions; this plan only sequences them. This plan replaces an earlier one written against the deleted `waml::ops` / `waml-wasm` architecture.

**Goal:** turn the `serve::run` stub (`crates/waml-cli/src/serve/mod.rs`, currently prints "not implemented" and exits 2) into the loopback HTTP server the spec describes, and make the editor's `BrowserBootSource::Api` boot and save through it.

## Already landed — do not redo

Groundwork from the earlier plan is on main and must not be re-planned:

- Workspace deps: `axum 0.8` (http1/json/tokio/query), `brotli 8`, `rand 0.9`, `subtle 2.6`, dev `reqwest 0.13` + `tempfile 3` are in the root `Cargo.toml` `[workspace.dependencies]` and wired into `crates/waml-cli/Cargo.toml` (including `waml = { features = ["serde"] }` and `waml-ops-dto`).
- `Command::Serve` variant, dispatch arm, and parse tests (`parses_serve_defaults`, `parses_serve_flags`) in `crates/waml-cli/src/main.rs`.
- The `ServeArgs` struct and stub `run` in `crates/waml-cli/src/serve/mod.rs`.
- Embedded web artifact: `crates/waml-cli/src/web_artifact.rs` (`EmbeddedAsset`, `embed-web` feature, `WebArtifactError::NotEmbedded`), shared assembler `crates/waml-cli/src/site.rs` with the `SiteSource::Api` arm (currently `#[allow(dead_code)]`).
- Editor boot selection: `crates/waml-editor/src/browser_boot.rs` already yields `BrowserBootSource::Api { base, token }` and ships `decode_boot_bundle`; `app.rs` currently dead-ends the `Api` arm at the start screen (the wasm `handle_startup` match).

## Architecture in one paragraph

New submodules under `crates/waml-cli/src/serve/` (mirroring how `crate::lsp` owns transport only): `paths` (confinement), `guard` (token + request checks), `state` (bundle, revision, the two write surfaces feeding one `prepare_candidate` funnel), `ui` (embedded-artifact serving), `routes` (axum router + status mapping), with `mod.rs` binding the socket, minting the token, printing the URL, and running tokio. All semantic work delegates to `waml` proper: writes go through the exact machinery `run_batch` in `main.rs` uses today (`to_batch` → `Batch::lower` under `EditContext` → `prepare_candidate` revalidation → `io::write_back`), and the documents route reuses the baseline-guarded shape of `native_save::save_bundle_atomic`. HTTP framework is **axum**: `waml-cli` already runs tokio (via `tower-lsp`), so axum adds a router to an existing async stack rather than a second one — this was decided and the dependency is already landed; hand-rolling on hyper or adding a second (sync) server stack like tiny_http was rejected for that reason.

## Conventions and constraints

- The gate runs clippy with `-D warnings`; `dead_code` is a hard error. A module landed before its consumer carries `#[allow(dead_code)]` with a comment naming the task that removes it — exactly the staging the current serve stub uses for `ServeArgs`.
- Integration tests that need the server in-process live in `#[cfg(test)]` modules inside the binary crate (they run under `cargo test -p waml-cli`); tests that need the real process live in `crates/waml-cli/tests/serve_e2e.rs` and spawn `env!("CARGO_BIN_EXE_waml")`, following `crates/waml-cli/tests/cli_e2e.rs` conventions (its collision-proof `tmp()` helper included). Do **not** split `waml-cli` into a lib + bin.
- `cargo test --workspace` must stay green without a wasm toolchain and without an embedded artifact (a default build has `EMBEDDED` empty).
- Token comparison uses `subtle::ConstantTimeEq`; token bytes come from the OS CSPRNG via `rand`.
- Editor-side logic that can be pure lives in a headless-testable module; only the thin wasm wiring is untestable on the host and gets browser verification via a non-gate script (like `scripts/measure-web-boot.mjs`).
- Browser tests use `playwright-core` driving the ms-playwright `chromium-1228` build, `chrome-win64` (not `chrome-win`).
- Out-of-scope list in the spec is binding: no websockets, no file watcher in `serve`, no TLS, no multi-client merge, no TS typings.

---

### Task 1: Bundle-path confinement (`serve/paths.rs`)

Pure functions, no I/O beyond symlink resolution, no server. Implements spec **Security §5**.

**Files:** create `crates/waml-cli/src/serve/paths.rs`; add `pub mod paths;` (with `#[allow(dead_code)]` staging note: consumed by Tasks 4–6) to `crates/waml-cli/src/serve/mod.rs`.

**Build:**
- `is_safe_rel(rel: &str) -> Result<(), String>` — syntactic check: reject empty, NUL, `.`/`..` segments, absolute paths, drive prefixes (`C:`), UNC (`\\`), and Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`, with or without extension), on every platform. Note: `site::is_safe_relative_path` stays as-is for the embedded manifest; this is the stricter live-filesystem sibling. Do not merge them — they check different inputs (our own build manifest vs wire input).
- `safe_join(root: &Path, rel: &str) -> Result<PathBuf, String>` — syntactic check first, then join and canonicalize (nearest existing ancestor for not-yet-created files, the same trick `native_save::resolved_target` uses) and require the result to stay under the canonicalized root.

**Tests (in-file `#[cfg(test)]`):** `accepts_ordinary_relative_paths`, `rejects_traversal_and_absolutes` (table: `../x`, `a/../../x`, `/etc/passwd`, `C:\x`, `\\server\share\x`, empty, `.`), `rejects_nul_and_windows_device_names` (incl. `con.md`, `COM1`, and the negative `console.md`), `safe_join_stays_under_root` (tempdir), `safe_join_rejects_symlink_escape` (create a dir symlink out of root; skip gracefully when symlink creation is denied, copying `native_save::tests::make_dir_link`).

### Task 2: Token and request guards (`serve/guard.rs`)

Pure functions. Implements spec **Security §1–4, §6**.

**Files:** create `crates/waml-cli/src/serve/guard.rs`; declare in `serve/mod.rs` (same staging note: consumed by Task 6).

**Build:**
- `Token`: `generate()` (32 CSPRNG bytes, URL-safe base64 without padding — a hand-rolled ~12-line encoder, no new dependency), `from_raw`, `as_str`, `matches(&self, presented) -> bool` using `ConstantTimeEq` (length compared in the clear; the token length is public).
- `Guard { token, origin, port, bind_all }`, `ReqFacts<'a> { bearer, query_token, origin, host, client_header, mutating }`, `Deny { Unauthorized, Forbidden(String) }`, and `check(&Guard, &ReqFacts) -> Result<(), Deny>`: token first (Bearer or `?token=` — the query form exists because the wasm editor has no JS-to-wasm header channel), exact-match Origin allowlist when an `Origin` header is present (never `*`, no PNA header), Host must be a loopback name on the bound port (any name on the port under `--bind-all` — anti-rebinding), `X-Waml-Client: 1` required when `mutating`.

**Tests:** `a_generated_token_is_long_and_unique`, `accepts_the_token_in_either_position`, `rejects_a_missing_or_wrong_token` (incl. prefix and one-char-longer strings), `rejects_a_foreign_origin`, `rejects_a_rebound_host` (foreign name, wrong port; accepts `[::1]:<port>`), `mutating_requests_need_the_client_header`, `bind_all_relaxes_only_the_host_check`.

### Task 3: Serve state — load, revision, read projections (`serve/state.rs`)

The semantic core's read half, no HTTP. Implements the spec's **read table** (`/api/bundle`, `/api/model`, `/api/diagnostics`) and the **revision counter** decision, plus the Contract test tier for the read surface.

**Files:** create `crates/waml-cli/src/serve/state.rs`; declare in `serve/mod.rs` (staged `#[allow(dead_code)]`, consumed by Task 6).

**Build:**
- `ServeState { root: PathBuf, prepared: <what prepare_candidate returns>, revision: u64 }`. `load(root)` reads via `io::read_bundle_rooted` (bundle-relative paths, exactly as `run_batch` does), then `prepare_candidate(source, None, 0)` once — the state holds the prepared analysis so reads don't re-analyze per request and writes have `EditContext`/`PreviousAnalyses` on hand. (If the prepared type turns out not to be storable owned, hold the `SourceBundle` and re-prepare per request/apply the way `run_batch` does — decide from the real signature, do not fight it.)
- `revision()`, `bundle_envelope() -> Result<String, String>` (`waml::bundle_envelope::encode_bundle_envelope` over `prepared.source().to_pairs()` — the `run_batch --stdout` codec), `model_json()` (serde-serialize the `uml::Projection` at `prepared.uml().projection`; the `waml/serde` feature is already enabled from waml-cli), `diagnostics()` (the same validation pass `waml check` renders — reuse `commands::diagnostics` shapes rather than re-deriving).

**Tests:** `load_reads_the_directory_and_starts_at_revision_zero`; `the_model_projection_equals_preparing_the_bundle_directly` (contract: JSON of `model_json()` equals serde of the projection `prepare_candidate` yields for the same pairs); `the_envelope_round_trips_to_the_same_bundle` (encode, then `waml::bundle_envelope::split_bundle` + `SourceBundle::try_from_pairs` reproduce the pairs — this is the codec the editor's `decode_boot_bundle` wraps, so it is the `/api/bundle` contract test on the CLI side); `a_directory_that_fails_to_prepare_is_an_error_not_a_panic` (hostile fixture).

A fixture helper (tempdir with a `uml.Package` index + a `uml.Class` doc, shape borrowed from `tests/cli_e2e.rs`) goes in this file and is reused by Tasks 4–6.

### Task 4: Serve state — `apply_ops`, the semantic write surface

Implements the spec's **`POST /api/ops`** semantics (one validation funnel, all-or-nothing, the `EditError` 422 shape) minus HTTP.

**Files:** modify `crates/waml-cli/src/serve/state.rs`.

**Build:**
- `pub enum ApplyFailure { Stale { current: u64 }, Edit(waml::edit::EditError), Invalid(<validation diagnostics>), Io(String) }`.
- `apply_ops(&mut self, at: u64, dtos: &[OpDto]) -> Result<Vec<(String, String)>, ApplyFailure>`: revision check → `waml_ops_dto::to_batch` (its error string carries the op index; map into `Edit`) → `batch.lower(EditContext { source, okf_analysis, session_revision, uml })` exactly as `run_batch` does → `prepare_candidate(changed, Some(PreviousAnalyses { okf, uml }), rev+1)`; a candidate that fails revalidation is `Invalid` and **nothing is written**. On success: confine every result path with `paths::safe_join` before writing, `io::write_back(&root, &old_pairs, &new_pairs)`, swap in the new prepared analysis, bump `revision`, and return only the `(path, markdown)` pairs whose text changed — the spec's `changed` response body ("returning the whole bundle per save is wasteful").

**Tests:** `apply_bumps_the_revision_and_returns_only_changed_files` (an `attr.add` in the same `OpDto` JSON `waml apply` accepts; the untouched sibling file's bytes on disk are identical after); `an_unformatted_op_result_lands_canonically_formatted` (the funnel claim made testable: the written file equals its canonical form); `a_failing_op_writes_nothing_and_names_its_index` (bad op second in a two-op batch: disk untouched, revision unbumped, index `1` in the error); `a_stale_revision_is_rejected_without_writing`.

### Task 5: Serve state — `apply_documents`, the baseline-guarded write surface

Implements the spec's **`POST /api/documents`** semantics: `{ revision, writes: [{ path, baseline, desired }] }`, all-or-nothing, candidate revalidation before anything lands — "nothing invalid ever lands on disk".

**Files:** modify `crates/waml-cli/src/serve/state.rs`.

**Build:**
- `pub struct DocumentWrite { pub path: String, pub baseline: Option<String>, pub desired: String }` (serde; `baseline: None` means a new file — mirroring `native_save`'s planned-write shape).
- `apply_documents(&mut self, at: u64, writes: &[DocumentWrite]) -> Result<(), ApplyFailure>`: revision check → confine each `path` (`paths::safe_join`) → verify each write's `baseline` against the in-memory document (mismatch → `Stale { current }`; the server's bundle is the authority, the same optimistic shape as `SaveTicket`) → build candidate pairs → `SourceBundle::try_from_pairs` → `prepare_candidate` with `PreviousAnalyses`; failure is `Invalid` with the validation diagnostics and **no write**. Success: `io::write_back`, swap analysis, bump revision.

**Tests:** `a_document_write_lands_byte_exactly` (the editor writes canonical text itself; the server must not reformat it); `a_write_whose_candidate_fails_validation_writes_nothing` (desired text that breaks a cross-document reference: `Invalid`, disk and revision untouched — the documents half of the "one validation funnel" obligation); `a_baseline_mismatch_is_a_conflict`; `a_new_file_write_with_no_baseline_is_accepted`; `a_confined_path_is_enforced_on_documents_too` (`../x.md` refused before any baseline logic runs).

### Task 6: API routes and the running server (`serve/routes.rs` + real `run`)

First socket. Implements the spec's **API table**, **status codes**, and the **Command surface** lifecycle (bind, token mint, printed URL, browser launch, Ctrl-C, exit codes). UI routes come in Task 7; until then every non-`/api` path is 404, which is also the final `--api-only` behaviour, so nothing here is throwaway.

**Files:** create `crates/waml-cli/src/serve/routes.rs`; rewrite `run` in `crates/waml-cli/src/serve/mod.rs`; create `crates/waml-cli/tests/serve_e2e.rs`; drop the `#[allow(dead_code)]` staging from `paths`/`guard`/`state` and from `ServeArgs`.

**Build:**
- `App { state: Arc<Mutex<ServeState>>, guard: Arc<Guard> }` and `router(app) -> axum::Router`: `GET /api/bundle` (envelope body, revision in `X-Waml-Revision`), `GET /api/model` and `GET /api/diagnostics` (`{ revision, ... }` JSON), `POST /api/ops` (`{ revision, ops: OpDto[] }` → `{ revision, changed: [path, markdown][] }`), `POST /api/documents` (`{ revision, writes }` → `{ revision }`). One guard layer extracts `ReqFacts` from headers + query before any body work; `Deny::Unauthorized` → 401, `Forbidden` → 403. `ApplyFailure` mapping: `Stale` → 409 with `{ current }`; `Edit` → 422 with `{ index, op, selector, reason }`; `Invalid` → 422 with the diagnostics; `Io` → 500 naming the path and OS error. `std::sync::Mutex`, never held across an `.await` (handlers do the CPU work synchronously under the lock; one editor at a time is the spec's model).
- `serve_on(listener: tokio::net::TcpListener, app: App)` so tests can inject an ephemeral listener.
- `run(args)`: load state (exit 2 with a named error on failure), `Token::generate()`, bind `127.0.0.1`/`0.0.0.0` (`--port 0` allowed; print the actual port), warn on stderr under `--bind-all` naming the exposure, print exactly one line — `waml serve  http://127.0.0.1:<port>/?api=/api&token=<token>   (serving <dir>)` — launch the browser unless `--no-open` (Windows `cmd /C start`, else `open`/`xdg-open`; a failed launch warns, it does not exit), run tokio until Ctrl-C, exit 0.

**Tests:** in-process `#[cfg(test)]` module in `routes.rs` (`#[tokio::test]` + `reqwest` + the Task 3 fixture, `serve_on` on port 0 per test): `reads_require_a_token` (401), `every_read_carries_the_revision` (header on bundle, field on model/diagnostics), `an_op_post_mutates_the_disk_and_answers_changed_files` (full round trip with token + `X-Waml-Client`: response body and bytes on disk), `a_documents_post_mutates_the_disk`, `a_rejected_batch_is_422_with_the_edit_error_shape`, `an_invalid_documents_candidate_is_422_with_diagnostics`, `a_stale_revision_is_409`, `a_mutating_post_without_the_client_header_is_403`, `a_foreign_origin_is_403`, `an_unknown_api_path_is_404`. Plus `tests/serve_e2e.rs`: `serve_prints_a_bootable_url_and_serves_the_directory` — spawn the binary with `serve <tmp> --port 0 --no-open --api-only`, parse the printed URL/token from stdout, GET `/api/model` with that token, assert 200 and the fixture's node, kill the child. This proves the printed token is the accepted token (spec **Boot and token delivery**).

### Task 7: UI routes from the embedded artifact

Implements the spec's **"The web artifact is embedded"** section: same-origin UI, `Content-Encoding: br` straight from stored bytes, 406 without `br`, 404 with no shell fallback, api-only degradation.

**Files:** modify `crates/waml-cli/src/serve/routes.rs` and `serve/mod.rs`; create `crates/waml-cli/src/serve/ui.rs`; modify `crates/waml-cli/src/site.rs` (remove the `#[allow(dead_code)]` on `SiteSource` once `serve` constructs `Api`).

**Build:**
- At startup (once, not per request): validate the embedded manifest with the same rules `assemble_site` applies (safe paths, no duplicates, `index.html` present — call into/alongside the shared assembler so the checks cannot drift) and build a `path -> (&'static [u8] /* br */, content_type)` table plus the generated `waml-boot.txt` (`?api=/api`, tokenless — the assembler's `SiteSource::Api` string). Embedded assets are served with `Content-Encoding: br` from the stored slices, never decompressed; the tiny generated boot file is served identity-encoded. Content types by extension (html/js/wasm/css/txt/ico/png).
- Router: `/` → `index.html`; a request whose `Accept-Encoding` lacks `br` gets 406 on br-served assets; unknown paths are 404 — never the shell (a typo must not become a silent blank page). UI routes do **not** require the token (they are static same-origin bytes; only `/api` is access-controlled — the token in the printed URL is for the editor to *use*, per the spec's Boot section).
- `run`: `--api-only`, or `web_artifact::embedded_artifact()` answering `NotEmbedded`, skips UI mounting with a one-line stderr warning quoting the `NotEmbedded` message (which already names the build flags); the API still runs.

**Tests:** in-process, constructing synthetic `EmbeddedAsset`s exactly as `site.rs`'s tests do (brotli-compressed fixtures, `Box::leak`): `the_shell_is_served_brotli_encoded_from_stored_bytes` (assert `Content-Encoding: br` and the raw body equals the stored compressed slice — no recompression), `a_client_without_brotli_gets_406`, `an_unknown_asset_is_404_not_the_shell`, `the_boot_config_names_the_api_and_carries_no_token`, `ui_routes_do_not_require_the_token`, `api_only_serves_no_ui`.

### Task 8: Editor boot from `?api=` (`GET /api/bundle`)

Implements spec **Editor-side changes: Boot**. The `Api` arm stops falling through to the start screen.

**Files:** modify `crates/waml-editor/src/app.rs` (the wasm `handle_startup` match, `handle_http_response`, `handle_http_request_error`), `crates/waml-editor/src/browser_boot.rs` (pure helpers), `crates/waml-editor/src/app/workspace.rs` (retain the chosen API backend).

**Build:**
- Pure, host-tested helpers in `browser_boot.rs`: `api_bundle_request(base: &str, token: Option<&str>) -> (String, Vec<(String, String)>)` building the `{base}/bundle` URL plus the `Authorization: Bearer` header when a token is present (URL join must handle `/api` vs `/api/` and absolute bases), and `api_boot_error(base: &str, status: Option<u16>) -> String` — a 401 must say the URL is missing or carries a stale token (the tokenless `waml-boot.txt` fallback path lands exactly here: a named failure, not a blank page).
- `handle_startup` `Api` arm: show the start screen (the fetch is async, same pattern as `?bundle=`), issue `cx.http_request(live_id!(boot_api), ...)` with the header set, retain `{ base, token }` pending.
- `handle_http_response` `boot_api` arm: on 2xx, `decode_boot_bundle` (byte-for-byte the `?bundle=` decoder — the reuse the spec's read table is built on), capture the revision from the `X-Waml-Revision` response header, `open_bundle` + `show_editor`, and commit the API backend (base, token, revision) into workspace state; on failure, `log!` the named error and stay on the start screen. Same `editor_shown` guard as the existing bundle fetch. `handle_http_request_error` names the base URL like the bundle path does.

**Tests:** host unit tests in `browser_boot.rs`: `api_bundle_request_joins_base_and_sets_the_bearer_header` (relative `/api`, absolute base, token and tokenless), `a_401_boot_failure_names_the_token`. The wasm wiring itself is exercised by Task 10's browser script. Gate stays green: new wasm-only code sits behind the existing `cfg(target_arch = "wasm32")` blocks with the `#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]` pattern the file already uses.

### Task 9: Editor save through `POST /api/documents`

Implements spec **Editor-side changes: Save** — the API arm as transport, not platform, and the 409 → reload path.

**Files:** create `crates/waml-editor/src/api_save.rs` (headless, host-tested); modify `crates/waml-editor/src/app/workspace.rs`, `crates/waml-editor/src/app.rs`, and the module mount in `crates/waml-editor/src/lib.rs`.

**Build:**
- `api_save.rs`, pure: `documents_request(ticket: &SaveTicket, revision: u64) -> Result<String /* JSON body */, String>` — derive `writes: [{ path, baseline, desired }]` by diffing `ticket.snapshot.persisted_source` against `ticket.snapshot.source` (the same planned-write computation `native_save::save_bundle_atomic` performs: skip documents whose baseline equals desired; `baseline: null` for new paths), serialized to the Task 5 wire shape. Plus `parse_save_response` / `parse_conflict` for the 200/409/422 bodies.
- `workspace.rs`: the save seam becomes a choice made **once at open time** — native dir, browser fragment, or API backend — held as state (e.g. an enum the wasm `save_backend` consults), so the backend difference stays behind the seam that already exists. The API arm cannot complete synchronously (`cx.http_request` answers via `handle_http_response`), so it stashes the in-flight `SaveTicket` (one at a time; a save requested while one is in flight goes back through the existing debounce), POSTs `/api/documents` with `Authorization`, `X-Waml-Client: 1`, `Content-Type: application/json`, and returns. On response: build `SaveCompletion { revision: ticket.revision, history_state: ticket.history_state, result }` and drive the existing `session.finish_save` + `save_feedback.finish_save` + `sync_save_error` path — `mark_dirty`, the debounce, `SaveFeedback`, and the quit-flush protocol are untouched above the seam, as the spec requires.
- 409 handling: issue `GET /api/bundle`, route each changed document through the existing `replace_external_document` ingress (dropping its `#[allow(dead_code)]`), update the held revision, and surface a visible "reloaded from disk" notice through the statusbar (the `sync_save_error` channel) — never a silent clobber. Every successful save response updates the held revision from the response body.

**Tests:** host tests in `api_save.rs`: `a_dirty_document_becomes_a_baseline_guarded_write` (build a real dirty session via `EditorSession` the way `native_save::tests::invalid_source_ticket_is_saved_byte_exactly` does; the JSON carries baseline+desired for the dirty doc and omits the clean one), `a_new_document_has_no_baseline`, `a_clean_ticket_produces_no_writes`, `conflict_and_error_responses_parse`. Plus one CLI-side pinning test added to `serve/state.rs`: `the_editor_wire_shape_round_trips` — deserialize a `documents_request`-shaped JSON literal into `DocumentWrite`, pinning the two crates (which share no dependency edge) to one wire spelling.

### Task 10: Browser verification script (non-gate)

Implements the spec's **Browser** test tier. A script, not a gate step, following `scripts/measure-web-boot.mjs` / `scripts/verify-web-artifact.mjs` conventions.

**Files:** create `scripts/serve-browser-check.mjs`.

**Build:** node script that (1) takes the path to a `--features embed-web` binary (refusing with a named explanation when the binary has no artifact, like the existing guard scripts); (2) spawns `waml serve <fixture> --port 0 --no-open` and parses the printed URL + token; (3) drives `playwright-core` chromium-1228 `chrome-win64`: load the printed `?api=&token=` URL, assert the wasm boots without a console panic and the served directory's model is open; (4) from a foreign origin (an `about:blank`/data-URL page) fetch the API and assert the browser blocks it — the regression guard on the finding that killed the hosted-UI option; (5) exercise a save round trip (POST `/api/documents` with the page's credentials or a driven edit) and assert the file changed on disk and a reload shows it; (6) exit non-zero with a named verdict, screenshot on panic. Not wired into `cargo test` or the vscode gate.

**Tests:** the script is the verification; its own smoke check is running it against a locally packaged artifact and recording the verdict in the task's commit message.

### Task 11: Docs and status

**Files:** modify `docs/waml/goals/share-and-publish/serve-locally.md`; update the status line in `docs/superpowers/specs/2026-07-25-waml-serve-design.md`; update the `Serve` doc comment in `crates/waml-cli/src/main.rs` only if wording drifted.

**Build:** flip `serve-locally.md` from `Status: planned — unverified` to implemented wording, record the actual surface (`--port/--bind-all/--api-only/--no-open`, the printed token URL, the two write routes), and soften "loopback interface only" to "loopback by default; `--bind-all` is opt-in and warns" so the doc matches the shipped flags and the spec's Security §6. Update the spec header from "groundwork landed, server pending". No new documentation files.

**Tests:** none (docs); the full gate must still pass untouched.

---

## Task ordering rationale

Tasks 1–5 are pure/headless and land the whole semantic core with `#[allow(dead_code)]` staging (tested but unconsumed — the gate's clippy `-D warnings` makes unstaged dead code a hard error); 6 consumes them and un-stages; 7 completes the server; 8–9 are editor-side and compile independently of 6–7 (they talk HTTP at runtime, not at build time); 10–11 close verification and docs. Every intermediate state compiles and passes the full gate.

## Spec coverage

| Spec section | Task |
| --- | --- |
| One validation funnel, two write surfaces | 4, 5 |
| Read table (`/api/bundle`, `/api/model`, `/api/diagnostics`) + contract tests | 3, 6 |
| Revision counter, 409, all-or-nothing 422 shapes | 4, 5, 6 |
| Embedded artifact, br from stored bytes, 406, 404-no-fallback, api-only degrade | 7 |
| Boot and token delivery (one printed URL, tokenless boot file → named 401) | 6, 7, 8 |
| Same-origin only / cross-origin regression guard | 7, 10 |
| Security §1 token / §2 client header / §3 Origin / §4 Host / §6 bind | 2, 6 |
| Security §5 path confinement (incl. device names, symlinks) | 1, 4, 5 |
| Editor boot via `decode_boot_bundle` | 8 |
| Editor save via `SaveTicket` → baseline-guarded writes, 409 → `replace_external_document` | 9 |
| Testing tiers: unit / integration / contract / browser | 1–2 / 6–7 / 3, 9 / 10 |
| Out of scope | not implemented, by construction |

## Risks / open questions (recorded, not blocking)

- **Response headers on wasm:** the editor reads `X-Waml-Revision` from the boot response; makepad's `HttpResponse` carries headers (`from_header_string`/`set_header` in the fork's `platform/network/src/types.rs`), but whether the wasm backend surfaces *response* headers must be verified early in Task 8. Fallback if it does not: take the initial revision from a follow-up `GET /api/model` (its JSON body carries `revision`); all later revisions arrive in POST response bodies either way.
- **Storability of the prepared analysis:** `ServeState` wants to hold what `prepare_candidate` returns; if it borrows, hold pairs and re-prepare per operation as `run_batch` does (small per-write cost, no design change). Decide at Task 3.
- **`Mutex` discipline:** semantic work is synchronous CPU; keep the `std::sync::Mutex` never held across `.await` so a slow validate cannot deadlock the runtime.
- **406-on-brotli** is unit-tested only (Task 7); every target browser sends `br`, by the spec's own argument.
- **Headed-Chrome loopback permission prompt:** explicitly untested, per the spec's "Not tested" note.
