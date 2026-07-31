# Whole-plan fix wave A report

## Scope

- `session.rs` and `edit.rs` transaction semantics.
- `document_ops.rs` and `unicode_ime.rs` regressions.
- No widget or layout production changes.

## Fixes

- Stale edits, revision overflow, and selection validation errors preserve active IME composition.
- Successful non-IME edits discard composition only after the transaction commits.
- Undo and redo inspect and prepare the last group before they pop it. Failed replay preserves group
  availability and ordering.
- Direct mutation APIs return `MarkdownEditError::ReadOnly` before changing IME, history, source, or
  selection state. Selection, navigation, copy, scrolling, and focus remain non-mutation paths.
- Typed stale and overflow errors retain their exact revisions.

## TDD and verification

- RED: the new read-only regressions did not compile because `MarkdownEditError::ReadOnly` was absent.
- GREEN: document operations 22 passed; Unicode and IME 16 passed; widget parity 23 passed.
- Full crate reached an unrelated concurrent layout regression:
  `scrolled_visible_window_is_recomputed_after_an_earlier_block_wraps` expected visible block start 0
  and observed 3. Layout files belong to another fix-wave agent and were not edited here.
- `git diff --check` passes. The existing three-line `app.rs` user hunk is unchanged.

