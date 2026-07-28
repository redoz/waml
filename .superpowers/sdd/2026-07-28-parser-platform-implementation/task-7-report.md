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

## Remaining concerns

This is not complete Task 7 parity yet: the new non-attribute productions are currently raw-content typed wrappers, rather than fully tokenized fixed slots; declared target resolution/located non-attribute diagnostics and projection construction still rely on the retained legacy projection. Member recovery does not yet enforce the full indentation grammar. Do not treat this commit as sufficient to close Task 7 without completing those requirements.

## Metrics

TokenSave context was consulted first; its index was stale by 3h13m and did not contain the Task 6 boundary. RTK global metrics at verification: 81.8M tokens saved (42.2%).
