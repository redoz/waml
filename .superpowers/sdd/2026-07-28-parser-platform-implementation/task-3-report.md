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

- The requested private-forged-locator unit test is currently a placeholder and should be strengthened with a crate-local test language before integration.
- Typed slots are represented by fixed `child_at` indexing rather than a dedicated `AstSlots` abstraction; later grammar wrappers will need a small slot helper to encode required/optional/list/recovery declarations.
- TokenSave reported no indexed symbols for this newly added crate; direct source inspection was used after the required context query. RTK global savings at collection time were 81.4M tokens (42.2%).
