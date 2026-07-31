# Markdown Editor Foundation SDD Ledger

## Task 6 clean re-review

- Re-reviewed at base `5aec24cf`.
- Task 6 implementation and typed IME commit failure fix are committed.
- The only unstaged production changes are the two pre-existing hunks in `crates/waml-editor/src/app.rs`.
- No Task 6 follow-up remains open.

## Task 7 start

- Started from base `5aec24cf`.
- Scope: variable-metric layout geometry and exact queries only.
- RED: `layout_geometry` failed with unresolved Task 7 geometry imports.
- GREEN: geometry queries, preferred-pixel session motion, reset behavior, and revision rejection pass.
- Verification: `layout_geometry` 5 passed, `unicode_ime` 8 passed, full crate 31 passed.

## Task 7 approval

- Approved at commit `e0549f4b`.
- No Task 7 follow-up remains open.

## Task 8 start

- Started from base `e0549f4b`.
- Scope: shaping, wrapping, fallback, incremental summaries, and viewport virtualization only.
- RED: `layout_geometry` failed with unresolved Task 8 layout-engine imports.
- GREEN: mixed-metric wrapping, viewport virtualization, width-only rewrap, and editable fallback pass.
- Verification: `layout_geometry` 9 passed, `unicode_ime` 8 passed, full crate 35 passed.

## Task 8 final approval

- Approved at commit `4f7e419e` after the geometry-contract, authoritative-bidi, and double-reorder fixes.
- No Task 8 follow-up remains open.

## Task 9 start

- Started from base `4f7e419e`.
- Scope: platform-neutral input, retained selection behavior, read-only behavior, and caret scrolling only.
- RED: `widget_parity` failed with unresolved Task 9 controller/input/scroll types and missing `set_read_only`.
- GREEN: retained pointer gestures, additive and extended selection, read-only copy/mutation suppression, and geometry-based caret scrolling pass.
- Verification: `widget_parity` 4 passed, `layout_geometry` 12 passed, `unicode_ime` 8 passed, full crate 44 passed.
