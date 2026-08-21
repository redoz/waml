Exit code: 0
Wall time: 2.2 seconds
Output:
# App Composition Root Reorganization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize `crates/waml-editor/src/app.rs` into focused child modules while `App` remains the single composition root and all current behavior stays unchanged.

**Architecture:** Keep the `App` state, Makepad root UI DSL, `MatchEvent`, `AppMain`, and the visible raw-event pipeline in `app.rs`. Move cohesive `impl App` blocks into child modules under `src/app/`, following the existing `app/actions.rs` pattern. Unify the shared post-session-change projection work and put Markdown asset-host ownership with the workspace lifecycle, but do not add controllers, traits, or a handler registry.

**Tech Stack:** Rust 2024, Makepad widgets and script DSL, existing `waml-editor` unit and mounted-widget tests, Cargo through RTK.

## Global Constraints

- Preserve all native and WebAssembly behavior.
- Keep `App` as the composition root. Do not introduce `NavigationController`, `DockManager`, `WorkspaceController`, or equivalent facade types.
- Keep `EditorSession` as the model mutation authority.
- Keep `DocumentHost` as the owner of open tabs, the active document, and document-view reconciliation.
- Keep `transition_to_location` as the navigation choke point.
- Preserve the observer and exclusive action order in `app/actions.rs`.
- Preserve the visible order of the phases in `AppMain::handle_event`.
- Do not add a handler registry, event bus, new trait, or new dependency.
- Use `pub(super)` only for methods and values that must cross `app` child-module boundaries. Do not widen the crate's public API.
- Keep the root `script_mod!` UI definition and widget registration order in `app.rs` in this plan. A later change can move the DSL only if Makepad macro scoping permits it without re-exports or duplicate registration.
- Use characterization tests for mechanical moves. Add behavior tests only when a task changes a choke point.
- Run all shell commands through `rtk`.

## Emerged Design

The initial file grew around one application object, but several stable responsibilities now exist:

1. Navigation owns locations, transitions, pending fragment and anchor restoration, view history, and navigation-tree projection.
2. Shell layout owns overlays, responsive dock state, caption interaction regions, status projection, and agent-row geometry.
3. Workspace lifecycle owns save, open, replacement, close, start-screen transitions, and the Markdown asset host attached to an open session.
4. Menu models are pure application presentation data and do not need access to `App` state.
5. Raw event handling is an ordered pipeline. Its order is behavior, not incidental control flow.
6. Session reconciliation has two legitimate ingress paths, but both must use one projection sequence after their path-specific `DocumentHost` update.

The high fan-out of `App` is expected for a composition root. This plan improves physical ownership and removes competing lifecycle sequences. It does not try to make the composition root unaware of its children.

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/waml-editor/src/app.rs` | Declare `App`, hold application state, define the root UI DSL, register widgets, implement `MatchEvent` and `AppMain`, and show the ordered event pipeline. |
| `crates/waml-editor/src/app/actions.rs` | Keep ordered action observation and exclusive action dispatch; own action-only pure mappings such as conflict deletion. |
| `crates/waml-editor/src/app/navigation.rs` | Own navigation messages, history controls, location transitions, pending reveal restoration, document close navigation, and tree projection. |
| `crates/waml-editor/src/app/shell.rs` | Own page overlays, responsive dock layout, document-shell projection, status projection, caption geometry helpers, and agent marks. |
| `crates/waml-editor/src/app/workspace.rs` | Own save, open, external replacement, close, start-screen lifecycle, recent labels, and Markdown asset-host installation and rollback. |
| `crates/waml-editor/src/app/menus.rs` | Own menu item builders, `LogoCommand`, document-switcher item projection, and menu constants. |
| `crates/waml-editor/src/app/event.rs` | Own named raw-event phase helpers while `AppMain::handle_event` keeps their order visible. |
| `crates/waml-editor/src/app/tests/mod.rs` | Declare app test groups and hold only helpers shared by two or more groups. |
| `crates/waml-editor/src/app/tests/navigation.rs` | Test navigation, history, document-header projection, fragment restoration, and breadcrumb reveal. |
| `crates/waml-editor/src/app/tests/shell.rs` | Test mounted shell geometry, docks, caption hit regions, responsive breakpoints, and overlays. |
| `crates/waml-editor/src/app/tests/workspace.rs` | Test save, replacement, open rollback, close, and start-screen lifecycle. |
| `crates/waml-editor/src/app/tests/menus.rs` | Test menu mappings, document-switcher projection, and conflict action mapping. |

---

### Task 1: Extract the Inline App Tests

**Files:**
- Modify: `crates/waml-editor/src/app.rs:3027-5675`
- Create: `crates/waml-editor/src/app/tests/mod.rs`
- Create: `crates/waml-editor/src/app/tests/navigation.rs`
- Create: `crates/waml-editor/src/app/tests/shell.rs`
- Create: `crates/waml-editor/src/app/tests/workspace.rs`
- Create: `crates/waml-editor/src/app/tests/menus.rs`

**Interfaces:**
- Consumes: Existing private `app` items through Rust descendant-module visibility.
- Produces: The same test bodies under `app::tests::{navigation,shell,workspace,menus}`.
- Produces: Shared test helpers in `app::tests`, imported by each child with `use super::*;`.

- [ ] **Step 1: Capture the current app-test baseline**

Run:

```powershell
rtk cargo test -p waml-editor app::tests
```

Expected: PASS. Save the test count in the task report so the post-move run can be compared with it.

- [ ] **Step 2: Replace the inline module with a file-backed test module**

Replace the inline declaration at the end of `app.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Create `app/tests/mod.rs` with this initial module structure:

```rust
use super::*;
use crate::doc_tabs::{DocTab, OpenTabs};
use crate::doc_view::{BodyWidgets, DocView, DocViewIdentity, ViewData};
use crate::dock::DockState;
use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
use crate::document_host::DocumentCommand;
use crate::icons::{Icon, IconSet};
use crate::nav::NavState;
use crate::navigation::{
    BreadcrumbSegment, NavigationIntent, NavigationTarget, OpenDisposition,
};
use crate::platform_browser::ExternalUrlAdapter;
use crate::popup::conflict_list::ConflictListAction;
use crate::tree::TreeKind;
use crate::view_history::{HistoryDirection, ViewAnchor, ViewLocation};
use makepad_widgets::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use waml_markdown_editor::layout::LayoutSnapshot;
use waml_markdown_editor::widget::MarkdownEditorWidgetRefExt;
use waml_syntax::{SourceText, TextChange, TextRange, TextSize};

mod menus;
mod navigation;
mod shell;
mod workspace;
```

Move helpers used by two or more groups into `mod.rs`. Keep group-specific probes and fixtures beside the tests that use them.

- [ ] **Step 3: Move the tests by responsibility without editing their bodies**

Use these assignments:

```text
navigation.rs
  navigation_app_with_anchor_probe through the navigation/history tests
  breadcrumb reveal tests
  pending fragment and anchor restore tests
  document header projection and mounted header tests
  source-range and Markdown navigation tests

shell.rs
  mounted production shell and dock helpers
  dock geometry and tab-row alignment tests
  narrow/wide breakpoint tests
  caption client-area tests

workspace.rs
  failed_open_restores_the_previous_markdown_asset_root
  browser save encoding tests
  final save, replacement, close, and save-error tests

menus.rs
  document switcher item tests
  conflict-delete mapping tests
  logo command mapping tests
```

Do not rename tests. Use `use super::*;` at the top of each group file. If one helper is used by only one group, move it into that group instead of exporting it from `mod.rs`.

- [ ] **Step 4: Format and verify that the test inventory is unchanged**

Run:

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::tests
```

Expected: PASS with the same number of app tests as Step 1.

- [ ] **Step 5: Commit the test-only move**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/tests
rtk git commit -m "refactor(editor): split app tests by concern"
```

---

### Task 2: Extract Navigation and View History

**Files:**
- Modify: `crates/waml-editor/src/app.rs:21-75,764-1360,2423-2449`
- Create: `crates/waml-editor/src/app/navigation.rs`
- Test: `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `App::{session,documents,view_history,nav_state,nav_kinds,pending_fragment,pending_anchor_restore,ui}`.
- Produces: `App::transition_to_location(&mut self, &mut Cx, ViewLocation, TransitionCause) -> bool` as the single navigation transition path.
- Produces: `App::refresh_nav(&mut self, &mut Cx, bool)` for workspace and action callers.
- Produces: Navigation and history methods visible only to sibling `app` modules through `pub(super)`.

- [ ] **Step 1: Run the focused navigation characterization tests**

```powershell
rtk cargo test -p waml-editor app::tests::navigation
rtk cargo test -p waml-editor global_history_chord_dispatches_before_the_widget_tree
```

Expected: PASS.

- [ ] **Step 2: Declare the navigation child module**

At the top of `app.rs`, use:

```rust
mod actions;
mod navigation;

use self::navigation::{PendingAnchorRestore, PendingFragment, TransitionCause};
```

Move `PendingFragment`, `PendingAnchorRestore`, and `TransitionCause` into `navigation.rs` and declare them `pub(super)`. Their fields stay private because only navigation code constructs or reads them.

- [ ] **Step 3: Move the navigation implementation as one unchanged block**

Start `navigation.rs` with:

```rust
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingFragment {
    pub(super) concept_id: String,
    pub(super) fragment: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingAnchorRestore {
    pub(super) document: crate::navigation::DocumentLocator,
    pub(super) anchor: ViewAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransitionCause {
    UserNavigation,
    UndoRedoReveal,
    HistoryTraversal,
    PassiveReconciliation,
}
```

Move the existing method bodies from `set_navigation_message` through `close_document`, plus `refresh_nav`, into `impl App` in this file. Do not change their internal call order. Mark the moved methods `pub(super)` so `actions`, `workspace`, `event`, and tests can use them without a crate-public API.

- [ ] **Step 4: Verify navigation behavior after the move**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::tests::navigation
rtk cargo test -p waml-editor app::actions::tests
```

Expected: PASS.

- [ ] **Step 5: Commit the navigation extraction**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/navigation.rs
rtk git commit -m "refactor(editor): extract app navigation"
```

---

### Task 3: Extract Shell Layout and Overlay Projection

**Files:**
- Modify: `crates/waml-editor/src/app.rs:21-50,550-575,1116-1157,1362-1823`
- Create: `crates/waml-editor/src/app/shell.rs`
- Test: `crates/waml-editor/src/app/tests/shell.rs`

**Interfaces:**
- Consumes: `App::{ui,narrow,pointer_in_narrow_dock,dock_layout,tree_gap_w,rule_overshoot,agent_badge,agent_tint,agent_row_w}`.
- Produces: `App::sync_document_shell`, dock and overlay operations, status projection, and event-tail geometry synchronization for sibling modules.
- Produces: Pure responsive-layout helpers and constants owned by the shell module.

- [ ] **Step 1: Run the shell characterization tests**

```powershell
rtk cargo test -p waml-editor app::tests::shell
rtk cargo test -p waml-editor breadcrumb_reveal_pins_tree_without_navigation
```

Expected: PASS.

- [ ] **Step 2: Declare the shell module and move its pure helpers**

Add:

```rust
mod shell;
```

Move these unchanged items into `app/shell.rs` under `use super::*;`:

```text
open_overlay_contains
should_dismiss_narrow_dock
project_document_header
TREE_BTN_W
NARROW_ENTER_W
NARROW_EXIT_W
next_narrow
```

Use `pub(super)` only for helpers called by `app.rs`, `event.rs`, or sibling tests.

- [ ] **Step 3: Move the shell methods without introducing a state wrapper**

Move `sync_document_shell` and the method run from `sync_diagram_switcher_current` through `sync_statusbar` into `impl App` in `shell.rs`. Keep fields directly on `App`; do not add `ShellLayoutState` in this task. Mark cross-module methods `pub(super)`.

The module header must be:

```rust
use super::*;
```

- [ ] **Step 4: Verify mounted geometry and responsive behavior**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::tests::shell
rtk cargo test -p waml-editor breakpoint_
```

Expected: PASS.

- [ ] **Step 5: Commit the shell extraction**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/shell.rs
rtk git commit -m "refactor(editor): extract app shell layout"
```

---

### Task 4: Extract Menu Models and Move Action Policy to Actions

**Files:**
- Modify: `crates/waml-editor/src/app.rs:2451-2650`
- Modify: `crates/waml-editor/src/app/actions.rs:1-70`
- Create: `crates/waml-editor/src/app/menus.rs`
- Test: `crates/waml-editor/src/app/tests/menus.rs`

**Interfaces:**
- Consumes: Existing `PopupItem`, `LiveId`, icon, tab, and conflict action values.
- Produces: The existing crate-visible paths `crate::app::{logo_menu_items,burger_menu_items,logo_command_for,LogoCommand}`.
- Produces: `doc_switcher_items` and `DOC_SWITCHER_MAX_H` for `app/actions.rs` through `pub(super)`.

- [ ] **Step 1: Run menu and action-policy tests**

```powershell
rtk cargo test -p waml-editor app::tests::menus
rtk cargo test -p waml-editor app::actions::tests
```

Expected: PASS.

- [ ] **Step 2: Move menu models and preserve the current public paths**

Add `mod menus;` and re-export only the items that are public today:

```rust
mod menus;

pub use menus::{burger_menu_items, logo_command_for, logo_menu_items, LogoCommand};
use menus::{doc_switcher_items, DOC_SWITCHER_MAX_H};
```

Move `logo_menu_items`, `burger_menu_items`, `DOC_SWITCHER_MAX_H`, `doc_switcher_items`, `LogoCommand`, and `logo_command_for` unchanged into `app/menus.rs`. Keep `doc_switcher_items` and `DOC_SWITCHER_MAX_H` as `pub(super)`.

- [ ] **Step 3: Put conflict action policy beside action dispatch**

Move `place_rm_for` unchanged from `app.rs` to `app/actions.rs`, above `impl App`. Keep it private to `actions.rs`. Move its three focused tests into the existing `app/actions.rs` test module so the helper does not need wider visibility.

- [ ] **Step 4: Verify menu and action behavior**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::tests::menus
rtk cargo test -p waml-editor app::actions::tests
```

Expected: PASS.

- [ ] **Step 5: Commit the menu extraction**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/app/menus.rs crates/waml-editor/src/app/tests/menus.rs
rtk git commit -m "refactor(editor): extract app menu models"
```

---

### Task 5: Extract Workspace Lifecycle and Centralize the Markdown Asset Host

**Files:**
- Modify: `crates/waml-editor/src/app.rs:575-665,1825-2449,2607-2660`
- Create: `crates/waml-editor/src/app/workspace.rs`
- Test: `crates/waml-editor/src/app/tests/workspace.rs`

**Interfaces:**
- Consumes: `EditorSession`, `DocumentHost`, `SharedMarkdownAssetHost`, native and browser asset policies, save tickets, and bundle loading.
- Produces: One private workspace module that owns save, open, replace, close, start-screen, recent-label, and asset-host lifecycle.
- Produces: `App::ensure_markdown_asset_host` and `App::prepare_open_documents` for internal session completion.
- Preserves: Native candidate-host rollback when opening a replacement bundle fails.

- [ ] **Step 1: Run workspace lifecycle characterization tests**

```powershell
rtk cargo test -p waml-editor app::tests::workspace
rtk cargo test -p waml-editor failed_open_restores_the_previous_markdown_asset_root
rtk cargo test -p waml-editor replacement_saves_old_document_before_loading_new_document
```

Expected: PASS.

- [ ] **Step 2: Declare the workspace module and move workspace-only values**

Add:

```rust
mod workspace;
```

Move these values and their existing tests into `workspace.rs`:

```text
SAVE_DEBOUNCE_SECS
BackingTransitionError
replace_after_save
close_after_save
SaveFeedback
should_flush_save
prevent_quit_after_failed_save
restore_markdown_asset_host_after_open
browser_save_fragment
format_opened
web_location_hash
```

Keep `SaveFeedback` and values referenced by the parent or sibling modules `pub(super)`. Add these imports in `app.rs` so `App` and the later event module keep unqualified access to them:

```rust
use self::workspace::{
    prevent_quit_after_failed_save, should_flush_save, SaveFeedback,
};
#[cfg(target_arch = "wasm32")]
use self::workspace::web_location_hash;
```

Do not group the corresponding `App` fields into a new state struct.

- [ ] **Step 3: Add narrow asset-host lifecycle methods**

Add these methods to `impl App` in `workspace.rs`:

```rust
pub(super) fn ensure_markdown_asset_host(
    &mut self,
    policy: crate::markdown_hosts::MarkdownAssetPolicy,
) -> crate::markdown_hosts::SharedMarkdownAssetHost {
    self.markdown_assets
        .get_or_insert_with(|| {
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(policy)
        })
        .clone()
}

pub(super) fn prepare_open_documents(
    &self,
) -> Result<Vec<Option<crate::document::OpenDocument>>, String> {
    let assets = self
        .markdown_assets
        .as_ref()
        .ok_or_else(|| "Markdown asset host is not initialized".to_string())?;
    Ok(self
        .documents
        .tabs()
        .iter()
        .map(|tab| {
            crate::documents::reopen_with_asset_host(
                self.session.okf_analysis(),
                self.session.uml_analysis(),
                tab,
                assets,
            )
        })
        .collect())
}
```

Use `ensure_markdown_asset_host(BrowserBundle)` for browser-backed sessions and internal edits. Native open must install a host made from `MarkdownAssetPolicy::native(&next_root)` before `open_bundle` and restore the previous host if `open_bundle` returns `false`.

- [ ] **Step 4: Move the workspace method block unchanged before simplifying callers**

Move the methods from `mark_dirty` through `refresh_nav` into `workspace.rs`, except:

```text
sync_conflict_badge and open_conflict_list -> shell.rs
refresh_nav -> navigation.rs
```

Move `format_opened` and `web_location_hash` with the workspace code. Mark methods called by `actions.rs`, `event.rs`, or `app.rs` as `pub(super)`. Keep platform `cfg` pairs adjacent.

- [ ] **Step 5: Replace direct host construction with the workspace methods**

In `open_bundle` and `complete_session_change`, replace direct `markdown_assets.is_none()` construction with:

```rust
self.ensure_markdown_asset_host(
    crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
);
```

Replace repeated tab reopening loops with `self.prepare_open_documents()?` where the caller is fallible. In `complete_session_change`, use `.expect("an open editor session owns one Markdown asset host")` after `ensure_markdown_asset_host`, because that path establishes the invariant itself.

- [ ] **Step 6: Verify native lifecycle behavior**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::tests::workspace
rtk cargo test -p waml-editor app::actions::tests
```

Expected: PASS, including failed-open rollback and save-before-replace ordering.

- [ ] **Step 7: Commit the workspace extraction**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/workspace.rs crates/waml-editor/src/app/navigation.rs crates/waml-editor/src/app/shell.rs crates/waml-editor/src/app/actions.rs
rtk git commit -m "refactor(editor): extract app workspace lifecycle"
```

---

### Task 6: Unify Post-Session-Change Projection

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs:827-902`
- Modify: `crates/waml-editor/src/app/workspace.rs` at `replace_external_document`
- Modify: `crates/waml-editor/src/app/shell.rs`
- Test: `crates/waml-editor/src/app/tests/navigation.rs`
- Test: `crates/waml-editor/src/app/tests/workspace.rs`

**Interfaces:**
- Consumes: A completed path-specific `DocumentHost::{after_session_change,after_external_replacement}` call and its `SessionChange`.
- Produces: `App::synchronize_session_change_projections(&mut self, &mut Cx, &SessionChange)` as the only projection of UML chrome, navigation, and conflicts after a session change.
- Preserves: Internal changes restore the current location passively and mark the session dirty. External replacement does neither.

- [ ] **Step 1: Run both ingress-path characterization suites**

```powershell
rtk cargo test -p waml-editor app::actions::tests
rtk cargo test -p waml-editor app::tests::navigation
rtk cargo test -p waml-editor app::tests::workspace
```

Expected: PASS.

- [ ] **Step 2: Add the shared projection helper**

Add to `impl App` in `shell.rs`:

```rust
pub(super) fn synchronize_session_change_projections(
    &mut self,
    cx: &mut Cx,
    change: &crate::editor_session::SessionChange,
) {
    if change.uml_changed {
        self.sync_document_shell(cx);
    }
    if change.navigation_changed {
        self.nav_kinds = crate::nav::kinds_in_model(
            self.session.okf_analysis(),
            self.session.uml_analysis(),
        );
        self.refresh_nav(cx, false);
    }
    if change.conflicts_changed {
        self.sync_conflict_badge(cx);
    }
}
```

Any future `SessionChange` projection flag must be handled in this method, not separately in each mutation ingress.

- [ ] **Step 3: Route internal completion through the helper**

Keep this order in `complete_session_change`:

```rust
self.documents
    .after_session_change(cx, &self.ui, &self.session, change.clone(), prepared);
self.synchronize_session_change_projections(cx, &change);
if let Some(current) = self.documents.capture_active_location(cx, &self.ui) {
    self.transition_to_location(cx, current, TransitionCause::PassiveReconciliation);
}
self.mark_dirty(cx);
self.sync_history_controls(cx);
```

- [ ] **Step 4: Route external replacement through the helper**

Keep this order after the external `DocumentHost` update:

```rust
self.documents.after_external_replacement(
    cx,
    &self.ui,
    &self.session,
    change.clone(),
    prepared,
);
self.synchronize_session_change_projections(cx, change);
self.sync_history_controls(cx);
```

Do not call `mark_dirty` or passive location restoration on this path.

- [ ] **Step 5: Verify both ingress paths**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor app::actions::tests
rtk cargo test -p waml-editor app::tests
```

Expected: PASS.

- [ ] **Step 6: Commit the reconciliation choke point**

```powershell
rtk git add crates/waml-editor/src/app/actions.rs crates/waml-editor/src/app/workspace.rs crates/waml-editor/src/app/shell.rs
rtk git commit -m "refactor(editor): unify session projections"
```

---

### Task 7: Make the Raw Event Pipeline Explicit

**Files:**
- Modify: `crates/waml-editor/src/app.rs:2818-3024`
- Create: `crates/waml-editor/src/app/event.rs`
- Test: `crates/waml-editor/src/app/tests/navigation.rs`
- Test: `crates/waml-editor/src/app/tests/shell.rs`
- Test: `crates/waml-editor/src/app/tests/workspace.rs`

**Interfaces:**
- Consumes: The existing event order and its early return for model undo and redo.
- Produces: Small `pub(super)` phase helpers in `event.rs`.
- Preserves: A short, visible, ordered `AppMain::handle_event` in `app.rs`; no registry or dynamic dispatch.

- [ ] **Step 1: Run tests that exercise ordering-sensitive phases**

```powershell
rtk cargo test -p waml-editor global_history_chord_dispatches_before_the_widget_tree
rtk cargo test -p waml-editor shutdown_and_quit_request_are_final_save_events
rtk cargo test -p waml-editor breadcrumb_reveal_pins_tree_without_navigation
```

Expected: PASS.

- [ ] **Step 2: Declare the event helper module**

Add:

```rust
mod event;
```

Create `app/event.rs` with `use super::*;` and move the current bodies into methods with these exact interfaces:

```text
rehydrate_for_event(&mut self, &mut Cx, &Event)
update_fps_meter(&mut self, &mut Cx, &Event)
handle_global_shortcuts(&mut self, &mut Cx, &Event) -> bool
handle_escape_event(&mut self, &mut Cx, &Event)
handle_persistence_event(&mut self, &mut Cx, &Event)
route_popup_event(&mut self, &mut Cx, &Event)
handle_draw_restores(&mut self, &mut Cx, &Event)
override_caption_drag_query(&mut self, &mut Cx, &Event)
synchronize_after_event(&mut self, &mut Cx)
```

Implement each method by moving the corresponding existing block without changing its statements. `handle_global_shortcuts` returns `true` only for the undo or redo branch that currently returns from `handle_event`.

- [ ] **Step 3: Replace the large body with the visible pipeline**

Keep `handle_event` in `app.rs` with this order:

```rust
fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
    self.rehydrate_for_event(cx, event);
    self.update_fps_meter(cx, event);
    if self.handle_global_shortcuts(cx, event) {
        return;
    }

    self.match_event(cx, event);
    self.handle_escape_event(cx, event);
    self.handle_persistence_event(cx, event);
    self.route_popup_event(cx, event);
    self.documents.route_ui_event(cx, &self.ui, event);
    self.handle_draw_restores(cx, event);
    self.override_caption_drag_query(cx, event);
    self.synchronize_after_event(cx);
}
```

The order above is the contract. Do not turn it into an array of callbacks.

- [ ] **Step 4: Verify event ordering and the complete editor crate**

```powershell
rtk cargo fmt --all
rtk cargo test -p waml-editor global_history_chord_dispatches_before_the_widget_tree
rtk cargo test -p waml-editor shutdown_and_quit_request_are_final_save_events
rtk cargo test -p waml-editor
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: All commands exit with status 0.

- [ ] **Step 5: Confirm the final ownership shape**

Run:

```powershell
rtk rg -n "^\s*fn |^\s*pub\(super\) fn " crates/waml-editor/src/app.rs crates/waml-editor/src/app
```

Confirm:

```text
app.rs contains composition, App state, MatchEvent, AppMain, and the short pipeline.
navigation.rs owns navigation and history methods.
shell.rs owns overlay, dock, chrome, and projection methods.
workspace.rs owns save, open, replacement, close, and asset-host methods.
menus.rs owns menu data.
event.rs owns only ordered raw-event phase helpers.
actions.rs remains the ordered action choke point.
```

- [ ] **Step 6: Commit the event pipeline extraction**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/app/event.rs
rtk git commit -m "refactor(editor): expose app event phases"
```

## Completion Criteria

- `app.rs` is a composition root, not the default home for new feature methods.
- All existing tests are present and pass after the test split.
- Internal edits and external replacements use one session-projection method after their path-specific `DocumentHost` update.
- An open session has one Markdown asset host, and native open rollback restores the previous host on failure.
- `handle_event` shows its phase order directly and keeps the undo or redo early return before widget dispatch.
- No new controller, facade, event registry, trait, or dependency exists.
- `EditorSession`, `DocumentHost`, `transition_to_location`, and ordered action dispatch keep their current authority.
