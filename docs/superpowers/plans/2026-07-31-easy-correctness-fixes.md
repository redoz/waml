# Easy Correctness Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix portable repeated configuration writes and ensure inline edits pin the exact active preview tab.

**Architecture:** Reuse the editor's existing platform replacement primitive instead of adding another filesystem authority. Replace subject-based preview promotion with a shell-owned Boolean intent that resolves to the active tab ID captured before other outcome handling can change tab identity.

**Tech Stack:** Rust 2021 workspace, Makepad editor, serde JSON configuration, platform-specific Windows file replacement, Cargo tests, rustfmt, Clippy.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\easy-correctness-fixes` on `codex/easy-correctness-fixes`.
- Read `AGENTS.md` and `RTK.md` before work, use TokenSave before source inspection, and prefix every shell command with `rtk`.
- Preserve the dirty primary checkout and its `issues.md` change.
- Write and run each failing regression before changing production code.
- Do not edit files outside the task's ownership list.
- Do not redesign persistence, navigation, or tabs in this batch.
- Do not modify the wire format, public crate API, or persisted WAML documents.
- Commit each task separately after its focused verification passes.

---

### Task 1: Portable repeated configuration writes

**Files:**
- Modify: `crates/waml-editor/src/config.rs:62-68,533-545`
- Modify: `crates/waml-editor/src/native_save.rs:318-348`
- Test: `crates/waml-editor/src/config.rs`

**Interfaces:**
- Consumes: existing `native_save::replace_file(temp: &Path, target: &Path) -> io::Result<()>` platform implementations.
- Produces: crate-visible `native_save::replace_file` and a `store_to` implementation that replaces an existing target on Windows and Unix.

- [ ] **Step 1: Add the repeated-store regression**

Add this test after `store_to_then_load_from_round_trips` in `config.rs`:

```rust
#[test]
fn store_to_twice_replaces_existing_file() {
    let tmp = TempDir::new();
    let first = EditorConfig {
        version: EDITOR_VERSION,
        recents: Vec::new(),
        theme: ThemeMode::Light,
    };
    let second = EditorConfig {
        version: EDITOR_VERSION,
        recents: vec![rec("/second", 2)],
        theme: ThemeMode::Dark,
    };

    store_to(tmp.path(), EDITOR_FILE, &first).unwrap();
    store_to(tmp.path(), EDITOR_FILE, &second).unwrap();

    let back: EditorConfig = load_from(tmp.path(), EDITOR_FILE);
    assert_eq!(back, second);
}
```

- [ ] **Step 2: Run the regression and verify RED**

Run:

```text
rtk cargo test -p waml-editor config::tests::store_to_twice_replaces_existing_file -- --exact
```

Expected on Windows: FAIL at the second `store_to(...).unwrap()` because
`editor.json` already exists. Confirm that the failure is an existing-target
replacement error, not a compile or test-setup error.

- [ ] **Step 3: Expose the existing platform replacement primitive**

Change the declaration of both cfg-selected functions in `native_save.rs`
without altering either body:

```diff
-fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
+pub(crate) fn replace_file(temp: &Path, target: &Path) -> io::Result<()> {
```

Apply this declaration-only change once under `#[cfg(not(windows))]` and once
under `#[cfg(windows)]`.

- [ ] **Step 4: Route configuration replacement through the primitive**

Replace the final line of `store_to` with:

```rust
    crate::native_save::replace_file(&tmp, &dir.join(file))
```

Do not change serialization, temporary-file naming, directory creation, or
corrupt-file backup behavior.

- [ ] **Step 5: Verify GREEN and adjacent replacement behavior**

Run:

```text
rtk cargo test -p waml-editor config::tests::store_to_twice_replaces_existing_file -- --exact
rtk cargo test -p waml-editor native_save::tests::existing_file_is_replaced_with_bundle_contents -- --exact
```

Expected: both PASS.

- [ ] **Step 6: Verify the editor crate**

Run:

```text
rtk cargo test -p waml-editor
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: all commands exit 0 with no warnings.

- [ ] **Step 7: Commit the task**

Stage only the two owned files and commit:

```text
rtk git add crates/waml-editor/src/config.rs crates/waml-editor/src/native_save.rs
rtk git commit -m "fix: replace existing editor config"
```

Record the RED failure, GREEN commands, commit hash, and final status in the
task report.

---

### Task 2: Promote the exact active preview tab

**Files:**
- Modify: `crates/waml-editor/src/document_host.rs:10-19,60-94,569-595`
- Modify: `crates/waml-editor/src/doc_view.rs:226-248,483-494`
- Modify: `crates/waml-editor/src/app/actions.rs:1020-1065`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs:75-83`
- Modify: `crates/waml-editor/src/class_diagram_view.rs:427-435`
- Test: `crates/waml-editor/src/document_host.rs`
- Test: `crates/waml-editor/src/doc_view.rs`

**Interfaces:**
- Consumes: `DocumentHost::active_id() -> LiveId` and existing `DocumentCommand::Promote(LiveId)`.
- Produces: `ViewOutcome { promote_active: bool, .. }`; removes `ViewOutcome::promote_subject` and `DocumentCommand::PromoteSubject`.

- [ ] **Step 1: Add the source-before-primary characterization regression**

Add this test after
`locator_lookup_distinguishes_primary_and_source_tabs_for_one_concept` in
`document_host.rs`:

```rust
#[test]
fn promotion_request_pins_active_primary_preview_not_earlier_source_tab() {
    let mut host = DocumentHost::default();

    let mut source =
        prepared("order", NavCategory::OkfDocument, Rc::new(Cell::new(0)));
    source.tab_id = crate::okf_documents::source_document_tab_id("order");
    source.kind = crate::view_history::DocumentKind::Source;
    host.apply_command(DocumentCommand::Open {
        document: source,
        persistent: true,
    });

    let primary = prepared("order", NavCategory::Class, Rc::new(Cell::new(0)));
    let primary_id = primary.tab_id;
    host.apply_command(DocumentCommand::Open {
        document: primary,
        persistent: false,
    });

    assert_eq!(host.active_id(), primary_id);
    assert!(host.active_tab().unwrap().preview);

    host.apply_command(DocumentCommand::PromoteSubject("order".into()));

    let primary = host
        .tabs()
        .iter()
        .find(|tab| tab.id == primary_id)
        .unwrap();
    assert!(!primary.preview);
}
```

- [ ] **Step 2: Run the characterization regression and verify RED**

Run:

```text
rtk cargo test -p waml-editor promotion_request_pins_active_primary_preview_not_earlier_source_tab
```

Expected: FAIL at `assert!(!primary.preview)`. The earlier source tab matches
the subject first, so the active primary remains a preview.

- [ ] **Step 3: Replace subject promotion with active-tab intent**

In `doc_view.rs`, replace the subject field and comment with:

```rust
    /// Ask the shell to promote (pin) the tab that was active when this
    /// outcome entered shell processing.
    pub promote_active: bool,
```

Update `view_outcome_default_is_all_empty`:

```rust
assert!(!o.promote_active);
```

In both `classifier_preview_view.rs` and `class_diagram_view.rs`, replace the
subject-producing branch with this shape:

```rust
if body
    .inspector(cx)
    .borrow_mut::<crate::inspector_panel::Inspector>()
    .and_then(|inspector| inspector.edited(actions))
    .is_some()
{
    out.promote_active = true;
    return out;
}
```

- [ ] **Step 4: Remove subject lookup from `DocumentHost`**

Remove this enum variant:

```rust
PromoteSubject(String),
```

Remove its complete match arm from `DocumentHost::apply_command`. Do not change
`DocumentCommand::Promote(LiveId)`.

Update the regression stimulus to use exact active identity:

```rust
let active_id = host.active_id();
host.apply_command(DocumentCommand::Promote(active_id));
```

Keep all setup and final assertions unchanged.

- [ ] **Step 5: Resolve the Boolean intent in the shell**

At the start of `apply_view_outcome`, before processing `outcome.edit`, capture:

```rust
let outcome_active_id = self.documents.active_id();
```

Track whether a supplied edit succeeded:

```rust
let edit_succeeded = if let Some(edit) = outcome.edit {
    self.apply_session_edit(cx, edit, "view edit failed").is_some()
} else {
    true
};
```

Replace the `promote_subject` block with:

```rust
if outcome.promote_active && edit_succeeded {
    self.documents.transition(
        cx,
        &self.ui,
        &self.session,
        DocumentCommand::Promote(outcome_active_id),
    );
    self.sync_document_shell(cx);
    flow = ActionFlow::Consumed;
}
```

The ID must be captured before navigation or `view_source`. A failed supplied
edit must not pin the preview. Outcomes with no edit retain promotion behavior.

- [ ] **Step 6: Verify GREEN and adjacent outcome behavior**

Run:

```text
rtk cargo test -p waml-editor promotion_request_pins_active_primary_preview_not_earlier_source_tab
rtk cargo test -p waml-editor view_outcome_default_is_all_empty
```

Expected: both PASS.

- [ ] **Step 7: Verify the editor crate**

Run:

```text
rtk cargo test -p waml-editor
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: all commands exit 0 with no warnings.

- [ ] **Step 8: Commit the task**

Stage only the five owned files and commit:

```text
rtk git add crates/waml-editor/src/document_host.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/class_diagram_view.rs
rtk git commit -m "fix: promote exact active preview tab"
```

Record the RED failure, GREEN commands, commit hash, and final status in the
task report.

---

## Combined branch verification

After both tasks and their task reviews are complete, run from the worktree:

```text
rtk cargo fmt --all -- --check
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk git diff --check efa865ca7baf8472f41419755c30f51131dfd389..HEAD
rtk git status --short --branch
```

Expected: formatting, 1,606-or-more workspace tests, strict Clippy, and diff
check all pass; the branch is clean. Dispatch a final whole-branch correctness
review before integration.
