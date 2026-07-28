# Task 19 report

## Outcome

- Added pure `waml::host::{add_document, replace_document, remove_document}` lifecycle ingress with explicit existing/missing errors and allocation-preserving candidates.
- Replaced the mutable LSP string workspace with one immutable `Arc<LspAnalysisState>` containing host index, source, OKF, UML, and one revision.
- Disk initialization reads each file once and shares the same `SourceDocument` allocation between the disk index and revision-zero source.
- Open/change/close implement deterministic overlay precedence, overlay-only removal, disk restoration, collision rejection, missing-change rejection, and idempotent missing close.
- Every successful ingress prepares the entire next parser-platform analysis before compare-and-swap; racing requests retry from a fresh base.
- Diagnostics are derived from the snapshot UML analysis and mapped through its catalog `LineIndex`; stale snapshot publication is suppressed.
- Removed the LSP's legacy `validate_from_source` reparse authority. FULL sync remains.
- Task 20 / VS Code files were not changed.

## RED / GREEN evidence

1. Host ingress RED: `rtk cargo test -p waml host::tests` failed with six missing-function errors. GREEN: 2 passed.
2. Atomic snapshot RED: focused LSP bundle test failed because `LspAnalysisState` did not exist. GREEN: 3 bundle tests passed.
3. Allocation RED: add-ingress test failed `Arc::ptr_eq` for the incoming document. GREEN: the new crate-private whole-document insertion helper preserves it.
4. Parser cutover integration exposed the legacy flow-message expectation; the e2e assertion now checks the parser-platform recovery message.

## Verification

- `rtk cargo test -p waml-cli lsp` — pass.
- `rtk cargo test -p waml-cli --test lsp_e2e` — pass.
- `rtk cargo test -p waml host::tests` — pass.
- `rtk cargo test -p waml-cli` — 61 passed.
- `rtk cargo test -p waml` — 555 passed.
- `rtk cargo test -p waml-editor` — 735 passed.
- `rtk cargo fmt --all -- --check` — pass after formatting.
- `rtk git diff --check` — pass.
- Prohibited LSP scan for `validate_from_source`, legacy `Workspace`, and `Arc<Mutex<Workspace>>` — no matches.

The full `rtk cargo test --workspace` gate has one unrelated feature-unification failure in existing test `waml --test serde_shape::package_node_and_model_path` (`okf.rs:394`, “non-reserved projection produces one concept”). The focused `waml` suite passes; Task 19 does not touch serde shape or that projection.

## TokenSave

The first query used a stale index and saved 0 tokens. The useful LSP/source query reported `before=4268 after=1278 saved=2990`.

## Scope notes

- The pre-existing modified `task-7-report.md` was preserved and is not staged.
- No VS Code / Task 20 changes were made.

## Formal fix round 1

Root causes:

- CAS retries carried only path/text, so a stale FULL change became indistinguishable from current work after retry.
- diagnostics used global analysis revision as the protocol document version, and the state check happened before an awaited send without ordering concurrent publishers.

RED evidence:

- `slow_v2_cannot_install_after_v3_wins_the_compare_and_swap` initially failed to compile because no client-version/open-generation contract existed.
- `delayed_old_publication_cannot_arrive_after_newer_publication` initially failed because no ordered publication boundary existed.
- the close/reopen stale-`didOpen` test initially failed because `open_expected` did not exist.

Fixes:

- Every open document now stores its client version and a monotonic open generation in the immutable host index.
- `didOpen` and `didChange` capture the request generation before preparation; every CAS retry validates it. Duplicate/older versions reject before preparation.
- Closing and reopening always allocates a new generation, so old work cannot affect the new open lifetime.
- Diagnostic batches carry the target document's client version; disk-only documents carry `None`.
- One async publication gate serializes at the actual client-send boundary. It holds no analysis-state lock across an await.
- Deterministic `Barrier`/`Notify` tests cover stale FULL changes, close/reopen work, delayed old publication, per-document cross-file reanalysis versions, lifecycle collisions, and unchanged identity/revision after rejected ingress.

Fresh verification:

- focused LSP: 12 passed; stdio e2e: 3 passed; full CLI: 65 passed.
- host: 2 passed; action: 4 passed; editor session: 18 passed.
- parser baseline: 5 passed; golden: 6 passed.
- full `waml`: 555 passed; full editor: 735 passed.
- workspace check passes with existing warnings.
- workspace tests retain the unrelated `serde_shape::package_node_and_model_path` failure at `okf.rs:394`.
- formatting, diff, and legacy-authority scans pass.

Remaining concern: the current Rust LSP exposes diagnostics and FULL lifecycle only; it has no formatting, rename, or code-action request handlers on which to add result-suppression tests in Task 19 without expanding the approved interface beyond the task brief.

## Formal fix round 2

- Root cause: `didClose` retried plain `close(path)` after CAS loss, so delayed G1 work could see reopened G2 as current and close it.
- RED: deterministic `Barrier` test failed because no `close_expected(path, generation)` boundary existed.
- GREEN: `didClose` captures the open generation before preparation and every retry calls `close_expected`; a missing or mismatched generation returns `Ok(None)`, preventing preparation, swap, revision/counter consumption, and publication.
- The race test proves G2 remains open with exact text, snapshot revision, client version, open generation, document ID/revision, and allocation-backed slice unchanged.
- Current-generation disk restoration, overlay removal, duplicate close, external collision, and ordered publication tests remain green.
- Fresh gates: focused LSP 13 passed; e2e 3; full CLI 66; host 2; session 18; full `waml` 555; editor 735; workspace check passed; fmt/diff/legacy scans passed.
