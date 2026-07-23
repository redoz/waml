# Node Context Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pivot the `waml-editor` node context menu from a radial wheel to a uniform linear `MenuPopup` (base actions View Source + Find in diagrams) that merges surface-contributed context items, and add a second entry point on the project tree.

**Architecture:** A new pure `popup/node_menu` module owns the base items, an id→command mapper, and `compose(context, base)`. The diagram surface (canvas → `ClassDiagramView`) and the project tree each emit a request carrying the subject key + anchor; `App` relays it to `PopupRoot::show_at(PopupSpec::Menu{..})` and dispatches the committed id back through the tag-filtered `PopupRoot::closed(actions, node_menu)` queue. View Source opens a new `TabKind::Source` tab (an empty placeholder document surface); Find in diagrams is a `log!` stub. The radial code paths (`node_radial_items`, `canvas::NodeCommand`, `node_command_for`) are removed; the shared `RadialPopup` surface is kept.

**Tech Stack:** Rust, Makepad (redoz/makepad fork, local `[patch]` clone at `C:\dev\makepad`), `script_mod!` MPSL DSL (NOT upstream `live_design!`), immediate-mode hand-rolled widgets. `waml-editor` is a **binary** crate.

## Global Constraints

Every task's requirements implicitly include these:

- **Test invocation:** `waml-editor` is a bin crate — run `cargo test -p waml-editor <filter>` (NOT `--lib`). The implement-plan gate additionally runs `cargo clippy -p waml-editor -- -D warnings`, which **promotes `dead_code` and `unused_imports` to hard errors**. Any item that lands ahead of its wiring must carry `#[allow(dead_code)]` (removed in the task that consumes it), matching the convention already used in `popup/base.rs` and `doc_view.rs` (both open with `#![allow(dead_code)]`).
- **DSL is `script_mod!`, not `live_design!`.** Widgets register via a `script_mod(vm)` fn; pure modules (like `popup/base.rs`, `doc_view.rs`, and the new `popup/node_menu.rs`) are plain Rust and register nothing.
- **Theme tokens only** in DSL — reference `atlas.<name>` (e.g. `atlas.canvas_ground`), never hardcode colors.
- **Icons:** reuse existing `crate::icons::Icon` variants only — `Icon::Braces` (View Source), `Icon::Search` (Find in diagrams). No changes to the `Icon` enum / `ALL` / `label` order invariant.
- **No production changes beyond the plan's tasks.** No separator affordance, no real source text, no populated diagram-context items (all deferred per spec §"Non-goals").

---

## File Structure

- **Create** `crates/waml-editor/src/popup/node_menu.rs` — `NodeMenuCommand`, `base_items()`, `command_for()`, `compose()`. Pure functions + tests. (Task 1)
- **Create** `crates/waml-editor/src/source_view.rs` — `SourceView` (a `DocView`) hosting the empty source tab body. (Task 2)
- **Modify** `crates/waml-editor/src/popup/mod.rs:7-13` — add `pub mod node_menu;`. (Task 1)
- **Modify** `crates/waml-editor/src/main.rs` — add `mod source_view;` in the alphabetical `mod` list. (Task 2)
- **Modify** `crates/waml-editor/src/doc_tabs.rs` — add `TabKind::Source`, `source_tab_id()`, `OpenTabs::open_source()` + tests. (Task 2)
- **Modify** `crates/waml-editor/src/doc_view.rs` — `make_view` `Source` arm; `BodyWidgets::source_view()`; change `PopupRequest::NodeRadial` → `NodeContextMenu`. (Tasks 2 + 3)
- **Modify** `crates/waml-editor/src/app.rs` — `source_view` DSL sibling; `sync_active_tab` visibility toggle; `node_menu_key` field; relay `NodeContextMenu`; dispatch swap; tree context handler; remove `node_radial_items` + `RadialOpen` import. (Tasks 2–5)
- **Modify** `crates/waml-editor/src/canvas.rs` — `NodeMenu{abs,key}` payload; `GraphCanvas::context_items()`; remove `NodeCommand` + `node_command_for` + its test. (Tasks 3 + 4)
- **Modify** `crates/waml-editor/src/class_diagram_view.rs:143-146` — select-on-right-click + context gather + `NodeContextMenu` request. (Task 3)
- **Modify** `crates/waml-editor/src/tree_panel.rs` — `is_classifier_kind()` helper + test; `ProjectTreeAction::ContextMenu`; `context_menu_request()` reader; right-click wiring. (Task 5)
- **Modify** `C:\dev\makepad\widgets\src\file_tree.rs` (FORK) — `FileTreeAction::FileRightClicked`, `FileTreeNodeAction::SecondaryClicked`, secondary-`FingerDown` arm, drain arm, `FileTreeRef::file_right_clicked()`. (Task 5)

---

## Reviewer flags — reconciliations vs the spec's approximate seam notes

Read these before implementing; verified against source at planning time.

1. **The canvas does NOT relay to `App::app.rs:1420` directly.** The secondary-click flows canvas → `GraphCanvasAction::NodeMenu` → `ClassDiagramView::handle` (`class_diagram_view.rs:143`) → `ViewOutcome.popup = PopupRequest::NodeRadial` → `App::relay_outcome` (`app.rs:1405`). The new menu routes through this same `DocView` seam — the diagram view gathers context + sets the inspector subject; the shell (`relay_outcome`) places the popup. The spec's "canvas emits ... App relays at :1420" is a simplification.

2. **`GraphCanvas` cannot be unit-instantiated.** It is `#[derive(Script, ScriptHook, Widget)]` with no `Default`, so `context_items` is not cleanly unit-testable; it is proven live (empty list → base-only menu). Task 3 asserts the *composition* behavior via `node_menu::compose` unit tests (Task 1) instead.

3. **The project-tree right-click needs a fork change.** The fork `FileTreeNode::handle_event` (`file_tree.rs:647`) consumes *any* `Hit::FingerDown` (no button check) and emits only `WasClicked`. A row-accurate right-click therefore requires a minimal fork addition (a secondary-`FingerDown` arm + a distinct action + a reader). Done in Task 5. `MouseButton::SECONDARY` and `fe.mouse_button()` are already reachable in `file_tree.rs` via its `makepad_draw::*` import (see `browser.rs:747` in the same crate).

4. **`find_widget_action` returns only the FIRST action for a uid**, so the tree cannot emit both `FocusClassifier` and `ContextMenu` and have `App` read both. The tree emits only `ContextMenu`; `App` performs the select-on-right-click itself (via `open_preview`) inside the `context_menu_request` handler.

5. **`node_menu_key` field, task ordering.** The committed menu id (`PopupResult::Invoked(id)`) carries no subject, so `App` stashes the right-clicked key in a new `#[rust] node_menu_key` field, written when the menu opens (Task 3) and read at dispatch (Task 4). It is write-only in Task 3, so it carries `#[allow(dead_code)]` there (removed in Task 4). This preserves the 5-task split: Task 3 opens the menu while the old `node_command_for` dispatch still returns `None` for the new ids (menu opens, commit is a harmless no-op), and Task 4 swaps the dispatch and deletes the radial command code.

6. **Markdown path: empty placeholder `View`, not the fork `Markdown` widget.** The fork ships `widgets/src/markdown.rs` (`Markdown::register_widget`), but it is NOT registered in the editor today (grep for "Markdown" in `crates/waml-editor/src` hits only the old `NodeCommand::Markdown` label). Hosting it would add a registration + shader-risk surface for zero user-visible benefit while the document is empty. The spec explicitly blesses this fallback ("ship an empty placeholder `View` in its slot ... 'empty markdown view for now' is satisfied either way"). This plan uses an opaque empty `SolidView` in the `source_view` slot; real `Markdown` rendering (with `Subject` → markdown-file text) is a recorded follow-up.

7. **`canvas.set_visible` is a no-op** (hand-rolled `GraphCanvas` draw_walk ignores the visible flag — same class of gotcha noted for `StartScreen` at `app.rs:586`). So the source tab does not hide the canvas by toggling it; instead the opaque `source_view` sibling is drawn *after* `canvas` in the `Overlay` flow and simply occludes it when visible. The tool dock is hidden via the existing `wants_tooldock()==false` path; the element picker via `inspector.set_picker_visible(false)`.

---

### Task 1: `popup/node_menu` module

**Files:**
- Create: `crates/waml-editor/src/popup/node_menu.rs`
- Modify: `crates/waml-editor/src/popup/mod.rs:7-13` (add `pub mod node_menu;`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/popup/node_menu.rs`

**Interfaces:**
- Produces:
  - `pub enum NodeMenuCommand { ViewSource, FindInDiagrams }`
  - `pub fn base_items() -> Vec<crate::popup::base::PopupItem>`
  - `pub fn command_for(id: LiveId) -> Option<NodeMenuCommand>`
  - `pub fn compose(context: Vec<PopupItem>, base: Vec<PopupItem>) -> Vec<PopupItem>`
- Consumes: `crate::popup::base::PopupItem`, `crate::icons::Icon` (first task otherwise).

- [ ] **Step 1: Register the module**

In `crates/waml-editor/src/popup/mod.rs`, add `pub mod node_menu;` keeping the list alphabetical:

```rust
pub mod base;
pub mod marking;
pub mod menu;
pub mod node_menu;
pub mod presenter;
pub mod radial;
pub mod root;
pub mod select;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/waml-editor/src/popup/node_menu.rs` with the full module + tests (the functions are trivial enough that the failing state is "module/functions do not exist yet" — write the whole file, then confirm the compile error, then it is already green; this task therefore writes the implementation and its tests together and the FAIL/PASS gate is the compile+run):

```rust
//! Uniform per-subject node menu: the two base actions plus a `compose()` that
//! merges surface-contributed context items above them. Pure functions + a
//! command enum; not a widget, so nothing registers with the vm. Lands ahead of
//! its wiring (Tasks 3-5), so like `popup/base.rs` and `doc_view.rs` a bin
//! crate's dead-code lint would flag every item until then.
#![allow(dead_code)]

use makepad_widgets::*;

use crate::icons::Icon;
use crate::popup::base::PopupItem;

/// Base (per-subject) node commands. Uniform across every invocation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeMenuCommand {
    ViewSource,
    FindInDiagrams,
}

/// The base items every node menu ends with, top to bottom. Ids are what
/// `MenuPopup` reports on commit; `command_for` maps them back (mirrors
/// `logo_command_for`).
pub fn base_items() -> Vec<PopupItem> {
    vec![
        PopupItem {
            id: live_id!(view_source),
            label: "View Source".into(),
            icon: Icon::Braces,
            danger: false,
            enabled: true,
        },
        PopupItem {
            id: live_id!(find_in_diagrams),
            label: "Find in diagrams".into(),
            icon: Icon::Search,
            danger: false,
            enabled: true,
        },
    ]
}

/// Map a menu-committed `LiveId` to a base command. `None` = not one of ours.
pub fn command_for(id: LiveId) -> Option<NodeMenuCommand> {
    if id == live_id!(view_source) {
        Some(NodeMenuCommand::ViewSource)
    } else if id == live_id!(find_in_diagrams) {
        Some(NodeMenuCommand::FindInDiagrams)
    } else {
        None
    }
}

/// Context items first, base items last (base is the stable bottom zone). With
/// an empty `context`, returns `base` unchanged.
pub fn compose(context: Vec<PopupItem>, base: Vec<PopupItem>) -> Vec<PopupItem> {
    let mut items = context;
    items.extend(base);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_items_yields_the_two_base_entries_in_order() {
        let items = base_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, live_id!(view_source));
        assert_eq!(items[0].label, "View Source");
        assert_eq!(items[1].id, live_id!(find_in_diagrams));
        assert_eq!(items[1].label, "Find in diagrams");
    }

    #[test]
    fn command_for_maps_ids_and_rejects_others() {
        assert_eq!(
            command_for(live_id!(view_source)),
            Some(NodeMenuCommand::ViewSource)
        );
        assert_eq!(
            command_for(live_id!(find_in_diagrams)),
            Some(NodeMenuCommand::FindInDiagrams)
        );
        assert_eq!(command_for(live_id!(nope)), None);
    }

    #[test]
    fn compose_puts_context_first_base_last() {
        let ctx = vec![PopupItem {
            id: live_id!(ctx_a),
            label: "Ctx A".into(),
            icon: Icon::Search,
            danger: false,
            enabled: true,
        }];
        let out = compose(ctx, base_items());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, live_id!(ctx_a));
        assert_eq!(out[1].id, live_id!(view_source));
        assert_eq!(out[2].id, live_id!(find_in_diagrams));
    }

    #[test]
    fn compose_empty_context_returns_base_unchanged() {
        let out = compose(vec![], base_items());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, live_id!(view_source));
        assert_eq!(out[1].id, live_id!(find_in_diagrams));
    }
}
```

- [ ] **Step 3: Run the tests to verify they compile-fail then pass**

Run: `cargo test -p waml-editor node_menu`
Expected: **FAIL first** if `pub mod node_menu;` was not added (compile error `file not found for module` / `unresolved import`); once Step 1 + Step 2 are both in place, the four tests **PASS**.

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/popup/node_menu.rs crates/waml-editor/src/popup/mod.rs
git commit -m "feat(popup): node_menu base items, command_for, compose"
```

---

### Task 2: `TabKind::Source` + `open_source` + `SourceView` body

**Files:**
- Modify: `crates/waml-editor/src/doc_tabs.rs:143-146` (add `Source` to `TabKind`), `:315-323` (add `source_tab_id`), `:210-248` (add `open_source`), `:732-887` (add tests)
- Create: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod source_view;`)
- Modify: `crates/waml-editor/src/doc_view.rs:146-153` (add `make_view` `Source` arm), `:29-48` (add `BodyWidgets::source_view`), `:184-189` (extend test)
- Modify: `crates/waml-editor/src/app.rs:193-251` (add `source_view` DSL sibling), `:340-374` (visibility toggle in `sync_active_tab`)
- Test: `doc_tabs.rs` + `doc_view.rs` inline test modules

**Interfaces:**
- Consumes: `TabKind` (Task's own new variant), `crate::doc_view::DocView`/`BodyWidgets`/`ViewOutcome`.
- Produces:
  - `TabKind::Source`
  - `pub fn source_tab_id(key: &str) -> LiveId`
  - `OpenTabs::open_source(key: impl Into<String>, title: impl Into<String>) -> LiveId`
  - `crate::source_view::SourceView` (a `DocView`), constructed `SourceView::new(key: String)`
  - `BodyWidgets::source_view(&self, cx: &mut Cx) -> WidgetRef`

- [ ] **Step 1: Write the failing `open_source` tests**

Append to the `#[cfg(test)] mod tests` in `crates/waml-editor/src/doc_tabs.rs` (after `activate_unknown_id_is_a_no_op`, before the closing `}` at `:887`):

```rust
    #[test]
    fn open_source_uses_the_preview_slot_and_is_a_source_tab() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let id = open.open_source("customer", "Customer");
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[1].kind, TabKind::Source);
        assert!(open.tabs[1].preview);
        assert_eq!(open.tabs[1].title, "Customer");
        assert_eq!(open.active, id);
    }

    #[test]
    fn open_source_twice_reuses_the_same_slot_and_focuses() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let a = open.open_source("a", "A");
        let b = open.open_source("a", "A");
        assert_eq!(a, b);
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.active, a);
    }

    #[test]
    fn open_source_replaces_an_existing_preview_in_place() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        open.open_preview("customer", "Customer", TreeKind::Class);
        let src = open.open_source("order", "Order");
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[1].id, src);
        assert_eq!(open.tabs[1].kind, TabKind::Source);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p waml-editor open_source`
Expected: FAIL — compile error `no variant ... Source` and `no method named open_source`.

- [ ] **Step 3: Add `TabKind::Source`, `source_tab_id`, and `open_source`**

In `crates/waml-editor/src/doc_tabs.rs`, extend `TabKind` (`:143`):

```rust
pub enum TabKind {
    Diagram,
    Classifier,
    Source,
}
```

Add `source_tab_id` next to `classifier_tab_id` (`:323`). It is used only by `open_source` (dead until Task 4 wires the dispatch), so guard it:

```rust
/// A source tab's id is derived from its key so re-opening the same element's
/// source reuses the same tab (mirrors `classifier_tab_id`).
#[allow(dead_code)]
pub fn source_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_source__{key}"))
}
```

Add `open_source` after `open_preview` (`:248`). It is called only from tests until Task 4, so guard it:

```rust
    /// View Source: open (or focus) the single preview slot as a `Source` tab
    /// for `key`. Mirrors `open_preview` -- never duplicates (id derives from
    /// key), reuses the preview slot in place, always activates.
    #[allow(dead_code)]
    pub fn open_source(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
    ) -> LiveId {
        let key = key.into();
        let title = title.into();
        let id = source_tab_id(&key);
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
            return id;
        }
        let tab = DocTab {
            id,
            key,
            title,
            kind: TabKind::Source,
            // No dedicated Source glyph; reuse the classifier glyph for the tab.
            node_kind: TreeKind::Class,
            preview: true,
        };
        if let Some(idx) = self.preview_index() {
            self.tabs[idx] = tab;
        } else {
            self.tabs.push(tab);
        }
        self.active = id;
        id
    }
```

- [ ] **Step 4: Run to verify the tab tests pass**

Run: `cargo test -p waml-editor open_source`
Expected: PASS (3 tests).

- [ ] **Step 5: Write the failing `make_view` Source test**

In `crates/waml-editor/src/doc_view.rs`, add to `#[cfg(test)] mod tests` (after `make_view_dispatches_on_tab_kind`):

```rust
    #[test]
    fn make_view_handles_source_kind() {
        let sv = make_view(&tab(TabKind::Source, TreeKind::Class));
        assert!(!sv.wants_tooldock(), "source view has no tool dock");
    }
```

- [ ] **Step 6: Run to verify it fails**

Run: `cargo test -p waml-editor make_view`
Expected: FAIL — `make_view` match is non-exhaustive for `TabKind::Source` (compile error) / `source_view` module missing.

- [ ] **Step 7: Create `SourceView` and register the module**

Create `crates/waml-editor/src/source_view.rs`:

```rust
//! `SourceView` -- the View Source tab body. Renders the shared empty
//! `source_view` slot (an opaque placeholder document surface; real Markdown
//! rendering of the element's markdown file is a deferred follow-up) and hides
//! the diagram chrome: the canvas is occluded by the opaque slot, the tool dock
//! by `wants_tooldock() == false`, the inspector's element picker explicitly.

use makepad_widgets::*;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};
use crate::inspector::Subject;

pub struct SourceView {
    /// The subject key whose source this tab shows.
    key: String,
}

impl SourceView {
    pub fn new(key: String) -> SourceView {
        SourceView { key }
    }
}

impl DocView for SourceView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::Classifier(self.key.clone()));
            // A source view is not a diagram: no element picker.
            inspector.set_picker_visible(cx, false);
        }
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _model: &Model,
    ) -> ViewOutcome {
        ViewOutcome::default()
    }

    fn wants_tooldock(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
```

In `crates/waml-editor/src/main.rs`, add `mod source_view;` in alphabetical order (it sits between `selection_toolbar`/`scene` neighbors and `theme_atlas`; place it after the existing `mod` that precedes `s...` names, e.g. right before `mod theme_atlas;` — keep the list sorted).

- [ ] **Step 8: Add the `make_view` Source arm + `BodyWidgets::source_view`**

In `crates/waml-editor/src/doc_view.rs`, extend `make_view` (`:146`):

```rust
pub fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(crate::class_diagram_view::ClassDiagramView::new()),
        TabKind::Classifier => Box::new(
            crate::classifier_preview_view::ClassifierPreviewView::new(tab.key.clone()),
        ),
        TabKind::Source => Box::new(crate::source_view::SourceView::new(tab.key.clone())),
    }
}
```

Add the accessor to `impl BodyWidgets` (after `selection_toolbar`, `:39`):

```rust
    pub fn source_view(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(source_view))
    }
```

- [ ] **Step 9: Add the `source_view` DSL slot + `sync_active_tab` toggle**

In `crates/waml-editor/src/app.rs`, inside the body `View{ flow: Overlay ... }`, add the `source_view` sibling immediately **after** the `canvas` child (`:200`) and before `tool_dock_wrap`, so it draws over the canvas but under the corner panels:

```rust
                        canvas := GraphCanvas{
                            width: Fill
                            height: Fill
                        }
                        // View Source tab body: an opaque, empty placeholder
                        // document surface. Toggled visible only on a Source tab
                        // (see `sync_active_tab`); when visible it occludes the
                        // canvas (whose `set_visible` is a no-op). Real Markdown
                        // rendering is a deferred follow-up.
                        source_view := SolidView{
                            width: Fill
                            height: Fill
                            visible: false
                            draw_bg.color: atlas.canvas_ground
                        }
```

In `sync_active_tab` (`app.rs:362`, right after `let body = ...` and before `view.sync`), toggle the slot:

```rust
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        body.source_view(cx)
            .set_visible(cx, active.kind == TabKind::Source);
        let view = self
            .views
            .entry(active.id)
            .or_insert_with(|| crate::doc_view::make_view(&active));
```

(`TabKind` is already imported at `app.rs:1`.)

- [ ] **Step 10: Run to verify the view tests pass + it builds**

Run: `cargo test -p waml-editor make_view`
Expected: PASS.
Run: `cargo build -p waml-editor`
Expected: builds clean (no `dead_code`/`unused` errors — `open_source`/`source_tab_id` are `#[allow]`-guarded; `SourceView`, `source_view`, and the toggle are all live).

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/main.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app.rs
git commit -m "feat(tabs): TabKind::Source + open_source + empty SourceView body"
```

---

### Task 3: Diagram pivot — linear menu on right-click

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs:457-490` (`NodeMenu{abs,key}` payload + secondary handler), add `GraphCanvas::context_items`
- Modify: `crates/waml-editor/src/doc_view.rs:80-89` (`PopupRequest::NodeRadial` → `NodeContextMenu`)
- Modify: `crates/waml-editor/src/class_diagram_view.rs:143-146` (select-on-right-click + context + request)
- Modify: `crates/waml-editor/src/app.rs:8` (drop `RadialOpen` import), `:274-324` (add `node_menu_key` field), `:738-773` (remove `node_radial_items`), `:1405-1416` (relay `NodeContextMenu`)

**Interfaces:**
- Consumes: `node_menu::compose`, `node_menu::base_items` (Task 1); `TabKind` (Task 2, unchanged use).
- Produces:
  - `GraphCanvasAction::NodeMenu { abs: DVec2, key: String }`
  - `GraphCanvas::context_items(&self, subject: &crate::inspector::Subject) -> Vec<crate::popup::base::PopupItem>`
  - `PopupRequest::NodeContextMenu { anchor: DVec2, key: String, context: Vec<crate::popup::base::PopupItem> }`
  - `App.node_menu_key: Option<String>` (`#[rust]`, written here, read in Task 4)

- [ ] **Step 1: Change the canvas `NodeMenu` payload**

In `crates/waml-editor/src/canvas.rs`, change the `GraphCanvasAction::NodeMenu` variant (`:465-469`) to carry the key instead of the index:

```rust
    /// A right-press landed on a node: open the node menu at `abs` for the
    /// node's `SceneNode::key`. Carries the key directly so `App` never re-maps
    /// an index (mirrors `NodeSelect`).
    NodeMenu { abs: DVec2, key: String },
```

Update the secondary-`FingerDown` handler (`:484-491`) to resolve the key:

```rust
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                let rects: Vec<waml::solve::Rect> =
                    self.scene.nodes.iter().map(|n| n.rect).collect();
                if let Some(node) = node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                    let key = self.scene.nodes[node].key.clone();
                    let uid = self.widget_uid();
                    cx.widget_action(uid, GraphCanvasAction::NodeMenu { abs: fe.abs, key });
                }
            }
```

- [ ] **Step 2: Add `GraphCanvas::context_items`**

Add these two `use` lines to the top `use` cluster of `crates/waml-editor/src/canvas.rs`:

```rust
use crate::inspector::Subject;
use crate::popup::base::PopupItem;
```

Add the method to the `impl GraphCanvas` block (near the other public methods like `set_scene`):

```rust
    /// Diagram-contributed context menu items for a right-clicked subject.
    /// Empty now -- this is the seam where per-node-type items land later
    /// (spec: "the canvas contributes an empty context list").
    pub fn context_items(&self, subject: &Subject) -> Vec<PopupItem> {
        let _ = subject;
        vec![]
    }
```

- [ ] **Step 3: Change `PopupRequest::NodeRadial` → `NodeContextMenu`**

In `crates/waml-editor/src/doc_view.rs`, replace the `NodeRadial` variant (`:80-82`). Add `use crate::popup::base::PopupItem;` next to the existing `use crate::popup::base::PopupResult;` (`:55`):

```rust
pub enum PopupRequest {
    /// The uniform node context menu -- `context` items (surface-contributed)
    /// followed by the base items, placed by the shell at `anchor`.
    NodeContextMenu {
        anchor: DVec2,
        key: String,
        context: Vec<PopupItem>,
    },
    /// Inspector element-picker flyout.
    ElementPicker {
        anchor_rect: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
}
```

- [ ] **Step 4: Diagram view — select-on-right-click + gather context + request**

In `crates/waml-editor/src/class_diagram_view.rs`, replace the `NodeMenu` match arm (`:143-146`):

```rust
            Some(crate::canvas::GraphCanvasAction::NodeMenu { abs, key }) => {
                // Select-on-right-click: point the inspector at the node (the
                // same call `NodeSelect` makes).
                if let Some(mut inspector) = body
                    .inspector(cx)
                    .borrow_mut::<crate::inspector_panel::Inspector>()
                {
                    inspector.set_subject(cx, model, Subject::Classifier(key.clone()));
                }
                // Gather the diagram's per-node context items (empty for now).
                let context = body
                    .canvas(cx)
                    .borrow::<crate::canvas::GraphCanvas>()
                    .map(|c| c.context_items(&Subject::Classifier(key.clone())))
                    .unwrap_or_default();
                out.popup = Some(PopupRequest::NodeContextMenu {
                    anchor: abs,
                    key,
                    context,
                });
                return out;
            }
```

(`Subject` and `PopupRequest` are already imported at `class_diagram_view.rs:8-9`.)

- [ ] **Step 5: Add the `node_menu_key` field**

In `crates/waml-editor/src/app.rs`, add to `struct App` (after the `views` field, `:324`). It is write-only until Task 4, so guard it:

```rust
    /// The key of the node whose context menu is currently open, stashed when
    /// the menu opens so the committed id (which carries no subject) can be
    /// dispatched against it. Read in the `node_closed` branch (Task 4).
    #[rust]
    #[allow(dead_code)]
    node_menu_key: Option<String>,
```

- [ ] **Step 6: Relay `NodeContextMenu`; remove `node_radial_items` + `RadialOpen`**

In `crates/waml-editor/src/app.rs`, replace the `PopupRequest::NodeRadial` relay arm (`:1405-1416`):

```rust
                    crate::doc_view::PopupRequest::NodeContextMenu {
                        anchor,
                        key,
                        context,
                    } => {
                        self.node_menu_key = Some(key);
                        pr.show_at(
                            cx,
                            PopupSpec::Menu {
                                tag: live_id!(node_menu),
                                anchor,
                                bounds,
                                items: crate::popup::node_menu::compose(
                                    context,
                                    crate::popup::node_menu::base_items(),
                                ),
                                open: MenuOpen::Popup,
                            },
                        );
                    }
```

Delete `node_radial_items()` entirely (`app.rs:738-773`, including its doc comment). With the `NodeRadial` arm gone, `RadialOpen` is no longer used in `app.rs` — remove it from the import at `:8`:

```rust
use crate::popup::root::{MenuOpen, PopupRoot, PopupSpec};
```

- [ ] **Step 7: Build — verify the pivot compiles (old dispatch still live)**

Run: `cargo build -p waml-editor`
Expected: builds clean. Note: `canvas::node_command_for` is still called at `app.rs:1030` and returns `None` for `view_source`/`find_in_diagrams`, so committing the menu is a harmless no-op this task — the dispatch swap is Task 4. `NodeCommand`/`node_command_for` and their test remain live here.

- [ ] **Step 8: Run the full unit suite**

Run: `cargo test -p waml-editor`
Expected: PASS — existing tests unaffected; `node_command_maps_the_four_committed_ids` still green (removed in Task 4).

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/canvas.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/app.rs
git commit -m "feat(canvas): pivot node menu radial->linear MenuPopup via DocView seam"
```

---

### Task 4: Dispatch swap + delete radial command code

**Files:**
- Modify: `crates/waml-editor/src/app.rs:1029-1033` (dispatch swap), `:274-324` (drop `node_menu_key` allow)
- Modify: `crates/waml-editor/src/doc_tabs.rs` (drop `open_source`/`source_tab_id` allows)
- Modify: `crates/waml-editor/src/canvas.rs:432-455` (remove `NodeCommand` + `node_command_for`), `:1133-1146` (remove its test)

**Interfaces:**
- Consumes: `node_menu::command_for`, `NodeMenuCommand` (Task 1); `OpenTabs::open_source` (Task 2); `App.node_menu_key` (Task 3).
- Produces: nothing new (removals + wiring).

- [ ] **Step 1: Swap the `node_closed` dispatch**

In `crates/waml-editor/src/app.rs`, replace the `node_closed` dispatch block (`:1029-1033`):

```rust
            if let Some(PopupResult::Invoked(id)) = node_closed {
                if let Some(cmd) = crate::popup::node_menu::command_for(id) {
                    let key = self.node_menu_key.clone().unwrap_or_default();
                    match cmd {
                        crate::popup::node_menu::NodeMenuCommand::ViewSource => {
                            if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
                                let title = node
                                    .concept
                                    .title
                                    .clone()
                                    .unwrap_or_else(|| node.key.clone());
                                self.tabs.open_source(key.clone(), title);
                                self.refresh_doc_tabs(cx);
                                self.sync_active_tab(cx);
                            }
                        }
                        crate::popup::node_menu::NodeMenuCommand::FindInDiagrams => {
                            log!("find in diagrams: {key}");
                        }
                    }
                }
            }
```

(This block runs after `drop(pr)` at `app.rs:996`, so `self` is free for `open_source`/`refresh_doc_tabs`/`sync_active_tab`.)

- [ ] **Step 2: Drop the now-consumed `#[allow(dead_code)]` guards**

In `crates/waml-editor/src/app.rs`, remove the `#[allow(dead_code)]` line above `node_menu_key` (it is now read). In `crates/waml-editor/src/doc_tabs.rs`, remove the `#[allow(dead_code)]` above `open_source` and above `source_tab_id` (both now reached from the live dispatch).

- [ ] **Step 3: Remove `NodeCommand` + `node_command_for` + its test**

In `crates/waml-editor/src/canvas.rs`, delete the `NodeCommand` enum (`:432-440`), `node_command_for` (`:442-455`), and the test `node_command_maps_the_four_committed_ids` (`:1133-1146`).

- [ ] **Step 4: Run the full suite + clippy gate**

Run: `cargo test -p waml-editor`
Expected: PASS. The removed test is gone; `node_menu`/`open_source` tests stay green.
Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean — no `dead_code` (all guards removed only where the item is now live; `NodeCommand`/`node_command_for` deleted), no `unused_imports`.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/app.rs crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/canvas.rs
git commit -m "feat(app): dispatch node_menu commands, remove radial NodeCommand stubs"
```

---

### Task 5: Project-tree entry point (fork right-click)

**Files:**
- Modify: `C:\dev\makepad\widgets\src\file_tree.rs:493-500` (`FileTreeAction::FileRightClicked`), `:502-507` (`FileTreeNodeAction::SecondaryClicked`), `:635-661` (secondary handler arm), `:876-907` (drain arm), `:945-971` (`FileTreeRef::file_right_clicked`)
- Modify: `crates/waml-editor/src/tree_panel.rs:256-276` (`ProjectTreeAction::ContextMenu`), `:901-923` (refactor file_clicked + right-click read), `:1043` (add `context_menu_request` reader), add `is_classifier_kind` helper + test
- Modify: `crates/waml-editor/src/app.rs:1190-1212` (tree context handler)

**Interfaces:**
- Consumes: `node_menu::compose`/`base_items` (Task 1); `OpenTabs::open_preview` (existing); `PopupSpec::Menu` (existing).
- Produces:
  - fork: `FileTreeAction::FileRightClicked { node_id: LiveId, abs: DVec2 }`, `FileTreeNodeAction::SecondaryClicked(DVec2)`, `FileTreeRef::file_right_clicked(&self, actions: &Actions) -> Option<(LiveId, DVec2)>`
  - `fn is_classifier_kind(kind: TreeKind) -> bool` (module-private in `tree_panel.rs`)
  - `ProjectTreeAction::ContextMenu { key: String, anchor: DVec2 }`
  - `ProjectTree::context_menu_request(&self, actions: &Actions) -> Option<(String, DVec2)>`

- [ ] **Step 1: Write the failing `is_classifier_kind` test**

In `crates/waml-editor/src/tree_panel.rs`, add to `#[cfg(test)] mod tests` (`:1053`):

```rust
    #[test]
    fn is_classifier_kind_covers_the_four_classifier_kinds_only() {
        assert!(is_classifier_kind(TreeKind::Class));
        assert!(is_classifier_kind(TreeKind::Interface));
        assert!(is_classifier_kind(TreeKind::Enum));
        assert!(is_classifier_kind(TreeKind::DataType));
        assert!(!is_classifier_kind(TreeKind::Diagram));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p waml-editor is_classifier_kind`
Expected: FAIL — `cannot find function is_classifier_kind`.

- [ ] **Step 3: Add the helper + refactor `file_clicked`**

In `crates/waml-editor/src/tree_panel.rs`, add the free helper near the top of the module body (e.g. just above `impl ProjectTree`):

```rust
/// The four `TreeKind`s that previews treat as classifiers (they used to share
/// `TreeKind::Class` before per-glyph rows split them out). Shared by the
/// left-click focus path and the right-click context-menu path.
fn is_classifier_kind(kind: TreeKind) -> bool {
    matches!(
        kind,
        TreeKind::Class | TreeKind::Interface | TreeKind::Enum | TreeKind::DataType
    )
}
```

Refactor the `file_clicked` block (`:901-923`) to use it:

```rust
            if let Some(id) = file_tree.file_clicked(actions) {
                let kind = self.id_to_kind.get(&id).copied();
                if let Some(key) = self.id_to_key.get(&id) {
                    match kind {
                        Some(TreeKind::Diagram) => {
                            cx.widget_action(uid, ProjectTreeAction::SelectDiagram(key.clone()));
                        }
                        Some(k) if is_classifier_kind(k) => {
                            cx.widget_action(uid, ProjectTreeAction::FocusClassifier(key.clone()));
                        }
                        _ => {}
                    }
                }
            }
```

- [ ] **Step 4: Run to verify the helper test passes**

Run: `cargo test -p waml-editor is_classifier_kind`
Expected: PASS.

- [ ] **Step 5: Fork — add the secondary-click action + reader**

In `C:\dev\makepad\widgets\src\file_tree.rs`, extend `FileTreeAction` (`:493-500`):

```rust
pub enum FileTreeAction {
    #[default]
    None,
    FileClicked(LiveId),
    FolderClicked(LiveId),
    ShouldFileStartDrag(LiveId),
    FileRightClicked { node_id: LiveId, abs: DVec2 },
}
```

Extend `FileTreeNodeAction` (`:502-507`):

```rust
pub enum FileTreeNodeAction {
    WasClicked,
    Opening,
    Closing,
    ShouldStartDrag,
    SecondaryClicked(DVec2),
}
```

Add a secondary-`FingerDown` arm **before** the existing `Hit::FingerDown(_)` arm in `FileTreeNode::handle_event` (`:647`), so a right-press emits only `SecondaryClicked` (no select / `WasClicked`):

```rust
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                actions.push((node_id, FileTreeNodeAction::SecondaryClicked(fe.abs)));
            }
            Hit::FingerDown(_) => {
```

Drain it in `FileTree`'s node-action loop, adding an arm after `ShouldStartDrag` (`:905`):

```rust
                FileTreeNodeAction::SecondaryClicked(abs) => {
                    cx.widget_action(uid, FileTreeAction::FileRightClicked { node_id, abs });
                }
```

Add the reader to `impl FileTreeRef` (after `folder_clicked`, `:971`):

```rust
    pub fn file_right_clicked(&self, actions: &Actions) -> Option<(LiveId, DVec2)> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let FileTreeAction::FileRightClicked { node_id, abs } = item.cast() {
                return Some((node_id, abs));
            }
        }
        None
    }
```

- [ ] **Step 6: Add `ProjectTreeAction::ContextMenu`, the reader, and the right-click wiring**

In `crates/waml-editor/src/tree_panel.rs`, extend `ProjectTreeAction` (`:256-276`):

```rust
    /// A secondary-button press over a classifier row. `App` selects the row
    /// (via `open_preview`) and opens the base node menu at `anchor`.
    ContextMenu {
        key: String,
        anchor: DVec2,
    },
```

In `handle_event`'s `Event::Actions` block, add a right-click read directly after the `file_clicked` block (Step 3):

```rust
            if let Some((id, abs)) = file_tree.file_right_clicked(actions) {
                if let (Some(kind), Some(key)) =
                    (self.id_to_kind.get(&id).copied(), self.id_to_key.get(&id))
                {
                    if is_classifier_kind(kind) {
                        cx.widget_action(
                            uid,
                            ProjectTreeAction::ContextMenu {
                                key: key.clone(),
                                anchor: abs,
                            },
                        );
                    }
                }
            }
```

Add the reader to `impl ProjectTree` (after `filter_request`, `:1043`):

```rust
    /// A right-click over a classifier row. `App` selects the row and relays
    /// the base node menu to `PopupRoot` (mirrors `scope_request`/`filter_request`).
    pub fn context_menu_request(&self, actions: &Actions) -> Option<(String, DVec2)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::ContextMenu { key, anchor } = item.cast() {
            Some((key, anchor))
        } else {
            None
        }
    }
```

- [ ] **Step 7: App — handle the tree context request (select + open base menu)**

In `crates/waml-editor/src/app.rs`, add this block in `handle_actions` immediately **before** the `focused_classifier` block (`:1194`):

```rust
        // Tree right-click: select the classifier (open/replace its preview
        // tab, same as a left-click focus) and open the base-only node menu at
        // the row.
        let tree_context = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
            .and_then(|panel| panel.context_menu_request(actions));
        if let Some((key, anchor)) = tree_context {
            if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
                let title = node
                    .concept
                    .title
                    .clone()
                    .unwrap_or_else(|| node.key.clone());
                self.tabs
                    .open_preview(key.clone(), title, crate::tree::kind_of(&node.ty));
                self.refresh_doc_tabs(cx);
                self.sync_active_tab(cx);
            }
            self.node_menu_key = Some(key);
            let bounds = self.window_bounds(cx);
            if let Some(mut pr) = self
                .ui
                .widget(cx, ids!(popup_root))
                .borrow_mut::<PopupRoot>()
            {
                pr.show_at(
                    cx,
                    PopupSpec::Menu {
                        tag: live_id!(node_menu),
                        anchor,
                        bounds,
                        items: crate::popup::node_menu::compose(
                            vec![],
                            crate::popup::node_menu::base_items(),
                        ),
                        open: MenuOpen::Popup,
                    },
                );
            }
            return;
        }
```

- [ ] **Step 8: Run the full suite + clippy gate + build**

Run: `cargo test -p waml-editor`
Expected: PASS (including `is_classifier_kind_covers_the_four_classifier_kinds_only`).
Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean.
Run: `cargo build -p waml-editor`
Expected: builds against the patched fork (the `file_tree.rs` additions compile; `file_right_clicked` is live via `tree_panel`, `context_menu_request` live via `app`).

- [ ] **Step 9: Manual visual verify (repo self-screenshot recipe)**

Per repo memory `screenshot-verify-hits-user-editor`, launch a dedicated instance by pid in one PowerShell call and screenshot that pid only (never kill-all). Confirm:
- Right-click a class node in a diagram → linear card opens at the cursor with rows [View Source, Find in diagrams]; the inspector follows the node.
- Right-click a classifier row in the project tree → same base-only card at the row; the row's preview opens.
- Commit **View Source** → a new source tab opens showing an empty (canvas-ground) document; tool dock + inspector picker hidden.
- Commit **Find in diagrams** → log line `find in diagrams: <key>`.

- [ ] **Step 10: Commit**

```bash
git add C:/dev/makepad/widgets/src/file_tree.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app.rs
git commit -m "feat(tree): right-click classifier row opens the base node menu"
```

---

## Self-Review

**Spec coverage:**
- Uniform base menu (View Source, Find in diagrams) → Task 1 (`base_items`, `command_for`), Task 4 (dispatch).
- `compose(context, base)` = context ++ base, empty context = base → Task 1 (tested).
- Diagram entry point (`NodeMenu{abs,key}`, `context_items`, select-on-right-click, `NodeContextMenu` relay to `PopupSpec::Menu`) → Task 3.
- Model-view entry point (`ProjectTreeAction::ContextMenu`, `context_menu_request`, empty context, reuse focus/select path) → Task 5.
- View Source opens an empty source tab (`TabKind::Source`, `open_source`, `SourceView`, placeholder `source_view` slot) → Task 2 + Task 4.
- Find in diagrams = `log!` stub → Task 4.
- Remove `node_radial_items` / `canvas::NodeCommand` / `node_command_for`; keep `RadialPopup` → Task 3 (remove `node_radial_items`) + Task 4 (remove `NodeCommand`/`node_command_for`); `PopupSpec::Radial` + `RadialOpen` remain defined in `popup/root.rs`.
- Non-goals honored: no separator, no real source text, empty context, no new `Subject` variant.

**Placeholder scan:** No TBD/TODO; every code step shows compilable Rust/DSL; every command has an expected result.

**Type consistency:** `NodeMenuCommand` variants (`ViewSource`/`FindInDiagrams`) consistent across Tasks 1/4. `PopupItem` field set (`id,label,icon,danger,enabled`) matches `popup/base.rs`. `NodeContextMenu { anchor: DVec2, key: String, context: Vec<PopupItem> }` produced in Task 3 (doc_view) and consumed in Task 3 (app relay). `node_menu_key: Option<String>` written Task 3, read Task 4. `file_right_clicked -> Option<(LiveId, DVec2)>` produced + consumed in Task 5. `is_classifier_kind(TreeKind) -> bool` defined + used in Task 5.

**Dead-code gate ordering:** every item that lands ahead of its consumer is `#[allow(dead_code)]`-guarded and un-guarded in the consuming task (`open_source`/`source_tab_id`: Task 2→4; `node_menu_key`: Task 3→4). `node_menu.rs` and `doc_view.rs` carry file-level `#![allow(dead_code)]`. The Task 3→4 split keeps `node_command_for` live until its dispatch is swapped.
