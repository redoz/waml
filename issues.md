# WAML codebase issues

Reviewed 2026-07-31 against local `main` at
`c61484ac250569eb722e19e2ce3a348003e08b75`. Reconciled 2026-08-04 against
`c31fdc51` after the seven-dimension review
(`docs/reviews/2026-08-04/SUMMARY.md`), which fixed 46 of its 52 findings in
the range `a24f03eb..c31fdc51`.

This document tracks active issues only. Completed items from the 2026-07-26
review were removed. In particular, native edits now use a real save path,
`EditorSession` owns in-memory transactions and savepoint identity, `DocView`
is a real lifecycle boundary, the old monolithic `GraphCanvas` is split, and
the legacy duplicate parser authority is gone.

Removed in the 2026-08-04 reconciliation:

- Bundle-envelope autodetection discarding authored bytes: the explicit
  versioned envelope v1 codec landed (`crates/waml/src/bundle_envelope.rs`);
  `source.rs` no longer autodetects comment markers. A narrower follow-up is
  tracked below (part markers matched by substring).
- Pages deployment not gated: `pages.yml` now calls `ci.yml` via
  `workflow_call` and the build/deploy jobs require it; PR CI is restored,
  cargo-makepad and the framework are pinned to the same revision, all
  `scripts/*.test.mjs` suites run on PRs, and a bounded weekly fuzz job exists.
- The `compat.rs` transitional layer: retire-compat is implemented
  (`56fdf772` deleted `waml::ops` and the `compat.rs` bridge; design at
  `docs/superpowers/specs/2026-08-04-retire-compat-design.md`). The residual
  public-API issue below was rewritten accordingly.
- Config rename not replace-existing on Windows: dismissed as false —
  `std::fs::rename` uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` (and
  POSIX-semantics rename on newer Windows), the temp file is written in the
  destination directory so cross-volume failure cannot occur, and the new
  `store_to_twice_second_value_wins` test locks in second-write-wins.
  Residual transient sharing-violation failures are already logged and
  swallowed by callers.

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
authority becomes stale, and native multi-file persistence has weaker
guarantees than CLI persistence. Delivery automation now gates the web build
on the full verification suite.

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

Partial progress (2026-08-04): hard shell *failures* now quarantine the
offending document and are surfaced instead of failing or silently passing the
bundle (`fa7eb34a`). Recoverable shell/frontmatter diagnostics still bypass the
CLI and LSP outputs — `commands.rs::diagnostics` reads only
`candidate.uml().diagnostics`.

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

The quadratic concepts × documents catalog lookup is fixed: `d30af731` indexes
the catalog by concept id in UML analyze.

Recommendation:

1. Add repeatable 1, 100, and 1,000-document edit benchmarks.
2. Only if measurements justify it, retain per-document semantic records and
   rebuild changed indexes and cross-document resolution.
3. Do not add another parser, title authority, or speculative invalidation
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

The transitional `compat.rs` layer is gone: retire-compat deleted `waml::ops`
and the bridge (`56fdf772`), and legacy op tests were ported onto Step/Batch.
The residual debt is narrower.

Evidence:

- Canonical domain operations are correctly split between
  `crates/waml/src/okf/ops.rs` and `crates/waml/src/uml/ops.rs`.
- `crates/waml-ops-dto/src/lib.rs` still hand-maps every wire operation in
  both directions.
- `crates/waml/src/lib.rs:3-23` publicly exposes about 20 major modules with
  no named supported facade.
- `crates/waml/src/solve/mod.rs` publicly exposes solver implementation
  topology.

Separate domain and wire contracts are correct, and round-trip tests are worth
keeping. The active debt is hand-maintained mapping boilerplate and the lack of
a named supported facade.

Recommendation:

1. Define deliberate parse, analyze, edit, validate, and solve entry points.
2. Reduce module visibility only after actual CLI/editor consumers use the
   facade.
3. Consider a declarative operation mapping only if it preserves explicit wire
   spelling, version metadata, golden JSON, and round-trip tests.

## P2 — Incremental reparse still does Θ(document) work per edit

Deferred from the 2026-08-04 review (P-4 remainder). The unconditional
per-keystroke oracle parse is fixed (`165470e2` made the hot incremental
reparse path oracle-free), but the incremental path still performs several
whole-document passes per edit.

Evidence:

- `crates/waml-syntax/src/incremental.rs` rebuilds the old shell map, walks
  reference-map guards, and runs restore/preserve traversals over the full
  tree on every edit, not just the reparse window.
- `crates/waml-syntax/src/markdown/snapshot.rs` reconstructs source and walks
  the tree bundle-wide during snapshot promotion.

Recommendation:

Carry the needed state (shell map, reference map, preservation indexes) on the
snapshot across edits instead of recomputing it from the whole document, so
per-edit cost is proportional to the edited window.

## P2 — `editor_session.rs` is a god object testable only whole

Deferred from the 2026-08-04 review (M-1/T-9).

Evidence:

- `crates/waml-editor/src/editor_session.rs` is 3,417 lines and sits at the
  editor's centre.
- Its behaviour is exercised only through 44 whole-session tests; no policy
  inside it can be tested in isolation.

Recommendation:

Extract components only where one can own a complete policy (history
compaction, savepoint identity, publication) and be tested without the rest of
the session. Do not split mechanically by size. This depends on the `lib.rs`
seam issue below.

## P2 — waml-editor has no library seam for cross-module tests

Deferred from the 2026-08-04 review (M-5/T-4). This is the structural cause of
the `editor_session.rs` issue above and of the untestable draw-path residue
noted by the review (T-3).

Evidence:

- `crates/waml-editor/src/lib.rs` is 2 lines and exports 2 modules.
- `main.rs` declares ~80 flat modules private to the binary, so cross-module
  editor behaviour has no integration-test seam; `tests/` can only reach the
  two exported modules.

Recommendation:

Move the module tree into the library crate (keeping the binary a thin shim)
so editor behaviour is reachable from `tests/`. Do this as enabling work when
next touching editor structure, not as a standalone churn project.

## P2 — Telemetry seam has no export bridge and no in-editor consumer

Follow-ups to the tracing telemetry seam that landed in `8eba08ca`
(`crates/waml-editor/src/telemetry.rs`, review O-3).

Evidence:

- Events go to an in-process ring buffer; there is no OTLP (or any external)
  export bridge, so nothing persists or aggregates telemetry.
- `telemetry::recent_events()` exists and is ready to feed a UI, but no
  in-editor log panel consumes it; failures are still invisible in the GUI
  beyond the statusbar messages.

Recommendation:

1. Add an OTLP export bridge behind the existing `tracing` seam.
2. Add an in-editor log panel consuming `telemetry::recent_events()`.

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

## P3 — Bundle-envelope part markers are matched by substring, not line-anchored

Deferred from the 2026-08-04 review (C-4).

Evidence:

- `crates/waml/src/bundle_envelope.rs:256` finds part markers by substring
  search, so a hand-edited envelope with a marker-shaped string mid-line can
  mis-split.
- The encoder legitimately emits mid-line markers for bodies that do not end
  in a newline, so simple line anchoring would reject valid encoder output.

A proper fix needs a wire-format change: make the separator newline mandatory
(version bump), then line-anchor the decoder. Machine-produced envelopes are
unaffected today; only hand-authored envelopes can mis-split.

## P3 — Edge router remains per-edge O(N²) grid plus A*

Deferred from the 2026-08-04 review (P-3 follow-up). The indexing work in
`d4c174bf` made the router about 2.4× faster, but each edge still builds its
own obstacle grid and runs A* independently.

A true shared-visibility-graph rewrite (build once per solve, mask per edge)
would change attachment geometry and is only worth doing if large diagrams
become a real workload. Measure wall-clock on realistic diagrams before
scheduling it.

## P3 — Eight indistinguishable `document*`/`doc*` sibling modules

Deferred from the 2026-08-04 review (M-11).

`crates/waml-editor/src/` contains `doc_tabs.rs`, `doc_view.rs`,
`document.rs`, `document_header.rs`, `document_host.rs`, `documents.rs`, and
related siblings whose names do not communicate their ownership boundaries.
Rename or regroup when the editor module tree moves behind the library seam;
do not rename in isolation.

## P3 — Thin direct unit coverage of UML parser internals

Deferred from the 2026-08-04 review (T-10).

`crates/waml/src/uml/syntax/parser.rs` and `uml/analysis.rs` are covered
almost entirely by integration-shaped tests. This is a debugging-cost issue,
not a shipping risk — the integration net is broad. Add direct unit tests
opportunistically when debugging in these files; the extent of the gap is
inferred, not measured by a coverage run.

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
3. Unify filesystem transactions, add the editor save/reload test, and fix
   native deletion/rollback and
   Windows configuration replacement.
4. Consolidate tab identity, per-tab anchors, preview promotion, and deferred
   history restoration. Reproduce or dismiss the combined-action hypothesis.
5. Centralize safe filesystem ingestion and structured load errors.
6. Measure structural undo retention, semantic edit costs, and incremental
   reparse per-edit costs; optimize only when the data warrants it.
7. Open the waml-editor library seam, then chip at `editor_session.rs` and
   the draw-path decision logic behind it.
8. Wire the telemetry seam to an exporter and an in-editor log panel.
9. Clean the remaining App, canvas, operation-mapping, and public-API
   boundaries as feature work reaches them.

This order protects user work first, makes automated feedback truthful, then
improves lifecycle and scale. It intentionally leaves the parser’s extra
complexity in place where that complexity provides losslessness, recovery,
provenance, and useful experimentation.
