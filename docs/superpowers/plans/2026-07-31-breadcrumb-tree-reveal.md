# Breadcrumb Tree Reveal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a breadcrumb click reveal its logical node in the project tree without document navigation or folder toggling.

**Architecture:** `DocumentHeader` emits a dedicated reveal action. `ProjectTree` validates the target, opens its ancestors, selects it, scrolls it into view, and draws a short pulse. `App` coordinates the reveal with wide and narrow dock state.

**Tech Stack:** Rust, Makepad widgets, WAML editor unit tests.

## Global Constraints

- Use ASD-STE100 Simplified Technical English in specifications, comments, and UI text.
- Do not change the pinned Makepad dependency.
- Do not change tree-row or Markdown-link navigation.
- Do not change the clicked directory's own fold state.
- Keep the tree open after a successful reveal.
- Use test-driven development for each task.
- Prefix every shell command with `rtk`.

## File Structure

- Modify `crates/waml-editor/src/document_header.rs` for the typed breadcrumb action and header tests.
- Modify `crates/waml-editor/src/tree_panel.rs` for target lookup, ancestor expansion, selection, scrolling, pulse drawing, and tree tests.
- Modify `crates/waml-editor/src/app/actions.rs` for application action routing and dock coordination.
- Modify `crates/waml-editor/src/app.rs` for integration tests and test helpers.

---

### Task 1: Emit a Dedicated Header Reveal Action

**Files:**
- Modify: `crates/waml-editor/src/document_header.rs:97-102`
- Modify: `crates/waml-editor/src/document_header.rs:273-280`
- Test: `crates/waml-editor/src/document_header.rs:604-620`
- Test: `crates/waml-editor/src/document_header.rs:760-830`

**Interfaces:**
- Consumes: `NavigationTarget` stored in each `BreadcrumbSegment`.
- Produces: `DocumentHeaderAction::RevealInTree(NavigationTarget)`.

- [ ] **Step 1: Change the header tests to require a reveal action**

Replace breadcrumb action expectations with:

```rust
assert_eq!(
    state.action_at(rect.pos + rect.size * 0.5),
    Some(DocumentHeaderAction::RevealInTree(
        segments[*index].target.clone()
    ))
);
```

Keep the `Navigate` variant temporarily so application code still compiles.

- [ ] **Step 2: Run the focused test and verify that it fails**

Run:

```text
rtk cargo test -p waml-editor document_header::tests::padded_hit_rects_keep_original_navigation_targets
```

Expected result: `FAIL`. The action is still `DocumentHeaderAction::Navigate`.

- [ ] **Step 3: Add the new action and emit it from hit testing**

Add the variant:

```rust
pub enum DocumentHeaderAction {
    Back,
    Forward,
    Navigate(NavigationTarget),
    RevealInTree(NavigationTarget),
    ToggleRightDock,
}
```

Change `DocumentHeaderState::action_at`:

```rust
.map(|segment| DocumentHeaderAction::RevealInTree(segment.target.clone()))
```

- [ ] **Step 4: Run all document header tests**

Run:

```text
rtk cargo test -p waml-editor document_header::tests
```

Expected result: `PASS`.

- [ ] **Step 5: Commit the header action**

```text
rtk git add crates/waml-editor/src/document_header.rs
rtk git commit -m "refactor(header): separate tree reveal action"
```

---

### Task 2: Resolve and Apply Tree Reveal State

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:375-465`
- Modify: `crates/waml-editor/src/tree_panel.rs:1017-1153`
- Test: `crates/waml-editor/src/tree_panel.rs:1260-1345`
- Test: `crates/waml-editor/src/tree_panel.rs:1745-1805`

**Interfaces:**
- Consumes: `&NavigationTarget` and the current `ProjectTreeData`.
- Produces: `ProjectTree::reveal_target(&mut self, &mut Cx, &NavigationTarget) -> bool`.
- Produces for Task 3: `reveal_key`, `pending_scroll_key`, `reveal_strength`, `reveal_started_at`, and `reveal_next_frame`.

- [ ] **Step 1: Write tests for target lookup and ancestor expansion**

Add tests that use `mounted_project_tree_test_context()` and
`nested_search_tree()`:

```rust
#[test]
fn reveal_document_opens_ancestors_selects_target_and_queues_scroll() {
    let (mut cx, mut panel, _) = mounted_project_tree_test_context();
    panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
    panel.open_directories.clear();

    assert!(panel.reveal_target(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "/sales/archive/order".into(),
            fragment: None,
        },
    ));
    assert_eq!(
        panel.open_directories,
        HashSet::from(["/sales".into(), "/sales/archive".into()])
    );
    assert_eq!(panel.selected_key.as_deref(), Some("/sales/archive/order"));
    assert_eq!(
        panel.pending_scroll_key.as_deref(),
        Some("/sales/archive/order")
    );
}

#[test]
fn reveal_directory_preserves_the_target_fold() {
    let (mut cx, mut panel, _) = mounted_project_tree_test_context();
    panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
    panel.open_directories.clear();

    assert!(panel.reveal_target(
        &mut cx,
        &NavigationTarget::Directory {
            address: "/sales/archive".into(),
        },
    ));
    assert_eq!(panel.open_directories, HashSet::from(["/sales".into()]));
}
```

The existing `node` fixture derives the document `concept_id` from its key.
Add one test for `ExternalUrl` and one test for an unknown document. Both tests
must compare all reveal state before and after the call.

- [ ] **Step 2: Run the focused tree tests and verify that they fail**

Run:

```text
rtk cargo test -p waml-editor tree_panel::tests::reveal_
```

Expected result: `FAIL`. `ProjectTree::reveal_target` and its state do not
exist.

- [ ] **Step 3: Add a pure target-path query**

Add a helper beside `directory_addresses`:

```rust
fn reveal_path(
    nodes: &[TreeNode],
    target: &NavigationTarget,
    ancestors: &mut Vec<String>,
) -> Option<(String, Vec<String>)> {
    for node in nodes {
        let matches = match target {
            NavigationTarget::Document { concept_id, .. } => {
                node.concept_id.as_deref() == Some(concept_id.as_str())
            }
            NavigationTarget::Directory { address } => {
                node.is_directory && node.key == address.as_str()
            }
            NavigationTarget::ExternalUrl(_) => false,
        };
        if matches {
            return Some((node.key.clone(), ancestors.clone()));
        }
        if node.is_directory {
            ancestors.push(node.key.clone());
            if let Some(path) = reveal_path(&node.children, target, ancestors) {
                return Some(path);
            }
            ancestors.pop();
        }
    }
    None
}
```

- [ ] **Step 4: Add reveal state and the public tree operation**

Add these `#[rust]` fields to `ProjectTree`:

```rust
reveal_key: Option<String>,
pending_scroll_key: Option<String>,
reveal_strength: f32,
reveal_started_at: f64,
reveal_next_frame: NextFrame,
```

Add the fixed pulse duration:

```rust
const REVEAL_PULSE_SECS: f64 = 0.7;
```

Implement:

```rust
pub fn reveal_target(
    &mut self,
    cx: &mut Cx,
    target: &NavigationTarget,
) -> bool {
    let Some((key, ancestors)) =
        reveal_path(&self.tree.roots, target, &mut Vec::new())
    else {
        return false;
    };
    let file_tree = self.view.file_tree(cx, ids!(file_tree));
    for address in ancestors {
        self.open_directories.insert(address.clone());
        file_tree.set_folder_is_open(
            cx,
            LiveId::from_str(&address),
            true,
            Animate::No,
        );
    }
    self.selected_key = Some(key.clone());
    self.reveal_key = Some(key.clone());
    self.pending_scroll_key = Some(key);
    self.reveal_strength = 1.0;
    self.reveal_started_at = cx.seconds_since_app_start();
    self.reveal_next_frame = cx.new_next_frame();
    self.view.redraw(cx);
    true
}
```

- [ ] **Step 5: Run the focused tree tests**

Run:

```text
rtk cargo test -p waml-editor tree_panel::tests::reveal_
```

Expected result: `PASS`.

- [ ] **Step 6: Run all tree panel tests**

Run:

```text
rtk cargo test -p waml-editor tree_panel::tests
```

Expected result: `PASS`.

- [ ] **Step 7: Commit tree reveal state**

```text
rtk git add crates/waml-editor/src/tree_panel.rs
rtk git commit -m "feat(tree): reveal breadcrumb target"
```

---

### Task 3: Scroll to and Pulse the Revealed Row

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:50-65`
- Modify: `crates/waml-editor/src/tree_panel.rs:600-750`
- Modify: `crates/waml-editor/src/tree_panel.rs:908-1015`
- Test: `crates/waml-editor/src/tree_panel.rs:1260-1345`

**Interfaces:**
- Consumes: reveal state from Task 2.
- Produces: one `scroll_focus_nav` trigger and a 0.7-second row pulse.

- [ ] **Step 1: Write tests for pulse restart and pulse completion**

Add a test-only time helper:

```rust
fn advance_reveal_pulse(panel: &mut ProjectTree, cx: &mut Cx, time: f64) {
    panel.update_reveal_pulse(cx, time);
}
```

Add these tests:

```rust
#[test]
fn repeated_reveal_restarts_the_pulse() {
    let (mut cx, mut panel, _) = mounted_project_tree_test_context();
    panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
    let target = NavigationTarget::Directory {
        address: "/sales/archive".into(),
    };
    assert!(panel.reveal_target(&mut cx, &target));
    let middle = panel.reveal_started_at + 0.5;
    advance_reveal_pulse(&mut panel, &mut cx, middle);
    assert!(panel.reveal_strength < 1.0);

    assert!(panel.reveal_target(&mut cx, &target));
    assert_eq!(panel.reveal_strength, 1.0);
}

#[test]
fn completed_pulse_clears_the_reveal_overlay() {
    let (mut cx, mut panel, _) = mounted_project_tree_test_context();
    panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
    assert!(panel.reveal_target(
        &mut cx,
        &NavigationTarget::Directory {
            address: "/sales/archive".into(),
        },
    ));

    let end = panel.reveal_started_at + REVEAL_PULSE_SECS;
    advance_reveal_pulse(&mut panel, &mut cx, end);
    assert_eq!(panel.reveal_strength, 0.0);
    assert_eq!(panel.reveal_key, None);
}
```

- [ ] **Step 2: Run the pulse tests and verify that they fail**

Run:

```text
rtk cargo test -p waml-editor tree_panel::tests::repeated_reveal_restarts_the_pulse
rtk cargo test -p waml-editor tree_panel::tests::completed_pulse_clears_the_reveal_overlay
```

Expected result: `FAIL`. `update_reveal_pulse` does not exist.

- [ ] **Step 3: Add the pulse overlay to the live design**

Add one draw object to `ProjectTree`:

```rust
draw_reveal: mod.draw.DrawColor {
    color: atlas.accent
}
```

Add the matching fields:

```rust
#[live]
draw_reveal: DrawColor,
#[live]
reveal_color: Vec4,
```

Set `reveal_color: atlas.accent` in the live design.

- [ ] **Step 4: Add pulse timing**

Implement:

```rust
fn update_reveal_pulse(&mut self, cx: &mut Cx, time: f64) {
    let elapsed = (time - self.reveal_started_at).max(0.0);
    self.reveal_strength =
        (1.0 - elapsed / REVEAL_PULSE_SECS).clamp(0.0, 1.0) as f32;
    if self.reveal_strength > 0.0 {
        self.reveal_next_frame = cx.new_next_frame();
    } else {
        self.reveal_key = None;
    }
    self.view.redraw(cx);
}
```

At the start of `handle_event`, handle the matching frame:

```rust
if let Some(frame) = self.reveal_next_frame.is_event(event) {
    self.update_reveal_pulse(cx, frame.time);
}
```

- [ ] **Step 5: Draw the pulse and report whether the target row was drawn**

Extend `draw_nodes` with these inputs:

```rust
draw_reveal: &mut DrawColor,
reveal_color: Vec4,
reveal_key: Option<&str>,
reveal_strength: f32,
) -> bool
```

For the reveal row, draw a second overlay:

```rust
draw_reveal.color = vec4(
    reveal_color.x,
    reveal_color.y,
    reveal_color.z,
    0.24 * reveal_strength,
);
draw_row_highlight(cx, draw_reveal, row_top);
```

Return `true` when the reveal row is drawn. Combine recursive results with
logical OR.

- [ ] **Step 6: Send one smooth focus-scroll trigger after drawing**

After the `self.view.draw_walk` loop, consume the pending request. Send the
existing Makepad trigger only when that draw produced the target row:

```rust
let pending_scroll = self.pending_scroll_key.take();
if reveal_was_drawn && pending_scroll.is_some() {
    let file_tree_area = self.view.file_tree(cx, ids!(file_tree)).area();
    cx.send_trigger(
        file_tree_area,
        Trigger {
            id: live_id!(scroll_focus_nav),
            from: self.draw_selection.area(),
        },
    );
}
```

Do not add a Makepad API. Consuming the request makes a missing row expire.
Do not retry it in a loop.

- [ ] **Step 7: Run tree tests and formatting**

Run:

```text
rtk cargo fmt --all -- --check
rtk cargo test -p waml-editor tree_panel::tests
```

Expected result: both commands pass.

- [ ] **Step 8: Commit scroll and pulse**

```text
rtk git add crates/waml-editor/src/tree_panel.rs
rtk git commit -m "feat(tree): focus and pulse revealed row"
```

---

### Task 4: Coordinate Reveal and Dock State in the Application

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs:40-105`
- Modify: `crates/waml-editor/src/app/actions.rs:546-582`
- Modify: `crates/waml-editor/src/app.rs:3802-3920`
- Modify: `crates/waml-editor/src/app.rs:4024-4100`
- Test: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `DocumentHeaderAction::RevealInTree(NavigationTarget)`.
- Consumes: `ProjectTree::reveal_target(&mut self, &mut Cx, &NavigationTarget) -> bool`.
- Produces: a successful reveal with `DockState::Pinned` for the tree.

- [ ] **Step 1: Write application tests for wide and narrow reveal**

Add test helpers that read project tree state:

```rust
fn mounted_project_tree_state(cx: &Cx, app: &App) -> DockState {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .expect("production shell mounts project_tree")
        .dock_state()
}

fn project_tree_selected_key(cx: &Cx, app: &App) -> Option<String> {
    app.ui
        .widget(cx, ids!(project_tree))
        .borrow::<crate::tree_panel::ProjectTree>()
        .and_then(|tree| tree.test_selected_key().map(str::to_owned))
}
```

Add a `#[cfg(test)]` accessor to `ProjectTree`:

```rust
pub(crate) fn test_selected_key(&self) -> Option<&str> {
    self.selected_key.as_deref()
}
```

Add a wide-layout test:

```rust
#[test]
fn breadcrumb_reveal_pins_tree_without_navigation() {
    let (mut cx, mut app) = navigation_app_with_active_order();
    app.apply_dock_states(&mut cx, DockState::Flag, DockState::Pinned);
    let active = app.documents.active_id();
    let history_len = app.view_history.len();
    let uid = app.ui.widget(&cx, ids!(document_header)).widget_uid();

    app.handle_action_batch(
        &mut cx,
        &[widget_action(
            uid,
            crate::document_header::DocumentHeaderAction::RevealInTree(
                NavigationTarget::Directory {
                    address: "/sales".into(),
                },
            ),
        )],
    );

    assert_eq!(app.documents.active_id(), active);
    assert_eq!(app.view_history.len(), history_len);
    assert_eq!(mounted_project_tree_state(&cx, &app), DockState::Pinned);
    assert_eq!(
        project_tree_selected_key(&cx, &app).as_deref(),
        Some("/sales")
    );
}
```

Add a narrow-layout test. Set `app.narrow = true`. Start with the tree at
`Flag` and the inspector at `Pinned`. Verify that the result is tree `Pinned`
and inspector `Flag`.

Add an unknown-target test. Start with the tree at `Flag`. Send a document
target that is not in the tree. Verify that the tree stays at `Flag`, the
selection does not change, and the active document and history do not change.

- [ ] **Step 2: Run the new application tests and verify that they fail**

Run:

```text
rtk cargo test -p waml-editor breadcrumb_reveal_
```

Expected result: `FAIL`. The application does not handle `RevealInTree`.

- [ ] **Step 3: Route the reveal action**

Rename `handle_document_header_navigation` to
`handle_document_header_action`. Update `EXCLUSIVE_ORDER` dispatch.

Handle the reveal before dock changes:

```rust
Some(crate::document_header::DocumentHeaderAction::RevealInTree(target)) => {
    let accepted = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
        .is_some_and(|mut tree| tree.reveal_target(cx, &target));
    if accepted {
        let (_, inspector) = self.dock_states(cx);
        let inspector = if self.narrow {
            crate::dock::DockState::Flag
        } else {
            inspector
        };
        self.apply_dock_states(
            cx,
            crate::dock::DockState::Pinned,
            inspector,
        );
    }
    ActionFlow::Consumed
}
```

Do not clear history feedback. Do not call `handle_navigation_intent`.

- [ ] **Step 4: Remove the old header navigation variant**

Remove `DocumentHeaderAction::Navigate` from `document_header.rs`. Remove the
header ingress from:

- `navigation_document_ingresses_share_target_and_preview_command`;
- `navigation_directory_intents_share_one_app_owned_toggle_path`.

Rename those tests so they name only the remaining tree and Markdown
ingresses.

- [ ] **Step 5: Run focused and regression tests**

Run:

```text
rtk cargo test -p waml-editor breadcrumb_reveal_
rtk cargo test -p waml-editor navigation_document_ingresses
rtk cargo test -p waml-editor navigation_directory_intents
rtk cargo test -p waml-editor document_header::tests
rtk cargo test -p waml-editor tree_panel::tests
```

Expected result: all commands pass.

- [ ] **Step 6: Run full verification**

Run:

```text
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
rtk cargo test -p waml-editor
```

Expected result: all commands pass with no warnings.

- [ ] **Step 7: Verify the running interaction**

Run:

```text
rtk cargo run -p waml-editor
```

Open a nested document. Close the tree. Click a breadcrumb directory and then
the current document crumb. Verify these results:

- The tree opens and stays open.
- The row is visible in the viewport.
- Its parents are open.
- The target folder keeps its previous fold state.
- The row shows one short pulse.
- The active document does not change.

Capture the window:

```text
rtk pwsh -File scripts/capture-window.ps1 -Out breadcrumb-tree-reveal.png -Process waml-editor
```

- [ ] **Step 8: Commit the application integration**

```text
rtk git add crates/waml-editor/src/document_header.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/app.rs
rtk git commit -m "feat(header): reveal crumbs in project tree"
```
