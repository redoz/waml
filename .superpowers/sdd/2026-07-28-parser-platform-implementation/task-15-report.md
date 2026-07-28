# Task 15 Report

## Result

- Added `UmlLoweringCursor` and `UmlLoweringState` with cumulative concept paths,
  touched-island reparsing, stable operation indices, and candidate-only finish.
- Routed `uml::Batch` through one cursor while retaining the compatibility
  single-step entry point for Task 16.
- Preserved raw/unknown/recovery text with surgical section/frontmatter edits;
  CRLF and UTF-8 source remain authored, and valid rename references are changed
  only in typed UML nodes.
- Added ordered add/set, rename/edit, placement/remove, rollback, and
  malformed/protected-text regressions.
- Updated the parser-platform editor baseline expectation to assert that raw
  `## Operations` bytes and trailing whitespace survive UML lowering.

## TDD Evidence

The initial `uml_lowering_order` run failed on rename/edit and
placement/remove exact-source behavior. After implementation:

- `rtk cargo test -p waml --test uml_lowering_order`: 5 passed.
- `rtk cargo test -p waml uml::ops::tests`: 5 passed.
- `rtk cargo test -p waml uml::rename::tests`: 7 passed.
- `rtk cargo test -p waml --test ops_golden`: 3 passed.
- `rtk cargo test -p waml`: 534 passed.
- `rtk cargo test -p waml-editor`: 727 passed.
- `rtk cargo check --workspace --all-features`: passed.
- `rtk cargo fmt --check`: passed after formatting.
- `rtk git diff --check`: passed.

## Known Baseline Gate Debt

- `rtk cargo test --workspace --all-features` reaches an unrelated, reproducible
  feature-gated failure in `okf::tests::package_node_and_model_path`
  (`non-reserved projection produces one concept`). The isolated command
  `rtk cargo test -p waml --all-features package_node_and_model_path` fails the
  same way; the normal full `waml` and editor suites pass.
- Strict `rtk cargo clippy -p waml --all-targets --all-features -- -D warnings`
  is blocked by 16 pre-existing warnings in `analysis.rs`, `uml/analysis.rs`,
  `uml/syntax/parser.rs`, and `validate.rs`. The same clippy run without
  `-D warnings` completes with zero errors and 20 warnings; none point to the
  Task 15-owned files.

## TokenSave

TokenSave queries saved approximately 14,728 tokens in total
(430 + 3,118 + 11,180).
