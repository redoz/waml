# Task 9 Report

## Scope

- Added typed platform-neutral editor input, key, pointer gesture, response, and controller interfaces.
- Lowered text, paste, cut, navigation, undo, redo, and IME input through the existing session command authority.
- Added retained click, drag, word, source-line, additive-selection, and extension behavior through layout hit testing.
- Added session-owned scroll state, caret visibility adjustment, scroll-anchor capture, and geometry-based anchor restoration.
- Added read-only selection and copy behavior with mutation suppression.

## TDD evidence

- RED: `rtk cargo test -p waml-markdown-editor --test widget_parity` failed with unresolved Task 9 controller/input/scroll types and missing `MarkdownDocumentSession::set_read_only`.
- GREEN: the same test target passed all 4 retained-behavior tests after implementation.

## Verification

- `rtk cargo test -p waml-markdown-editor --test widget_parity`: 4 passed.
- `rtk cargo test -p waml-markdown-editor --test layout_geometry`: 12 passed.
- `rtk cargo test -p waml-markdown-editor --test unicode_ime`: 8 passed.
- `rtk cargo test -p waml-markdown-editor`: 44 passed.
- `rtk git diff --check`: passed.
- Task 9 brief matches the approved plan section exactly.

## Preservation

- The two pre-existing unstaged hunks in `crates/waml-editor/src/app.rs` remain unchanged and are not part of Task 9.
