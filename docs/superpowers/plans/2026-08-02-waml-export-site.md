# WAML Export Site Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `waml export site`, publish the WAML documentation through that command, load embedded `.waml` bundles in the browser, and let every editor export its current model as a `.waml` file.

**Architecture:** A CLI-owned web-artifact assembler produces one virtual file map from embedded, brotli-compressed editor assets plus either a static bundle or a future API boot source. `waml export site` writes the static map to disk and Pages consumes the same command. The editor selects browser startup from fragment, API query, or bundle query in that order; its burger-menu export delegates to a cross-platform adapter backed by a native save dialog or a generic Makepad browser-download primitive.

**Tech Stack:** Rust 2024 workspace, clap, Makepad fork, bundle-envelope v1, brotli, Node 22 artifact scripts, GitHub Actions, browser integration tests.

## Global Constraints

- Execute `docs/superpowers/plans/2026-07-31-bundle-envelope-v1.md` first; this plan consumes `waml::bundle_envelope::{encode_bundle_envelope, split_bundle}` and does not duplicate that work.
- Keep `embed-web` disabled by default; ordinary `cargo build` and `cargo test --workspace` must not require a WASM toolchain or prebuilt artifact.
- Do not accept an `--artifact` flag. Embedded assets must come from the pinned, CI-built editor artifact.
- `waml export site` writes raw files. Only the future `waml serve` sink may retain brotli content encoding.
- Browser boot precedence is `#w1.` then `?api=` then `?bundle=` then the start screen.
- Exported-site edits update `#w1.` with `history.replaceState`; they never modify `bundle.waml`.
- The UI command is the existing burger-menu row **Export WAML bundle…**.
- Do not implement op-log download, `Op -> OpDto`, IndexedDB, service workers, `file://`, `?repo=`, `export svg`, `export json`, or Mermaid in this slice.
- Pin the exact Makepad commit that contains `Cx::download_file`; never pin a branch tip.

---

### Task 1: Add a generic Makepad browser-download primitive

**Repositories:** `C:/dev/makepad`, then `C:/dev/waml`

**Files:**
- Modify: `C:/dev/makepad/platform/src/os/web/from_wasm.rs`
- Modify: `C:/dev/makepad/platform/src/os/web/web.js`
- Modify: `C:/dev/makepad/platform/src/os/web/web_worker.js`
- Modify: `C:/dev/makepad/platform/src/os/web/web.rs`
- Modify: `C:/dev/makepad/platform/src/cx_api.rs`
- Test: `C:/dev/makepad/platform/src/os/web/web.js` adjacent JS unit seam or a new focused Node test under `C:/dev/makepad/platform/src/os/web/`
- Modify after the Makepad commit is available remotely: `crates/waml-editor/Cargo.toml`
- Modify after the Makepad commit is available remotely: `.github/workflows/pages.yml`

**Interfaces:**
- Produces: `pub fn download_file(&mut self, name: &str, bytes: Vec<u8>, mime_type: &str)` on `makepad_platform::Cx`.
- Produces: `FromWasmDownloadFile { name: String, mime_type: String, data: WasmDataU8 }`.
- Consumes later: Task 3's WASM export adapter.

- [ ] **Step 1: Write a failing bridge-shape test**

Assert that dispatching `FromWasmDownloadFile` creates a Blob with the supplied MIME type, assigns the requested filename to a temporary anchor, clicks it once, removes it, and calls `URL.revokeObjectURL`.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `rtk node --test platform/src/os/web/download_file.test.mjs`

Expected: FAIL because `FromWasmDownloadFile` and the JS handler do not exist.

- [ ] **Step 3: Add the typed wasm message and Cx API**

Implement:

```rust
#[derive(FromWasm)]
pub struct FromWasmDownloadFile {
    pub name: String,
    pub mime_type: String,
    pub data: WasmDataU8,
}

impl Cx {
    pub fn download_file(&mut self, name: &str, bytes: Vec<u8>, mime_type: &str) {
        self.os.from_wasm(FromWasmDownloadFile {
            name: name.to_owned(),
            mime_type: mime_type.to_owned(),
            data: bytes.into(),
        });
    }
}
```

Follow the existing `FromWasmTextCopyResponse` and `FromWasmOpenUrl` routing pattern in both main-thread and worker web runtimes. Reject path components in `name` at the Rust boundary and retain only the final filename.

- [ ] **Step 4: Implement Blob delivery in both browser runtimes**

Use `new Blob([args.data], {type: args.mime_type})`, a temporary `<a download>`, one click, synchronous removal, and `URL.revokeObjectURL` in a queued microtask.

- [ ] **Step 5: Run focused Makepad verification**

Run: `rtk node --test platform/src/os/web/download_file.test.mjs`

Run: `rtk cargo check -p makepad-platform`

Expected: both PASS.

- [ ] **Step 6: Commit Makepad and obtain explicit authorization before pushing**

Commit: `feat(web): add generic file download bridge`

Do not update WAML to an unreachable local SHA. After the Makepad commit is pushed, update both exact pins in WAML and run `rtk cargo update -p makepad-widgets` so `Cargo.lock` records the same revision.

- [ ] **Step 7: Commit the WAML pin update**

Commit: `build: pin makepad file download support`

---

### Task 2: Package and embed the canonical web artifact

**Files:**
- Create: `crates/waml-cli/build.rs`
- Create: `crates/waml-cli/src/web_artifact.rs`
- Modify: `crates/waml-cli/Cargo.toml`
- Create: `scripts/package-web-artifact.mjs`
- Create: `scripts/package-web-artifact.test.mjs`
- Modify: `scripts/prune-web-fonts.mjs`
- Modify: `scripts/prune-web-fonts.test.mjs`

**Interfaces:**
- Produces: `pub(crate) struct EmbeddedAsset { pub path: &'static str, pub brotli: &'static [u8] }`.
- Produces: `pub(crate) fn embedded_artifact() -> Result<&'static [EmbeddedAsset], WebArtifactError>`.
- Produces: deterministic manifest `target/waml-web-embed/manifest.txt` with `path<TAB>compressed-file` rows.
- Consumes later: Task 5's assembler.

- [ ] **Step 1: Write failing artifact-package tests**

Create a fixture with `index.html`, JS, WASM, nested resources, and unused Makepad widget fonts. Assert stable lexical paths, brotli round trips, traversal rejection, duplicate normalized-path rejection, and pruning across both `waml_editor/resources` and `makepad_widgets/resources`.

- [ ] **Step 2: Run script tests and verify RED**

Run: `rtk node --test scripts/package-web-artifact.test.mjs scripts/prune-web-fonts.test.mjs`

Expected: FAIL because the packager does not exist and widget-resource pruning is incomplete.

- [ ] **Step 3: Implement deterministic packaging**

`package-web-artifact.mjs <artifact-dir> <out-dir>` must delete and recreate only the named output directory, walk regular files without following links, reject absolute/traversing output paths, brotli-compress each file, and write the sorted manifest last.

- [ ] **Step 4: Generate embedded Rust only when `embed-web` is enabled**

Add Cargo feature:

```toml
[features]
default = []
embed-web = ["dep:brotli-decompressor"]
```

`build.rs` reads `WAML_WEB_EMBED_DIR` only under `CARGO_FEATURE_EMBED_WEB`, validates the manifest, and generates `OUT_DIR/web_artifact.rs` containing `include_bytes!` entries. Without the feature it generates an empty provider whose error says how CI or a release builder must package the artifact.

- [ ] **Step 5: Run focused Rust and script tests**

Run: `rtk node --test "scripts/*.test.mjs"`

Run: `rtk cargo test -p waml-cli web_artifact`

Expected: PASS without `embed-web`; the explicit missing-artifact error is asserted.

- [ ] **Step 6: Commit**

Commit: `build(web): package editor artifact for embedding`

---

### Task 3: Export the current `.waml` bundle from the editor burger menu

**Files:**
- Create: `crates/waml-editor/src/bundle_export.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/app/menus.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app/tests/menus.rs`
- Modify: `crates/waml-editor/Cargo.toml`

**Interfaces:**
- Produces: `pub(crate) fn encode_current_bundle(source: &SourceBundle) -> Result<Vec<u8>, String>`.
- Produces: `pub(crate) trait BundleExportAdapter { fn export(&mut self, cx: &mut Cx, suggested_name: &str, bytes: Vec<u8>) -> Result<(), String>; }`.
- Produces: native and WASM `PlatformBundleExport` implementations.

- [ ] **Step 1: Write failing pure export and menu tests**

Assert that the menu contains enabled `live_id!(export_waml)` only when a model is open; encoding uses `encode_bundle_envelope(&source.to_pairs())`; the filename normalizer converts the bundle title to a safe `.waml` name; and an injected adapter receives byte-exact envelope output.

- [ ] **Step 2: Run tests and verify RED**

Run: `rtk cargo test -p waml-editor app::tests::menus bundle_export`

Expected: FAIL because the row, command, and adapter do not exist.

- [ ] **Step 3: Implement the flat burger-menu command**

Add `Export WAML bundle…` between Open model and Close model. Route it through `observe_popup_results` without changing the exclusive action order.

- [ ] **Step 4: Implement platform delivery**

Native: use `rfd::FileDialog::new().set_file_name(...).add_filter("WAML bundle", &["waml"]).save_file()` and write the encoded bytes.

WASM: call `cx.download_file(suggested_name, bytes, "application/vnd.waml.bundle")`.

Both arms export `self.session.snapshot().source`, so unsaved/share-URL edits are included. Cancellation is not an error; encoding and I/O failures go to the status bar.

- [ ] **Step 5: Run focused tests**

Run: `rtk cargo test -p waml-editor app::tests::menus bundle_export`

Expected: PASS.

- [ ] **Step 6: Commit**

Commit: `feat(editor): export current WAML bundle`

---

### Task 4: Add browser boot-source selection and static bundle fetching

**Files:**
- Create: `crates/waml-editor/src/browser_boot.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/event.rs`
- Modify: `crates/waml-editor/src/app/workspace.rs`
- Test: `crates/waml-editor/src/browser_boot.rs`
- Test: `crates/waml-editor/src/app/tests/workspace.rs`

**Interfaces:**
- Produces: `enum BrowserBootSource { Share(String), Api { base: String, token: Option<String> }, Bundle(String), Start }`.
- Produces: `fn select_browser_boot(search: &str, hash: &str) -> Result<BrowserBootSource, String>`.
- Produces: one pending HTTP request id for the bundle load and an event handler that decodes envelope v1 into `SourceBundle`.

- [ ] **Step 1: Write the precedence table tests**

Cover share-over-api, share-over-bundle, api-over-bundle, percent-decoded absolute and relative bundle URLs, empty values, malformed query encoding, non-share anchors, HTTP failure, CORS failure text, malformed envelope, and a valid envelope.

- [ ] **Step 2: Run tests and verify RED**

Run: `rtk cargo test -p waml-editor browser_boot app::tests::workspace`

Expected: FAIL because only fragment startup exists.

- [ ] **Step 3: Implement the pure selector**

Read `WebParams.search` and `WebParams.hash` once. Do not let application shell code inspect platform-specific URLs after startup.

- [ ] **Step 4: Implement `?bundle=` fetch and visible failure**

Issue `cx.http_request(live_id!(boot_bundle), HttpRequest::new(url, HttpMethod::GET))`. On success require a valid bundle envelope, open it as display name `exported`, and keep the existing fragment save backend. On any network, CORS, status, or decode error, show the start screen and put the specific message in the existing status/error surface; never leave an empty editor.

- [ ] **Step 5: Prove the first edit replaces the hash**

Extend `browser_save_fragment` tests to start from `?bundle=bundle.waml`, apply one edit, and assert the resulting URL retains the query and has exactly one `#w1.` fragment via `replaceState` semantics.

- [ ] **Step 6: Run focused tests**

Run: `rtk cargo test -p waml-editor browser_boot app::tests::workspace`

Expected: PASS.

- [ ] **Step 7: Commit**

Commit: `feat(web): load exported WAML bundles`

---

### Task 5: Build one virtual-site assembler and `waml export site`

**Files:**
- Create: `crates/waml-cli/src/site.rs`
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/tests/cli_e2e.rs`
- Modify: `crates/waml-cli/Cargo.toml`

**Interfaces:**
- Produces: `pub(crate) enum SiteSource { Static(Vec<u8>), Api }`.
- Produces: `pub(crate) fn assemble_site(artifact: &[EmbeddedAsset], source: SiteSource) -> Result<BTreeMap<String, Vec<u8>>, SiteError>`.
- Produces: clap hierarchy `Command::Export { target: ExportCommand }` and `ExportCommand::Site { dir, out, force }`.

- [ ] **Step 1: Write failing assembler tests**

For `Static`, assert raw decompressed assets, `bundle.waml`, and an `index.html` whose boot URL is `?bundle=bundle.waml`. For `Api`, assert no bundle and `?api=/api`. Reject duplicate paths, unsafe paths, missing `index.html`, and decompression failures.

- [ ] **Step 2: Write failing output-guard and clap tests**

Cover absent output directory, empty output directory, non-empty refusal, non-empty `--force`, default `./site`, and `waml export site DIR --out OUT --force` parsing.

- [ ] **Step 3: Run tests and verify RED**

Run: `rtk cargo test -p waml-cli site parses_export_site`

Expected: FAIL because the assembler and command group do not exist.

- [ ] **Step 4: Implement assembler and guarded writer**

Decompress every embedded asset with a bounded decoder, patch only the generated HTML boot URL through an exact sentinel owned by `scripts/inject-runtime-shell.mjs`, encode the source directory with `encode_bundle_envelope`, and build the complete map before the first filesystem write.

Validate the resolved output path. Without `--force`, require missing or empty. With `--force`, replace only paths present in the assembled map; do not recursively delete unrelated output files.

- [ ] **Step 5: Implement feature-off diagnostics**

Both `export site` and the future `serve` entry must compile without `embed-web`. Running them returns exit code 2 and a message that names `--features embed-web` and `WAML_WEB_EMBED_DIR`.

- [ ] **Step 6: Run focused and E2E tests**

Run: `rtk cargo test -p waml-cli site`

Run: `rtk cargo test -p waml-cli --test cli_e2e export_site`

Expected: PASS.

- [ ] **Step 7: Commit**

Commit: `feat(cli): export self-contained WAML sites`

---

### Task 6: Make GitHub Pages consume `waml export site`

**Files:**
- Modify: `.github/workflows/pages.yml`
- Modify: `scripts/inject-runtime-shell.mjs`
- Modify: `scripts/inject-runtime-shell.test.mjs`
- Modify: `scripts/verify-web-artifact.mjs`
- Modify: `scripts/verify-web-artifact.test.mjs`

**Interfaces:**
- Consumes: Tasks 2 and 5 packaging/CLI.
- Produces: Pages upload directory created only by `waml export site docs/waml --out target/pages`.

- [ ] **Step 1: Extend shell-script tests with a stable boot sentinel**

The generated `index.html` must contain one exact placeholder that the assembler can replace with `?bundle=bundle.waml` or `?api=/api`; multiple or missing placeholders are errors.

- [ ] **Step 2: Run script tests and verify RED**

Run: `rtk node --test "scripts/*.test.mjs"`

Expected: FAIL because the sentinel contract is absent.

- [ ] **Step 3: Fix pruning across dependency resource trees**

Derive the keep set from WAML and Makepad live-design sources, delete the measured duplicate/unreferenced widget fonts, and retain a fixture assertion for every font path still referenced by either source tree.

- [ ] **Step 4: Replace hand assembly in Pages**

Keep the nightly, cargo-makepad, binaryen, branding, shell injection, script tests, and artifact verification steps. Then package the artifact, build `waml-cli --release --features embed-web` with `WAML_WEB_EMBED_DIR`, run `target/release/waml export site docs/waml --out target/pages`, verify `target/pages`, and upload only `target/pages`.

- [ ] **Step 5: Run workflow-adjacent verification locally**

Run: `rtk node --test "scripts/*.test.mjs"`

Run: `rtk cargo test -p waml-cli site`

Expected: PASS.

- [ ] **Step 6: Commit**

Commit: `ci(pages): publish through waml export site`

---

### Task 7: Browser acceptance and full verification

**Files:**
- Create: `scripts/export-site-browser.test.mjs`
- Modify: `README.md`
- Modify: `docs/waml/architecture/concepts/runtime/github-pages.md`
- Modify: `docs/waml/architecture/concepts/runtime/native-web-delivery.md`

**Interfaces:**
- Consumes: the release binary and built artifact from Tasks 1–6.
- Produces: end-to-end evidence for bundle boot, share-URL persistence, and `.waml` download.

- [ ] **Step 1: Add the browser acceptance test**

Build a temporary two-document bundle, export it, serve the directory over an ephemeral HTTP port, and use the repository's pinned Playwright Chromium. Assert no console panic, expected model title, one edit changing the hash to `#w1.`, a refresh preserving the edit, and a captured download named `*.waml` whose bytes decode to the edited source.

- [ ] **Step 2: Run browser verification**

Run: `rtk node --test scripts/export-site-browser.test.mjs`

Expected: PASS. Do not accept `file://` as a substitute.

- [ ] **Step 3: Update user and architecture documentation**

Document:

```text
waml export site docs/waml --out site
python -m http.server --directory site
```

State that the embedded bundle is immutable, edits persist in the share URL, and **Export WAML bundle…** downloads the edited source.

- [ ] **Step 4: Run the complete verification gate**

Run: `rtk cargo fmt --all -- --check`

Run: `rtk cargo test --workspace`

Set `WAML_WEB_EMBED_DIR` to the packaged artifact from Task 2, then run:

`rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`

Run: `rtk node --test "scripts/*.test.mjs"`

Run: `rtk node scripts/verify-web-artifact.mjs target/pages`

Expected: every command exits 0.

- [ ] **Step 5: Commit**

Commit: `test(export): verify standalone WAML sites`

---

## Spec Coverage

- Extensible `waml export` command group: Task 5.
- One assembler for static and API sources: Task 5; future `serve` consumes `SiteSource::Api`.
- Default-off embedded artifact and raw export decompression: Tasks 2 and 5.
- Font pruning: Tasks 2 and 6.
- Browser precedence and `?bundle=`: Task 4.
- Self-contained share URL after edits: Task 4.
- Burger-menu `.waml` export on native and web: Tasks 1 and 3.
- Pages as standing consumer of the command: Task 6.
- Browser and full-workspace verification: Task 7.
