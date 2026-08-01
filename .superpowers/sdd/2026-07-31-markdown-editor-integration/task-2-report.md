# Task 2 report: immutable editor-session transaction seam

Status: **DONE**

Date: 2026-08-01

## Outcome

Task 2 makes the editor session publish one immutable snapshot. A promoted
Markdown edit validates the current source and syntax identities, installs the
exact source and syntax `Arc` values, records one inverse history entry, and
returns a semantic-analysis request. Semantic completion installs only when its
session revision, source identity, and Markdown snapshot identities still match.

The transaction path does not depend on the semantic catalog. Therefore, it can
accept a second local edit while semantic analysis for the first edit is still
pending. Immediate undo restores the first accepted text. Stale proposals,
invalid changes, result mismatches, base-identity mismatches, and path-map
invariants return typed errors and do not mutate the session or history.

Task 2 also adds the required Task 1 exact-source helper. It validates the
accepted source-text identity directly, preserves the replacement allocation,
and uses the existing reversible source edit implementation. The legacy
`ExactSourceEdit` API delegates to the same implementation.

## RED/GREEN evidence

- The first focused RED gate reported 44 compile errors for the missing Task 2
  types and APIs.
- A structural RED test showed that semantic-catalog lookup could not accept a
  second source edit while analysis lagged. The exact-source helper removed that
  dependency.
- `rtk cargo test -p waml-editor editor_session::tests --bin waml-editor`:
  33 passed.
- `rtk cargo test -p waml --test incremental_analysis`: 22 passed.
- `rtk cargo test -p waml-editor --test editor_history`: 10 passed.
- `rtk cargo check -p waml-editor --bin waml-editor`: passed. Cargo reported
  only two existing duplicate-package warnings from the Makepad checkout.
- `rtk cargo fmt --all` and `rtk git diff --check`: passed.

The unrelated untracked
`crates/waml-syntax/tests/properties.proptest-regressions` file remains
unchanged and unstaged.

TokenSave semantic exploration and review saved approximately 66,440 tokens.
