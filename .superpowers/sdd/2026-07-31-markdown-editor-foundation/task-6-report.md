# Task 6 Report: IME Composition

## Status

Complete. The editor keeps IME preedit text as uncommitted session state. Commit publishes one replacement edit. Cancel restores the captured snapshot and selection.

## TDD Evidence

- RED: `rtk cargo test -p waml-markdown-editor --test unicode_ime` failed with nine missing IME API errors.
- GREEN: `rtk cargo test -p waml-markdown-editor --test unicode_ime` passed 7 tests.
- Gate: `rtk cargo test -p waml-markdown-editor --test document_ops` passed 16 tests.
- Full crate: `rtk cargo test -p waml-markdown-editor` passed 25 tests.

The test counts are higher than the brief because Task 5 added an affinity regression before Task 6 started.

## Implementation

- Added `ImeComposition` and `ImeError` with UTF-16 selection validation.
- Added begin, update, commit, cancel, and state access APIs to `MarkdownDocumentSession`.
- Kept preedit updates outside the committed snapshot and revision stream.
- Made commit publish one `TextChange` and clear composition state.
- Made cancel restore the captured `Arc` snapshot and selection without a revision change.
- Made normal committed edit paths cancel an active composition first.

## Self-review

- TokenSave found no new duplication, dead code, complexity, or coupling warnings.
- `git diff --check` passed.
- The pre-existing unstaged `crates/waml-editor/src/app.rs` changes were not modified, staged, or reverted.

## Commit

`feat: add markdown IME composition`
