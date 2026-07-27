# Tree Double-Click Regression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore project-tree double-click promotion so a document entry opens or focuses its tab and makes any preview tab persistent.

**Architecture:** Keep `FileTree` as the owner of row-click actions. After the child handles the pointer event, let `ProjectTree` observe that same event through Makepad's capture-overload hit path so it can retain the framework-provided `tap_count`; keep the existing `ProjectTreeAction::OpenDocument { persistent }` and `DocumentHost` transition unchanged.

**Tech Stack:** Rust, Makepad widgets, `waml-editor` unit tests, WAML Markdown documentation.

## Global Constraints

- Single-clicking a document tree entry opens or focuses it as a preview.
- Double-clicking a document tree entry opens or focuses it and removes preview status.
- Double-clicking an already-open preview promotes that same tab without duplicating it.
- Persistent tabs stay persistent and folder open/close behavior stays unchanged.
- Do not change Makepad or update the pinned Makepad dependency.
- Add a regression test that fails when the parent panel uses an ordinary exclusive hit-test after `FileTree` consumes the pointer event.
- Document the user-visible interaction in `docs/waml/`.

---

### Task 1: Restore tap-count observation and document the contract

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `docs/waml/architecture/concepts/runtime/native-editor.md`
- Test: `crates/waml-editor/src/tree_panel.rs`

**Interfaces:**
- Consumes: Makepad `Event::hits_with_capture_overload(cx, area, true)`.
- Produces: the existing `pending_tap_count` value consumed by `document_action(key, node_kind, tap_count)`.

- [ ] **Step 1: Add a failing nested-hit regression test**

Add a focused `tree_panel::tests` characterization test with two overlapping Makepad `Area::Rect` values in a CPU-only `Cx`. Send one primary `MouseDownEvent` through the child area first so the event is handled and captured. Then send the same event through a small private `tree_panel_hit(event, cx, panel_area)` helper and assert it still yields `Hit::FingerDown`. The test must fail against the current ordinary `event.hits` behavior because the child already handled the pointer event.

The production change that must make this test fail is replacing capture-overload observation with ordinary exclusive observation.

- [ ] **Step 2: Verify RED**

Run:

```powershell
rtk cargo test -p waml-editor tree_panel::tests
```

Expected: the nested-hit regression fails because the enclosing panel cannot observe the child-consumed mouse down.

- [ ] **Step 3: Implement the minimal event bridge**

Add the private helper:

```rust
fn tree_panel_hit(event: &Event, cx: &mut Cx, area: Area) -> Hit {
    event.hits_with_capture_overload(cx, area, true)
}
```

Use it for the existing panel hit match after `self.view.handle_event(...)`. Preserve the primary-button guards and every existing header, search, filter, keyboard, folder, right-click, and document-action branch.

- [ ] **Step 4: Verify GREEN**

Run:

```powershell
rtk cargo test -p waml-editor tree_panel::tests
rtk cargo test -p waml-editor document_host::tests
```

Expected: both focused suites pass.

- [ ] **Step 5: Document the interaction**

Under `## Notes` in `docs/waml/architecture/concepts/runtime/native-editor.md`, add:

```markdown
- Project-tree document entries use preview tabs: a single click opens or
  focuses the shared preview, while a double click opens or focuses the same
  tab and makes it persistent. Double-clicking an already-open preview promotes
  it in place; persistent tabs are not duplicated or demoted. Folder expansion
  is unchanged.
```

- [ ] **Step 6: Verify and commit**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml-editor --tests -- -D warnings
rtk cargo test -p waml-editor
rtk git diff --check
```

Expected: all commands pass and the full editor suite remains green.

Commit only the regression implementation, regression test, WAML documentation, and this plan.
