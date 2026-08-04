# Issue 32 — Stale #[allow(dead_code)] scaffolding defeats the -D warnings gate

## Context

The workspace gate runs clippy with `-D warnings`, which promotes `dead_code` to a
hard error. That makes the lint a real detector — but only where no `#[allow(dead_code)]`
sits on top of it. `crates/waml-editor/src/editor_session.rs` carries six allows annotated
"Task 2 API is mounted by the editor integration in Task 4". Task 4 landed:
`app/actions.rs:961` calls `promote_source_edit` and `app/actions.rs:967` calls
`install_semantic_completion`. The allows are now pure scaffolding that suppresses future
dead-code detection — notably the struct-level allow on the 13-field `EditorSessionSnapshot`,
which silences *any* field that later becomes unused.

Crate-wide, waml-editor has 165 `#[allow(dead_code)]` occurrences across 44 files
(popup/radial.rs: 19, frame.rs: 17, overlay_shell.rs: 15, editor_session.rs: 10). Nothing
retires them; each one is a permanent blind spot in the gate.

## Verdict evidence (verified 2026-08-04, worktree HEAD)

Stale "Task 4" allows still present in `crates/waml-editor/src/editor_session.rs`:
- :35  on `EditorSessionSnapshot` (struct-level, blankets all 13 fields)
- :210 on `ProposedSourceEdit`
- :243 on `SourceEditError`
- :352 on `SessionChange::source_only`
- :741 on `EditorSession::promote_source_edit`
- :874 on `EditorSession::install_semantic_completion`

Consumers landed:
- `crates/waml-editor/src/app/actions.rs:961` → `promote_source_edit(...)`
- `crates/waml-editor/src/app/actions.rs:967` → `install_semantic_completion(...)`
- Plus ~40 test call sites in editor_session.rs itself.

Also stale in the same file (same Task-2 scaffolding era, no comment):
- :118 impl-level allow on `EditorSessionSnapshot`
- :230 impl-level allow on `ProposedSourceEdit`
- :654 `EditorSession::source`, :664 `EditorSession::persisted_bundle`

## Ordering / conflict flags

- **`crates/waml-editor/src/editor_session.rs` is also edited by issue 36
  Task 1**, which moves the file's 2,390-line inline `mod tests` into
  `editor_session/tests.rs`. **Land this plan first**: removing the allows may
  require deleting genuinely dead items or narrowing an allow, and doing that
  while the test module is mid-move makes the gate output far harder to read.
  Order: **32 → 36 (T1)**.

## Design decisions

1. **Scope**: remove the six annotated allows plus the four unannotated companions in
   editor_session.rs (ten total in the file). Do NOT attempt the full 165-site crate sweep
   in this plan — that is a separate, larger effort; this plan restores detection where the
   stated justification is provably expired and establishes the convention.
2. **Gate discipline**: removal may reveal genuinely dead items (e.g. an unused
   `EditorSessionSnapshot` field, an unused `SourceEditError` variant, an unbuilt helper).
   Each revelation is handled per-item: delete it if it is dead scaffolding, or re-add a
   *narrow* allow (on the specific item, never the struct/impl) with a comment naming a
   concrete unlanded consumer. No blanket re-allow.
3. **Convention**: record in `.claude/rules/maintainability.md` that an
   `#[allow(dead_code)]` must name a concrete unlanded consumer, and landing that consumer
   removes the allow in the same commit.
4. Items used only by `#[cfg(test)]` code still count as dead to rustc; if one surfaces,
   prefer moving it under `#[cfg(test)]` over re-allowing.

### Task 1: Remove the stale allows in editor_session.rs

- In `crates/waml-editor/src/editor_session.rs`, delete the `#[allow(dead_code)]`
  attributes (and their trailing comments) at lines 35, 118, 210, 230, 243, 352, 654,
  664, 741, 874 (line numbers as of this draft — locate by attribute + item name, not
  raw line).
- Build with the gate lint: `cargo clippy -p waml-editor --all-targets -- -D warnings`.
- For each dead-code error that surfaces, decide per design decision 2: delete the dead
  item (preferred) or re-add a narrow, item-level allow with a comment naming the concrete
  unlanded consumer. List every such decision in the commit message body.
- Tests: `cargo test -p waml-editor` must stay green (editor_session.rs has a large
  in-file test module exercising promote_source_edit / install_semantic_completion).

### Task 2: Record the allow convention

- Add one bullet to `.claude/rules/maintainability.md` under "Design Quality":
  an `#[allow(dead_code)]` must name a concrete unlanded consumer in its comment;
  the commit that lands the consumer removes the allow; struct-/impl-level blanket
  allows are not acceptable — allow the specific item.
- No code change; no test.

### Task 3: Full workspace gate

- Run the full gate: `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- Note (memory): local main gate has 2 pre-existing icon-table test failures unrelated
  to this change — verify any red test fails identically before the diff (stash + re-run)
  before attributing it to this change.
