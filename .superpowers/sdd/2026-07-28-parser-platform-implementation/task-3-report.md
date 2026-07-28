# Task 3 report

## Changes

- Added red syntax occurrences with tree/path identity, navigation, locators, checked resolution, diagnostics, visitor traversal, and structural rewriter entry point.
- Added public typed-AST traits and annotation tracking/rebuild helpers.
- Extended green nodes with immutable annotation storage and an internal annotated-node rebuild helper.
- Added focused public integration coverage for adjacent shared zero-width tokens, locator tree binding, navigation, traversal, rewriting, and annotations.

## Verification

- `rtk cargo test -p waml-syntax --test red_ast` — 3 passed.
- `rtk cargo test -p waml-syntax` — 10 passed across 4 suites.
- `rtk cargo fmt` — completed.

## Design notes

- Red equality and hashing are based on the generated tree-instance ID and full child-index path; green sharing is queried separately with `same_green`.
- Annotation attachment checks the tree-bound locator before rebuilding only the ancestor path.
- Rewriters preserve green arcs when visitors return unchanged elements.

## Warnings / concerns

- The private-forged-locator test and dedicated `AstSlots` abstraction were completed in review fix round 1.
- TokenSave reported no indexed symbols for this newly added crate; direct source inspection was used after the required context query. RTK global savings at collection time were 81.4M tokens (42.2%).

## Review fix round 1

- Split occurrence `SyntaxAnnotation` storage from token diagnostic codes and rebuilt the exact node or token selected by the checked locator. Shared token occurrences now diverge only at the annotated path.
- Corrected root red ranges to `0..root_green.width()` and covered non-empty and zero-width roots.
- Added `AstSlots`, with declared-index accessors for required/optional node and token slots, lists, and recovery nodes. The representative wrapper test keeps skipped material nested in its declared recovery slot.
- Added node green-identity queries and tests for same-tree facades plus cross-rewrite structural sharing.
- Replaced the vacuous forged-locator test with an exact `KindMismatch` assertion and added a compile-fail locator privacy example.
- Strengthened rewriting coverage: a token replacement produces fresh changed ancestors while retaining the untouched annotated sibling allocation and annotation.

Verification:

- `rtk cargo test -p waml-syntax --test red_ast`: 7 passed.
- `rtk cargo test -p waml-syntax`: 15 passed across 4 suites, including the compile-fail doctest.
- `rtk cargo check --workspace`: passed; only pre-existing duplicate-package warnings were emitted after removing the new dead-code warning.
- `rtk cargo fmt --check`: passed.

## Review fix round 2

- Replaced the direct `AstSlots` smoke test with a representative typed `DeclaredPair` wrapper that declares required name/colon/trailing tokens, an optional value, a repeated list range, and a recovery node through fixed indices.
- Added paired fixtures with zero and three skipped tokens nested inside the recovery element. Both parent forms retain exactly six declared slots; every wrapper accessor resolves its declared child path and the trailing slot after recovery remains index 5.
- Added minimal `AstSlots::len`/`is_empty` introspection so the contract test can assert the parent slot layout directly.

Verification:

- Exact strengthened AstSlots test: 1 passed.
- `rtk cargo test -p waml-syntax --test red_ast`: 7 passed.
- `rtk cargo test -p waml-syntax`: 15 passed across 4 suites.
