# WAML review orientation map — 2026-08-04

Repo root: `C:\dev\waml` (Rust workspace, MPL-2.0, edition 2021, rust-version 1.80).
All paths below are repo-relative unless noted. Sizes are lines of Rust unless stated.

Reviewer rule files: `docs/review-rules/{correctness,performance,testability,security,maintainability,observability,resilience}.md`.

---

## 1. Workspace overview

`Cargo.toml` members: `crates/waml`, `crates/waml-cli`, `crates/waml-ops-dto`, `crates/waml-editor`, `crates/waml-syntax`, `crates/waml-markdown-editor`. `fuzz/` is `exclude`d (own workspace).
`crates/waml-wasm/` is **NOT** a crate — it is a checked-in prebuilt `pkg/` (`waml_wasm_bg.wasm`, `waml_wasm.js`, `.d.ts`). Treat it as a binary artifact, not source.

| Crate | ~LoC | Purpose | Depends on |
|---|---|---|---|
| `waml-syntax` | 18.6k | Domain-neutral lossless syntax: green/red trees, Markdown parser, incremental reparse | `pulldown-cmark` only |
| `waml` | 56.5k | Headless core: model, analysis, UML/OKF lowering, ops, layout solver, share/bundle | `waml-syntax`, `regex`, `miniz_oxide`, `ttf-parser`, optional `serde` |
| `waml-ops-dto` | 1.5k (single `src/lib.rs`) | Serde wire contract (`OpDto`) between CLI and core | `waml` (serde) |
| `waml-cli` | 6.8k | `waml` binary: check/fmt/share/bundle/export-site/serve + LSP server | `waml`, `waml-ops-dto`, clap, tokio, tower-lsp, axum, brotli, rand, subtle |
| `waml-markdown-editor` | 22.2k | WAML-owned Markdown editing widget/layout foundation (makepad) | `waml-syntax`, makepad fork, unicode-segmentation/bidi |
| `waml-editor` | 71.9k | Native + wasm GPU editor app (makepad immediate-mode) | `waml`, `waml-markdown-editor`, makepad fork, taffy, dirs, rfd |

Dependency direction is a clean DAG:
`waml-syntax` → `waml` → {`waml-ops-dto`, `waml-cli`}; `waml-syntax` → `waml-markdown-editor` → `waml-editor`; `waml` → `waml-editor`.
Nothing in `waml`/`waml-syntax` depends on makepad — the headless/frontend split holds at the manifest level.

makepad is pinned by **SHA** to a fork: `github.com/redoz/makepad rev 62f515dc…` (see the long rationale comment in `crates/waml-editor/Cargo.toml:13-40`). `unicode-bidi` is also a git dep on that fork (workspace `Cargo.toml`). Two of the three git deps must move together.

`editors/vscode/` — standalone Node/TypeScript project (not in the cargo workspace, not in a pnpm workspace). 3 source files: `src/extension.ts`, `src/serverPath.ts`, `src/serverPath.test.ts`. Uses `vscode-languageclient` 9, vitest 4, eslint 10, TS 5.9. Own `build`/`test`/`lint`/`format` scripts and own `pnpm-lock.yaml`.

Other top-level: `fuzz/` (cargo-fuzz), `scripts/` (web artifact pipeline, mjs + node:test), `docs/`, `resources/`, `lsp-demo/`, `.github/workflows/{ci,pages}.yml`, `run.ps1`, `build.ps1`/`build.sh`, `config.ps1`, `scripts/capture-window.ps1`.

---

## 2. Architecture

### Headless core vs frontends
- **Syntax layer** (`crates/waml-syntax/src`): `green.rs` (immutable green nodes, 468) / `red.rs` (SyntaxTree/SyntaxNode/SyntaxToken, 530) / `text.rs` (`SourceText`, `LineIndex`, `TextRange`, `DocumentRevision`) / `ast.rs` / `annotation.rs` / `shell.rs`. Public surface is re-exported wholesale from `src/lib.rs` — a good place to judge API discipline.
- **Markdown parse** lives in `crates/waml-syntax/src/markdown/`: `parser.rs` (738) drives `block.rs` (1588) and `inline.rs` (1760); `scan/mod.rs` + `scan/pulldown.rs` are the **seam over pulldown-cmark** (recent commits ef05d711, 9a29e227, f82d2f03 all hardened this seam — raw event ranges are screened before use). `snapshot.rs` (1239) builds `MarkdownSyntaxSnapshot`/`MarkdownStructureMap` consumed by everything above. `projection.rs`, `reference.rs`, `kind.rs`, `reparse.rs`.
- **Incremental reparse**: `crates/waml-syntax/src/incremental.rs` (1597) — `TextChange`/`ChangeMap`/`ChangeSegment`, `rebase_unchanged_green`, `transfer_mapped_annotations`, `ReparseOutcome`, `FullReparseReason`. Entry `ChangeMap::checked` (`incremental.rs:362`). Markdown-level driver: `markdown/reparse.rs` → `reparse_markdown`. This is the single most subtle code in the repo (see MEMORY: a proptest-found bug where reparse windows swallowed trailing EOF whitespace).
- **Model / analysis / solve** (`crates/waml/src`):
  - `model.rs` (1410) domain types; `source.rs` `SourceBundle`/`SourceDocument`/`BundlePath`; `okf.rs` + `okf/lower.rs` (1311) and `uml/` (`syntax/parser.rs` 4734, `analysis.rs` 3521, `lower.rs` 1689, `sequence.rs` 1699, `format.rs`, `ops.rs`, `rename.rs`, `repair.rs`, `selector.rs`) — two lowering front-halves feeding one model.
  - `analysis.rs` (1802): `DocumentCatalog`, `SyntaxSet`, `OkfAnalysis`, `PreparedCandidate`, `prepare_candidate` (:768), claim validation (`validate_disjoint_claims` :673), `AnalysisStage`/`AnalysisError`. This is the incremental-analysis orchestrator.
  - `solve/`: `mod.rs` (1290, `solve_diagram`/`solve_diagram_reported`/`solve_diagram_routed` :266-332, `place_labels_with_reroute` :494), `route.rs` (1965 — **the edge router**, orthogonal only by policy), `geometry.rs` (1393), `label.rs` (1268), `flow.rs` (1071), `interaction.rs` (1070), `stress.rs`, `sizing.rs`, `resolve.rs`, `potentials.rs`.
  - `ops/mod.rs` (1716) + `edit/` — the mutation/undo surface; `compat.rs` batches.
  - Transport/serialization: `share.rs` (deflate → `#w1.` URL fragment; `encode_source`/`decode_source`/`is_share_link`), `bundle_envelope.rs` (`split_bundle`, `encode_bundle_envelope`), `index_md.rs`, `frontmatter.rs`, `site_boot.rs`, `host.rs`.
- **Editor** (`crates/waml-editor`): note `src/lib.rs` exports only `editor_history` and `view_history` — **everything else is private to `src/main.rs`'s module tree**, so `tests/` integration tests can only reach those two; all other testing must be `#[cfg(test)]` in-module. This shapes the whole test landscape.
  - App loop: `src/app.rs` (`impl AppMain for App` :864, `handle_event` :958, `handle_actions` :844) with `src/app/{shell.rs, actions.rs, navigation.rs, workspace.rs, event.rs, menus.rs}`.
  - Immediate-mode widgets each own `draw_walk`/`handle_event` — 39 files implement `draw_walk` (list in §3).
  - Document/session state: `editor_session.rs` (3417 — largest non-generated file), `document_host.rs`, `documents.rs`, `doc_tabs.rs`, `scene.rs` (1809), `markdown_hosts.rs`.
  - `icons.rs` is 4987 lines / 220 KB of generated SDF icon data — skim, don't review (generator `scripts/gen-icon.py`; `gen-all-icons.py` is stale per MEMORY).
- **Web/wasm path**: `crates/waml-editor/src/browser_boot.rs` decides startup source purely from the URL — precedence Share (`#w1.`) > `?api=` > `?bundle=` > start screen (`select_browser_boot`). `platform_browser.rs`, `bundle_export.rs`, `load.rs`. Build pipeline is CI-only (`.github/workflows/pages.yml`): cargo-makepad wasm build → `scripts/prune-web-fonts.mjs` → `brand-web-artifact.mjs` → `inject-runtime-shell.mjs` (19.9 KB, the biggest script) → `verify-web-artifact.mjs` → `package-web-artifact.mjs` → `cargo build -p waml-cli --features embed-web` → `waml export site` → verify again. `measure-web-boot.mjs`/`attribute-web-boot.mjs` are perf probes.
- **LSP/vscode path**: `crates/waml-cli/src/lsp/{server.rs 541, bundle.rs 853, query.rs 451, map.rs 425}` over tower-lsp/tokio; `editors/vscode/src/extension.ts` spawns the `waml` binary resolved by `serverPath.ts`.

---

## 3. Hot paths & entry points

**Parse**
- `waml_syntax::parse_markdown` / `reparse_markdown` (`crates/waml-syntax/src/markdown/parser.rs`, `reparse.rs`).
- Scan seam: `crates/waml-syntax/src/markdown/scan/pulldown.rs` (203) — every raw pulldown event range is screened here.
- `waml::analysis::prepare_candidate` (`crates/waml/src/analysis.rs:768`) — the incremental analysis entry.
- UML text → AST: `crates/waml/src/uml/syntax/parser.rs` (4734, largest core file).
- Bundle in: `waml::okf::Bundle::parse` (`crates/waml/src/okf.rs:270`), `waml::source::BundlePath::parse` (`source.rs:35`).

**Editor loop**
- `App::handle_event` (`src/app.rs:958`) → `app/event.rs` → `app/actions.rs` (1306) → session mutation in `editor_session.rs`.
- Draw: `App` draws `app/shell.rs` (941) which walks the dock/panels; per-widget `draw_walk` in 39 files, heaviest: `tree_panel.rs` (1846), `class_diagram_view.rs` (1859), `canvas/class/widget.rs` (1835), `inspector_panel.rs` (1594), `node_design_editor.rs` (1691), `doc_tabs.rs` (1258), `popup/radial.rs` (1022), `document_header.rs`, `statusbar.rs`, `view_bar.rs`.
- Canvas/viewport transform + hit-testing: `src/canvas/viewport.rs` (798), `canvas/geometry.rs`, `canvas/linework.rs`, `canvas/class/placement.rs` (1285).
- Markdown widget: `crates/waml-markdown-editor/src/widget.rs` (1804) + `layout/engine.rs` (2871) + `layout/makepad.rs` (1417) + `layout/geometry.rs` (1105); presentation mode in `src/presentation/*`.

**Layout / routing solver**
- `waml::solve::solve_diagram_routed` (`crates/waml/src/solve/mod.rs:332`), routing in `solve/route.rs` (1965), label placement `solve/label.rs` + `place_labels_with_reroute` (`solve/mod.rs:494`), sizing `solve/sizing.rs`.
- `taffy` is a `waml-editor` dep but **headless-only** — never for widget layout (makepad Turtle owns that).

**Export / bundle / share**
- `waml::share::{encode_source, decode_source, is_share_link}` (`crates/waml/src/share.rs:74-122`) — deflate + URL fragment.
- `waml::bundle_envelope::{split_bundle:243, encode_bundle_envelope:283}`.
- CLI: `run_bundle` (`waml-cli/src/main.rs:929`), `run_export_site` (:970) → `crates/waml-cli/src/site.rs` (`assemble_site:82`, `write_site:118`, `is_safe_relative_path:164`, `decompress:150`) with brotli-compressed embedded assets (`web_artifact.rs`, `build.rs` 141).
- Editor-side export: `src/bundle_export.rs`, `src/native_save.rs` (764, includes a Win32 `unsafe extern "system"` block at :342).

**File IO**
- `crates/waml-cli/src/io.rs` (1159) — the single biggest IO surface: `collect_md`, `read_bundle_rooted`, `read_analysis_bundle`, `read_files`, `read_physical_bundle`, `read_ndjson`, and `write_back` (:207) with a staging-dir + rename + permission-restore + symlink-metadata dance (`fs::symlink_metadata` at :290/:322). Highest-risk IO code in the repo.
- Editor: `src/config.rs` (22 KB, `~/.waml` root via `dirs::home_dir`), `src/native_save.rs`, `src/load.rs`, `rfd::FileDialog` in `app/workspace.rs:642` and `bundle_export.rs:82`.
- Site writer: `waml-cli/src/site.rs` (whole site assembled in memory before any write — deliberate).

---

## 4. Test landscape

- **`waml`** — 36 integration files in `crates/waml/tests/` (~430 KB). Heaviest: `incremental_analysis.rs` (2292 lines), `golden.rs` (946), `interaction_solver_golden.rs` (1202), `sequence_semantics.rs` (1073), `parser_platform_properties.rs` (1056, proptest), `flow_solver_golden.rs`, `uml_*_syntax.rs` ×5, `no_legacy_authority.rs`, `serde_shape.rs`. Fixtures: `crates/waml/tests/fixtures/{behavior, parser-platform, orders-domain.md}`. Proptests: `tests/bundle_envelope_properties.rs`, `tests/parser_platform_properties.rs`. Example baseline: `examples/parser_platform_baseline.rs`.
- **`waml-syntax`** — 14 files in `tests/` plus in-src tests: `src/incremental/low_level_tests.rs` (1377), `src/incremental/properties.rs` (379, proptest), `src/markdown/parser_tests.rs` (550). `tests/markdown_conformance.rs` (1305) + `markdown_gfm.rs` + `markdown_extensions.rs` are the CommonMark/GFM conformance suite; `tests/properties.rs` (586, proptest); `tests/scan_seam.rs` guards the pulldown seam; `tests/public_surface.rs` guards the exported API.
- **`waml-markdown-editor`** — 14 files, ~250 KB, incl. `layout_geometry.rs` (3400), `draw_layers.rs` (949), `document_ops.rs` (871), `widget_parity.rs`, `unicode_ime.rs`, `presentation_*.rs`. Has `tests/fixtures/` and `tests/goldens/`. Best-tested frontend code by a wide margin.
- **`waml-editor`** — only 5 integration files (`editor_history.rs`, `view_history.rs`, `history_integration.rs`, `markdown_authority.rs`, `markdown_integration.rs`) because `lib.rs` exposes just two modules. Real coverage is in-module: `editor_session.rs` 44 tests, `inspector.rs` 38, `scene.rs` 33, `card/mod.rs` 32, `app/tests/navigation.rs` 31 (61 KB), `dock.rs` 25, `tree_panel.rs` 24, plus `app/tests/{shell,workspace,menus}.rs`. `tests/README.md` (13 KB) documents the strategy. Harnesses: `src/bin/{icon,logo,logo_pulse,node_editor,markdown_presentation}_harness.rs`.
- **`waml-cli`** — 2 files: `tests/cli_e2e.rs` (554), `tests/lsp_e2e.rs` (506), plus in-module tests in `io.rs`, `site.rs`, `lsp/bundle.rs`, `main.rs` (clap parse tests).
- **`waml-ops-dto`** — in-module tests in the single `lib.rs`.
- **fuzz** — `fuzz/fuzz_targets/{parse_write, syntax_edits, outer_mapping, uml_islands}.rs` + `support.rs`, seed corpora in `fuzz/seeds/*`. Not in the cargo workspace, so **not run by CI**.
- **Node** — `scripts/{inject-runtime-shell, package-web-artifact, prune-web-fonts, verify-web-artifact, export-site-browser}.test.mjs` via `node --test`. CI (`ci.yml`) runs **only** `inject-runtime-shell.test.mjs`; `pages.yml` runs `node --test "scripts/*.test.mjs"`. vscode: `editors/vscode/src/serverPath.test.ts` (vitest) — `extension.ts` itself is untested.

**Known gaps**: `waml-cli/src/serve/mod.rs` is a stub (`run` prints "not implemented" and returns 2) though `ServeArgs`, axum/rand/subtle deps and CLI parsing exist; fuzz targets excluded from CI; `scripts/*.test.mjs` only partially gated on PRs; `waml-editor` draw code is untestable through `tests/` by construction; `browser_boot`'s wasm-only branches are host-tested only for the pure selection function.

---

## 5. Risk index per dimension

### Correctness
- `crates/waml-syntax/src/incremental.rs:99` `rebase_unchanged_green` / `:241` `transfer_mapped_annotations` / `:362` `ChangeMap::checked` — offset arithmetic across edits; already produced one shipped bug.
- `crates/waml-syntax/src/markdown/scan/pulldown.rs` + `scan/mod.rs` — byte ranges from a third-party parser reinterpreted as tree spans; four recent fix commits cluster here.
- `crates/waml/src/uml/syntax/parser.rs` (4734) — hand-written parser, largest correctness surface; paired with `uml/analysis.rs` (3521).
- `crates/waml/src/solve/route.rs` (1965) + `solve/geometry.rs` — orthogonal routing invariants (endpoints must land on the target border, see `docs`/MEMORY).
- `crates/waml/src/analysis.rs:673` `validate_disjoint_claims`, `:768` `prepare_candidate` — staleness/claim rules for incremental analysis.
- `crates/waml/src/uml/lower.rs` + `okf/lower.rs` — two lowerers feeding one model; `tests/*_lowering_order.rs` exist because order has bitten before.

### Performance
- `crates/waml/src/solve/route.rs`, `solve/stress.rs`, `solve/label.rs` — the per-frame-ish solver; `place_labels_with_reroute` (`solve/mod.rs:494`) can re-route.
- `crates/waml-markdown-editor/src/layout/engine.rs` (2871) + `layout/makepad.rs` — text shaping/measurement per draw; MEMORY notes zoom draws cost 500–1200 ms due to per-size font rasterization.
- `crates/waml-editor/src/scene.rs` (1809) and `canvas/viewport.rs` — immediate-mode redraw scope; what invalidates what.
- `crates/waml-syntax/src/incremental.rs` — the whole point is avoiding full reparse; check `FullReparseReason` escape hatches (`incremental.rs:40`) for how often they fire.
- Web boot: `scripts/measure-web-boot.mjs`, `attribute-web-boot.mjs`, `prune-web-fonts.mjs`; MEMORY: shader-link batching already took boot 9081→1730 ms, per-frame compile budgets are a dead end.
- `crates/waml-syntax/benches/markdown_parse.rs` (declared `[[bench]]`, harness=false) — the only bench in the repo.

### Testability
- `crates/waml-editor/src/lib.rs` (2 lines) — the structural cause of thin integration coverage; everything else is behind `main.rs`.
- `crates/waml-editor/src/editor_session.rs` (3417) — 44 in-module tests but a god-object; hard to test in pieces.
- `crates/waml-cli/src/serve/mod.rs` — deps and args exist, implementation and tests do not.
- `fuzz/` — 4 targets + seeds, excluded from the workspace and from CI.
- `.github/workflows/ci.yml` "Web artifact script tests" step runs 1 of 5 `.test.mjs` files.
- `crates/waml-editor/tests/README.md` — the documented strategy; judge it against what actually exists.
- `editors/vscode/src/extension.ts` — no test; only `serverPath.ts` is covered.

### Security
- `crates/waml-cli/src/io.rs:207` `write_back` and its staging/rename/permissions/symlink handling (`:290`, `:322`, `:580`) — TOCTOU and symlink-escape surface on user-supplied paths.
- `crates/waml-cli/src/site.rs:164` `is_safe_relative_path` + `:150` `decompress` — zip-slip-class path validation and brotli decompression of embedded assets.
- `crates/waml/src/share.rs` — deflate decode of attacker-controlled `#w1.` URL fragments (`decode_source:97`); decompression-bomb and malformed-input handling.
- `crates/waml/src/bundle_envelope.rs:243` `split_bundle` — parses untrusted multi-document text.
- `crates/waml-editor/src/browser_boot.rs` — `?api=`/`?bundle=` fetch targets taken from the URL; check for SSRF-ish / arbitrary-origin fetch and token handling (`Api { base, token }`).
- `crates/waml-cli/src/serve/` — `axum` + `rand` + `subtle` are already dependencies (bearer-token comparison intended) but the server is unimplemented; review the intended design and the unused-dep footprint.
- `crates/waml-editor/src/native_save.rs:342` — the only meaningful `unsafe` (Win32 `extern "system"` call).

### Maintainability
- `crates/waml/src/uml/analysis.rs` (3521) and `crates/waml/src/uml/syntax/parser.rs` (4734) — the two largest core files.
- `crates/waml-editor/src/editor_session.rs` (3417) — god object at the centre of the editor.
- `crates/waml-editor/Cargo.toml:13-40` — a ~28-line prose comment justifying one git SHA pin; the fork lineage is load-bearing tribal knowledge.
- `crates/waml/src/compat.rs` (23 KB) + `crates/waml/tests/no_legacy_authority.rs` (22.6 KB) — a legacy/compat layer policed by a test; is it shrinking?
- `crates/waml-syntax/src/lib.rs` — flat re-export of ~60 symbols; module boundaries vs public surface.
- `crates/waml-editor/src/icons.rs` (4987 lines, 220 KB generated) — generated data in-tree, generator partly stale (`scripts/gen-all-icons.py`).
- `crates/waml-editor/src/main.rs` — ~80 `mod` declarations flat at the binary root, with `#[allow(dead_code)]` on `markdown_hosts`.

### Observability
- No logging framework anywhere: zero `tracing::`/`log::`/`env_logger` uses across `crates/*/src`. The only diagnostics channel in the editor is makepad's `log!` macro, used as TODO/stub markers (`crates/waml-editor/src/app/actions.rs:250, 262, 291, 618, 774`).
- `crates/waml-cli/src/main.rs` — 55 `eprintln!`/`println!` calls; exit codes are the real API surface (0 ok / 2 IO). Check consistency and machine-readability.
- `crates/waml/src/diagnostic.rs` + `analysis.rs:551` `DiagnosticSource` / `:557` `RevisionedDiagnostic` — the structured-diagnostic pipeline that reaches the LSP; the one real observability asset.
- `crates/waml-cli/src/lsp/server.rs` — what surfaces to the client on internal failure.
- `crates/waml-editor/src/fps_meter.rs` (10.6 KB) and `statusbar.rs` — in-app perf/status telemetry.
- `scripts/attribute-web-boot.mjs` / `measure-web-boot.mjs` — the only production-path measurement tooling; not wired into CI.

### Resilience
- `crates/waml-cli/src/io.rs:207-420` `write_back` — staging dir, partial-failure cleanup (`fs::remove_dir_all` at :342, :389, :409), displaced-file restore (:553-563). The failure ladder here deserves line-by-line attention.
- `crates/waml-cli/src/site.rs:82/118` — "assembled in memory before any write" is the stated crash-safety property; verify it holds.
- `crates/waml-syntax/src/incremental.rs:40` `FullReparseReason` — the designed fallback when incremental cannot proceed; check it's total.
- Panic density in core: `crates/waml/src/ops/mod.rs` (58 `unwrap`/`expect`), `okf/ops.rs` (42), `uml/analysis.rs` (41), `analysis.rs` (34), `okf.rs` (33) — these run in-process inside the editor, so a panic is a lost document.
- `crates/waml/src/share.rs` / `bundle_envelope.rs` — malformed-input paths reached from a URL; must degrade, not panic.
- `crates/waml-editor/src/browser_boot.rs` — network boot sources with no retry/timeout logic visible; also MEMORY: `SystemTime::now()` panics on wasm.
- `crates/waml-editor/src/native_save.rs` / `config.rs` — save + `~/.waml` config read/write failure handling.

---

## 6. Build & gate

**Local, everything:**
```
./build.ps1            # pnpm install (editors/vscode) + cargo build --workspace + pnpm build
./build.ps1 -Test      # + cargo test --workspace + pnpm -C editors/vscode test
./build.ps1 -Lint      # + cargo clippy --workspace --all-targets --all-features -- -D warnings + pnpm lint
```
(`build.sh` is the POSIX twin.)

**Rust only:** `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
Note MEMORY: clippy `-D warnings` promotes `dead_code` to a hard error — new-but-unused code fails the gate.

**VS Code extension** (own project, `cd editors/vscode`): `pnpm install --frozen-lockfile`, `pnpm build` (tsc), `pnpm test` (vitest), `pnpm lint` (eslint), `pnpm format:check` (prettier).

**Node artifact scripts:** `node --test scripts/inject-runtime-shell.test.mjs` (CI), `node --test "scripts/*.test.mjs"` (pages only).

**CI** (`.github/workflows/ci.yml`) — `pull_request` + `workflow_call` + `workflow_dispatch`, no `push: main` (pages.yml calls it). Matrix ubuntu + **windows** (deliberate: the parser is path/line based). Steps: pnpm install → Linux system deps for makepad → rust stable + rustfmt/clippy → `Swatinem/rust-cache` → cargo-nextest → `cargo fmt --check` → `cargo nextest run --workspace --profile ci` → `cargo test --workspace --doc` (nextest skips doctests) → clippy `-D warnings` → node artifact test → vscode build/test/lint.

**Pages** (`.github/workflows/pages.yml`, 10.7 KB) — gated on ci.yml; nightly Rust + cargo-makepad + binaryen; build wasm → prune fonts → brand → inject runtime shell → test scripts → verify → package → `cargo build -p waml-cli --release --features embed-web` → `waml export site docs/waml --out target/pages` → verify → deploy.

**Running the editor:** `./run.ps1 [fixture]` (release by default; `-Empty` for start screen, `-DebugBuild`, `-Title`/`-Color` per-agent window markers). It kills only stragglers whose exe path matches this checkout, then `cargo build -p waml-editor` and `cargo run`. Default fixture `crates/waml-editor/tests/fixtures/mini`. `scripts/capture-window.ps1` screenshots a window (capture/kill by pid, never by name). `config.ps1` configures local env.

**Gotchas worth knowing before reviewing:** the makepad SHA pin must not be moved to a branch name; `crates/waml-wasm/pkg/` is a committed binary; `fuzz/` needs its own `cargo fuzz` invocation; `editors/vscode` has no root package.json above it.
