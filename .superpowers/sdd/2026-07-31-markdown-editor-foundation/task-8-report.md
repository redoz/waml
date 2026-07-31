# Task 8 Report

## Scope

- Added the layout engine, text-shaper contract, Makepad shaper adapter, block summaries, viewport virtualization, width invalidation, and per-block plain-text fallback.
- Extended layout snapshots with visible visual lines and full-document block summaries.
- Kept source positions and geometry element identities stable across wrapping.

## TDD evidence

- RED: `rtk cargo test -p waml-markdown-editor --test layout_geometry` failed because the Task 8 layout-engine interfaces did not exist.
- GREEN: the same test target passed all 9 tests after implementation.

## Verification

- `rtk cargo test -p waml-markdown-editor --test layout_geometry`: 9 passed.
- `rtk cargo test -p waml-markdown-editor --test unicode_ime`: 8 passed.
- `rtk cargo test -p waml-markdown-editor`: 35 passed.
- `rtk git diff --check`: passed.

## Preservation

- The two pre-existing unstaged hunks in `crates/waml-editor/src/app.rs` remain unchanged and are not part of Task 8.

## Important review fixes

- RED: three regressions failed with a zero nested x-origin, an accepted revision-9 syntax update for a revision-8 document, and a collapsed mixed-direction boundary affinity.
- GREEN: nested document, block, and ancestor insets now offset visual lines, clusters, carets, and selection rectangles.
- GREEN: syntax invalidation rejects an update snapshot whose revision does not own the current layout presentation.
- GREEN: shaped bidi levels control visual cluster order and direction-aware caret placement. The Makepad adapter no longer infers bidi levels from glyph order.
- Review verification: `layout_geometry` 12 passed, `unicode_ime` 8 passed, full crate 38 passed, and `git diff --check` passed.
