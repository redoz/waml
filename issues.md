# WAML codebase issues

Reviewed 2026-07-31 against local `main` at
`c61484ac250569eb722e19e2ce3a348003e08b75`. Reconciled 2026-08-04 against
`c31fdc51` after the seven-dimension review
(`docs/reviews/2026-08-04/SUMMARY.md`), which fixed 46 of its 52 findings in
the range `a24f03eb..c31fdc51`. Extended 2026-08-04 with the five-domain
code-smell review (god modules, misplaced behavior, idiomatic Rust) against
`2fd4b609`; its findings are the dated section near the end.

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

## P1 — Incremental reparse can publish a tree that is not the published source

Found 2026-08-04 while implementing the frontmatter YAML alignment plan. **Not
caused by that change** — verified by replaying the seed at `dac9764c`, the
commit before the plan's first commit (`9ce823f7`), where it fails identically.

`randomized_full_and_incremental_snapshots_agree`
(`crates/waml-syntax/tests/properties.rs`) fails for a specific 6-edit sequence
with:

```
StructuralInvariant { reason: "Markdown snapshot tree does not own the published source" }
```

That is the incremental reparse publishing a snapshot whose tree text does not
equal the source it claims to describe. Every consumer downstream — spans,
diagnostics, the editor's coloring and hit-testing, the LSP — reads positions
out of a tree that disagrees with the buffer.

Reproduce (the seed file is deliberately NOT committed, per the repo rule
against `proptest-regressions`; recreate it to replay):

```
crates/waml-syntax/tests/properties.proptest-regressions
cc d993b095ce3216268e454865b33514e3954a283aee22dc6bb0fe74b2c24d4d81 # shrinks to edits = [(51, 139, 32), (94, 46, 1), (139, 60, 1), (153, 144, 33), (73, 35, 60), (19, 13, 0)]
```

Frequency: 100% with that seed, and 0 failures across 3 × 512 fresh cases
without it — so a normal gate run is green and this will not block CI. It is
rare, not absent; the shrunk case is six edits, so the trigger is a specific
accumulation of reparse windows rather than any single edit.

Note this is a *different* property from
`frontmatter_interior_edits_full_and_incremental_agree`, which passes.

Prior art: [[waml-syntax-incremental-proptest-bug]] was the same class of
defect (reparse windows swallowing trailing EOF whitespace, fixed in
`10f66dc9`) — worth reading before diagnosing this one.

## P2 — A second incremental/full divergence lives only in a test comment

Surfaced 2026-08-04 by the issue-21 review; not introduced by it.

`crates/waml-syntax/tests/properties.rs:616` documents a still-live divergence
in prose and then shapes the test to avoid triggering it: a length-changing
edit near an inline link's `(x)` destination makes incremental and full parse
disagree on destination-range tracking.

This is recorded nowhere else — no issue entry, no `TODO`, no failing test.
It is distinct from the source-ownership defect above (different property,
different trigger). Filed here so it survives the next edit to that test.

Fix: reproduce it as an explicit ignored/expected-fail case rather than a
comment, then diagnose alongside the source-ownership defect — both are
incremental-vs-full disagreements in markdown reparse and may share a cause.

## P2 — IME window ignores the gutter, mirroring the fixed pointer bug

Surfaced 2026-08-04 by the issue-20 review; pre-existing and out of that
plan's scope.

`crates/waml-markdown-editor/src/widget.rs:1507,1530` — the reverse mapping
for `cx.show_text_ime` subtracts `scroll_y` but never adds the gutter width
back. With line numbers on, the IME/candidate window renders `gutter` px left
of the caret.

Same coordinate contract as the pointer-event bug fixed in `1155f9ea`, in the
opposite direction: that fix added the gutter on the event path, this is the
draw path back out. The shared `abs_to_layout_point()` helper introduced by
that fix has no inverse; adding one would close this and stop the contract
being hand-maintained in both directions.

Related, same file, both pre-existing and both currently harmless:
`widget.rs:1567` adds back only vertical scroll while the draw path subtracts
the full `get_scroll_pos()` (safe only because the DSL sets
`show_scroll_x: false`), and `widget.rs:722` vs `:847` assume
`scroll_bars.area().rect(cx).pos` equals `cx.peek_walk_turtle(walk).pos`.

## P2 — Gutter width is read one frame before the faces it measures are refreshed

Surfaced 2026-08-04 by the issue-33 end-review; pre-existing.

`crates/waml-markdown-editor/src/widget.rs:997` — `draw_walk_with_session`
calls `gutter_width()` (`widget.rs:1141`), which consults the cached
`GutterMetrics`, *before*
`install_layout()` runs `refresh_faces()`. On the first frame after a theme or
live-apply swap of the mono family, the gutter width — and therefore the wrap
width fed into layout — is computed from the previous face's digit advance.
The frame paints one gutter and wraps to another, then self-heals on the next
frame.

This is exactly the cached-metric drift class that issue 33 Task 3 set out to
remove; the ordering inside the draw prologue kept one instance alive.

Fix: refresh the faces before the first metric read of the frame (hoist
`refresh_faces` out of `install_layout` into the top of
`draw_walk_with_session`), or invalidate `GutterMetrics` from the same signal
that marks the faces stale, so a swap cannot be observed half-applied.

## P3 — Deserialized bundles are not checked for duplicate ids

`crates/waml/src/okf.rs:274` — `Bundle::parse` rejects duplicate concept ids
and directory addresses (`BundleError::DuplicateConceptId`), but the serde
deserialization path never has. Since `825dd558` the accessors use
`binary_search_by`, so a duplicate now resolves to an *arbitrary* one of the
duplicates where `iter().find()` previously returned the first.

No live caller is known to feed a duplicate-bearing bundle through serde, so
this is latent. Enforcing uniqueness on deserialize (or reusing `parse`'s
validation) closes it.

## P3 — The gutter-measurement fallback is scaled by the value that may have failed

Surfaced 2026-08-04 by the issue-33 end-review; pre-existing.

`crates/waml-markdown-editor/src/widget.rs:273` — when shaping the digit run
fails, the fallback is `GutterMetrics::FALLBACK.scaled(mono.font_scale)`. A
degenerate `font_scale` (0.0, or NaN from a bad live-apply) yields a
`digit_width`/`ascent` of 0 or NaN: the gutter collapses to `GUTTER_GAP` and
every line number paints stacked at `right`. The degraded path multiplies by
the very quantity that is the most likely cause of the measurement failing, so
a bad scale is laundered into a bad layout instead of a legible one.

A clamped scale (`font_scale.is_finite() && > 0.0`, else 1.0) or the unscaled
constant would degrade to a readable gutter instead of a pile of glyphs.

Note the existing test `a_failed_measurement_is_not_cached`
(`widget.rs:1889`) pins `scaled(0.0)` as the intended result, so changing this
means changing that assertion deliberately — it is not an oversight to patch
silently.

Fix: clamp `font_scale` to a finite positive value before scaling the
fallback, and update `a_failed_measurement_is_not_cached` to assert the
clamped shape.

## P2 — `MarkdownEditorError` has no `Display`

Surfaced 2026-08-04 by the triage-batch end-reviews; verified against
`12a0ec59`.

`crates/waml-markdown-editor/src/widget.rs:102` — `MarkdownEditorError`
derives `Debug` and implements `From<ControllerError>`, but has no `Display`
impl. Every site that reports one therefore emits a `Debug` dump, including
the quarantine path issue 31 added: a `StalePresentation { .. }` struct
literal reaches the log where a sentence belongs.

The same review class fixed this once already — `install_presentation`'s
revision was switched to `Display` in `99687d4b`. The error type itself was
not.

Fix: implement `Display` (and `std::error::Error`) for `MarkdownEditorError`,
then let the reporting sites format with `{}` instead of `{:?}`.

## P2 — `validate_fragments` detects operand under-run but not over-run

Surfaced 2026-08-04 by the triage-batch end-reviews; verified against
`12a0ec59`.

`crates/waml/src/uml/sequence.rs:1394` — `validate_fragments` zips runtime
`SeqNode::Fragment`s against `declared_fragments` (the declared fragments
filtered by `value(&fragment.kind).is_some()`). When the runtime side has more
fragments than the declared side, `declared_fragments.next()` returns `None`
and a `debug_assert!(false, "runtime fragment without typed declared
fragment")` fires. The opposite drift is unchecked: the iterator is dropped at
the end of the loop with no test for leftover declared fragments, so declared
fragments that never got a runtime node are silently skipped and never
diagnosed.

Both sides are guarded by the same coupled-filter comment; only one direction
is enforced.

Fix: after the loop, assert (or diagnose) that `declared_fragments.next()`
is `None`.

## P3 — Two "empty children" failures are reported as `ParseError::WidthOverflow`

Surfaced 2026-08-04 by the triage-batch end-reviews; verified against
`12a0ec59`.

`crates/waml-syntax/src/incremental.rs:176` and `:1164` both compute
`root_green().children().len().checked_sub(1).ok_or(ParseError::WidthOverflow)?`
to find the EOF child index. The `checked_sub` can only fail when the root has
zero children — an empty-tree condition, not a text-width overflow. Every
other `WidthOverflow` in the file comes from a genuine `TextSize`/`TextRange`
conversion.

Consequence is a misleading fallback: the incremental path bails to a full
reparse citing width overflow for a tree that is merely empty, and the reason
is wrong in any log or diagnostic that surfaces it.

Fix: give `ParseError` a distinct variant for a root with no children (or
assert the invariant if it is genuinely unreachable), and stop overloading
`WidthOverflow`.

## P3 — Editor scroll position has four writers and no owner

Surfaced 2026-08-04 by the triage-batch end-reviews; verified against
`12a0ec59`.

`crates/waml-markdown-editor/src/widget.rs` writes the scroll position from
several places, each pairing `self.scroll_y = …` with a
`set_scroll_pos_no_clip(…)` call that must agree: `:850`/`:856`,
`:1015`/`:1016`, `:1022`/`:1023`, `:2162`/`:2165`, `:2297`/`:2300`, plus bare
`self.scroll_y` writes at `:877` and `:1093`. Nothing owns the pair, so the
two halves can be updated independently and the cached `scroll_y` can drift
from the scroll bars' actual position.

Fix: funnel every write through one method that sets both, and make the field
private to it.

## P3 — Fixture `mid()` hashes where issue 29 established `MessageId == index`

Surfaced 2026-08-04 by the triage-batch end-reviews; verified against
`12a0ec59`.

`crates/waml/tests/interaction_solver_golden.rs:16` — `mid()` derives a
`MessageId` from an FNV-1a hash of a human-readable test id. Issue 29
established that production `MessageId`s are document-order indices, and the
function now carries a doc comment saying exactly that. The comment documents
the divergence rather than removing it: the fixtures exercise the solver with
`MessageId`s that no real document can produce, so any solver logic that comes
to depend on the index invariant will pass these tests and fail in production.

Fix: assign fixture ids by declaration order (a counter or an explicit index
per fixture) and keep the readable name in a side map.

## P3 — Four linear scans survive over the document catalog

Surfaced 2026-08-04 by the triage-batch end-reviews; the first two verified
against `12a0ec59`, the other two carry pre-triage line numbers and need
re-locating.

`crates/waml/src/uml/analysis.rs:174` (`referrers`) and
`crates/waml/src/uml/lower.rs:446` (`resolve_index`) each walk
`catalog().documents()` / `work.documents()` with `find_map` / `position`,
then fall through to a second full pass via `unique_match`. Two more scans of
the same shape were flagged in `lower.rs` around `:853` and `:1367`.

These are O(documents) per lookup on a path that runs per reference, so cost
is O(references × documents) per analysis. Bundles are small today, which is
why this is P3 rather than a performance issue.

Fix: build the id and slug indexes once per catalog and share them across the
lookup sites; re-verify the two `lower.rs` line numbers before touching them.

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

## 2026-08-04 code-smell review (five-domain, against `2fd4b609`)

Five parallel staff-engineer reviews over the `docs/review-rules` rulebooks:
UML domain, headless core (solve/okf), waml-syntax, waml-editor, and
waml-markdown-editor + waml-cli. New findings only; overlaps with existing
entries are cross-referenced rather than duplicated.

### P1 — Markdown-editor pointer events ignore the gutter offset

- `crates/waml-markdown-editor/src/widget.rs:770,785,790` convert pointer
  positions with `event.abs - area.rect(cx).pos + scroll` only; the draw path
  (`widget.rs:852`) additionally translates by the gutter width. With line
  numbers on, `point_to_source`, `navigation_position`, and `embedded_at`
  receive a point ~36px too far right — clicks land characters left of the
  glyph, link activation and embedded-block hit-tests shift with it.
- No compensating subtraction exists anywhere on the event path; no test
  exercises pointer hits with `LineNumberMode != Off`.

Fix: one shared `abs_to_layout_point()` helper (the translation is already
copy-pasted three times), plus a pointer test with line numbers enabled.

### P1 — Incremental reference-use scan drops the rest of the line

- `crates/waml-syntax/src/markdown/reparse.rs:133`: when a bracket pair is
  followed by `(`, the scan returns `("", after.len())`, consuming the whole
  remainder of the line. For `[a](x) see [b][id]`, the `[b][id]` use is never
  seen, so `change_may_affect_reference_use` fails to force full-parse
  fallback and the incremental splice can resolve links differently from a
  full parse. The debug oracle compares island counts, not link resolution,
  so it cannot catch this.

Fix: consume only the balanced `(...)` (or `close + 1`) and keep scanning;
add a fixture editing a definition line while `[a](x) [b][id]` sits in an
untouched window, asserting incremental == full.

### P1 — Hostile nesting overflows the stack and kills the session

- `crates/waml-syntax/src/markdown/inline.rs:79` (`rebuild`),
  `parse_inlines` (self-recursive per emphasis/strikethrough/link pair),
  `projection::visit`, `incremental::collect_occurrences`, and
  `red::SyntaxTree::rewrite` all recurse on tree depth or inline nesting.
  10k `>` or 10k `*a ` overflows the stack — uncatchable, kills the LSP,
  poisons the wasm instance. (Known open defect: parser overflow at 10k
  quotes.) The codebase already has the standard: `recover_exact_source` is
  iterative with a 2,048-deep test.

Fix (cheapest durable): cap container/inline nesting at the block scanner
(cmark-gfm precedent) and emit a diagnostic beyond it — one bound at the
entry point covers every recursive consumer.

### P1 — `FieldEdit` serde round-trip turns `Unchanged` into `Clear`

- `crates/waml/src/uml/ops.rs:36-55`: `Serialize` collapses both `Unchanged`
  and `Clear` to `serialize_none()`; `Deserialize` maps `None` → `Clear`.
  Unless every containing struct carries both
  `skip_serializing_if(FieldEdit::is_unchanged)` and `serde(default)`,
  serializing an op with `FieldEdit::Unchanged` and reading it back yields a
  silently destructive edit (deletes authored multiplicity). The contract
  lives in attributes the impl cannot enforce.

Fix: error/debug-panic on serializing `Unchanged`, or a tested newtype
helper; at minimum a round-trip test asserting `Unchanged` survives the wire
on every op that carries one.

### P2 — `okf::Bundle` linear-scan accessors make per-edit work quadratic

- `crates/waml/src/okf.rs:279`: every accessor is `iter().find()` over a
  `Vec`; consumers loop over them — `okf/shell.rs:241-265` is O(A²) per
  directory build, `default_member_order` (`shell.rs:527-546`) scans per
  concept, the authored-order merge (`shell.rs:273-283`) is `contains` in a
  loop, `index_md.rs:88,98` scans per member. This runs inside
  `okf::shell::derive`, i.e. on every accepted edit.
- Same pattern copied in `Model::node` (`model.rs:1143`) and
  `SourceBundle::document_by_concept_id` (`source.rs:357`).

Fix: the vectors are already sorted at construction — switch accessors to
`binary_search_by`; have `default_member_order` and the merge use them.

### P2 — Content-reachable `expect`s in `okf::project`; dead `project_document`

- `crates/waml/src/okf.rs:392-430`: `.expect("non-reserved projection
  produces one concept")` is false under the quarantine design —
  `analyze_okf_inner` (analysis.rs:1193,1295) quarantines instead of
  erroring, so `Bundle::parse` can succeed with zero concepts and the
  `expect` panics (poisons wasm). Also wrong for reserved filenames.
- `project_document` has no callers anywhere in the workspace and carries two
  more `expect`s.

Fix: delete `project_document`; make `project` return `Option`/`Result`
while its only callers are tests.

### P2 — Concept→path resolution scans the catalog seven times

- The pattern `catalog.documents().values().find(|d| id_of(d.path()) ==
  concept_id)` appears at `uml/analysis.rs:986,1059,1151,1244,1493` and as
  `path_for_concept` in `uml/sequence.rs:200-209` (called per interaction-use
  and per target). `analyze` builds exactly this index at `analysis.rs:266-272`
  — with a comment explaining why — and does not pass it down. (The earlier
  `d30af731` fix covered `analyze` itself; these six sites still scan.)

Fix: thread the `BTreeMap<String, &Document>` (or a small `ValidationCtx`)
through `validate_declared_semantics`, `declared_projection`, and
`sequence::lower`.

### P2 — UML validation rules exist in parallel copies with no ownership rule

Validation verdicts live in three layers — `validate_declared_semantics`,
`declared_projection`'s admission filters, and `sequence::lower`'s inline
checks — and each feature added its checks to whichever was nearest.
Concrete duplications, each a drift bomb:

- Interaction-use binding checks: `uml/sequence.rs:250-337`
  (`interaction_use_graph`, silent) vs `:581-643` (`lower`, diagnosed);
  the `is_graph_link` cross-check (`:644-649`) only works while the copies
  agree. Extract one `validate_use_bindings` with a report/silent flag.
- Relationship-end validity: `uml/analysis.rs:1749-1775` (`ends_valid`,
  admission) vs `:1019-1046` (diagnostic) — a one-ended `composes` is
  already dropped at `:1776` with no message. One `ends_valid(kind, from,
  to) -> EndVerdict` consumed by both.
- `describes` link parsed two ways: `uml/sequence.rs:966-972` hand-splits
  `"]("`; `uml/analysis.rs:1894-1913` has `parse_link_ref`/`resolve_describes`
  with different tolerance. Export the analysis helper.
- Branch-join lattice hand-coded three times in `uml/sequence.rs`
  (`walk_return_items` :1041, `repeated_deletes` :1349, `walk` :1460), which
  is also why the file carries four `too_many_arguments` allows. Extract a
  generic `fold_fragment` skeleton.

Decide the ownership rule before the next UML feature: projection admits or
drops, validate diagnoses, both consume one shared verdict function per rule.

### P2 — Incremental guards re-lex text instead of querying the parser

- Frontmatter fence recognition exists three times:
  `incremental.rs:1290` (`frontmatter_fences`), `markdown/parser.rs:95,604`,
  `markdown/mod.rs:186-193`. The parser copy handles a BOM; the incremental
  copy does not — verify `\u{FEFF}---` for a live incremental/full
  divergence. The pending frontmatter-YAML-alignment plan must otherwise land
  in three places.
- Link resolution duplicated inside `markdown/inline.rs` (`bracket_match_end`
  :723 vs `parse_link` :805) — drift becomes `ParseError::StructuralInvariant`
  for the whole document. Record what matched in `BracketMatch` instead.
- Frontmatter-entry extraction exists three times: `frontmatter.rs:272`
  (`parse_closed_syntax`), `okf/lower.rs:508-562` (`frontmatter_value`),
  `uml/lower.rs:669,788` — already disagree on non-`Str` values. Make
  `frontmatter_value` call `parse_closed_syntax`.

Direction: guards should be derived from parser output (structure/reference
maps from the tree), and the debug oracle at `incremental.rs:981-1000` should
compare full trees, not island counts.

### P2 — Content-reachable panics and catch-alls on domain enums

- `uml/sequence.rs:1294-1296`: `.expect("each runtime fragment has a typed
  declared fragment")` holds only because `lower` (`:756`) applies the same
  filter 500 lines away; nothing couples the two sites. Pass the declared
  fragment into `SeqNode::Fragment` or debug_assert-and-skip.
- `uml/analysis.rs:651`: catch-all `_ =>` maps any future
  `UmlSyntaxDiagnosticCode` variant to `MalformedAttribute`. Five variants
  already explicit; finish the match.
- `incremental.rs:282-340`: five width-arithmetic `unwrap`s on the
  per-keystroke path in a file that otherwise threads `Result` through 15
  `map_err`s for the same arithmetic; correct degraded behavior is
  full-parse fallback. Return `ParseError::WidthOverflow`.
- `solve/route.rs:1002`: `Side` round-trips through `u8` with a
  `_ => Side::Bottom` catch-all purely to key a `BTreeMap` — derive
  `Eq, Ord` on `Side`, delete `side_disc`/`disc_to_side`.
- `uml/sequence.rs:69-77,929`: `MessageId` is `format!("m{index}")` re-parsed
  by `report_message` with silent diagnostic drop on parse failure. Carry the
  index in the type.

### P2 — Core `analysis.rs` hard-codes the UML specialization it abstracts

- `crates/waml/src/analysis.rs:234,424,910`: the module carries plugin
  scaffolding (`AnalysisStage::Specialization`, `ClaimSet`,
  `validate_disjoint_claims`) then hard-codes the single plugin, and hosts
  ~150 lines of UML syntax-highlighting classification (`waml_code_role`,
  `collect_waml_code_spans`, `WamlCodeSyntaxSnapshot` over `UmlLanguage`)
  that belong in `uml/`. Result: a 1,916-line module mixing catalog/session
  mechanics, candidate preparation, quarantine policy, and highlighting.

Fix: move the highlighting quartet into `uml/`; leave the claims machinery
but do not extend it until a second specialization is real.

### P2 — Quarantine messages are Debug dumps shown to users

- `crates/waml/src/analysis.rs:660`: `Display` for `AnalysisError` is
  `write!(f, "analysis error: {self:?}")`, and that string is stored as the
  user-facing quarantine message (`format!("{error}")` at :1194,:1296). Write
  real `Display` arms for at least `SourceTooLarge` and `Shell`.

Related editor-side visibility faults:

- `crates/waml-editor/src/class_diagram_view.rs:742`: the `ToggleExpand`
  re-solve drops `build_scene` diagnostics into `log!` while the `sync` path
  routes them to `set_scene_diagnostics` (:457). One-line fix; call it here
  too.
- `crates/waml-markdown-editor/src/widget.rs:653,1652`: `draw_walk` logs its
  failure every frame while the condition persists (`StalePresentation` is a
  steady state — buries the console); `install_presentation` swallows a
  validation failure with a bare `return` — no log, no action, editor keeps
  showing the old revision. Log once per distinct error; surface the
  validation failure.

### P2 — Stale `#[allow(dead_code)]` scaffolding defeats the `-D warnings` gate

- `crates/waml-editor/src/editor_session.rs:35,210,243,352,741,874`: six
  allows annotated "mounted by Task 4" — Task 4 landed (`app/actions.rs:961`
  calls `promote_source_edit`, `:967` `install_semantic_completion`). A
  blanket allow on the 13-field `EditorSessionSnapshot` keeps any
  later-unused field forever. The crate has 90+ allow sites (19 in
  `popup/radial.rs`, 17 in `frame.rs`) and nothing retires them.

Fix: delete the six now; adopt the convention that an allow must name a
concrete unlanded consumer and landing that consumer removes the allow in the
same commit. One sweep restores dead-code detection crate-wide.

### P2 — `MarkdownEditor`'s 54 fields hide a hand-reset state machine

- `crates/waml-markdown-editor/src/widget.rs:464-582`: the layout/motion
  pipeline is ten fields (`installed`, `target_layout`, `previous_layout`,
  `frame_layout`, `motion`, `pending_cause`, `pending_invalidation`,
  `last_layout_width`, `next_frame`, `scroll_y`) reset by enumeration in
  `clear_presentation` (:1664-1676) — which already misses
  `draw_commands_cache` and `scroll_y`, so a document swap can carry stale
  scroll and cache into the next document. Extract a `LayoutPipeline` struct
  with `reset()`/`invalidate()`. (Palette and `DrawText` fields must stay
  flat for the live system.)
- Font plumbing needs five coordinated edits per text face
  (`widget.rs:122-191,512-527,1345-1366`), and `install_layout` re-clones all
  eight `FontFamily`s per layout install. Collapse to
  `[Option<FontFamily>; 8]` indexed by `TextFace`.
- Gutter geometry from hardcoded font metrics
  (`widget.rs:455-461`, `GUTTER_DIGIT_WIDTH = 6.6`): a theme font swap
  silently misaligns line numbers. Measure one digit through the shaper at
  layout time, or accept and document.

### P2 — Per-frame and per-keystroke hot-path costs

- `crates/waml-markdown-editor/src/widget.rs:853-856`: even on a cache hit,
  every frame re-allocates the full translated command list; pass
  `content_origin` into the paint functions instead.
- `widget.rs:1185-1191`: `paint_text` does a linear `find` over
  `glyph_clusters()` per text command — O(runs²) per frame; index once per
  snapshot.
- `crates/waml-syntax/src/markdown/inline.rs:297,323,569`: `parse_inlines`
  is quadratic on adversarial inline runs (linear pair-vector `find`s per
  byte, `code_spans`/`angle_spans` recomputed per recursion level,
  `format!` per raw-HTML candidate) — fuzz-reachable DoS inside the LSP
  keystroke path. Index pairs by open offset; pass protected spans down.
- `crates/waml/src/analysis.rs:141-195,341`: `code_spans` scans every
  markdown document to validate an owner it can look up directly, and
  `WamlCodeSyntaxSnapshot::code_spans()` re-walks, sorts, and dedups per
  call. Look up by owner; compute once in `attach_code_syntax` and store
  `Arc<[WamlCodeSpan]>`.
- `crates/waml/src/edit/batch.rs:197-282`: each step full-parses every
  touched document (`claimed_id`) to classify invalidations, with
  O(removed × inserted) rename matching. Fine for one interactive op; a
  directory move does N discarded parses. Read only the frontmatter fence,
  or consult the shell cache — and comment the intent either way.

### P2 — `analyze` and `reparse_okf_markdown_with_structure` decomposition

- `crates/waml/src/uml/analysis.rs:248-700`: `analyze` is four functions —
  island reuse, declared-bundle extraction, inline attribute lowering
  (~100 lines of open code at :405-505 while fifteen sibling categories have
  `declared_*` functions — the file contains the pattern and its violation as
  competing precedents), and parser-diagnostic translation (:619-667).
  Mechanical extraction; also unlocks isolated tests for
  `validate_declared_semantics`/`declared_projection`, whose high crap
  scores reflect untestable shape, not zero coverage (both run under the
  golden suites via `analyze`).
- `crates/waml-syntax/src/incremental.rs:667-1009`: the `full(reason)`
  fallback closure is defined at :774 after two hand-expanded copies of it
  (:686-699, :703-717) — hoist now, before the copies drift. Then extract
  `plan_window_reparse(...) -> Result<WindowPlan, FullReparseReason>` so the
  ~25 returns become `?`.

### P3 — Smaller consolidations, when next touched

- First-match-wins action router: `class_diagram_view.rs:488-841` is ~10
  sequential `if let ... { return }` blocks scanning the same action batch;
  `camera_changed` (:686) already had to break the pattern because "a zoom
  can share a batch with a click". When the next branch is added, split a
  pure `route(actions) -> Vec<Intent>` (headlessly testable) from dispatch.
- `crates/waml-editor/src/editor_session.rs`: move the 2,390-line inline
  `mod tests` to `editor_session/tests.rs` per the `app.rs:1204` precedent
  (child module keeps private-field access). The non-test 1,038 lines are
  cohesive — not a god module.
- `crates/waml/src/uml/syntax/parser.rs` (4,734 lines): not a god module —
  87 free functions, clean seams, no shared state. Opportunistic mechanical
  split into `parser/{sequence,flow,layout,classifier,scan}.rs`; same
  verdict for extracting analysis.rs's `declared_*` block (~2300-3534) into
  `declared_extract.rs`.
- `crates/waml-markdown-editor/src/layout/engine.rs` (2,871 lines): real
  seams exist (shaping ledger :461-640, table intrinsics :1315-1450 and
  :1618-1786, block placement :1454-1560, row assembly :1969+). Split
  `table.rs`/`intrinsic.rs`/`assemble.rs` before the next feature lands
  here.
- `crates/waml-syntax/src/markdown/snapshot.rs:142`:
  `MarkdownSyntaxQueries` is eight hand-maintained `Arc<[T]>` +
  `_by_owner` map pairs (and `entities` already shipped without an index).
  A 20-line `IndexedByOwner<T>` collapses 16 fields to 8.
- `crates/waml-syntax/src/ast.rs:37-50`: `optional_node`/`optional_token`/
  `recovery` are aliases of `required_*` — names promise semantics that
  don't exist; `list(range)` is O(n²). Delete the aliases until a caller
  needs the distinction.
- `crates/waml/src/solve/route.rs:167,154`: inflated-obstacle list computed
  twice per edge; per-edge obstacle mask clones full `Obstacle`s (with
  `BoxId` string clones); `nudge` clones endpoint `String`s per `Seg`.
  Mechanical, do with the `Side` fix above.
- Two hand-rolled minimal-diff algorithms: `analysis.rs:1449`
  (`single_text_change`) vs `edit/reversible.rs:150` (`text_splice`) — both
  verified correct; below the three-instance threshold. Add
  cross-referencing comments so the third copy triggers the merge.
- `crates/waml/src/uml/analysis.rs:21-34`: `Analysis` mixes six `pub`
  fields with five getter-wrapped private ones, no rule distinguishing
  them; unique-basename disambiguation exists in three partial variants
  (`analysis.rs:172-176`, `lower.rs:137-144`, `:1424-1428`). Consolidate
  when next touched.

### Metric ghosts — checked and dismissed, do not re-litigate

- `serve/guard.rs` "fan-in 104" is not real: one pure constant-time
  `check()`, one caller, guard-before-body-parse tested. Model file.
- `serve/mod.rs ↔ routes.rs ↔ state.rs` is a chain, not a cycle.
- `multiplicity.rs` fan-in 112 is a leaf domain newtype doing its job.
- `ClassDiagramSurface`'s 37 fields are 21 required shader-pen handles plus
  extracted headless controllers; `App`'s 35 fields are documented shell
  state; `BehaviorSurface` shares `ViewportController` rather than
  duplicating it.
- Model-logic-in-widgets: checked explicitly — clean. `scene.rs` is plain
  data over `waml::solve` with zero makepad; placement previews call the
  headless solver.

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
