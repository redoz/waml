# Rust codebase review

Reviewed 2026-07-26. Scope: the Rust workspace only (`crates/*`); the Svelte/TypeScript front end was explicitly excluded.

## Executive judgment

The core is substantially better than the median young Rust codebase. Crates have intelligible purposes, data structures are typed, operations are atomic in memory, malformed source is generally preserved rather than destroyed, and the test suite is unusually broad. On this checkout, the full `cargo test --workspace` suite passes, as does strict `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

The code is nevertheless becoming expensive to change. The main risk is not low-level Rust quality; it is that product policy, state transitions, parsing/projection, and drawing behavior are accumulating in a few enormous files. A change can compile and have excellent local tests while still missing one of several orchestration paths or one target-specific behavior. The disabled CI gate makes that risk operational rather than theoretical.

The order below is intentional. Fix the product/data and delivery hazards before performing aesthetic decomposition.

## P0 — Native edits are accepted, shown, and then silently never persisted

Evidence:

- `crates/waml-editor/src/app.rs:1114` starts a save timer after an edit.
- `crates/waml-editor/src/app.rs:1124` presents `save` as persistence “by whatever means this build has.”
- The native implementation at `crates/waml-editor/src/app.rs:1151` is an empty function.
- The nearby comment explicitly says drag-to-place edits remain in memory only on desktop.

This is worse than a missing feature because the rest of the state machine communicates success: the operation is applied, the model is rebuilt, the canvas updates, conflicts update, and a save is scheduled. Closing the application loses the work without an error or persistent dirty indication.

Recommendation:

1. Until durable saving exists, disable native authoring or put the application in an explicit read-only mode. Do not schedule a fake save.
2. Implement a storage boundary that writes changed documents through temporary sibling files plus atomic rename where the platform permits it.
3. Keep the opened directory and an on-disk revision/fingerprint; reject or reconcile external changes instead of overwriting them.
4. Make `save_backend` return a result and retain dirty state on failure. Surface the failure in the UI.
5. Add an integration test: open a temporary bundle, apply an operation, save, reload from disk, and compare the semantic model.

## P0 — The only general quality gate is disabled, while publishing still runs on every push to `main`

Evidence:

- `.github/workflows/ci.yml:3` says CI is temporarily disabled.
- Its only trigger is `workflow_dispatch`; push and pull-request triggers are commented out.
- That workflow contains the relevant cross-platform `cargo test --workspace` job.
- `.github/workflows/pages.yml` still builds and deploys the editor on every push to `main`.

Local tests being healthy today does not substitute for a gate. The repository can publish a commit that never ran the Rust tests, Clippy, or the Windows matrix. The comment already records that cross-platform parser bugs have reached `main` before.

Recommendation:

1. Restore pull-request and `main` push triggers immediately. If the combined Rust/Node workflow is flaky, split it; do not disable the Rust gate with it.
2. Require the Rust check before merging and make deployment depend on the tested commit.
3. Run at least `cargo test --workspace` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` on Linux and Windows.
4. Add `cargo fmt --check` explicitly rather than relying on incidental cleanliness.

## P1 — `App` is the transaction manager, persistence coordinator, router, view registry, and global UI controller

Evidence:

- `crates/waml-editor/src/app.rs` is 2,993 lines.
- `App` owns the raw bundle, resolved model, tabs, heterogeneous views, navigation state, recents, persistence timer, popups, agent state, and much of the chrome.
- `handle_actions` spans the product’s global action policy.
- The edit transaction “apply ops → replace bundle → rebuild whole model → resolve active view → sync conflicts → mark dirty” appears independently around `app.rs:2035` and `app.rs:2518`.
- Opening a bundle, switching a diagram, live-edit rehydration, platform startup, saving, and popup outcomes all mutate overlapping parts of the same state.

This is the classic point where adding a feature becomes shotgun surgery. The duplicated transaction is already evidence: a future concern such as undo history, validation, navigation refresh, dirty-file tracking, telemetry, or save-error handling must be remembered in every path. Rust’s borrow checker cannot protect an omitted policy step.

Recommendation:

Introduce a small, non-UI `EditorSession` (or `WorkspaceSession`) that owns:

- source bundle and storage origin;
- resolved model and active revision;
- `apply_ops` as the single mutation transaction;
- dirty/save state;
- validation/result reporting.

Have it return a typed `SessionChange` describing what invalidated (model, active diagram, navigation, source tab, persistence). `App` should translate those changes into widget updates; it should not implement document transactions itself. Extract startup/storage and popup/chrome coordination separately after that seam exists.

Do not begin by mechanically splitting `app.rs` into files while leaving `App` as shared mutable gravity. That changes navigation, not changeability.

## P1 — `DocView` is a façade rather than an ownership boundary

Evidence:

- `crates/waml-editor/src/doc_view.rs:133` says each live view owns synchronization and action handling, but `App::sync_active_tab` still special-cases `TabKind::Source` to feed source text.
- `App` downcasts `dyn DocView` to `ClassDiagramView` both to push the active diagram identity and to refresh the scene after a model mutation.
- `body_chrome` and `tab_accent` construct throwaway views instead of querying the registered live view.
- Adding a view therefore still requires shell-side type knowledge and synchronization choreography even though dispatch is expressed through trait objects.

The abstraction hides concrete types syntactically without transferring authority. In practice it behaves like an indirect tagged union: the application pays for allocation and dynamic dispatch while central branching remains the real extension mechanism.

Recommendation:

1. Give each registered view all immutable tab identity and source data it needs to synchronize itself; remove the source-tab branch from `App`.
2. Replace concrete downcasts with trait-level lifecycle and post-mutation hooks owned by the live view.
3. Query chrome and accent metadata from the registered live view rather than constructing temporary views.
4. Retain only outcome channels with real producers and consumers. Remove the unused `open_preview` and `open_right_dock` relay scaffolding until behavior needs them.
5. Keep model mutation and popup placement in the shell/session boundary, but let the active view decide how a successful mutation invalidates and refreshes its presentation.

## P1 — `GraphCanvas` combines too many reasons to change

Evidence:

- `crates/waml-editor/src/canvas.rs` is 4,353 lines even before its current uncommitted edits.
- It contains camera and pinch behavior, hit testing, selection, node and edge rendering, marker geometry, group chrome, constraint visualization, placement-dial policy, conflict focus, animation, Makepad widget plumbing, and a large test module.
- `class_diagram_view.rs` reaches into canvas constants, zone conversion, widget internals, conflict preview policy, camera commands, and actions. The supposed view/canvas boundary is therefore porous.

Rendering math, interaction state, authoring policy, and framework integration evolve for different reasons and should be independently testable. Today a placement-language change and an antialiasing change collide in the same high-churn file and object. That raises merge conflict rate and makes regression review depend on understanding thousands of unrelated lines.

Recommendation:

Keep `GraphCanvas` as a thin widget adapter and extract cohesive, framework-light units:

- `CanvasController`: input event to typed intent/action;
- `CameraController`: pan, zoom, fit, glide, pinch;
- `SelectionState` and hit-test index;
- `PlacementDial`: zones, preview, authored operation;
- `SceneRenderer`: ordered drawing passes;
- `edge_geometry` / `marker_geometry`;
- `constraint_overlay`.

The boundary should pass immutable scene data plus explicit state, not expose Makepad widget borrows and canvas constants to `class_diagram_view`.

## P1 — One document is parsed/projection-scanned twice, and the duplicate semantics are maintained by comment

Evidence:

- `crates/waml/src/parse.rs:511` calls `parse_document(text)` for every bundle entry.
- Immediately afterward, `parse_bundle` calls `okf::project(path, text)`.
- `okf::project` parses frontmatter again, runs a second Markdown pass to find the first H1, and separately scans links and citations.
- `okf.rs` says its H1 extraction “mirrors `parse::parse` ... byte-for-byte.” That is a synchronization obligation, not a durable invariant.
- `parse.rs` is 2,193 lines and mixes syntax parsing, semantic projection, reference resolution, package discovery, class diagrams, activities/state machines, sequences, and instance construction.

The immediate performance cost is avoidable repeated work on every full-model rebuild. More importantly, the semantic cost is two interpretations of title/frontmatter/body that can drift. Every editor operation currently rebuilds the entire model, magnifying both costs.

Recommendation:

Create one parsed source record per file containing frontmatter, body ranges, heading facts, links/citations, syntax sections, and diagnostics. Derive both `Concept` and WAML semantic models from that record. Make title precedence a single function, not duplicated algorithms with a promise in prose.

Then split semantic builders by substrate (`class`, `package`, `activity`, `sequence`, `instance`) behind a `ModelBuilder` with shared resolution indexes. This is a more valuable split than separating arbitrary line ranges.

Only pursue incremental rebuilds after the single-pass representation exists and profiling shows a need. A clean full rebuild is preferable to a clever invalidation graph built over duplicated parsing.

## P1 — The deployed Makepad tool revision contradicts the manifest pin and its own maintenance comment

Evidence:

- `crates/waml-editor/Cargo.toml` pins `makepad-widgets` to revision `ec009e50`.
- `.github/workflows/pages.yml` installs `cargo-makepad` at revision `9147a9a0`.
- The workflow comment says this is “Pinned to the same makepad rev as `crates/waml-editor/Cargo.toml`” and says the tool and framework share an ABI.

Either the revisions are intentionally different, in which case the comment and upgrade procedure are false, or they are unintentionally different, in which case the production build uses an untested tool/framework pairing. Both make the next upgrade hazardous.

Recommendation:

Define the Makepad revision once and have both dependency resolution and build tooling consume it, or add a small CI script that verifies the two pins. If tool and framework genuinely require different commits, name both explicitly and document the compatibility pair rather than saying they are the same revision.

## P2 — Adding an operation requires synchronized edits across multiple exhaustive representations

Evidence:

- The domain operation enum and dispatcher live in `crates/waml/src/ops/mod.rs`.
- The wire operation enum, `to_op`, and `from_op` live separately in the 1,077-line `crates/waml-ops-dto/src/lib.rs`.
- A new operation therefore requires at least four coordinated changes: domain variant, domain dispatch, DTO variant, and both conversion directions, plus tests and any CLI surface.
- The round-trip tests are good, but they detect forgotten mapping only after all variants compile; they do not reduce the number of places that encode the same schema.

Some separation between domain and wire contracts is correct—wire compatibility should not be an accidental derive. The problem is the hand-maintained boilerplate and monolithic files, not the existence of a DTO.

Recommendation:

First split operations into cohesive modules with their command types and handlers, leaving one public `Op` sum type. Then consider a small declarative macro or generated mapping table that emits the DTO enum/conversions while still requiring explicit version and field metadata. Keep golden JSON tests so wire spelling remains deliberate.

Avoid collapsing the wire type directly into the domain type merely to remove code; that would trade visible duplication for protocol coupling.

## P2 — The core crate exposes implementation topology as its public API

Evidence:

- `crates/waml/src/lib.rs` publicly exposes every major module: parser internals, syntax tree, grammar helpers, model, solver internals, operations, OKF projection, serialization, and validation.
- `solve/mod.rs` publicly exposes geometry, potentials, resolution, routing, sizing, and stress modules.
- The package version is `0.0.0`, so there is no useful compatibility signal for consumers.

This makes refactoring expensive because callers can bind to implementation-shaped entry points. It also obscures the intended happy path: parse a bundle, validate it, apply operations, solve a diagram. Broad visibility is convenient during rapid development but becomes an architectural mortgage.

Recommendation:

Inventory actual consumers (`waml-cli`, `waml-wasm`, and `waml-editor`), define a small facade of supported types and workflows, and make the remaining modules private or `pub(crate)`. Give the crate a real version once the facade is named. Keep syntax/lossless editing APIs public only if they are deliberately supported as a separate layer.

## What should not be “fixed”

- Do not break up `icons.rs` merely because it is 4,444 lines; it is catalog-like data and has a narrow reason to change. Generated or declarative bulk is different from a 4,000-line stateful controller.
- Do not replace typed enums with strings or dynamic maps to reduce edit count.
- Do not introduce traits for every module. The valuable seams here are ownership and transaction boundaries, not abstraction density.
- Do not chase clone elimination without a profile. Repeated whole-document parsing and rebuilding is the meaningful performance/design issue; ordinary boundary clones are mostly clarity costs, not proven bottlenecks.
- Do not weaken the strong semantic and golden tests. They are the codebase’s best asset.

## Suggested sequence

1. Make native editing honestly read-only or durably saved.
2. Restore required Rust CI and gate deployment/merge.
3. Correct and centralize the Makepad compatibility pins.
4. Introduce `EditorSession::apply_ops` and route every edit through it.
5. Establish the canvas/controller/rendering boundaries.
6. Unify the per-document parse/projection pass, then split semantic builders.
7. Reduce operation mapping boilerplate and narrow the public facade.

That sequence reduces user risk first, then creates seams that make the larger refactors safe. It avoids the common failure mode of spending weeks rearranging modules while persistence and delivery remain untrustworthy.
