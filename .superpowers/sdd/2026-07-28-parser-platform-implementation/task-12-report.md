# Task 12 Report

## Outcome

- Base verified at `8eeda50f5fa5a38a7f6ccf2ff3e7cd4a73919c4e`.
- `EditorSession` now owns the complete immutable `OkfAnalysis` and
  `uml::Analysis` for one session revision.
- `replace` prepares a clean candidate before assigning any session field.
- `apply` lowers once, passes the current analyses as reuse-only
  `PreviousAnalyses`, prepares once, and commits source/catalog/shell/OKF/UML
  state and the revision/dirty marker together.
- The private `apply_with_preparer` seam supports injected preparation failures
  without exposing another product mutation API.
- Native and shared bundle opening no longer build or pass a separate UML
  projection. Legacy `EditContext` construction in editor tests now consumes a
  prepared parser snapshot.
- Session consumers use `source`, `okf_analysis`, `okf`, `uml_analysis`, and
  `uml_projection`; the former public `bundle` and `model` aliases are gone.
- Added parser-action fixtures plus coverage for revision-scoped provenance,
  every preparation-stage rollback, retry identity/revision behavior,
  revision-bound syntax actions, stale-action rollback, recoverable malformed
  commits, and save/allocation sharing.

## TDD Evidence

- RED:
  `rtk cargo test -p waml-editor
  editor_session::tests::replacement_owns_one_revision_scoped_analysis_snapshot`
  failed on the missing one-argument fallible `replace`, missing analysis
  accessors, old session storage, and the Task 11 legacy `EditContext` fields.
- GREEN: the same focused test passed after the minimal prepare/commit boundary
  and editor call-site migration.
- Focused rollback, stale-action, and recoverable-source tests were then run
  individually and as part of the full session suite.

## Verification

- PASS: `rtk cargo test -p waml-editor editor_session::tests` — 14 passed.
- PASS: `rtk cargo test -p waml-editor app::actions::tests` — 4 passed.
- PASS: `rtk cargo test -p waml --test syntax_actions` — 10 passed.
- PASS: parser-platform baseline session regression — 1 passed.
- PASS: `rtk cargo test -p waml-editor --all-features` — 724 passed.
- PASS: `rtk cargo test -p waml --test syntax_actions --all-features` — 10 passed.
- PASS: `rtk cargo check --workspace --all-features`.
- PASS: `rtk cargo fmt --all -- --check`.
- PASS: `rtk git diff --check`.
- EXPECTED PRE-EXISTING FAILURE:
  `rtk cargo test --workspace --all-features` reaches all Task 12 coverage and
  fails only `waml --test serde_shape::package_node_and_model_path`. The test
  calls the retained OKF compatibility projection with reserved `index.md` and
  panics at `okf.rs`'s “non-reserved projection produces one concept”
  assertion, matching the Task 10/11 documented baseline.

## Commits

- Implementation: `b78a21e` (`refactor: atomically install parser analyses`).

## TokenSave

- Initial plan/context query saved approximately 3,278 tokens.
- `EditorSession` symbol query saved approximately 3,116 tokens.
- Total reported savings: approximately 6,394 tokens.

## Concerns

- Workspace check reports only Cargo's two existing duplicate-package warnings
  for Makepad's `bitflags` and `cfg-if`.
- The unrelated modified `task-7-report.md` orchestration file was preserved
  and excluded from staging and commits.

## Fix Round 1

- Added the opt-in `waml/test-support` feature and enabled it only through
  `waml-editor`'s dev-dependency. Its hidden `PreparationProbe` implements the
  existing private `PreparationHooks`, so counts and injected failures occur at
  the real shell, OKF, UML, and claims boundaries inside
  `prepare_candidate_inner`; the normal editor build retains only the public
  production `prepare_candidate` boundary.
- Added a successful session transaction regression proving the exact phase
  trace is `shell, okf, uml, claims`, with one occurrence each.
- Added failure/retry coverage for every phase. Each injected failure records
  exactly the prefix through the failing phase, invokes no later phase, leaves
  the committed catalog/revision unchanged, and a retry records exactly one
  complete phase sequence with one revision advance.
- RED evidence: the focused editor test failed because
  `analysis::test_support::{PreparationProbe, prepare_candidate_with_probe}`
  did not exist.
- GREEN evidence:
  - session tests: 16 passed;
  - editor action tests: 4 passed;
  - syntax action tests: 10 passed;
  - full editor all-features: 726 passed;
  - workspace all-features check: passed;
  - normal editor binary check: passed;
  - format and diff checks: passed.
- Workspace checks retain only the two documented Makepad duplicate-package
  warnings.
- Fix implementation commit:
  `1d04a80968b8f8a4a930e62e7404901692357dfd`.
- TokenSave reported approximately 693 tokens saved for the fix-round context
  query.
