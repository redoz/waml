# Task 14 Report: Cumulative OKF Shell Lowering

Implementation commit: `adf83ea`

## Checklist

- [x] `okf::Batch` lowers through one ordered `OkfLoweringCursor`.
- [x] Candidate source is cloned once and every operation reads cumulative source/path state.
- [x] Initial revision/catalog/source identity is validated before the first semantic step.
- [x] Structural add/remove/rename state is updated deterministically after collision-free `SourceBundle` mutation.
- [x] Touched shell documents are invalidated and reparsed from cumulative candidate text.
- [x] Index retitle/reorder/sort use shell-confirmed heading/list ranges without constructing `okf::Bundle` or specialization analysis.
- [x] Unknown Index headings/body, UTF-8 text, and CRLF line endings are preserved.
- [x] Synthesized Index documents support cumulative retitle-then-sort and import-then-retitle.
- [x] Stable late-step error indices and input atomicity are covered.
- [x] Untouched `Arc<String>` identities remain shared.
- [x] Legacy `ops::apply` parity remains covered.
- [x] Task 15 UML island lowering was not changed.

## TDD evidence

Initial `rtk cargo test -p waml --test okf_lowering_order` failed in the two exact-preservation cases:

- import-then-retitle converted CRLF and lost unknown Markdown;
- rename-then-retitle canonically rebuilt the Index, collapsing the preserved body.

After implementation, the focused suite passes 4/4.

## Verification

Passing:

- `rtk cargo fmt --all -- --check`
- `rtk cargo test -p waml --test okf_lowering_order` — 4 passed
- `rtk cargo test -p waml okf::ops::tests` — 3 passed
- `rtk cargo test -p waml --test ops_golden` — 2 passed
- `rtk cargo test -p waml --test syntax_actions` — 10 passed
- `rtk cargo test -p waml parser_platform_baseline` — 5 passed
- `rtk cargo test -p waml` — 526 passed
- `rtk cargo test -p waml-editor editor_session::tests` — 17 passed
- `rtk cargo test -p waml-editor` — 727 passed
- `rtk cargo check --workspace --all-features` — passed
- `rtk git diff --check`
- Prohibited scan found no `Bundle::parse`, UML analysis, `prepare_candidate`, or derived-analysis mutation in Task 14 production lowering. Matches in `okf/ops.rs` are pre-existing unit-test setup only.

Repository-wide pre-existing failures outside Task 14 ownership:

- `rtk cargo test --workspace` and `--all-features` fail only in `crates/waml/tests/serde_shape.rs::package_node_and_model_path`, which panics in unchanged `crates/waml/src/okf.rs:394`.
- `rtk cargo clippy -p waml --all-features -- -D warnings` reports 14 unchanged findings in `analysis.rs`, `uml/analysis.rs`, and `uml/syntax/parser.rs`; none are in Task 14 files.

## TokenSave

TokenSave semantic exploration and failure tracing saved approximately 19,771 tokens.

## Concerns

- The workspace/clippy failures above prevent claiming those repository-wide gates are green. They were not changed because they are outside Task 14’s owned files and behavior.
- The unrelated modified `task-7-report.md` was preserved and never staged.
