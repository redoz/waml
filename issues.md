# WAML codebase issues

Reviewed 2026-07-31 against local `main` at
`c61484ac250569eb722e19e2ce3a348003e08b75`.

This document tracks active issues only. Completed items from the 2026-07-26
review were removed. In particular, native edits now use a real save path,
`EditorSession` owns in-memory transactions and savepoint identity, `DocView`
is a real lifecycle boundary, the old monolithic `GraphCanvas` is split, and
the legacy duplicate parser authority is gone.

Priority meanings are calibrated for a hobby project:

- **P1:** can lose or hide user work, report false success, or produce stale
  workspace state;
- **P2:** verified correctness, scalability, portability, or ownership debt
  that should be fixed when working in the affected area;
- **P3:** a hypothesis or hygiene item that needs evidence before substantial
  work.

## Executive judgment

WAML has strong foundations: lossless syntax preservation, defensive
incremental parsing, typed models, atomic in-memory edits, prepared/commit
undo, savepoint-aware history compaction, bounded view history, broad semantic
tests, and useful fuzz targets.

The main risks are now at subsystem boundaries rather than in the core parser
or model. Diagnostics are not aggregated across parsing layers, LSP disk
authority becomes stale, one input format can discard authored bytes, and
native multi-file persistence has weaker guarantees than CLI persistence.
Delivery automation does not stop these defects from reaching the web build.

The order below is intentional. Fix user-work and data-authority defects first.
Measure performance before adding more incremental-parser complexity. Treat
the remaining architectural cleanup as work that should make future features
more fun, not as an emergency rewrite.

## P1 — Shell and frontmatter diagnostics disappear at public boundaries

Evidence:

- `crates/waml-syntax/src/shell.rs:27-32` produces diagnostics such as
  `FrontmatterNotClean`, `MissingFrontmatterFence`, and
  `MalformedFrontmatterEntry`.
- `crates/waml-cli/src/commands.rs:166-182` reports only UML diagnostics.
- `crates/waml-cli/src/lsp/bundle.rs:268-296` publishes only UML diagnostics.
- `crates/waml/src/validate.rs:14-17,37-45` can turn analysis failure into an
  empty diagnostic result.

Verified malformed frontmatter fixtures can produce shell diagnostics while
`waml check --format json` returns `[]` with exit code 0. The parser recovers
correctly, but the user-facing layers report false success.

Recommendation:

1. Make `PreparedCandidate` own one provenance-bearing aggregate diagnostic
   stream: shell diagnostics first, followed by OKF and specialization
   diagnostics.
2. Make CLI, LSP, editor, and public validation consume only this aggregate.
3. Return an error separately from an empty valid diagnostic set.
4. Run the same malformed fixture through every adapter and assert identical
   code, file, range, and provenance.

## P1 — LSP disk and workspace authority becomes stale

These failures have the same owner: the LSP has no complete filesystem
lifecycle.

Evidence:

- `crates/waml-cli/src/lsp/bundle.rs:53-70` captures startup disk bytes.
- `crates/waml-cli/src/lsp/bundle.rs:191-210` restores those cached bytes when
  a document closes.
- `crates/waml-cli/src/lsp/server.rs:127-194` implements open, change, and
  close, but not save or watched-file changes.
- `crates/waml-cli/src/lsp/server.rs:70-90` publishes diagnostics only for the
  current URI set, so a removed URI does not receive an empty publication.

Consequences:

- open → edit → save to disk → close restores startup bytes instead of the
  saved bytes;
- Git changes, generated files, and edits in another program do not update
  unopened documents;
- diagnostics can remain in the client after a document is removed.

Recommendation:

1. Advertise and implement save synchronization. Refresh disk authority from
   saved text or a checked disk read.
2. Register Markdown watched files and reconcile create, change, and delete
   while preserving open-buffer overlays.
3. Diff the previous and current published URI sets and send `diagnostics: []`
   for removed URIs.
4. Test the complete lifecycle with multi-document semantic dependencies.

## P1 — Bundle-envelope autodetection can discard authored bytes

Evidence:

- `crates/waml/src/source.rs:6-8` accepts a line-shaped HTML comment ending in
  `.md` as a bundle marker.
- `crates/waml/src/source.rs:24-34` starts the first decoded document after the
  first marker and does not preserve or reject the preceding bytes.
- `crates/waml-cli/src/io.rs:12-23` trusts this detection for normal input.

An ordinary Markdown document containing a comment such as
`<!-- something.md -->` can be interpreted as a bundle. All content before the
comment is silently omitted from analysis and serialization. The marker also
handles LF and CRLF differently.

Recommendation:

1. Use an explicit, versioned bundle-envelope sentinel.
2. Until that format exists, require the first marker at byte zero and reject
   a non-blank preamble instead of discarding it.
3. Test ordinary HTML comments, comments in fenced code, malformed markers,
   non-empty preambles, and both LF and CRLF.

## P1 — Native and CLI persistence have different transaction guarantees

Evidence:

- `crates/waml-editor/src/native_save.rs:23-33` rejects removed paths.
- `crates/waml-editor/src/native_save.rs:77-103` replaces pending files one at
  a time. A late operational failure can leave earlier files at the new
  revision and later files at the old revision.
- `crates/waml-cli/src/io.rs:237-400` independently implements a journal,
  rollback, additions, updates, and deletions.

The old fake-save defect is fixed. The remaining issue is that two persistence
authorities provide different guarantees. Structural in-memory operations can
represent changes that the native save path cannot commit, and recovery logic
must be maintained twice.

Recommendation:

1. Establish one filesystem transaction implementation shared by CLI and the
   native editor.
2. Support baseline conflict checks, add/update/delete, journaled rollback,
   deterministic recovery reporting, and tested replace-existing behavior.
3. Keep caller-specific policy, paths, and UI messages outside the transaction
   core.
4. Test failures after each transaction phase over a multi-file bundle.
5. Add an editor-level test that opens a temporary bundle, edits it, saves it,
   reloads it from disk, and compares source plus semantic state.

## P2 — Pages deployment is not gated by repository verification

Evidence:

- `.github/workflows/ci.yml:3-10` disables push and pull-request triggers;
  only manual dispatch remains.
- `.github/workflows/pages.yml:3-10` deploys pushes to `main` without running
  the workspace test and lint gate.
- `crates/waml-editor/Cargo.toml:23` pins `makepad-widgets` to
  `c38f529984eda61e258ca69fb50c6712d85c74c1`, while
  `.github/workflows/pages.yml:55` installs `cargo-makepad` from
  `25d78a4d917ea2e943df6af5e037817248443bd7`.

The two Makepad revisions are adjacent, and the review did not demonstrate a
current tool/framework incompatibility. The defect is that the documented pin
invariant is manual and already textually false. More importantly, a push can
fail Rust tests and still replace the web editor.

Recommendation:

1. Restore push and pull-request CI.
2. Make Pages depend on a reusable verification job for the exact commit.
3. Include formatting, workspace tests, strict Clippy, extension checks,
   runtime-shell tests, fuzz-target compilation, and a bounded seed smoke.
4. Define or verify the Makepad revision invariant in one machine-checked
   place. Do not infer incompatibility only from different hashes.

## P2 — Tab and navigation state does not have one identity/lifecycle policy

Several verified defects come from the same missing boundary.

Evidence:

- Manual tab selection creates a location with `ViewAnchor::None` at
  `crates/waml-editor/src/app/actions.rs:836`; class synchronization then clears
  selection and may refit the camera.
- `crates/waml-editor/src/document_host.rs:77` promotes the first tab matching a
  concept subject. It can pin a source tab instead of the active primary
  preview for the same concept.
- `crates/waml-editor/src/app.rs:1021-1124` commits Back/Forward traversal before
  deferred anchor restoration finishes.
- `crates/waml-editor/src/document_host.rs:301-313` retains an old live tab when
  its locator no longer resolves after rename or deletion.

Consequences include lost per-tab camera/selection, an edited preview that can
still be replaced, rapid traversal overwriting history with stale anchors, and
tabs that disagree with the current model.

Recommendation:

1. Cache a `ViewAnchor` per live tab and restore it after target synchronization.
2. Promote the exact active tab identity, not the first matching subject.
3. Commit navigation only after successful generation-tagged restoration, or
   block another traversal while restoration is pending.
4. Choose and test an explicit close-versus-tombstone policy for unresolved
   tabs, including undo restoration.

## P2 — Repeated configuration writes are not replace-existing portable

Evidence:

- `crates/waml-editor/src/config.rs:62-67` writes `editor.json.tmp` and then
  calls `std::fs::rename` over the destination.
- On Windows, rename does not provide replace-existing semantics for an
  existing destination.
- Existing tests cover the first write into an empty directory but not two
  consecutive stores.

Theme, recent-file, or pin changes can therefore stop persisting after the
first successful configuration write on a primary target platform.

Recommendation:

Reuse the tested platform replacement primitive from the shared persistence
work. Add a test that stores two different values consecutively and reloads the
second value on Windows.

## P2 — Bundle ingestion has three filesystem authorities and follows links

Evidence:

- CLI ingestion: `crates/waml-cli/src/io.rs:28-42`.
- Native-editor ingestion: `crates/waml-editor/src/load.rs:44-59`.
- LSP ingestion: `crates/waml-cli/src/lsp/bundle.rs:338-357`.
- LSP ingestion silently skips directory-entry, read, and UTF-8 failures in
  `crates/waml-cli/src/lsp/bundle.rs:338-358`.

All three recurse independently with `path.is_dir()`. Directory symlinks or
junctions can escape the selected root or form cycles, and error policy already
differs across adapters.

Recommendation:

Create one host-ingestion API with explicit no-follow or containment policy,
visited-directory identity tracking, deterministic ordering, size/encoding
policy, and structured per-path errors. Do not silently omit unreadable or
non-UTF-8 Markdown. Add platform-gated link-cycle, root-escape, and file-load
error tests.

## P2 — Incremental syntax still feeds bundle-wide semantic work

The former duplicate-parser issue is complete. There is one shell parser
authority, OKF uses the shared syntax snapshot and structure map, and UML
specializes recognized islands with incremental reuse and safe fallback.

The active performance issue is narrower:

- `crates/waml/src/okf/shell.rs:208-237` traverses all documents during OKF
  projection and still scans selected prose for links.
- `crates/waml/src/uml/analysis.rs:161-260` revisits every claimed concept.
- `crates/waml/src/uml/analysis.rs:509-510,596-860,1095-1103` rebuilds global
  validation and projection.
- The document lookup around `crates/waml/src/uml/analysis.rs:166-186` is
  quadratic in concepts × documents.

Recommendation:

1. Remove the quadratic lookup by carrying `DocumentId` provenance or using a
   catalog index.
2. Add repeatable 1, 100, and 1,000-document edit benchmarks.
3. Only if measurements justify it, retain per-document semantic records and
   rebuild changed indexes and cross-document resolution.
4. Do not add another parser, title authority, or speculative invalidation
   framework.

## P2 — `App` remains the persistence and global-shell coordinator

`EditorSession` now owns current/persisted source, analyses, revisions,
history, and atomic edit publication. `DocumentHost` owns tabs and live views.
The original P1 “App owns everything” issue is therefore complete in substance.

The residual issue is that `App` still coordinates storage roots, save timers,
save-error policy, recents, navigation, overlays, chrome, and global routing in
`crates/waml-editor/src/app.rs:650-743`.

Recommendation:

Extract a small workspace-backing or persistence coordinator first. Continue
to extract only where one component can own a complete policy and be tested
without widgets. Do not split `app.rs` mechanically while retaining shared
mutable authority.

## P2 — The class-diagram view still reaches through the canvas facade

The old 4,353-line `GraphCanvas` no longer exists. Viewport, geometry,
selection, placement interaction, class interaction, hit testing, render
snapshots, and rendering passes now have separate modules.

Residual evidence:

- `crates/waml-editor/src/class_diagram_view.rs:248,287,338,354` directly
  borrows the concrete typed widget.
- `crates/waml-editor/src/class_diagram_view.rs:517-520,625-658` depends on
  canvas camera and placement constants and performs placement preview solving.

Recommendation:

Move zoom commands, zone identities, placement-candidate preparation, and
preview results behind typed surface commands and outcomes. Keep popup
presentation in the view. Do not restart a broad canvas decomposition project.

## P2 — Operation and public-API boundaries amplify changes

These two historical issues now meet at the transitional compatibility layer.

Evidence:

- Canonical domain operations are correctly split between
  `crates/waml/src/okf/ops.rs` and `crates/waml/src/uml/ops.rs`.
- `crates/waml/src/compat.rs:47-239` still has an exhaustive legacy-to-domain
  conversion.
- `crates/waml-ops-dto/src/lib.rs:398-674` and following code separately map
  every wire operation in both directions.
- `crates/waml/src/lib.rs:3-25` publicly exposes about 20 major modules plus a
  hidden-but-public compatibility module.
- `crates/waml/src/solve/mod.rs:8-15` publicly exposes solver implementation
  topology.

Separate domain and wire contracts are correct, and round-trip tests are worth
keeping. The active debt is hand-maintained mapping boilerplate and the lack of
a named supported facade.

Recommendation:

1. Decide whether `compat` is supported or transitional; `#[doc(hidden)]` does
   not seal a public API.
2. Define deliberate parse, analyze, edit, validate, and solve entry points.
3. Reduce module visibility only after actual CLI/editor consumers use the
   facade.
4. Consider a declarative operation mapping only if it preserves explicit wire
   spelling, version metadata, golden JSON, and round-trip tests.

## P3 — A combined navigation and editor-action batch may drop the final edit

Status: **hypothesis; not yet reproduced**.

Evidence:

- Navigation handlers run before `ActiveDocumentView` in
  `crates/waml-editor/src/app/actions.rs:30`.
- The dispatch loop returns after the first consumed handler at
  `crates/waml-editor/src/app/actions.rs:71`.
- Properties focus loss and a description change can be emitted in one action
  pass from `crates/waml-editor/src/diagram_properties.rs:595`.
- `EditIntent` does not carry its originating tab or document identity.

If Makepad batches a final text or IME change with the click that navigates,
navigation might consume the batch before the active view observes the edit.

Recommendation:

First create one synthetic action batch containing a description change and
tree or tab navigation. Promote this issue to P1 only if the edit is lost.

## P3 — Structural undo entries may retain excessive bundle metadata

Evidence:

- `crates/waml/src/edit/reversible.rs:34-59` stores complete before and after
  `SourceBundle` values for document-set or ordering changes.
- Document text is shared through `Arc<String>`, so these are shallow snapshots
  rather than full text copies.
- `crates/waml/src/edit/reversible.rs:98-106` reports structural snapshot text
  storage as zero and does not measure retained bundle metadata or shared text
  lifetimes.

The 1,024-entry bound limits entry count. Repeated structural edits could still
retain `O(history × documents)` metadata, but the practical cost has not been
measured.

Recommendation:

Measure retained allocations under realistic large-bundle structural edits.
Use path-level insert, remove, move, or reorder deltas only if the measurements
justify the added complexity.

## What should not be “fixed”

- Do not split `icons.rs` only because it is about 4,500 lines. It is typed
  catalog data with one narrow responsibility.
- Do not replace typed enums with strings or dynamic maps to reduce mapping
  work.
- Do not introduce traits for every module. The valuable seams are ownership,
  lifecycle, persistence, and transaction boundaries.
- Do not chase general clone elimination without a profile. The known scale
  costs are bundle-wide projection and structural-history retention.
- Do not weaken semantic, lossless, incremental-oracle, golden, authority, or
  round-trip tests.
- Do not add a second parser or independent frontmatter/title authority.
- Do not decompose files only by size. Split when one component can own and
  test a coherent policy.

## Sequenced improvement roadmap

1. Aggregate diagnostics and make all adapters report the same failures.
2. Complete LSP save, watched-file, and diagnostic-removal
   lifecycles.
3. Make bundle-envelope recognition explicit and lossless.
4. Unify filesystem transactions, add the editor save/reload test, and fix
   native deletion/rollback and
   Windows configuration replacement.
5. Restore required CI and gate Pages on the verified commit.
6. Consolidate tab identity, per-tab anchors, preview promotion, and deferred
   history restoration. Reproduce or dismiss the combined-action hypothesis.
7. Centralize safe filesystem ingestion and structured load errors.
8. Measure structural undo retention and semantic edit costs; remove the
   quadratic UML lookup; optimize only when the data warrants it.
9. Clean the remaining App, canvas, compatibility, operation-mapping, and
    public-API boundaries as feature work reaches them.

This order protects user work first, makes automated feedback truthful, then
improves lifecycle and scale. It intentionally leaves the parser’s extra
complexity in place where that complexity provides losslessness, recovery,
provenance, and useful experimentation.
