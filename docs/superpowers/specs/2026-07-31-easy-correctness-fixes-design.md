# Easy correctness fixes design

Date: 2026-07-31

## Goal

Fix two verified, independent editor defects with small changes and focused
regression tests. Keep larger parser, persistence, LSP, delivery, and
architecture decisions out of this batch.

## Constraints

- Work only in the `codex/easy-correctness-fixes` worktree.
- Preserve the dirty primary checkout and its `issues.md` change.
- Use test-driven development: add and run the failing regression before
  production changes.
- Keep the two implementations in non-overlapping ownership groups.
- Do not redesign persistence or tab/navigation architecture in this batch.

## Fix 1: portable configuration replacement

### Problem

`config::store_to` writes `editor.json.tmp` and calls `std::fs::rename` over
the existing destination. Windows does not provide replace-existing behavior
for that call, so the second configuration store can fail.

### Design

- Add an unconditionally compiled regression test that stores two different
  configurations consecutively and reloads the second value.
- Make the existing Unix and Windows `native_save::replace_file` functions
  visible within the crate.
- Call that tested primitive from `config::store_to`.
- Keep the current temp-file naming, serialization, corruption backup, and
  durability behavior unchanged.
- Do not copy the Windows FFI or introduce a new persistence abstraction.

### Owned files

- `crates/waml-editor/src/config.rs`
- `crates/waml-editor/src/native_save.rs`

### Verification

- The new regression must fail for the expected destination-exists reason
  before production code changes on Windows.
- Run the new config test, the existing native replacement test, all editor
  tests, formatting, and strict editor Clippy.

## Fix 2: promote the exact active preview tab

### Problem

`PromoteSubject` pins the first tab whose concept ID matches. A persistent
source tab and a primary preview can share that subject, so an edit in the
active primary preview can pin the source tab and leave the edited preview
replaceable. A class-diagram inspector can also edit a nested subject that does
not identify the diagram tab.

### Design

- Add a regression with a persistent source tab followed by an active primary
  preview for the same concept. Verify that the primary preview is promoted.
- Replace `ViewOutcome::promote_subject: Option<String>` with a Boolean
  `promote_active` intent.
- Capture `DocumentHost::active_id()` when view-outcome processing begins,
  before navigation or source-opening changes the active tab.
- Apply the edit first. Only after a successful edit, send the existing
  `DocumentCommand::Promote(captured_id)`.
- Remove `DocumentCommand::PromoteSubject`.
- Keep all other `DocumentCommand::Promote(LiveId)` behavior unchanged.
- Keep one shell synchronization after promotion.

### Owned files

- `crates/waml-editor/src/document_host.rs`
- `crates/waml-editor/src/doc_view.rs`
- `crates/waml-editor/src/app/actions.rs`
- `crates/waml-editor/src/classifier_preview_view.rs`
- `crates/waml-editor/src/class_diagram_view.rs`

### Verification

- The characterization regression must fail because the earlier source tab is
  promoted before the production refactor.
- Run the new promotion test, the `ViewOutcome` default test, all editor tests,
  formatting, and strict editor Clippy.

## Integration and review

The fixes are independent and can be implemented in parallel. Each worker
must commit only its owned files and provide the red/green command evidence.
After both commits are present, run the full workspace test suite, workspace
format check, and strict workspace Clippy. A final reviewer must examine the
combined branch for scope, correctness, and unintended coupling.

## Deferred work

The following items require separate design work and are not authorized by
this document:

- aggregate shell and specialization diagnostics;
- LSP save, watched-file, and diagnostic-removal lifecycles;
- bundle-envelope format changes;
- native/CLI transaction unification;
- CI and Pages policy;
- per-tab anchors and deferred history restoration;
- parser performance, public API, App, and canvas architecture work.
