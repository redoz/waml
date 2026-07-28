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

## Fix Round 1 — Remove Legacy Lowering Authority

- Replaced the remaining `Document`/`Line` parse-mutate-serialize flow with
  direct `waml_syntax` tree/range edits for frontmatter, H1 title, Attributes,
  Values, Relationships, Layout, classifier creation/removal, and diagram
  display fields.
- Seeded cumulative state with revision-bound UML trees, then reparsed every
  touched claimed island before the next operation.
- Rebuilt title, claim, type-reference, selector, duplicate, placement, and
  referrer queries from current source plus syntax nodes.
- Removed the compatibility-only `uml::ops::lower_one` and DiagramSet
  canonicalization branch. Compatibility UML steps now enter a one-operation
  syntax-native `uml::Batch`; Task 16's one mixed cumulative cursor remains
  unimplemented.
- Added `uml_lowering_authority.rs`, which failed first on `parse_document` and
  now rejects all live `parse_document`, `serialize_document`,
  `crate::syntax::Document`, `Line<`, and `uml::ops::lower_one` references in
  the Task 15/compat path.
- Expanded ordered regressions to prove field, diagram, layout, malformed
  recovery, raw Operations, trailing whitespace, and stable rollback behavior.

Fresh verification:

- `rtk cargo test -p waml --test uml_lowering_authority`: 1 passed.
- `rtk cargo test -p waml --test uml_lowering_order`: 7 passed.
- `rtk cargo test -p waml`: 537 passed across 20 suites.
- `rtk cargo test -p waml-editor`: 727 passed across 5 suites.
- `rtk cargo check --workspace --all-features`: passed.
- `rtk cargo fmt --check`: passed.
- `rtk git diff --check`: passed.
- Direct prohibited-symbol scan: no matches.
- `rtk cargo test --workspace --all-features` remains blocked only by the
  independently reproducible pre-existing
  `okf::tests::package_node_and_model_path` failure.

Fix-round TokenSave queries saved approximately 18,924 additional tokens
(7,734 + 11,190).

## Fix Round 2 — Deterministic Rename Destinations

- Added one `destination_path` authority shared by mutation and cumulative
  state. Bare destinations preserve the exact source directory; destinations
  containing directories are validated as bundle-root-relative full IDs.
- Removed the post-rename first-basename scan. State now captures the exact
  pre-rename path, derives the exact destination before mutation, and rebinds
  only through `SourceBundle::document(&BundlePath)`.
- Scoped `./slug.md` rewrites to the exact source directory so duplicate
  basenames elsewhere are untouched. Full-path moves derive the correct
  relative destination href.
- Preserved the active operation index for invalid/colliding destinations and
  retained batch rollback behavior.
- Added regressions for duplicate basenames, local basename renames, explicit
  full destinations, cumulative follow-up operations, stable error indices,
  and rollback.

TDD evidence:

- The duplicate/full-destination suite initially failed because state used the
  first matching basename and explicit paths were interpreted locally.
- A strengthened duplicate regression then failed on an unrelated directory's
  self-reference; exact-directory reference scoping made it pass.
- A full-path reference regression failed on `./invoice.md`; exact relative
  href derivation made it pass as `../archive/invoice.md`.

Fresh verification:

- `rtk cargo test -p waml --test uml_lowering_order`: 10 passed.
- `rtk cargo test -p waml uml::rename::tests`: 7 passed.
- `rtk cargo test -p waml --test uml_lowering_authority`: 1 passed.
- `rtk cargo test -p waml --test ops_golden`: 3 passed.
- `rtk cargo test -p waml`: 540 passed across 20 suites.
- `rtk cargo test -p waml-editor`: 727 passed across 5 suites.
- `rtk cargo check --workspace --all-features`: passed with two duplicate
  dependency warnings.
- `rtk cargo fmt --all -- --check`: passed.
- `rtk git diff --check`: passed.
- Direct prohibited-symbol scan: no matches.

Implementation commit: `f2626c2`.

Fix-round TokenSave queries saved approximately 5,537 additional tokens
(2,364 + 3,173).

## Fix Round 3 — Exact Per-Referrer Href Rewrites

- Changed rename reference discovery from a source-directory basename scan to
  every cumulatively claimed document.
- Each syntax-owned href is resolved against its referrer's exact pre-rename
  `BundlePath` and rewritten only when it resolves to the renamed document's
  exact old path.
- Replacement hrefs are rendered relative to each referrer's post-rename path,
  including the renamed/moved document itself.
- Limited edits to typed link destinations, linked attribute destination
  ranges, layout link atoms, and exact layout operands. Protected Markdown
  regions and unrelated same-basename targets remain byte-identical.
- Preserved query/fragment suffixes, optional relative-prefix spelling,
  backslash spelling, surrounding destination trivia/angle brackets, CRLF, and
  UTF-8.
- Retained pre-edit collision validation and cumulative rename-then-edit
  behavior.

TDD evidence:

- The new cross-directory regression first failed on the moved document's
  unchanged self-reference.
- The first typed-token implementation still failed because linked attribute
  types currently live in an `Attribute` recovery node rather than a
  `TypeToken`. Narrowing the edit to that typed attribute's single href range
  made the regression pass without restoring section-wide replacement.
- Existing rename tests then exposed leading trivia in bare layout word token
  ranges; trimming only for resolution while retaining the trivia restored the
  expected exact operand rewrite.

Fresh verification:

- `rtk cargo test -p waml --test uml_lowering_authority`: 1 passed.
- `rtk cargo test -p waml --test uml_lowering_order`: 11 passed.
- `rtk cargo test -p waml uml::rename::tests`: 7 passed.
- `rtk cargo test -p waml uml::ops::tests`: 5 passed.
- `rtk cargo test -p waml --test ops_golden`: 3 passed.
- `rtk cargo test -p waml`: 541 passed across 20 suites.
- `rtk cargo test -p waml-editor`: 727 passed across 5 suites.
- `rtk cargo check --workspace --all-features`: passed with two duplicate
  dependency warnings.
- `rtk cargo fmt --all -- --check`: passed.
- `rtk git diff --check`: passed.
- Direct prohibited-symbol scan: no matches.

Implementation commit: `574b5d2`.

Fix-round TokenSave queries saved approximately 7,310 additional tokens
(1,080 + 6,230).
