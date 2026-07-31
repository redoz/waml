# Task 7 Report: Variable-metric Geometry

## Status

Complete. The foundation crate now owns neutral layout input contracts and one immutable geometry snapshot for caret, point, selection, and vertical-motion queries.

## TDD evidence

- RED: `rtk cargo test -p waml-markdown-editor --test layout_geometry` failed with unresolved layout geometry types.
- GREEN: `rtk cargo test -p waml-markdown-editor --test layout_geometry` passed 5 tests.
- Affected gate: `rtk cargo test -p waml-markdown-editor --test unicode_ime` passed 8 tests.
- Full crate: `rtk cargo test -p waml-markdown-editor` passed 31 tests.
- `rtk git diff --check` passed before report creation.

The focused count is higher than the three plan fixtures because session preferred-x reset and stale-layout revision behavior have direct tests.

## Implementation

- Added foundation-owned layout document, block flow, font, metric, inset, column, and stable element identity types.
- Added immutable visual lines, clusters, caret stops, block geometry, and layout snapshots.
- Added exact-affinity source-to-point mapping and line-aware point-to-source mapping.
- Added wrapped mixed-height selection rectangles.
- Added vertical movement that preserves a logical-pixel x coordinate.
- Added session preferred-x state, revision checks, and reset behavior for horizontal motion, pointer placement, and committed edits.
- Added public document-hidden constructors for integration-test geometry fixtures.

## Self-review

- TokenSave reported no new coupling warnings.
- TokenSave recommended the focused geometry and Unicode suites; both pass.
- Reported session complexity and duplicate-name findings pre-date Task 7 and are outside this task.
- No presentation semantics or Task 8 shaping behavior entered the Task 7 types.
- The two pre-existing `crates/waml-editor/src/app.rs` hunks remain unstaged and unchanged.

## Commit

`feat: define variable metric markdown geometry`
