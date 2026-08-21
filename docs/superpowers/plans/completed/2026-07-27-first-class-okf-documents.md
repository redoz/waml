# First-Class OKF Documents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire the legacy Svelte/web-only TypeScript and WASM product stack, then make the source-authoritative OKF Knowledge Bundle the native editor's semantic root, derive UML as a selective projection, and open every unclaimed Concept in a dedicated Markdown-only Generic OKF view without regressing retained UML editing, persistence, tabs, CLI/LSP, or VS Code integration.

**Architecture:** First remove the unsupported Svelte application, its TypeScript domain/state packages, the Rust-to-JavaScript WASM bridge, generated bindings, and browser deployment pipeline while reducing pnpm to the independent VS Code extension. Then introduce an `Arc<String>`-backed `SourceBundle`, parse it into a domain-neutral `okf::Bundle`, and derive `uml::Projection` only from recognized UML Concepts. Domain-owned OKF and UML Lowerers feed one sealed, atomic `EditorSession::apply` transaction; static editor providers prepare `OpenDocument` values so `DocumentHost` owns lifecycle without knowing document families.

**Tech Stack:** Rust 2021 (workspace MSRV), `Arc<String>` copy-on-write source storage, existing WAML parser/serializer plus serde DTOs for Rust CLI/LSP consumers, Makepad widgets/script DSL, the retained TypeScript VS Code extension, inline Rust tests in the binary-only `waml-editor` crate, and PowerShell/PrintWindow native visual verification.

## Global Constraints

- Treat `docs/superpowers/specs/2026-07-27-first-class-okf-documents-design.md` at commit `7222efb` as the approved architecture.
- Delete `packages/web`, `packages/core`, `packages/okf`, `packages/wasm`, and `crates/waml-wasm`; do not carry their APIs or generated files through the OKF/UML migration.
- Remove web/WASM build, deployment, runtime-shell, asset-branding, template-generation, and TypeScript-generation tooling; do not retain dormant scripts.
- Keep `packages/vscode`, its TypeScript build/test dependencies, its `vscode-languageclient` integration, and the Rust `waml-cli` LSP server it launches. After retirement, all remaining `.ts` files must live under `packages/vscode`.
- Keep `waml-ops-dto` and its serde wire contract because the Rust CLI consumes it; remove only its `wasm` feature, `tsify-next`, and `wasm-bindgen` integration.
- Keep `waml`'s `serde` feature; remove its `wasm` feature and all tsify/wasm-bindgen-only derives, attributes, dependencies, and tests.
- Preserve completed documents under `docs/superpowers/specs/` and `docs/superpowers/plans/` as historical records. Update or remove active README/build/CI/deployment/backlog and `docs/waml/` pages that describe the retired product.
- The authoritative semantic root is an OKF Knowledge Bundle; UML remains a statically composed specialization.
- Do not add a global document-family enum, runtime provider registry, generic operation visitor, or plugin system.
- Do not make unclaimed Concepts UML `Node`s, `ElementType::Unknown` canvas boxes, or diagram members.
- Reserved `index.md` and `log.md` documents are not Concepts.
- Core OKF code must call physical hierarchy `Directory`/`Index`, never package; `uml.Package` remains valid only for explicitly authored UML Concepts.
- Preserve one atomic, ordered, source-authoritative edit transaction; a failed edit changes no source, projection, revision, or dirty field.
- Parser diagnostics remain visible but do not by themselves reject a successfully lowered edit.
- Share source text across current, persisted, candidate, and semantic representations; copy text only for documents touched by lowering.
- Keep `GenericOkfView` read-only, Markdown-only, and distinct from `SourceView`.
- Preserve existing preview replacement, persistent tabs, View Source, UML interactions, save timing, native persistence behavior, and action priority.
- Initial user-facing behavior targets the native editor and shared Rust projections; CLI/LSP/VS Code behavior changes only where required for retained Rust API/serde compatibility.
- Add no new dependencies unless a stage proves the existing standard-library and workspace dependencies insufficient.
- Keep tests inline under `#[cfg(test)]` in `waml-editor`; it is a binary-only crate with no `--lib` target.
- Every shell command in this plan must be prefixed with `rtk`, per `RTK.md`.
- Preserve unrelated worktree changes, especially the existing modification to `crates/waml-editor/tests/fixtures/mini/orders-diagram.md` and untracked `.idea/`, `git_history.txt`, and `scale_search.txt`.
- Each stage must compile and pass its focused tests before the next stage begins.

---

## Written-Spec Review and Locked Clarifications

The approved architecture is internally consistent, but the written spec leaves four migration details implicit. Treat the following as implementation rules, not architecture changes.

1. **Construction is fallible at the new boundary.** `SourceBundle::try_from_pairs` validates and normalizes paths, and `okf::Bundle::parse` rejects duplicate normalized Concept IDs. New code does not use an infallible `okf::build_bundle`; retained tuple/serde/CLI adapters convert these errors to their existing Rust error channels.
2. **`SourceSlice` preserves a string serde shape.** Its private `Arc<String>` and byte range never serialize as implementation fields. Custom serde serializes/deserializes it as a JSON string for retained Rust consumers; there is no tsify or TypeScript shape to preserve.
3. **Legacy heterogeneous DTO arrays remain compatibility-only.** New product code emits `okf::Batch` or `uml::Batch`. The old `waml::ops::Op` and CLI `Vec<OpDto>` boundary maps to a sealed `compat::Batch` that preserves cross-domain order and atomicity while delegating to the two Lowerers against one candidate `SourceBundle`; it is not a new product-domain operation enum.
4. **Projection rebuilds happen once per transaction.** Lowerers may parse individual touched documents while rewriting source, but they do not rebuild the complete OKF Bundle or UML Projection between operations. `EditorSession` parses the final candidate once, projects UML once, then commits all four state fields together.
5. **Reserved documents remain structural.** Directory rows get title/description/order from authored or synthesized `Index` values. `Index` and `Log` never use the Generic OKF provider, and this feature does not introduce dedicated Index/Log tabs.
6. **Generic rendering uses semantic body text.** `GenericOkfView` resolves the `Concept` by ID and renders `Concept.body.as_str()` on the shared Markdown surface. A stale tab whose Concept/source no longer exists renders the existing italic missing-source fallback.
7. **View identity stays provider-owned.** A provider chooses `LiveId`, presentation, and concrete `DocView` together in `OpenDocument`. `DocumentHost` stores those prepared values and never reconstructs a view from a kind. Source tabs continue through a separate editor-local `open_source` constructor.
8. **Explicit UML packages are ordinary claimed UML Concepts.** They are projected into the UML element pool and decorated by the UML provider; they do not stand in for physical `okf::Directory` records.

## File Structure

The migration ends with these ownership boundaries:

```text
repository root/
├── Cargo.toml / Cargo.lock   # Rust workspace without crates/waml-wasm
├── package.json              # pnpm scripts for packages/vscode only
├── pnpm-workspace.yaml       # includes packages/vscode only
├── pnpm-lock.yaml            # lockfile regenerated after web package deletion
├── build.ps1 / build.sh      # Rust + VS Code build/test entry points; no wasm-pack
└── .github/workflows/ci.yml  # Rust and VS Code checks; no web deploy job

crates/waml/src/
├── source.rs                 # BundlePath, SourceDocument, SourceBundle, SourceSlice
├── okf.rs                    # Bundle, Concept, Index, Log, Directory and lookup
├── okf/
│   └── ops.rs                # OKF Op/Batch and Lowerer
├── uml.rs                    # Projection facade and recognizer
├── uml/
│   └── ops.rs                # UML Op/Batch and Lowerer
├── edit.rs                   # sealed EditBatch, EditContext, PendingEdit, EditError
├── compat.rs                 # legacy ordered OKF/UML batch adapter for Rust DTO/CLI
├── parse.rs                  # document parser reused by the UML projector
├── model.rs                  # existing UML implementation types; Model compatibility name
├── index_md.rs               # deprecated compatibility wrapper over OKF index lowering
└── ops/
    ├── mod.rs                # deprecated compatibility Op/Batch adapter
    ├── pkg.rs                # removed after behavior moves to okf::ops
    ├── rename.rs             # UML rename helpers or removed after migration
    └── selector.rs           # shared UML selector implementation

crates/waml-editor/src/
├── editor_session.rs         # SourceBundle + OKF Bundle + UML Projection transaction
├── document.rs               # presentation records and prepared OpenDocument
├── documents.rs              # static UML-first / Generic-OKF-fallback composition root
├── uml_documents.rs          # UML claimant, presentation, and concrete view constructors
├── okf_documents.rs          # Generic OKF and explicit Source constructors
├── generic_okf_view.rs       # read-only Markdown-only DocView
├── markdown_surface.rs       # neutral helper for the shared Markdown widget
├── doc_view.rs               # ViewData over all session representations; PendingEdit outcome
├── document_host.rs          # prepared-document tab/view lifecycle only
├── doc_tabs.rs               # pure tab state; no TabKind factory dispatch
├── tree.rs                   # provider-decorated navigator records
├── nav.rs                    # browse/search/filter over OKF hierarchy
├── tree_panel.rs             # emits concept IDs from openable presentation records
├── load.rs                   # filesystem adapter -> SourceBundle
├── native_save.rs            # SourceBundle persistence adapter
└── app.rs / app/actions.rs   # composition, initial selection, session outcome routing

packages/
└── vscode/                   # sole retained TypeScript surface; launches Rust LSP
```

The removed `packages/web`, `packages/core`, `packages/okf`, `packages/wasm`, and
`crates/waml-wasm` trees have no compatibility successors. Native seed/template
data stays in Rust (`crates/waml/src/seed.rs` and its tests); native
`waml-editor` resources such as `resources/icon.ico`, fonts, and images remain
because the desktop binary still consumes them.

## Stage Gates

| Stage | Deliverable | Required gate |
|---|---|---|
| 1 | Characterization coverage | current behavior is green before structural changes |
| 2 | Legacy web retirement | only `packages/vscode` remains in pnpm; Rust/VS Code checks pass with no WASM/web pipeline |
| 3 | Shared source storage | all existing retained behavior passes on `SourceBundle` |
| 4 | First-class OKF Bundle | hierarchy/index tests pass without `Model` |
| 5 | Selective UML projection | unclaimed Concepts and directories are absent from UML |
| 6 | Domain Lowerers/session | all edit paths are atomic through one session apply |
| 7 | Static document composition | host has no `TabKind`/domain factory dispatch |
| 8 | Generic OKF UX | mixed and OKF-only bundles navigate/open correctly |
| 9 | Retained compatibility | serde, DTO, CLI/LSP, and VS Code consumers compile and pass |
| 10 | Full/native verification | workspace gates and required screenshots pass |

---

### Task 1: Freeze the Current Source, Projection, Operation, and Editor Contracts

**Files:**
- Modify: `crates/waml/src/okf.rs` inline tests
- Modify: `crates/waml/src/parse.rs` inline tests
- Modify: `crates/waml/src/index_md.rs` inline tests
- Modify: `crates/waml/src/ops/mod.rs` inline tests
- Modify: `crates/waml-editor/src/editor_session.rs` inline tests
- Modify: `crates/waml-editor/src/tree.rs` inline tests
- Modify: `crates/waml-editor/src/nav.rs` inline tests
- Modify: `crates/waml-editor/src/doc_tabs.rs` inline tests
- Modify: `crates/waml-editor/src/document_host.rs` inline tests

**Interfaces:**
- Consumes: current tuple-bundle APIs, `parse::build_model`, monolithic `ops::Op`, `TreeKind`, `TabKind`, and `EditorSession::apply_ops`.
- Produces: failing-safe characterization coverage that later stages deliberately update when the approved behavior changes.

- [ ] **Step 1: Record the clean baseline without touching unrelated fixture changes**

Run:

```powershell
rtk git status --short
rtk cargo test -p waml
rtk cargo test -p waml-editor
rtk cargo test -p waml-ops-dto
rtk cargo test -p waml-wasm
```

Expected: all four test commands pass. Record pre-existing failures rather than editing `tests/fixtures/mini/orders-diagram.md`.

- [ ] **Step 2: Characterize source identity and existing duplication**

Add tests that demonstrate current tuple cloning and exact path/source lookup before it is replaced:

```rust
#[test]
fn replacement_keeps_current_and_persisted_text_equal() {
    let pairs = vec![("notes.md".into(), "# Notes\n".into())];
    let model = waml::parse::build_model(&pairs);
    let mut session = EditorSession::default();
    session.replace(pairs.clone(), model);
    assert_eq!(session.bundle(), pairs.as_slice());
    assert_eq!(session.persisted_bundle(), pairs.as_slice());
}

#[test]
fn nested_source_identity_uses_the_full_okf_id() {
    let pairs = vec![
        ("sales/order.md".into(), "# Sales order".into()),
        ("support/order.md".into(), "# Support order".into()),
    ];
    assert_eq!(
        crate::load::source_for(&pairs, "support/order"),
        Some("# Support order")
    );
}
```

- [ ] **Step 3: Characterize unknown-document projection and structural packages**

Add one test for an arbitrary OKF `type`, one for a missing `type`, one for an unknown `uml.*` metaclass, and one nested-directory test. Assert the current pre-migration result explicitly:

```rust
let model = build_model(&[
    ("plain.md".into(), "# Plain\n".into()),
    ("vendor.md".into(), "---\ntype: vendor.Runbook\n---\n# Runbook\n".into()),
    ("future.md".into(), "---\ntype: uml.FutureThing\n---\n# Future\n".into()),
]);
assert_eq!(model.nodes.len(), 3);
assert!(model.nodes.iter().all(|node| matches!(node.ty, ElementType::Unknown(_))));
```

Name the tests with `_before_selective_projection` so Task 5 can replace their assertions without obscuring the intentional behavior change.

- [ ] **Step 4: Characterize index/package operations independently of editor UI**

Cover `PkgMove`, `PkgRename`, `PkgDelete`, `PkgReorder`, `PkgSort`, `PkgRetitle`, and `PkgInsert` with one focused assertion per semantic effect. Include:

```rust
#[test]
fn retitle_changes_index_content_without_changing_child_paths() {
    let before = vec![("sales/order.md".into(), "# Order\n".into())];
    let after = apply(
        &before,
        &[Op::PkgRetitle {
            path: "sales".into(),
            title: "Sales Domain".into(),
        }],
    )
    .unwrap();
    assert!(after.iter().any(|(path, text)| {
        path == "sales/index.md" && text.contains("# Sales Domain")
    }));
    assert!(after.iter().any(|(path, _)| path == "sales/order.md"));
}
```

- [ ] **Step 5: Characterize atomic editor application**

Retain the existing failure/success tests and add a multi-operation batch whose second operation fails. Capture every field before applying and assert the session is byte-for-byte unchanged:

```rust
let revision = session.revision();
let source = session.bundle().to_vec();
let model = session.model().clone();
let result = session.apply_ops(&[
    Op::PkgRetitle {
        path: "sales".into(),
        title: "Sales Domain".into(),
    },
    Op::NodeRename {
        from: "sales/order".into(),
        to: "customer".into(),
    },
]);
assert!(result.is_err());
assert_eq!(session.revision(), revision);
assert_eq!(session.bundle(), source);
assert_eq!(session.model(), &model);
assert!(!session.is_dirty());
```

- [ ] **Step 6: Characterize navigator and tab dispatch**

Add explicit tests for:

- structural directory ordering from `index.md`;
- `TreeKind::Unknown` producing no `OpenDocument` action;
- a diagram row opening a diagram tab;
- a classifier row opening a classifier preview;
- same-subject classifier/source IDs remaining distinct;
- preview replacement and double-click persistence;
- `DocumentHost` creating a view through the current `TabKind` factory.

- [ ] **Step 7: Run the focused characterization gate**

Run:

```powershell
rtk cargo test -p waml okf::tests
rtk cargo test -p waml parse::tests
rtk cargo test -p waml index_md::tests
rtk cargo test -p waml ops::tests
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor tree::tests
rtk cargo test -p waml-editor nav::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor document_host::tests
```

Expected: PASS; the pre-selective-projection tests still assert the old `Unknown`/package behavior.

- [ ] **Step 8: Commit the characterization stage**

```powershell
rtk git add crates/waml/src/okf.rs crates/waml/src/parse.rs crates/waml/src/index_md.rs crates/waml/src/ops/mod.rs crates/waml-editor/src/editor_session.rs crates/waml-editor/src/tree.rs crates/waml-editor/src/nav.rs crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/document_host.rs
rtk git commit -m "test: freeze OKF document migration behavior"
```

### Task 2: Retire the Legacy Web, TypeScript-Domain, and WASM Stack

**Files:**
- Delete: `packages/web/`
- Delete: `packages/core/`
- Delete: `packages/okf/`
- Delete: `packages/wasm/`
- Delete: `crates/waml-wasm/`
- Delete: `scripts/build-wasm.mjs`
- Delete: `scripts/gen-hero-hash.ts`
- Delete: `scripts/gen-template-bundles.mjs`
- Delete: `scripts/brand-web-artifact.mjs`
- Delete: `scripts/inject-runtime-shell.mjs`
- Delete: `scripts/inject-runtime-shell.test.mjs`
- Delete: `scripts/prune-web-fonts.mjs`
- Delete: `.github/workflows/pages.yml`
- Delete: `render.yaml`
- Delete: `docs/waml/wasm.md`
- Delete: `docs/waml/new-package-flow.md`
- Delete: `docs/waml/new-package-dialog.md`
- Delete: `docs/waml/model-store.md`
- Delete: `docs/waml/canvas-inner.md`
- Delete: `docs/waml/architecture/views/import-export-and-share.md`
- Delete: `docs/waml/architecture/views/github-pages-deployment.md`
- Delete: `docs/waml/architecture/concepts/workflows/exchange-and-sharing.md`
- Delete: `docs/waml/architecture/concepts/runtime/browser.md`
- Delete: `docs/waml/architecture/concepts/runtime/github-pages.md`
- Delete: `docs/waml/architecture/concepts/runtime/native-web-delivery.md`
- Delete: `docs/waml/architecture/concepts/runtime/share-recipient.md`
- Delete: `docs/waml/architecture/concepts/runtime/wasm-web-artifact.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/waml/Cargo.toml`
- Modify: `crates/waml/src/diagnostic.rs`
- Modify: `crates/waml/src/frontmatter.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/okf.rs`
- Modify: `crates/waml/src/slug.rs`
- Modify: `crates/waml/src/syntax.rs`
- Modify: `crates/waml/src/solve/mod.rs`
- Modify: `crates/waml-ops-dto/Cargo.toml`
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `crates/waml-editor/src/card/mod.rs`
- Modify: `crates/waml/tests/serde_shape.rs`
- Modify: `package.json`
- Modify: `pnpm-workspace.yaml`
- Modify/regenerate: `pnpm-lock.yaml`
- Modify: `eslint.config.mjs`
- Modify: `.prettierignore`
- Modify: `build.ps1`
- Modify: `build.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `issues.md`
- Modify: `docs/superpowers/backlog/repo-hygiene-open-decisions.md`
- Modify: `docs/superpowers/backlog/drag-place-viz-threads.md`
- Modify: `docs/waml/architecture/index.md`
- Modify: `docs/waml/architecture/concepts/index.md`
- Modify: `docs/waml/architecture/views/index.md`
- Modify: `docs/waml/architecture/views/system-context.md`
- Modify: `docs/waml/architecture/concepts/workflows/index.md`
- Modify: `docs/waml/architecture/concepts/runtime/index.md`
- Preserve unchanged: `packages/vscode/`, `crates/waml-cli/`, `crates/waml-ops-dto` serde DTO behavior, `scripts/run-native.ps1`, `scripts/run-native.sh`, `scripts/capture-window.ps1`, `config.ps1`, `tsconfig.base.json`, and native editor resources

**Interfaces:**
- Produces a Rust workspace containing `waml`, `waml-cli`, `waml-ops-dto`, and `waml-editor`; `crates/waml-wasm` is not a workspace member.
- Produces a pnpm workspace containing only `packages/vscode`, with root `build` and `test` scripts delegating to `@waml/vscode`.
- Keeps `waml` feature `serde = ["dep:serde"]`; removes feature `wasm` and dependencies `tsify-next`, `wasm-bindgen`, and `wasm-bindgen-utils`.
- Keeps `OpDto: Serialize + Deserialize` and `OpDto::to_op`/`from_op` behavior for the Rust CLI at this stage; removes only tsify/wasm ABI derives and the `waml-ops-dto/wasm` feature.
- Keeps the VS Code extension contract: compiled `packages/vscode/dist/extension.js` starts the configured `waml` executable through `vscode-languageclient`.

- [ ] **Step 1: Record the red retirement audit**

Run these read-only scans before deletion:

```powershell
rtk rg -l "packages/(web|core|okf|wasm)|@waml/(web|core|okf|wasm)|waml-wasm|build:wasm|wasm-pack|tsify|wasm-bindgen|Svelte|\\.svelte" Cargo.toml crates package.json pnpm-workspace.yaml eslint.config.mjs .prettierignore build.ps1 build.sh render.yaml scripts .github README.md issues.md docs/waml docs/superpowers/backlog
rtk rg --files -g "*.ts" -g "*.tsx" -g "*.svelte" -g "*.d.ts"
```

Expected: the first command lists the manifests, source, tooling, CI, README,
backlog, and active documentation owned above; the second includes TypeScript
and Svelte files outside `packages/vscode`. Save the output in the task notes,
not in a repository file. Do not scan `docs/superpowers/` for deletion: those
completed specs/plans are historical records.

- [ ] **Step 2: Remove the five retired implementation trees**

Delete `packages/web`, `packages/core`, `packages/okf`, `packages/wasm`, and
`crates/waml-wasm` as complete ownership units. Do not transplant their state
stores, grammar types, generated declarations, tests, or adapter APIs into
Rust. Native seed/template behavior remains owned by
`crates/waml/src/seed.rs`; if a deleted TypeScript template has no Rust
equivalent, it is retired product content rather than a migration requirement.

- [ ] **Step 3: Remove web/WASM build and deployment tooling**

Delete the exact scripts and deployment files listed above. In particular,
remove both the legacy bridge generator (`build-wasm.mjs`) and the Makepad
browser deployment (`pages.yml` plus branding/font/runtime-shell scripts):
the desktop editor remains the product and no browser artifact is published.

Do not delete `crates/waml-editor/resources/` wholesale. Verify
`crates/waml-editor/build.rs` still consumes `resources/icon.ico`, and retain
all fonts/images/icons referenced by native Makepad source. Assets located
inside the four deleted package trees disappear with their owning product.

- [ ] **Step 4: Remove Rust WASM/tsify features while retaining serde**

Update `Cargo.toml` workspace members, remove the `wasm` features/dependencies
from both retained crates, and remove every
`#[cfg_attr(feature = "wasm", ...)]`/tsify attribute from the listed Rust
source files. Do not remove serde derives, serde field attributes, JSON golden
tests, `waml-ops-dto`, or the CLI dependency on that crate.

Update the `waml-ops-dto` package description to say the serde contract is
consumed by the CLI. Update `crates/waml/tests/serde_shape.rs` so its comments
describe the Rust JSON contract rather than `packages/okf/src/types.ts`.
Regenerate `Cargo.lock` with:

```powershell
rtk cargo check --workspace
```

Expected: the workspace resolves without `crates/waml-wasm`; `waml` and
`waml-ops-dto` still compile with serde.

- [ ] **Step 5: Reduce pnpm to the VS Code extension**

Set `pnpm-workspace.yaml` to:

```yaml
packages:
  - "packages/vscode"
```

Replace root product scripts with:

```json
{
  "scripts": {
    "build": "pnpm --filter @waml/vscode build",
    "test": "pnpm --filter @waml/vscode test",
    "lint": "eslint packages/vscode/src",
    "format": "prettier --write packages/vscode",
    "format:check": "prettier --check packages/vscode"
  }
}
```

Keep the pinned pnpm version and only root lint/format dependencies actually
used by those scripts. `typescript` and `vitest` remain declared by
`packages/vscode/package.json`; remove duplicate root dependencies when pnpm
shows they are unused. Regenerate the lockfile, never hand-edit it:

```powershell
rtk pnpm install --lockfile-only
```

Expected: the importer list contains only the root and `packages/vscode`; no
Svelte, Vite, jsdom, `@xyflow/svelte`, or retired `@waml/*` package remains.

Slim `eslint.config.mjs` to lint the retained VS Code TypeScript surface and
root JavaScript configuration only; remove Svelte globals, retired package
globs, and WASM-generated-file exceptions. Remove retired package/build output,
Svelte, and generated WASM entries from `.prettierignore`; retain ignores that
still apply to Rust/native build artifacts or `packages/vscode/dist`.

- [ ] **Step 6: Rewrite cross-platform build and CI entry points**

Remove `wasm-pack` checks and `build:wasm` calls from `build.ps1`/`build.sh`.
Both scripts must run the same retained sequence:

```text
rtk pnpm install --frozen-lockfile
rtk cargo build --workspace
rtk pnpm build
optional: `rtk cargo test --workspace` and `rtk pnpm test`
optional: `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` and `rtk pnpm lint`
```

Update `.github/workflows/ci.yml` to install stable Rust without a wasm target
or wasm-pack, run `cargo fmt --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`pnpm build`, and `pnpm test`. Remove stale comments about generated WASM,
web-package build order, and browser deployment. Do not replace the deleted
Pages workflow with another deployment.

- [ ] **Step 7: Clean active guidance and backlog without rewriting history**

Rewrite `README.md` setup/architecture text around the Rust native editor,
CLI/LSP, and `packages/vscode`; remove web dev-server, Render, WASM, and retired
package commands. In `issues.md`, remove or resolve backlog findings whose
subject was the deleted Pages/web/WASM delivery path, change the reviewed
consumer inventory from `waml-wasm` to `waml-cli`/`waml-editor`, and retain
unrelated architectural findings.

Update `docs/superpowers/backlog/repo-hygiene-open-decisions.md` and
`docs/superpowers/backlog/drag-place-viz-threads.md` so open decisions no longer
name the retired Svelte packages, WASM bridge, generated bindings, or browser
deployment as live work. These two files are active backlog, not historical
plans. Preserve every completed record under `docs/superpowers/specs/` and
`docs/superpowers/plans/`.

Delete the listed active `docs/waml/` pages whose subject is the retired
Svelte/browser product. Remove their links and ordering entries from the
active architecture indexes/views listed above, including
`docs/waml/architecture/concepts/index.md`, and update
`system-context.md` to show the native editor plus CLI/LSP/VS Code only. Preserve
every file under `docs/superpowers/specs/` and `docs/superpowers/plans/`,
including completed web-era records.

- [ ] **Step 8: Run the green retirement scans**

```powershell
rtk rg -l "packages/(web|core|okf|wasm)|@waml/(web|core|okf|wasm)|waml-wasm|build:wasm|wasm-pack|tsify|wasm-bindgen|Svelte|\\.svelte" Cargo.toml crates package.json pnpm-workspace.yaml eslint.config.mjs .prettierignore build.ps1 build.sh scripts .github README.md issues.md docs/waml docs/superpowers/backlog
rtk rg --files -g "*.ts" -g "*.tsx" -g "*.svelte" -g "*.d.ts"
rtk rg -n "wasm =|feature = \"wasm\"|tsify|wasm_bindgen" crates/waml crates/waml-ops-dto
rtk proxy pwsh -NoProfile -Command '$removed = @("packages/web", "packages/core", "packages/okf", "packages/wasm", "crates/waml-wasm", "scripts/build-wasm.mjs", "scripts/gen-hero-hash.ts", "scripts/gen-template-bundles.mjs", "scripts/brand-web-artifact.mjs", "scripts/inject-runtime-shell.mjs", "scripts/inject-runtime-shell.test.mjs", "scripts/prune-web-fonts.mjs", ".github/workflows/pages.yml", "render.yaml", "docs/waml/wasm.md", "docs/waml/new-package-flow.md", "docs/waml/new-package-dialog.md", "docs/waml/model-store.md", "docs/waml/canvas-inner.md", "docs/waml/architecture/views/import-export-and-share.md", "docs/waml/architecture/views/github-pages-deployment.md", "docs/waml/architecture/concepts/workflows/exchange-and-sharing.md", "docs/waml/architecture/concepts/runtime/browser.md", "docs/waml/architecture/concepts/runtime/github-pages.md", "docs/waml/architecture/concepts/runtime/native-web-delivery.md", "docs/waml/architecture/concepts/runtime/share-recipient.md", "docs/waml/architecture/concepts/runtime/wasm-web-artifact.md"); $present = $removed | Where-Object { Test-Path -LiteralPath $_ }; if ($present) { Write-Error ("Retired paths still present: " + ($present -join ", ")); exit 1 }; "ALL_RETIRED_PATHS_ABSENT"'
```

Expected:

- the first and third scans return no matches;
- every path from the second scan is beneath `packages/vscode`;
- no active document links to a deleted `docs/waml/` page;
- native editor resources and Rust seed tests are still present.
- the explicit absence check prints exactly `ALL_RETIRED_PATHS_ABSENT`.

- [ ] **Step 9: Run the independent retirement gate**

```powershell
rtk cargo fmt --check
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk pnpm install --frozen-lockfile
rtk pnpm build
rtk pnpm test
```

Expected: PASS. This stage is independently releasable: the native Rust
workspace, CLI/LSP, and VS Code extension work before any first-class OKF model
change begins.

- [ ] **Step 10: Commit the retirement stage**

```powershell
rtk git add -A -- Cargo.toml Cargo.lock crates/waml/Cargo.toml crates/waml/src/diagnostic.rs crates/waml/src/frontmatter.rs crates/waml/src/model.rs crates/waml/src/okf.rs crates/waml/src/slug.rs crates/waml/src/syntax.rs crates/waml/src/solve/mod.rs crates/waml/tests/serde_shape.rs crates/waml-ops-dto/Cargo.toml crates/waml-ops-dto/src/lib.rs crates/waml-editor/src/card/mod.rs crates/waml-wasm
rtk git add -A -- package.json pnpm-workspace.yaml pnpm-lock.yaml eslint.config.mjs .prettierignore build.ps1 build.sh render.yaml packages/web packages/core packages/okf packages/wasm scripts/build-wasm.mjs scripts/gen-hero-hash.ts scripts/gen-template-bundles.mjs scripts/brand-web-artifact.mjs scripts/inject-runtime-shell.mjs scripts/inject-runtime-shell.test.mjs scripts/prune-web-fonts.mjs
rtk git add -A -- .github/workflows/ci.yml .github/workflows/pages.yml README.md issues.md docs/superpowers/backlog/repo-hygiene-open-decisions.md docs/superpowers/backlog/drag-place-viz-threads.md docs/waml/wasm.md docs/waml/new-package-flow.md docs/waml/new-package-dialog.md docs/waml/model-store.md docs/waml/canvas-inner.md docs/waml/architecture/index.md docs/waml/architecture/concepts/index.md docs/waml/architecture/views/index.md docs/waml/architecture/views/system-context.md docs/waml/architecture/views/import-export-and-share.md docs/waml/architecture/views/github-pages-deployment.md docs/waml/architecture/concepts/workflows/index.md docs/waml/architecture/concepts/workflows/exchange-and-sharing.md docs/waml/architecture/concepts/runtime/index.md docs/waml/architecture/concepts/runtime/browser.md docs/waml/architecture/concepts/runtime/github-pages.md docs/waml/architecture/concepts/runtime/native-web-delivery.md docs/waml/architecture/concepts/runtime/share-recipient.md docs/waml/architecture/concepts/runtime/wasm-web-artifact.md
rtk git commit -m "refactor: retire legacy web and WASM stack"
```

Before committing, inspect `rtk git diff --cached --name-status` and unstage
anything outside this task's file list. In particular, do not stage the
pre-existing modified mini fixture or unrelated untracked files.

### Task 3: Introduce Validated, Shared, Copy-on-Write Source Storage

**Files:**
- Create: `crates/waml/src/source.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/frontmatter.rs`
- Modify: `crates/waml/src/okf.rs`
- Modify: `crates/waml/src/parse.rs`
- Modify: `crates/waml/src/validate.rs`
- Modify: `crates/waml/src/index_md.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Modify: `crates/waml/src/share.rs`
- Modify: `crates/waml-editor/src/load.rs`
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/native_save.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/source_view.rs`

**Interfaces:**
- Produces:

```rust
pub struct BundlePath(String);

#[derive(Clone)]
pub struct SourceDocument {
    path: BundlePath,
    text: Arc<String>,
}

#[derive(Clone, Default)]
pub struct SourceBundle {
    documents: Vec<SourceDocument>,
    by_path: BTreeMap<BundlePath, usize>,
}

#[derive(Clone)]
pub struct SourceSlice {
    source: Arc<String>,
    range: Range<usize>,
}

impl SourceBundle {
    pub fn try_from_pairs<I, P, T>(pairs: I) -> Result<Self, SourceError>
    where
        I: IntoIterator<Item = (P, T)>,
        P: Into<String>,
        T: Into<String>;
    pub fn documents(&self) -> &[SourceDocument];
    pub fn document(&self, path: &BundlePath) -> Option<&SourceDocument>;
    pub(crate) fn document_mut(&mut self, path: &BundlePath) -> Option<&mut SourceDocument>;
    pub fn document_by_concept_id(&self, id: &str) -> Option<&SourceDocument>;
    pub fn to_pairs(&self) -> Vec<(String, String)>;
}

impl SourceDocument {
    pub fn path(&self) -> &BundlePath;
    pub fn text(&self) -> &str;
    pub(crate) fn text_mut(&mut self) -> &mut String;
    pub fn slice(&self, range: Range<usize>) -> Result<SourceSlice, SourceError>;
}
```

- Consumes: normalized bundle-relative Markdown paths and existing storage adapters.
- Later tasks rely on `SourceBundle: Clone` sharing every unchanged document's `Arc<String>`.

- [ ] **Step 1: Write path validation tests**

Cover slash normalization, case preservation, `.md` enforcement, absolute paths, drive prefixes, empty segments, `.`/`..`, duplicate normalized paths, and UTF-8:

```rust
#[test]
fn bundle_path_is_relative_case_preserving_and_slash_normalized() {
    let path = BundlePath::parse(r"Sales\Orders\Order.md").unwrap();
    assert_eq!(path.as_str(), "Sales/Orders/Order.md");
    assert_eq!(path.concept_id().unwrap(), "Sales/Orders/Order");
}

#[test]
fn traversal_and_absolute_paths_are_rejected() {
    for invalid in ["../order.md", "sales/../../order.md", "/order.md", "C:/order.md"] {
        assert!(BundlePath::parse(invalid).is_err(), "{invalid}");
    }
}
```

- [ ] **Step 2: Implement `BundlePath` and `SourceBundle` invariants**

Use private fields, `Display`/`AsRef<str>`, deterministic document order, and a `BTreeMap` index. `try_from_pairs` must normalize first and reject collisions before exposing a bundle. Do not silently drop or overwrite a duplicate.

- [ ] **Step 3: Write and implement `SourceSlice` boundary tests**

```rust
#[test]
fn source_slice_shares_allocation_and_validates_utf8_boundaries() {
    let doc = SourceDocument::new(
        BundlePath::parse("notes.md").unwrap(),
        "# Café\nBody".into(),
    );
    let body = doc.slice(8..12).unwrap();
    assert_eq!(body.as_str(), "Body");
    assert!(doc.slice(3..4).is_err());
    assert!(Arc::ptr_eq(doc.text_arc(), body.source_arc()));
}
```

Keep `text_arc`/`source_arc` `pub(crate)` or `#[cfg(test)]`; production consumers use `text()`/`as_str()`.

- [ ] **Step 4: Preserve the string serde shape**

Implement serde manually:

```rust
#[cfg(feature = "serde")]
impl serde::Serialize for SourceSlice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}
```

Deserialize a wire string into a new `Arc<String>` spanning the full allocation. Add JSON assertions that `Concept.body`, `Index.body`, and `Log.body` remain strings and that no `source` or `range` implementation fields escape. Do not add TypeScript-generation attributes: Task 2 removed that surface.

- [ ] **Step 5: Add a non-copying frontmatter span API**

Keep `parse_frontmatter(&str) -> (Frontmatter, String)` as a compatibility wrapper, and add:

```rust
pub struct ParsedFrontmatter {
    pub frontmatter: Frontmatter,
    pub body_range: Range<usize>,
}

pub fn parse_frontmatter_spanned(text: &str) -> ParsedFrontmatter;
```

The wrapper obtains `text[body_range].to_owned()`. New OKF parsing uses the range to construct `SourceSlice`.

- [ ] **Step 6: Migrate core readers to borrow `SourceBundle`**

Add `*_from_source` entry points first, then move `parse`, `validate`, index
reconciliation, lower-level operations, and `share.rs` bundle
encoding/decoding to iterate `SourceDocument`. Keep tuple wrappers only at
public compatibility boundaries. Avoid calling `to_pairs()` inside core
parsing or lowering because that would defeat the memory model.

- [ ] **Step 7: Migrate editor load/session/save adapters**

Change:

```rust
pub fn read_bundle(dir: &Path) -> Result<SourceBundle, LoadError>;
pub(crate) fn save_bundle_atomic(
    root: &Path,
    baseline: &SourceBundle,
    current: &SourceBundle,
) -> io::Result<()>;
```

`EditorSession::replace` receives `SourceBundle`; `mark_saved` clones the bundle
structure and therefore shares all document text. Update `app.rs`,
`doc_view.rs`, and `source_view.rs` to borrow source through the session rather
than owning or reconstructing tuple vectors. The retained
`crates/waml-cli/src/{commands.rs,io.rs,main.rs,lsp/}` tuple/DTO boundaries are
explicitly deferred to Task 9, where their public behavior is characterized
and migrated together.

- [ ] **Step 8: Prove copy-on-write behavior**

Add tests that compare `Arc` identities before and after clone, save, successful one-document edit, and failed edit:

```rust
let candidate = current.clone();
assert!(current.shares_text_with(&candidate, "a.md"));
assert!(current.shares_text_with(&candidate, "b.md"));

let mut edited = candidate;
let path = BundlePath::parse("a.md").unwrap();
*edited.document_mut(&path).unwrap().text_mut() = "# Changed\n".into();
assert!(!current.shares_text_with(&edited, "a.md"));
assert!(current.shares_text_with(&edited, "b.md"));
```

- [ ] **Step 9: Run the shared-source gate**

```powershell
rtk cargo test -p waml source::tests
rtk cargo test -p waml frontmatter::tests
rtk cargo test -p waml
rtk cargo test -p waml-editor load::tests
rtk cargo test -p waml-editor native_save::tests
rtk cargo test -p waml-editor editor_session::tests
rtk cargo check --workspace
```

Expected: PASS; no behavior changes beyond invalid source paths now returning construction errors.

- [ ] **Step 10: Commit the source-storage stage**

```powershell
rtk git add crates/waml/src/source.rs crates/waml/src/lib.rs crates/waml/src/frontmatter.rs crates/waml/src/okf.rs crates/waml/src/parse.rs crates/waml/src/validate.rs crates/waml/src/index_md.rs crates/waml/src/ops/mod.rs crates/waml/src/share.rs crates/waml-editor/src/load.rs crates/waml-editor/src/editor_session.rs crates/waml-editor/src/native_save.rs crates/waml-editor/src/app.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/source_view.rs
rtk git commit -m "refactor: share validated bundle source"
```

### Task 4: Build the First-Class OKF Bundle and Move Directory/Index Semantics Out of UML

**Files:**
- Modify: `crates/waml/src/okf.rs`
- Modify: `crates/waml/src/index_md.rs`
- Modify: `crates/waml/src/parse.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectoryAddress(String); // "/" or "/sales/orders"

pub struct Concept {
    pub id: String,
    pub ty: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub resource: Option<String>,
    pub tags: Vec<String>,
    pub timestamp: Option<String>,
    pub body: SourceSlice,
    pub links: Vec<Link>,
    pub citations: Vec<Citation>,
    pub extra: Frontmatter,
}

pub struct Index {
    pub directory: DirectoryAddress,
    pub title: Option<String>,
    pub description: Option<String>,
    pub members: Vec<String>,
    pub body: Option<SourceSlice>,
    pub authored: bool,
}

pub struct Log {
    pub directory: DirectoryAddress,
    pub body: SourceSlice,
}

pub struct Directory {
    pub address: DirectoryAddress,
    pub parent: Option<DirectoryAddress>,
    pub child_directories: Vec<DirectoryAddress>,
    pub concepts: Vec<String>,
}

pub struct Bundle {
    concepts: Vec<Concept>,
    indexes: Vec<Index>,
    logs: Vec<Log>,
    directories: Vec<Directory>,
}

impl Bundle {
    pub fn parse(source: &SourceBundle) -> Result<Self, BundleError>;
    pub fn concept(&self, id: &str) -> Option<&Concept>;
    pub fn index(&self, address: &str) -> Option<&Index>;
    pub fn log(&self, address: &str) -> Option<&Log>;
    pub fn directory(&self, address: &str) -> Option<&Directory>;
}
```

- Removes: `ConceptRole` and any internal path that places `index.md`/`log.md` into `Bundle::concepts`.
- Later tasks rely on deterministic hierarchy/member order and synthesized `Index { authored: false }`.

- [ ] **Step 1: Write separate-domain-type tests**

Construct a bundle containing root/nested Concepts, authored root/nested indexes, and logs. Assert exact collection membership and that no reserved ID resolves through `concept`.

- [ ] **Step 2: Implement rooted directory addresses**

`DirectoryAddress::parse` accepts `/` and normalized rooted subdirectories, rejects `..`, trailing file names, and non-rooted values. Add `parent()`, `join_directory()`, `concept_parent(id)`, and `index_path()` helpers so no operation hand-builds rooted/unrooted strings.

- [ ] **Step 3: Parse Concepts with zero-copy bodies**

Replace `okf::project(path, src)` with a source-document parser that:

- ignores reserved filenames;
- obtains `body_range` from `parse_frontmatter_spanned`;
- creates `SourceSlice` from the document allocation;
- preserves arbitrary/missing `type`;
- extracts current links, citations, H1 fallback, and unknown frontmatter exactly.

Keep a deprecated tuple/string wrapper only if a compatibility consumer still requires it.

- [ ] **Step 4: Parse authored Index and Log records**

Use existing `index_md` parsing rules for H1 title, intro/description, and link ordering. A Log retains directory identity and a full/body `SourceSlice`; detailed log-entry parsing remains absent.

- [ ] **Step 5: Derive the directory forest and synthesized indexes**

Create `/` unconditionally. Add parent directories implied by all source paths. For each directory without `index.md`, create:

```rust
Index {
    directory: address.clone(),
    title: None,
    description: None,
    members: default_member_order(&directory),
    body: None,
    authored: false,
}
```

Authored order lists known members first in authored order and appends omitted concepts/directories deterministically. Never create source while merely parsing.

- [ ] **Step 6: Reject duplicate Concept IDs**

After path normalization and before returning the Bundle, insert every Concept ID into a set. Return `BundleError::DuplicateConceptId { id, first_path, second_path }` on collision. Index/Log identities are checked separately by normalized source-path validation.

- [ ] **Step 7: Move reindexing to the OKF Bundle**

Refactor `index_md::reindex_bundle` to:

1. parse `okf::Bundle`;
2. derive directory membership/order without `parse::build_model`;
3. materialize one `index.md` per directory in a candidate `SourceBundle`;
4. preserve every Concept and Log allocation;
5. expose the old tuple function as a deprecated compatibility wrapper.

- [ ] **Step 8: Update the semantic serde shape**

Update native serde tests to expect:

```json
{
  "concepts": [],
  "indexes": [],
  "logs": [],
  "directories": []
}
```

Keep field names stable and body fields string-shaped. Remove generated `ConceptRole`.

- [ ] **Step 9: Run the OKF-core gate**

```powershell
rtk cargo test -p waml okf::tests
rtk cargo test -p waml index_md::tests
rtk cargo test -p waml --test serde_shape
rtk cargo check --workspace
```

Expected: PASS; all hierarchy/index tests run without constructing a UML `Model`.

- [ ] **Step 10: Commit the OKF Bundle stage**

```powershell
rtk git add crates/waml/src/okf.rs crates/waml/src/index_md.rs crates/waml/src/parse.rs crates/waml/src/lib.rs crates/waml/tests/serde_shape.rs
rtk git commit -m "feat: make OKF bundle first class"
```

### Task 5: Add the Selective UML Projection Facade

**Files:**
- Create: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/parse.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/solve/resolve.rs`
- Modify: `crates/waml/src/validate.rs`
- Modify: `crates/waml/tests/golden.rs`
- Modify: `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Produces:

```rust
pub type Projection = crate::model::Model;

pub fn recognizes(concept: &okf::Concept) -> bool;
pub fn project(bundle: &okf::Bundle) -> Projection;

impl Projection {
    pub fn contains_concept(&self, concept_id: &str) -> bool;
}
```

- Compatibility: `parse::build_model(&[(String, String)]) -> Model` remains temporarily and delegates through `SourceBundle -> okf::Bundle -> uml::project`.
- Removes from normal projection: `ElementType::Unknown` nodes and synthesized structural `Model::packages`.

- [ ] **Step 1: Replace characterization assertions with selective-projection tests**

The three Task 1 unknown-document tests now assert:

```rust
let source = SourceBundle::try_from_pairs(pairs).unwrap();
let bundle = okf::Bundle::parse(&source).unwrap();
let projection = uml::project(&bundle);
assert!(projection.nodes.is_empty());
assert!(projection.diagrams.is_empty());
assert!(bundle.concept("plain").is_some());
assert!(bundle.concept("vendor").is_some());
assert!(bundle.concept("future").is_some());
```

- [ ] **Step 2: Define the recognizer as the only claim rule**

Claim:

- every `ElementType::Uml(_)`, including explicitly authored `uml.Package`;
- every supported `ElementType::Behavior(_)`;
- current supported diagram documents (`type: Diagram`);

Return false for empty/missing type, arbitrary families, and `ElementType::Unknown`, including unknown `uml.*`.

- [ ] **Step 3: Rework parsed-document construction from Concepts**

Create the UML parser input only for claimed Concepts, using the Concept's source/body allocation and semantic metadata. Keep existing parsing of attributes, values, relationships, diagram layouts, flows, and interactions. Do not reparse Index/Log/Directory records as UML.

- [ ] **Step 4: Remove structural package synthesis from `Model` construction**

Delete `build_packages` from the normal projection path. Set legacy `Model::path`/`packages` only in the serde compatibility adapter if a retained CLI/LSP consumer still needs them during Task 9; editor navigation must stop reading them in Task 7.

Explicit `uml.Package` Concepts remain claimed UML elements and must be present in the normal UML element pool with their authored Concept identity.

- [ ] **Step 5: Restrict keysets and relationship resolution**

Build classifier/diagram member keysets only from claimed Concepts. Add tests proving:

- a supported classifier resolves;
- an unclaimed Concept does not resolve as a relationship target or diagram member;
- an unknown `uml.FutureThing` does not resolve;
- a supported diagram containing an unclaimed member degrades with the existing diagnostic/skip behavior and does not synthesize a node.

- [ ] **Step 6: Add editor-facing UML naming without a big-bang type rename**

Use `uml::Projection` and `uml_projection` in new APIs and editor fields. Retain `model::Model` and `parse::build_model` as deprecated compatibility names until Task 9. Do not mechanically rename every internal `Model` use in rendering/solver code.

- [ ] **Step 7: Update validation to distinguish unsupported OKF from malformed UML**

Arbitrary/missing `type` remains valid OKF and must not receive the old "unknown UML type" warning. Unknown `uml.*` may retain a UML-specific unsupported-metaclass diagnostic, but it remains unclaimed and openable as Generic OKF.

- [ ] **Step 8: Run the selective-UML gate**

```powershell
rtk cargo test -p waml uml
rtk cargo test -p waml parse::tests
rtk cargo test -p waml validate::tests
rtk cargo test -p waml --test golden
rtk cargo test -p waml --test serde_shape
rtk cargo check --workspace
```

Expected: PASS; no normal projection produces `ElementType::Unknown` or a structural directory package.

- [ ] **Step 9: Commit the UML projection stage**

```powershell
rtk git add crates/waml/src/uml.rs crates/waml/src/lib.rs crates/waml/src/parse.rs crates/waml/src/model.rs crates/waml/src/solve/resolve.rs crates/waml/src/validate.rs crates/waml/tests/golden.rs crates/waml/tests/serde_shape.rs
rtk git commit -m "refactor: derive selective UML projection"
```

### Task 6: Split Domain Lowerers and Route All Edits Through One Atomic Session Transaction

**Files:**
- Create: `crates/waml/src/edit.rs`
- Create: `crates/waml/src/compat.rs`
- Create: `crates/waml/src/okf/ops.rs`
- Create: `crates/waml/src/uml/ops.rs`
- Modify: `crates/waml/src/uml.rs` submodule declaration updates
- Modify: `crates/waml/src/okf.rs` submodule declaration updates
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Modify/remove after migration: `crates/waml/src/ops/pkg.rs`
- Modify/remove after migration: `crates/waml/src/ops/rename.rs`
- Modify: `crates/waml/src/ops/selector.rs`
- Modify: `crates/waml/src/index_md.rs`
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/diagram_properties.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`

**Interfaces:**
- Produces:

```rust
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf: &'a okf::Bundle,
    pub uml: &'a uml::Projection,
}

mod sealed {
    pub trait Sealed {}
}

pub trait EditBatch: sealed::Sealed {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError>;
}

pub struct PendingEdit(Box<dyn EditBatch>);

impl EditBatch for PendingEdit {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        self.0.lower(context)
    }
}

pub struct SessionChange {
    pub revision: u64,
    pub source_changed: bool,
    pub okf_changed: bool,
    pub uml_changed: bool,
    pub navigation_changed: bool,
    pub conflicts_changed: bool,
}

impl EditorSession {
    pub fn apply<B: EditBatch>(
        &mut self,
        batch: B,
    ) -> Result<SessionChange, EditError>;
}
```

- Domain batches:

```rust
pub struct okf::Batch(pub Vec<okf::Op>);
pub struct uml::Batch(pub Vec<uml::Op>);
```

- `ViewOutcome` produces `Option<PendingEdit>`, not `Vec<waml::ops::Op>`.
- Retained DTO/CLI code consumes:

```rust
#[doc(hidden)]
pub enum compat::Step {
    Okf(okf::Op),
    Uml(uml::Op),
}

#[doc(hidden)]
pub struct compat::Batch {
    steps: Vec<compat::Step>,
}

impl compat::Batch {
    pub fn new(steps: Vec<compat::Step>) -> Self;
    pub fn steps(&self) -> &[compat::Step];
}

pub fn compat::apply(
    source: &SourceBundle,
    batch: &compat::Batch,
) -> Result<SourceBundle, EditError>;
```

- [ ] **Step 1: Define exact OKF operation vocabulary and tests**

Use:

```rust
pub enum okf::Op {
    ConceptMove { id: String, to_directory: DirectoryAddress },
    DirectoryRename { directory: DirectoryAddress, name: String },
    DirectoryMove { directory: DirectoryAddress, to_parent: DirectoryAddress },
    DirectoryDelete { directory: DirectoryAddress, cascade: bool },
    IndexReorder { directory: DirectoryAddress, order: Vec<String> },
    IndexSort { directory: DirectoryAddress },
    IndexRetitle { directory: DirectoryAddress, title: String },
    BundleImport {
        parent: DirectoryAddress,
        name: String,
        bundle: SourceBundle,
    },
}
```

Tests must distinguish rename/move/retitle, reject root rename/move/delete, materialize missing indexes only when mutated, detect destination collision before mutation, and re-root imported source/link paths.

- [ ] **Step 2: Move package/index implementation into the OKF Lowerer**

Move behavior from `ops/pkg.rs` and `index_md.rs` without importing `model`, `ElementType`, or any UML type. Lower one ordered batch against one cloned `SourceBundle`; use `Arc::make_mut` only for touched documents.

- [ ] **Step 3: Define exact UML operation vocabulary**

Rename the current operations:

```rust
pub enum uml::Op {
    AttributeAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
    },
    AttributeSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: FieldEdit<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttributeRemove {
        node: String,
        name: String,
    },
    ValueAdd {
        node: String,
        literal: String,
    },
    ValueRemove {
        node: String,
        literal: String,
    },
    RelationshipAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelationshipSet {
        selector: Selector,
        ends: Option<(RelEnd, RelEnd)>,
        name: Option<NameSpec>,
    },
    RelationshipRemove {
        selector: Selector,
    },
    ClassifierNew {
        slug: String,
        directory: DirectoryAddress,
        ty: ElementType,
        title: String,
        stereotype: Vec<String>,
        description: Option<String>,
        abstract_: bool,
    },
    ClassifierSet {
        id: String,
        title: Option<String>,
        description: Option<String>,
        stereotype: Option<Vec<String>>,
        abstract_: Option<bool>,
        ty: Option<ElementType>,
    },
    ClassifierRemove {
        id: String,
        cascade: bool,
    },
    ClassifierRename {
        from: String,
        to: String,
    },
    DiagramSet {
        key: String,
        title: Option<String>,
        description: Option<String>,
        clear_description: bool,
        display: Option<DiagramDisplaySet>,
    },
    PlacementSet {
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        directions: Vec<Direction>,
    },
    PlacementRemove {
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
}
```

`ClassifierNew/Remove/Rename` compose private OKF source/path primitives instead of calling public OKF domain operations recursively.

- [ ] **Step 4: Move UML implementation into the UML Lowerer**

Move attribute/value/relationship/classifier/diagram/placement logic and selector code under `uml::ops`. Validate target Concepts against the current UML Projection when required, then rewrite the candidate source. Keep ordered-batch semantics and existing collision/link-cascade behavior.

- [ ] **Step 5: Implement the sealed Rust compatibility batch**

Implement `compat::Step`, `compat::Batch`, and `compat::apply` with the exact
interfaces above. Retain `waml::ops::Op` and `waml::ops::apply` as deprecated
Rust adapters that convert to this batch. Each step delegates to the relevant
domain lowering primitive against one candidate. Do not rebuild the complete
OKF/UML projections between steps. `compat::Batch` is the sole mixed-domain
type and exists only for the retained serde DTO/CLI contract; editor producers
must construct `okf::Batch` or `uml::Batch`.

Add a test with this ordered mixed batch:

```rust
[
    Op::PkgRetitle { path: "sales".into(), title: "Sales".into() },
    Op::NodeRename { from: "sales/order".into(), to: "purchase-order".into() },
    Op::PlaceSet {
        diagram: "sales/orders-diagram".into(),
        subject_title: "Purchase Order".into(),
        subject_slug: "sales/purchase-order".into(),
        reference_title: "Customer".into(),
        reference_slug: "sales/customer".into(),
        directions: vec![Direction::RightOf],
    },
]
```

Assert success is atomic and final projection rebuild counters equal one each. Add the same batch with a final collision and assert no state change.

- [ ] **Step 6: Implement `EditorSession::apply` as prepare-then-commit**

Use local candidates:

```rust
let candidate_source = batch.lower(EditContext {
    source: &self.source,
    okf: &self.okf,
    uml: &self.uml_projection,
})?;
let candidate_okf = okf::Bundle::parse(&candidate_source)?;
let candidate_uml = uml::project(&candidate_okf);

self.source = candidate_source;
self.okf = candidate_okf;
self.uml_projection = candidate_uml;
self.revision = self.revision.wrapping_add(1);
self.dirty_revision = Some(self.revision);
```

Do not mutate any `self` field until all candidate construction succeeds.

- [ ] **Step 7: Erase edits only at `ViewOutcome`**

Change producers to emit `PendingEdit::new(uml::Batch(...))`. Change `apply_view_outcome` to call `session.apply(pending)` once. Conflict deletion and shell-produced edits use the same helper. Generic OKF emits no edit in this feature.

- [ ] **Step 8: Prove transaction and memory invariants**

Test:

- failed lowering leaves all fields and all `Arc` identities unchanged;
- candidate OKF parse failure leaves state unchanged;
- success increments revision once and marks dirty;
- save snapshot shares current allocations;
- one touched document gets one new allocation;
- untouched documents remain shared;
- OKF and UML projection functions are invoked once per successful batch.

Use test-only projection counters or injected test builders rather than production global mutable counters.

- [ ] **Step 9: Run the operations/session gate**

```powershell
rtk cargo test -p waml okf::ops::tests
rtk cargo test -p waml uml::ops::tests
rtk cargo test -p waml ops::tests
rtk cargo test -p waml --test ops_golden
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor app::actions::tests
rtk cargo check --workspace
```

Expected: PASS; no editor mutation bypasses `EditorSession::apply`.

- [ ] **Step 10: Commit the lowering/session stage**

```powershell
rtk git add crates/waml/src/edit.rs crates/waml/src/compat.rs crates/waml/src/okf.rs crates/waml/src/okf/ops.rs crates/waml/src/uml.rs crates/waml/src/uml/ops.rs crates/waml/src/lib.rs crates/waml/src/ops crates/waml/src/index_md.rs crates/waml-editor/src/editor_session.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/diagram_properties.rs crates/waml-editor/src/app/actions.rs
rtk git commit -m "refactor: split OKF and UML lowerers"
```

### Task 7: Replace Kind-Based Host Dispatch with Static Document Providers

**Files:**
- Create: `crates/waml-editor/src/document.rs`
- Create: `crates/waml-editor/src/documents.rs`
- Create: `crates/waml-editor/src/uml_documents.rs`
- Create: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/doc_tabs.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/tree.rs`
- Modify: `crates/waml-editor/src/nav.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/accent.rs`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/source_view.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavCategory {
    Directory,
    OkfDocument,
    Class,
    Interface,
    Enum,
    DataType,
    Diagram,
    Behavior,
    Sequence,
    Note,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPresentation {
    pub icon: Icon,
    pub accent: Option<Vec4>,
    pub category: NavCategory,
}

pub struct OpenDocument {
    pub tab_id: LiveId,
    pub concept_id: String,
    pub title: String,
    pub presentation: DocumentPresentation,
    pub view: Box<dyn DocView>,
}

pub fn documents::open(
    bundle: &okf::Bundle,
    uml: &uml::Projection,
    concept_id: &str,
) -> Option<OpenDocument> {
    uml_documents::open(bundle, uml, concept_id)
        .or_else(|| okf_documents::open(bundle, concept_id))
}
```

- `DocumentHost` consumes prepared `OpenDocument`; it never matches on `NavCategory`, UML, OKF, classifier, diagram, or source.
- Navigator rows carry `openable: bool`, `concept_id: Option<String>`, and presentation; the tree panel does not infer behavior from category.

- [ ] **Step 1: Write provider precedence tests**

Cover:

```rust
assert!(uml_documents::open(&bundle, &projection, "order").is_some());
assert!(okf_documents::open(&bundle, "order").is_some());
assert_eq!(
    documents::open(&bundle, &projection, "order").unwrap().tab_id,
    uml_document_tab_id("order")
);

assert!(uml_documents::open(&bundle, &projection, "runbook").is_none());
assert_eq!(
    documents::open(&bundle, &projection, "runbook").unwrap().tab_id,
    okf_document_tab_id("runbook")
);
```

Also assert Index/Log IDs return `None` from the Generic provider.

- [ ] **Step 2: Introduce presentation-only navigator categories**

Replace `TreeKind` with `NavCategory` only where labels/icons/filters need a closed editor-local vocabulary. Remove `Unknown`; unclaimed Concepts receive `OkfDocument`. Add `openable` and context capability flags to row presentation so neither `tree_panel` nor `DocumentHost` switches on category.

- [ ] **Step 3: Build the navigator from `okf::Bundle`**

Change:

```rust
pub fn build_tree(
    bundle: &okf::Bundle,
    uml: &uml::Projection,
    root_fallback: &str,
) -> ProjectTree;
```

Walk `Directory.child_directories` and `Directory.concepts` in Index order. Decorate claimed IDs through `uml_documents::presentation`; decorate unclaimed IDs through `okf_documents::presentation`. Use Index title/description for directory rows. Do not read `Model::packages` or `Model::path`.

- [ ] **Step 4: Refactor tab state around prepared identity**

Remove `TabKind` and `node_kind` from `DocTab`:

```rust
pub struct DocTab {
    pub id: LiveId,
    pub concept_id: String,
    pub title: String,
    pub presentation: DocumentPresentation,
    pub preview: bool,
}
```

Change `OpenTabs::open_preview` to accept a prepared tab record. Preserve stable-ID deduplication, shared preview replacement, promotion, close fallback, activation, and source/document distinctness.

- [ ] **Step 5: Delete the host factory**

Remove `make_view`. `DocumentCommand::Open` carries `OpenDocument` and persistence intent. `DocumentHost` inserts the supplied `view` under `tab_id` while moving the supplied identity/presentation into `OpenTabs`. Source opening also receives a prepared `OpenDocument` from `okf_documents::open_source`.

Add a source scan gate:

```powershell
rtk rg "make_view|TabKind|match .*NavCategory|match .*TreeKind" crates/waml-editor/src/document_host.rs crates/waml-editor/src/doc_tabs.rs
```

Expected: no host factory or semantic kind dispatch matches.

- [ ] **Step 6: Route tree opening through the composition root**

`ProjectTreeAction::OpenDocument` carries only `concept_id` and `persistent`. `App::handle_tree_document_open` calls `documents::open(session.okf(), session.uml_projection(), &id)` and hands the resulting `OpenDocument` to the host.

UML-only context menus are enabled by provider-produced row capabilities; Generic OKF rows never set classifier-edit/delete capabilities.

- [ ] **Step 7: Reconcile titles without model-kind matching**

After a session change, the shell resolves current non-source Concept IDs through `documents::open` and gives provider-produced titles/presentation to a host reconciliation method. The host compares IDs/presentation and replaces a live view only when the provider-supplied `tab_id` changed; it performs no family match.

Source tabs retain source identity and receive title updates through `okf_documents::open_source`.

- [ ] **Step 8: Run the static-composition gate**

```powershell
rtk cargo test -p waml-editor document::tests
rtk cargo test -p waml-editor documents::tests
rtk cargo test -p waml-editor uml_documents::tests
rtk cargo test -p waml-editor okf_documents::tests
rtk cargo test -p waml-editor tree::tests
rtk cargo test -p waml-editor nav::tests
rtk cargo test -p waml-editor tree_panel::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor document_host::tests
rtk cargo check -p waml-editor
```

Expected: PASS; the host and tab state contain no concrete document-view factory.

- [ ] **Step 9: Commit the provider/host stage**

```powershell
rtk git add crates/waml-editor/src/document.rs crates/waml-editor/src/documents.rs crates/waml-editor/src/uml_documents.rs crates/waml-editor/src/okf_documents.rs crates/waml-editor/src/main.rs crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/document_host.rs crates/waml-editor/src/tree.rs crates/waml-editor/src/nav.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/accent.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/source_view.rs
rtk git commit -m "refactor: compose prepared document providers"
```

### Task 8: Add Generic OKF Markdown Documents and Mixed-Bundle Startup

**Files:**
- Create: `crates/waml-editor/src/generic_okf_view.rs`
- Create: `crates/waml-editor/src/markdown_surface.rs`
- Create: `crates/waml-editor/tests/fixtures/mixed-okf/index.md`
- Create: `crates/waml-editor/tests/fixtures/mixed-okf/order.md`
- Create: `crates/waml-editor/tests/fixtures/mixed-okf/orders-diagram.md`
- Create: `crates/waml-editor/tests/fixtures/mixed-okf/runbook.md`
- Create: `crates/waml-editor/tests/fixtures/okf-only/index.md`
- Create: `crates/waml-editor/tests/fixtures/okf-only/notes.md`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/app.rs` Makepad script IDs and `open_bundle`
- Modify: `crates/waml-editor/src/load.rs`
- Modify: `crates/waml-editor/src/cli.rs`
- Modify: `crates/waml-editor/src/nav.rs`

**Interfaces:**
- Produces:

```rust
pub struct GenericOkfView {
    concept_id: String,
}

impl DocView for GenericOkfView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>);
    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome;
    fn chrome(&self) -> BodyChrome;
}
```

- `ViewData` supplies `source`, `okf`, `uml`, and `revision`.
- Shared widget/body APIs use `markdown_surface`, `show_markdown`, and `set_markdown`; no reusable helper is called `source_view`.

- [ ] **Step 1: Add dedicated fixture bundles without modifying `tests/fixtures/mini`**

`mixed-okf/runbook.md`:

```markdown
---
type: vendor.Runbook
title: Order Recovery
tags: [operations]
---

# Order Recovery

Restart the order processor, then verify the queue depth.
```

`okf-only/notes.md` omits `type` entirely. The mixed fixture also contains one recognized classifier and diagram so both providers are exercised in one native window.

- [ ] **Step 2: Rename the shared surface to Markdown-neutral IDs**

Rename script/widget IDs and body methods:

```rust
pub fn markdown_surface(&self, cx: &mut Cx) -> WidgetRef;
pub fn show_markdown(&self, cx: &mut Cx);
pub fn set_markdown(&self, cx: &mut Cx, markdown: &str);
```

Move the visibility/text mechanics into `markdown_surface.rs`. Update `SourceView` with no intentional visual or chrome change.

- [ ] **Step 3: Implement `GenericOkfView`**

`sync`:

```rust
let markdown = data
    .okf
    .concept(&self.concept_id)
    .map(|concept| Cow::Borrowed(concept.body.as_str()))
    .unwrap_or_else(|| {
        Cow::Owned(format!("*No source for `{}`*", self.concept_id))
    });
body.show_markdown(cx);
body.set_markdown(cx, markdown.as_ref());
```

`handle` returns `ViewOutcome::default()`. `chrome` returns `BodyChrome::HIDDEN`, which hides tool dock, view bar, canvas overlays, and right inspector. `tab_accent` uses the stable Generic OKF presentation accent supplied by the provider.

- [ ] **Step 4: Add stable Generic OKF identity tests**

Assert for one Concept:

```rust
assert_ne!(okf_document_tab_id("runbook"), uml_document_tab_id("runbook"));
assert_ne!(okf_document_tab_id("runbook"), source_tab_id("runbook"));
assert_eq!(okf_document_tab_id("runbook"), okf_document_tab_id("runbook"));
```

Open Generic OKF and explicit Source tabs for the same Concept and assert both can persist simultaneously.

- [ ] **Step 5: Add Generic view behavior tests**

Test semantic body rendering, missing-source fallback, no emitted edits, hidden chrome, and absence of canvas synchronization. Keep SourceView's right-inspector behavior unchanged.

- [ ] **Step 6: Implement initial document selection**

Replace the diagram-only startup branch with:

```rust
pub enum InitialDocument<'a> {
    Diagram(&'a str),
    Concept(&'a str),
    None,
}
```

Selection order:

1. requested supported UML diagram;
2. first supported UML diagram;
3. first navigable Concept in OKF Index/tree order;
4. no tab for an empty Bundle.

Resolve the selected ID through `documents::open`, so an OKF-only Bundle gets a Generic preview.

- [ ] **Step 7: Extend navigation filters and actions**

Add `NavCategory::OkfDocument` with the exact filter label `"OKF"`, its
icon/accent, and filter ordering. Generic rows single-click preview,
double-click persist, allow explicit View Source, and expose no
classifier-only context action.

- [ ] **Step 8: Run the Generic OKF/editor gate**

```powershell
rtk cargo test -p waml-editor generic_okf_view::tests
rtk cargo test -p waml-editor markdown_surface::tests
rtk cargo test -p waml-editor okf_documents::tests
rtk cargo test -p waml-editor source_view::tests
rtk cargo test -p waml-editor nav::tests
rtk cargo test -p waml-editor doc_tabs::tests
rtk cargo test -p waml-editor load::tests
rtk cargo test -p waml-editor cli::tests
rtk cargo test -p waml-editor
```

Expected: PASS; mixed and OKF-only fixtures open without touching the modified mini fixture.

- [ ] **Step 9: Commit the Generic OKF stage**

```powershell
rtk git add crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/markdown_surface.rs crates/waml-editor/tests/fixtures/mixed-okf crates/waml-editor/tests/fixtures/okf-only crates/waml-editor/src/main.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/okf_documents.rs crates/waml-editor/src/app.rs crates/waml-editor/src/load.rs crates/waml-editor/src/cli.rs crates/waml-editor/src/nav.rs
rtk git commit -m "feat: open generic OKF documents"
```

### Task 9: Migrate Retained Serde DTO, CLI, LSP, and VS Code Surfaces

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-cli/src/lsp/bundle.rs`
- Modify: `crates/waml-cli/src/lsp/map.rs`
- Modify: `crates/waml-cli/src/lsp/server.rs`
- Modify: `crates/waml/tests/serde_shape.rs`
- Test/verify unchanged contract: `packages/vscode/src/extension.ts`
- Test/verify unchanged contract: `packages/vscode/src/serverPath.ts`
- Test: `packages/vscode/src/serverPath.test.ts`

**Interfaces:**
- Keeps every current serialized `OpDto` tag, `v` default/version check, field name, nullable-field behavior, and JSON round trip required by the Rust CLI.
- Produces:

```rust
impl OpDto {
    pub fn to_compat_step(&self) -> Result<waml::compat::Step, String>;
    pub fn from_compat_step(step: &waml::compat::Step) -> Self;
}

pub fn to_batch(dtos: &[OpDto]) -> Result<waml::compat::Batch, String>;
```

- `run_apply` validates source with `SourceBundle::try_from_pairs`, converts the entire DTO list once with `to_batch`, invokes `waml::compat::apply` once, and writes returned source pairs only after success.
- LSP continues to start as `waml lsp --stdio`; the VS Code extension continues to use `vscode-languageclient` and does not gain a WASM or bundled-language-core fallback.

- [ ] **Step 1: Write DTO ownership and wire-preservation tests**

Keep the existing exhaustive JSON round-trip fixture and add assertions that
each legacy wire operation maps to the design's owning domain:

```rust
assert!(matches!(
    pkg_retitle.to_compat_step().unwrap(),
    waml::compat::Step::Okf(waml::okf::Op::IndexRetitle { .. })
));
assert!(matches!(
    attr_add.to_compat_step().unwrap(),
    waml::compat::Step::Uml(waml::uml::Op::AttributeAdd { .. })
));
```

Cover every mapping:

- `pkg.move` -> `ConceptMove`;
- legacy `pkg.rename` -> `DirectoryRename` or `DirectoryMove` according to its existing field semantics;
- `pkg.delete/reorder/sort/retitle/insert` -> `DirectoryDelete`, `IndexReorder`, `IndexSort`, `IndexRetitle`, `BundleImport`;
- attribute/value/relationship/node/diagram/place tags -> the renamed UML variants.

Expected initial failure: `to_compat_step`, `from_compat_step`, and `to_batch`
do not exist.

- [ ] **Step 2: Implement DTO-to-compat conversion without a new product enum**

Replace `to_op`/`from_op` internals with the exact compatibility APIs above.
Keep deprecated `to_op`/`from_op` wrappers only if a retained Rust caller still
uses them after `rtk rg "to_op\\(|from_op\\(" crates packages/vscode`; remove
them if that scan finds no caller. Wire-only `Pkg*`, `Node*`, `Attr*`, and
`Place*` names remain isolated to `OpDto` and its tests.

- [ ] **Step 3: Prove mixed DTO order and atomic failure**

Add a test whose serialized DTO vector alternates:

```text
pkg.retitle -> node.rename -> place.set
```

Convert the full vector with `to_batch`, apply it once to a `SourceBundle`, and
assert the success result preserves source-order semantics. Repeat with a final
destination collision; assert the error includes the original DTO index and
the input bundle's paths, text, and `Arc` identities remain unchanged.

- [ ] **Step 4: Update Rust serde shape assertions**

Assert with `serde_json`:

- every `SourceSlice` body is a JSON string;
- private `source`/`range` fields and `ConceptRole` are absent;
- `Bundle` contains separate `concepts`, `indexes`, `logs`, and `directories`;
- the UML compatibility projection contains no unclaimed Concepts or
  structural directory packages;
- normal projection produces no `ElementType::Unknown`;
- every legacy DTO tag/version/nullable field retains its existing wire
  spelling.

Do not add generated declarations, tsify attributes, or TypeScript mirror
types.

- [ ] **Step 5: Migrate CLI mutation commands to `SourceBundle`**

In `commands.rs`, `io.rs`, and `main.rs`, keep filesystem discovery and output
formatting at the adapter boundary. Change `run_apply`/`run_mutation` so they:

1. read disk files into `SourceBundle::try_from_pairs`;
2. parse the OKF Bundle and UML projection once for validation;
3. convert all input `OpDto` records with `to_batch`;
4. call `waml::compat::apply` once;
5. write changed files only after the full batch succeeds.

Add CLI tests for invalid traversal input, duplicate normalized paths, mixed
OKF/UML success, late collision rollback, and unchanged JSON diagnostic
format.

- [ ] **Step 6: Migrate LSP bundle overlays without changing transport**

Change `lsp/bundle.rs` to build a validated `SourceBundle` from disk plus
open-buffer overlays while preserving its existing normalized-key precedence.
Change `lsp/map.rs`/`server.rs` only where the OKF/UML split changes diagnostic
inputs. Add tests proving arbitrary/missing-type OKF Concepts do not receive an
unknown-UML diagnostic, supported UML errors still map to the same URI/range,
and repeated `--stdio` remains accepted.

- [ ] **Step 7: Verify the independent VS Code client contract**

Keep `extension.ts` using `vscode-languageclient` with
`TransportKind.stdio`. In `serverPath.test.ts`, assert both the default
executable name `waml` and an explicit `waml.serverPath` still produce the Rust
server command; assert no code imports a removed `@waml/*` package or WASM
binding.

- [ ] **Step 8: Run retained compatibility gates**

```powershell
rtk cargo test -p waml-ops-dto
rtk cargo test -p waml-cli
rtk cargo test -p waml --test serde_shape
rtk cargo check --workspace
rtk pnpm --filter @waml/vscode build
rtk pnpm --filter @waml/vscode test
```

Expected: PASS; the CLI/LSP and extension retain their wire/launch behavior,
and no JavaScript/WASM compatibility layer exists.

- [ ] **Step 9: Commit the retained compatibility stage**

```powershell
rtk git add crates/waml-ops-dto/src/lib.rs crates/waml-cli/src crates/waml/tests/serde_shape.rs packages/vscode/src/extension.ts packages/vscode/src/serverPath.ts packages/vscode/src/serverPath.test.ts
rtk git commit -m "refactor: migrate retained OKF compatibility"
```

### Task 10: Run Full Automated and Native Visual Verification

**Files:**
- Modify: none expected; a failure returns to the owning Task 2-9 commit and is fixed/tested there before this gate restarts.
- Create execution evidence outside the repository under `C:\tmp\first-class-okf-verification\`.

**Interfaces:**
- Consumes: the complete first-class OKF implementation.
- Produces: automated, static-architecture, memory, and native visual evidence for the success criteria.

- [ ] **Step 1: Format and run focused package suites**

```powershell
rtk cargo fmt --check
rtk cargo test -p waml
rtk cargo test -p waml-editor
rtk cargo test -p waml-ops-dto
rtk cargo test -p waml-cli
```

Expected: PASS.

- [ ] **Step 2: Run workspace and lint gates**

```powershell
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: PASS with no warnings.

- [ ] **Step 3: Run the retained VS Code and retirement gates**

```powershell
rtk pnpm install --frozen-lockfile
rtk pnpm build
rtk pnpm test
rtk rg --files -g "*.ts" -g "*.tsx" -g "*.svelte" -g "*.d.ts"
rtk rg -l "packages/(web|core|okf|wasm)|@waml/(web|core|okf|wasm)|waml-wasm|build:wasm|wasm-pack|tsify|wasm-bindgen|Svelte|\\.svelte" Cargo.toml crates package.json pnpm-workspace.yaml eslint.config.mjs .prettierignore build.ps1 build.sh scripts .github README.md issues.md docs/waml docs/superpowers/backlog
rtk proxy pwsh -NoProfile -Command '$removed = @("packages/web", "packages/core", "packages/okf", "packages/wasm", "crates/waml-wasm", "scripts/build-wasm.mjs", "scripts/gen-hero-hash.ts", "scripts/gen-template-bundles.mjs", "scripts/brand-web-artifact.mjs", "scripts/inject-runtime-shell.mjs", "scripts/inject-runtime-shell.test.mjs", "scripts/prune-web-fonts.mjs", ".github/workflows/pages.yml", "render.yaml", "docs/waml/wasm.md", "docs/waml/new-package-flow.md", "docs/waml/new-package-dialog.md", "docs/waml/model-store.md", "docs/waml/canvas-inner.md", "docs/waml/architecture/views/import-export-and-share.md", "docs/waml/architecture/views/github-pages-deployment.md", "docs/waml/architecture/concepts/workflows/exchange-and-sharing.md", "docs/waml/architecture/concepts/runtime/browser.md", "docs/waml/architecture/concepts/runtime/github-pages.md", "docs/waml/architecture/concepts/runtime/native-web-delivery.md", "docs/waml/architecture/concepts/runtime/share-recipient.md", "docs/waml/architecture/concepts/runtime/wasm-web-artifact.md"); $present = $removed | Where-Object { Test-Path -LiteralPath $_ }; if ($present) { Write-Error ("Retired paths still present: " + ($present -join ", ")); exit 1 }; "ALL_RETIRED_PATHS_ABSENT"'
```

Expected: pnpm build/tests pass; every TypeScript path is under
`packages/vscode`; the final scan returns no matches; the absence check prints
exactly `ALL_RETIRED_PATHS_ABSENT`.

- [ ] **Step 4: Run static architecture scans**

```powershell
rtk rg "ConceptRole" crates packages
rtk rg "ElementType::Unknown" crates/waml/src/parse.rs crates/waml/src/uml.rs
rtk rg "build_model" crates/waml/src/index_md.rs crates/waml/src/okf
rtk rg "TabKind|make_view" crates/waml-editor/src/document_host.rs crates/waml-editor/src/doc_tabs.rs
rtk rg "TreeKind::Unknown|NavCategory::.*=>" crates/waml-editor/src/document_host.rs
rtk rg "Package" crates/waml/src/okf.rs crates/waml/src/okf crates/waml/src/source.rs
rtk rg "apply_ops|Vec<waml::ops::Op>" crates/waml-editor/src
```

Expected:

- no `ConceptRole`;
- no normal unknown-node projection;
- no OKF index operation depending on UML model construction;
- no host/tab factory dispatch;
- no host category dispatch;
- no native editor edit path bypassing `EditorSession::apply`;
- `"Package"` appears in OKF/source code only inside tests that reject the terminology or compatibility comments, not domain types/operations.

- [ ] **Step 5: Build and launch the native editor**

```powershell
rtk cargo build -p waml-editor
rtk proxy pwsh -NoProfile -Command "New-Item -ItemType Directory -Force -Path 'C:\tmp\first-class-okf-verification' | Out-Null"
```

Launch the worktree-built executable against the dedicated fixtures without stopping any user-owned editor process. Record the launched PID and keep display scale/window dimensions fixed across captures.

- [ ] **Step 6: Capture the required native states**

Use:

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out C:\tmp\first-class-okf-verification\mixed-navigator.png -Process waml-editor
```

Capture separate files for:

1. UML diagram;
2. UML classifier preview;
3. Generic OKF document;
4. mixed navigator with UML and OKF rows;
5. OKF-only startup with first Concept previewed;
6. explicit Source view for the Generic Concept;
7. switching between persisted UML, Generic OKF, and Source tabs;
8. empty Bundle with no tab.

Verify native pixel dimensions are consistent and Generic OKF has no tool dock, view bar, canvas overlay, or right inspector.

- [ ] **Step 7: Exercise failure and persistence paths manually**

In a disposable copy of the fixture:

- attempt a destination collision and confirm no source/tab/nav partial update;
- retitle a synthesized nested Index and confirm `index.md` materializes;
- save a dirty native session and confirm only touched source text changes;
- reopen and confirm UML/Generic provider selection and tab titles are stable.

Do not edit the repository fixture during this manual pass.

- [ ] **Step 8: Inspect final diff and unrelated changes**

```powershell
rtk git status --short
rtk git diff --stat
rtk git diff --check
```

Expected: the pre-existing unrelated paths remain present and unmodified by this work; `git diff --check` reports no whitespace errors.

## Completion Criteria

- `SourceBundle` is validated, source-authoritative, and copy-on-write.
- `okf::Bundle` has separate Concept, Index, Log, and Directory collections with rooted Index lookup.
- UML is derived only from recognized Concepts; unclaimed Concepts and structural directories are absent from the projection.
- OKF and UML operations have separate vocabularies and Lowerers.
- One sealed `EditorSession::apply` atomically commits source, OKF, UML, revision, and dirty state.
- `DocumentHost` accepts prepared documents and contains no family/kind factory dispatch.
- Mixed and OKF-only Bundles expose clickable Generic OKF rows and Markdown-only tabs.
- Existing UML editing, diagram rendering, View Source, tabs, persistence, and chrome behavior remain green.
- The legacy Svelte product, TypeScript domain/state packages, Rust WASM bridge, generated bindings, web build scripts, and deployment workflows are absent.
- TypeScript remains only in `packages/vscode`, which builds/tests and continues to launch `waml lsp --stdio`.
- `waml-ops-dto` and serde preserve the retained Rust CLI wire contract while exposing the new OKF Bundle shape.
- Active README/build/CI/backlog/architecture guidance describes only retained products; completed `docs/superpowers` records remain intact.
- Focused, workspace, clippy, VS Code, static architecture, and native visual gates all pass.
