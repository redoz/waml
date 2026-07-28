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
