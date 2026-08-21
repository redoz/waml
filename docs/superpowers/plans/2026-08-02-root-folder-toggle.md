# Root Folder Toggle Implementation Plan

## Status — 2026-08-21: PARTIAL

Triage verdict from the A39 planning-hygiene pass.

**Landed** — the "without resetting navigation or dock state" half. Root
activation no longer resets scope, query, filter or docks; the two tests this
plan names for deletion
(`navigation_root_restores_scope_and_clears_query_and_filter`,
`navigation_root_uses_narrow_mutual_exclusion_and_preserves_wide_inspector`)
are gone and
`crates/waml-editor/src/app/tests/navigation.rs:1525`
`navigation_root_opens_the_folder_view_without_resetting_navigation_or_docks`
stands in their place. `ProjectTree::toggle_directory` remains the single
folder-state owner (`crates/waml-editor/src/tree_panel.rs:1243`).

**Did not land, and the goal moved** — "activating `/` always toggles its
folder state". Activating the root now *opens the root folder view* instead,
because `2026-08-05-folder-view-middleware.md`
(`completed/2026-08-05-folder-view-middleware.md`) made a directory a
first-class document surface. The plan's Task 1 regression test
(`navigation_root_toggles_without_resetting_navigation_or_docks`) does not
exist.

**Decide before implementing:** whether root should toggle *or* open the
folder view is now a product question, not a bug. If open-the-view is the
intended behaviour, close this plan out; if both are wanted, this plan needs
rewriting against the folder-view surface.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the project-tree root directory toggle open and closed without resetting application navigation or dock state.

**Architecture:** Keep `ProjectTree` as the single owner of folder open state. Remove the root-only branch at the application navigation boundary so `/` follows the same `ProjectTree::toggle_directory` path as all other directory addresses.

**Tech Stack:** Rust, Makepad widgets, Cargo test framework.

## Global Constraints

- Activating `/` always toggles its folder state.
- Root activation preserves scope, query, filter, active document, and dock state.
- Non-root directory behavior and the shared directory toggle path do not change.
- Do not add a new action type or duplicate folder-state owner.

---

### Task 1: Route the root directory through the shared toggle path

**Files:**
- Modify and test: `crates/waml-editor/src/app.rs:1033, 4687`

**Interfaces:**
- Consumes: `App::navigate_with`, `NavigationTarget::Directory`, and `ProjectTree::toggle_directory(&mut self, &mut Cx, &str) -> bool`.
- Produces: One directory match arm in `App::navigate_with` that handles `/` and non-root addresses identically.

- [ ] **Step 1: Replace the obsolete root-reset tests with a failing root-toggle regression test**

Replace `navigation_root_restores_scope_and_clears_query_and_filter` and `navigation_root_uses_narrow_mutual_exclusion_and_preserves_wide_inspector` with:

```rust
#[test]
fn navigation_root_toggles_without_resetting_navigation_or_docks() {
    let (mut cx, mut app) = navigation_app();
    let mut browser = FakeBrowser::default();
    app.narrow = true;
    app.nav_state = NavState {
        scope: "/sales".into(),
        query: "order".into(),
        filter: Some(TreeKind::Class),
    };
    app.ui
        .widget(&cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
        .expect("test project tree is mounted")
        .close_dock(&mut cx);
    app.ui
        .widget(&cx, ids!(inspector))
        .borrow_mut::<crate::inspector_panel::Inspector>()
        .expect("test inspector is mounted")
        .open_dock(&mut cx);
    let expected_nav = app.nav_state.clone();
    let expected_document = app.documents.active_id();
    let expected_docks = app.dock_states(&mut cx);

    assert!(project_tree_folder_is_open(&mut cx, &app, "/"));
    for expected_open in [false, true] {
        assert!(app.navigate_with(
            &mut cx,
            NavigationTarget::Directory {
                address: "/".into(),
            },
            OpenDisposition::Preview,
            &mut browser,
        ));
        assert_eq!(
            project_tree_folder_is_open(&mut cx, &app, "/"),
            expected_open
        );
        assert_eq!(app.nav_state, expected_nav);
        assert_eq!(app.documents.active_id(), expected_document);
        assert_eq!(app.dock_states(&mut cx), expected_docks);
    }
}
```

- [ ] **Step 2: Run the focused test and verify the RED state**

Run:

```powershell
rtk cargo test -j 1 -p waml-editor navigation_root_toggles_without_resetting_navigation_or_docks
```

Expected: FAIL after the first root activation because the root remains open, navigation state resets, or dock state changes.

- [ ] **Step 3: Remove the root-only branch from `App::navigate_with`**

Delete this match arm:

```rust
crate::navigation::NavigationTarget::Directory { address } if address == "/" => {
    self.nav_state.scope = "/".into();
    self.nav_state.query.clear();
    self.nav_state.filter = None;
    let (_, inspector) = self.dock_states(cx);
    let inspector = if self.narrow {
        crate::dock::narrow_entry_states(DockState::Pinned, inspector).1
    } else {
        inspector
    };
    self.apply_dock_states(cx, DockState::Pinned, inspector);
    self.refresh_nav(cx, true);
    self.set_navigation_message(cx, None);
    true
}
```

Keep the following directory arm unchanged so it accepts every address:

```rust
crate::navigation::NavigationTarget::Directory { address } => {
    let toggled = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
        .is_some_and(|mut tree| tree.toggle_directory(cx, &address));
    if toggled {
        self.set_navigation_message(cx, None);
    }
    toggled
}
```

- [ ] **Step 4: Run the focused test and verify the GREEN state**

Run:

```powershell
rtk cargo test -j 1 -p waml-editor navigation_root_toggles_without_resetting_navigation_or_docks
```

Expected: PASS with one matching test and zero failures.

- [ ] **Step 5: Run editor regression tests and formatting verification**

Run:

```powershell
rtk cargo test -j 1 -p waml-editor
rtk cargo fmt --all -- --check
```

Expected: Both commands exit with code 0.

- [ ] **Step 6: Commit the implementation**

```powershell
rtk git add -- crates/waml-editor/src/app.rs
rtk git commit -m "fix(tree): toggle the root folder"
```
