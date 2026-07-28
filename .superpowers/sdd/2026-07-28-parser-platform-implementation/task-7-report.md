# Task 7 report

## Implemented

- Added lossless typed island wrappers for values, slots, relationships, members, member groups, and inline instances.
- Extended the shared UML parser to recognize confirmed `## Values`, `## Slots`, `## Relationships`, and `## Members` sections, including H3--H6 member-group headings.
- Added declared records for those island types and preserved the existing legacy semantic projection as the validated-model compatibility boundary.
- Added focused CRLF/Unicode/lossless coverage plus a recovery fixture.

## Verification

- `rtk cargo test -p waml --test uml_classifier_syntax` — PASS (1 test).
- `rtk cargo test -p waml parser_platform_baseline` — PASS (5 tests).
- `rtk cargo test -p waml uml::tests` — PASS (3 tests).
- `rtk cargo test -p waml --all-features` — 15 passed / 1 failed: pre-existing `package_node_and_model_path` panic in unchanged `crates/waml/src/okf.rs:394`; reproduced with the exact all-features selector and confirmed no diff in that file from `7caddd1`.
- `rtk git diff --check` — PASS after `rtk cargo fmt`.

## Follow-up increment

- Reproduced `package_node_and_model_path` with `RUST_BACKTRACE=1` at both
  `50e63cdf` and the requested detached baseline `7caddd1d`: both panic at
  `crates/waml/src/okf.rs:394` (`non-reserved projection produces one concept`)
  from `serde_shape.rs:160`. It is pre-existing and untouched by Task 7.
- Added a red/green regression that prohibits `RawMarkdownToken` children under
  classifier item nodes. Values, slots, relationships, members, and inline
  instances now retain authored text as structured bullet/link/identifier/
  delimiter tokens; declared conversion obtains its exact source slice through
  the syntax-node range, preserving source provenance after removal of the raw
  wrapper.
- `rtk cargo test -p waml --test uml_classifier_syntax` — PASS (2 tests).
- Added direct fixed-slot accessors for value, slot, relationship, member, and
  inline-instance syntax. Link-target access follows the immediate fixed Link
  child only; it does not perform a descendant search. The focused suite now
  asserts the direct-token values and link leaf shape (3 tests).

## Latest foundation

- Classifier declared lowering is syntax-only: values, slots, relationships,
  members, and inline instances no longer call `grammar.rs` to reparse source.
  The direct declared-field regression brings the focused suite to 4 tests.

## Latest incremental regression

- Added a red/green fixed-slot regression for bare, quoted, link, and missing
  slot values. `SlotSyntax::value_kind` distinguishes those authored variants
  without descendant searching; a missing colon is represented by the required
  zero-width `ColonToken` and emits the existing missing-colon recovery
  diagnostic. The focused suite now has 5 tests.

## Remaining concerns

This is not complete Task 7 parity yet: slot values still lack distinct
relationship names/ends/multiplicities and skipped-token recovery are not fixed
slots; member groups and inline clauses do not enforce indentation-aware
structure; and the exhaustive CRLF/Unicode/range/progress matrix is absent.
Most importantly, `uml::analysis` still starts with legacy
`super::project(context.okf)` and only replaces attributes, so values, slots,
members, relationships, targets, diagnostics, and edges are not yet wholly
syntax-authoritative. Target resolution against the claimed-document index and
located `UnresolvedTarget` provenance remain unimplemented.

## Metrics

TokenSave context was consulted first; its index was stale by 3h13m and did not contain the Task 6 boundary. RTK global metrics at verification: 81.8M tokens saved (42.2%).
