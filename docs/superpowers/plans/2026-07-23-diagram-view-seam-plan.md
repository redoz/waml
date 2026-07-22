# Diagram View Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a logical seam between the waml-editor *app shell* (`App`) and per-tab *document views*, so the class-diagram surface and the single-element preview each become a self-owned `DocView` object, without changing a single pixel on screen.

**Architecture:** Each open tab owns a plain Rust object (`Box<dyn DocView>`) holding its live per-tab state (camera lives in the canvas widget today; `expanded` moves into the diagram view). The one existing body widget (canvas + inspector + tool dock + selection toolbar) is the single shared draw surface — the active view is asked to push its state into that surface on `sync`, and to consume tab-routed `Actions` on `handle`, returning a `ViewOutcome` the shell relays (applies `Op`s, opens previews, places popups). There is **no** dynamic Makepad widget instancing (this codebase is hand-rolled immediate-mode with `draw_abs` + manual hit-rects — see the spec §4); switching tabs swaps which Rust object drives the surface.

**Tech Stack:** Rust, Makepad (redoz fork) hand-rolled immediate-mode widgets, `waml::model::Model` + `waml::ops::Op`, existing `waml-editor` crate modules.

## Global Constraints

- **No visual change.** The screen must be pixel-identical before and after every task (spec §2, §6). This is the primary acceptance bar.
- **Behavior-preserving per task.** Each task compiles, keeps the existing suite (299+ unit tests) green, and is independently landable (spec §6).
- **No dynamic widget-subtree instancing.** Views are plain Rust objects sharing the one existing body surface. Do NOT introduce PortalList / TabBar / mounted-per-tab subtrees (spec §2, §4).
- **Views never touch `Model` / `OpenTabs` / `popup_root` directly.** They render from a borrowed `&Model` and emit intent via `ViewOutcome`; the shell is the only place that applies edits, opens tabs, and places popups (spec §3 rules 1-3).
- **Out of scope:** `Op::PlaceSet` layout write-back; Sequence/Activity/OKF views; any real `Op` application (none exists in the shell today — the `ViewOutcome.ops` channel is wired but drains empty in this migration).
- **`Cx`, not `Cx2d`.** The spec §5 sketch shows `sync(&mut self, cx: &mut Cx2d, ...)`, but this codebase pushes widget state imperatively with plain `&mut Cx` (e.g. `self.ui.widget(cx, ids!(canvas)).borrow_mut::<GraphCanvas>()`), the same path `sync_active_tab` uses. The trait uses `&mut Cx`.
- **Builds/screenshots run the worktree's OWN copy.** Use `./scripts/run-native.ps1 -Optimized` from the worktree root (it kills only the instance built from THIS checkout's exe path and relinks that exe). Screenshot-verify by capturing the launched process by its **explicit pid** in one PowerShell call — never screenshot/kill `waml-editor` by name (that grabs/kills the user's own running editor). See `MEMORY.md` notes "run-native builds the script's dir" and "Screenshot-verify hits user's editor".

---

## File Structure

- `crates/waml-editor/src/doc_view.rs` — **new.** The seam contract: `DocView` trait, `ViewOutcome`, `PopupRequest`, the `BodyWidgets` accessor bundle, and the `make_view` factory. Pure Rust, no DSL / `script_mod`.
- `crates/waml-editor/src/class_diagram_view.rs` — **new.** `ClassDiagramView`: owns `expanded`, drives canvas full-scene + inspector-with-picker + tool dock + selection toolbar.
- `crates/waml-editor/src/classifier_preview_view.rs` — **new.** `ClassifierPreviewView`: owns the previewed `key`, drives the focus canvas + inspector-without-picker, no tool dock.
- `crates/waml-editor/src/app.rs` — **modified.** Shell delegates to views and relays `ViewOutcome`; the `TabKind` `match` arms in `sync_active_tab` / `handle_actions` are progressively emptied and finally deleted; a `views` registry field is added.
- `crates/waml-editor/src/main.rs` — **modified.** Add `mod doc_view; mod class_diagram_view; mod classifier_preview_view;` to the module list (around lines 12-13, alphabetical is not enforced there).

Nothing new is a widget, so **no `script_mod` registration** is required (the `AppMain::script_mod` chain in `app.rs` is untouched).

---

### Task 1: Extract the `BodyWidgets` accessor bundle

**Files:**
- Create: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod doc_view;`)
- Modify: `crates/waml-editor/src/app.rs` (`sync_active_tab`, `set_diagram_toolbars`, and the canvas/inspector/tool-dock/selection-toolbar reads in `handle_actions` use the bundle)

**Why:** Spec §6 step 1 and §8 ("Hidden shared state"): surface every body widget the future views poke through *one* named struct before any behavior moves. Pure mechanical grouping — no behavior change.

**Interfaces:**
- Produces: `pub struct BodyWidgets { ui: WidgetRef }` with:
  - `pub fn new(cx: &mut Cx, ui: &WidgetRef) -> BodyWidgets` — clones the five body-widget refs off the shell's `ui`.
  - `pub fn canvas(&self, cx: &mut Cx) -> WidgetRef` (id `canvas`)
  - `pub fn inspector(&self, cx: &mut Cx) -> WidgetRef` (id `inspector`)
  - `pub fn tool_dock(&self, cx: &mut Cx) -> WidgetRef` (id `tool_dock`)
  - `pub fn selection_toolbar(&self, cx: &mut Cx) -> WidgetRef` (id `selection_toolbar`)
  - `pub fn set_tool_dock_visible(&self, cx: &mut Cx, show: bool)` — toggles `tool_dock_wrap` visibility (the body of today's `set_diagram_toolbars`).
- Consumes: nothing (first task).

**Design note:** `self.ui.widget(cx, ids!(x))` returns a cloned `WidgetRef` handle; a caller then does `.borrow_mut::<T>()` on it inline (the handle must live to the end of the statement). `BodyWidgets` holds `ui: self.ui.clone()` and each accessor is a thin `self.ui.widget(cx, ids!(x))` — identical to today's inline call, just named in one place.

- [ ] **Step 1: Create `doc_view.rs` with the bundle**

```rust
//! The app-shell / document-view seam (spec 2026-07-23-diagram-view-seam-design).
//!
//! `BodyWidgets` names the one shared body draw surface the per-tab views push
//! into; the `DocView` trait + `ViewOutcome` + `make_view` factory land in later
//! tasks. Pure Rust — nothing here is a widget, so there is no `script_mod`.

use makepad_widgets::*;

/// Typed handles to the single shared body surface (canvas + inspector + tool
/// dock + selection toolbar) the active `DocView` renders through. Cheap: holds
/// a clone of the shell's root `ui`; each accessor is the same `ui.widget(..)`
/// lookup the shell used inline, gathered in one place so the seam surface is
/// explicit.
pub struct BodyWidgets {
    ui: WidgetRef,
}

impl BodyWidgets {
    pub fn new(_cx: &mut Cx, ui: &WidgetRef) -> BodyWidgets {
        BodyWidgets { ui: ui.clone() }
    }

    pub fn canvas(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(canvas))
    }
    pub fn inspector(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(inspector))
    }
    pub fn tool_dock(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(tool_dock))
    }
    pub fn selection_toolbar(&self, cx: &mut Cx) -> WidgetRef {
        self.ui.widget(cx, ids!(selection_toolbar))
    }

    /// Show/hide the left tool dock wrapper (`tool_dock_wrap`). Body of the
    /// shell's old `set_diagram_toolbars`.
    pub fn set_tool_dock_visible(&self, cx: &mut Cx, show: bool) {
        self.ui.widget(cx, ids!(tool_dock_wrap)).set_visible(cx, show);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/waml-editor/src/main.rs`, add after line 12 (`mod doc_tabs;`):

```rust
mod class_diagram_view;
mod classifier_preview_view;
mod doc_view;
```

(Declare all three now; the latter two files land empty-then-filled in Tasks 3-4. To keep this task compiling on its own, create `class_diagram_view.rs` and `classifier_preview_view.rs` as empty files with a single `//! placeholder — filled in Task 3/4` line, OR add only `mod doc_view;` here and add the other two `mod` lines in Tasks 3 and 4. Prefer the latter: add only `mod doc_view;` in this task.)

Net for this task — add only:

```rust
mod doc_view;
```

- [ ] **Step 3: Route `sync_active_tab` + `set_diagram_toolbars` through the bundle**

In `app.rs`, replace the inline `self.ui.widget(cx, ids!(canvas)).borrow_mut::<..>()` / `ids!(inspector)` / `ids!(selection_toolbar)` reads inside `sync_active_tab` (lines ~351-421) and the `set_diagram_toolbars` body (lines 427-431) so they go through a locally-built `BodyWidgets`. Example for the Diagram arm's canvas push:

```rust
let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
// ...
if let Some(mut canvas) = body.canvas(cx).borrow_mut::<crate::canvas::GraphCanvas>() {
    canvas.set_scene(cx, scene);
}
```

and `set_diagram_toolbars` becomes:

```rust
fn set_diagram_toolbars(&mut self, cx: &mut Cx, show: bool) {
    crate::doc_view::BodyWidgets::new(cx, &self.ui).set_tool_dock_visible(cx, show);
}
```

Do **not** move any logic — only swap the widget-lookup expression. `project_tree`, `statusbar`, `doc_tabs`, and `diagram_switcher` are shell chrome and stay as direct `self.ui.widget(..)` reads (they are NOT in the bundle).

- [ ] **Step 4: Build + test**

Run: `cargo test -p waml-editor`
Expected: PASS, same test count as before (299+). No new tests — this is a pure refactor with no new testable unit.

Run: `cargo build -p waml-editor --bin waml-editor`
Expected: clean build, zero new warnings (the gate promotes `dead_code` to a hard error — `BodyWidgets` must be used, which Step 3 ensures).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "refactor(view-seam): extract BodyWidgets accessor bundle (no behavior change)"
```

**Acceptance:** Compiles, `cargo test -p waml-editor` green at the same count, screen unchanged (pure refactor — no runtime path altered, only where widget refs are looked up).

---

### Task 2: Introduce `DocView` + `ViewOutcome` + `PopupRequest` + registry + factory (dead)

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs` (add trait, outcome, request, factory)
- Modify: `crates/waml-editor/src/app.rs` (add `views` registry field; `use` the factory so it is not dead)

**Why:** Spec §6 step 2: add the contract and an empty registry *alongside* the existing `match`. Nothing drives a view yet — this task only defines and wires the plumbing so Tasks 3-5 have types to target.

**Interfaces:**
- Produces:
  - `pub struct ViewOutcome` (all fields `pub`, `#[derive(Default)]`):
    - `ops: Vec<waml::ops::Op>` — edit intents applied by the shell (empty in this migration; forward-looking).
    - `open_preview: Option<String>` — request the shell open an element preview by key (spec §5; unused this migration — the project tree still drives previews as shell chrome).
    - `popup: Option<PopupRequest>` — a cross-tree popup the shell must place via `popup_root`.
    - `promote_subject: Option<String>` — request the shell promote the tab whose `key` matches (from an inspector `Edited`).
    - `close_active: bool` — request the shell close the active tab (from the selection toolbar `Delete` on a preview).
    - `statusbar_dirty: bool` — request the shell re-push the statusbar snapshot (from a tool-dock `ModeChanged`).
  - `pub enum PopupRequest`:
    - `NodeRadial { center: DVec2 }` — the node command wheel; items are always `crate::app::node_radial_items()`, tag `node_menu`, open `RadialOpen::Marking`.
    - `ElementPicker { anchor_rect: Rect, min_width: f64, items: Vec<crate::popup::select::SelectItem> }` — the inspector element-picker flyout; tag `element_picker`.
  - `pub trait DocView`:
    - `fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model);`
    - `fn handle(&mut self, cx: &mut Cx, body: &BodyWidgets, actions: &Actions, model: &Model) -> ViewOutcome;`
    - `fn on_popup_result(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model, tag: LiveId, result: crate::popup::base::PopupResult) -> ViewOutcome { let _ = (cx, body, model, tag, result); ViewOutcome::default() }`
    - `fn wants_tooldock(&self) -> bool;`
    - `fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) { let _ = (cx, body); }`
    - `fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) { let _ = (cx, body); }`
  - `pub fn make_view(tab: &crate::doc_tabs::DocTab) -> Box<dyn DocView>` — dispatches on `tab.kind`.
- Consumes: `BodyWidgets` (Task 1).

**Deviations from the spec §5 sketch (all documented in-code):** trait uses `&mut Cx` not `Cx2d`; `handle`/`sync` take `&BodyWidgets`; `ViewOutcome` adds `promote_subject` / `close_active` / `statusbar_dirty` — the real tab-lifecycle + statusbar intents the current behavior needs, which the "sketch" (spec's word) did not enumerate; `on_popup_result` is added so document-scoped popup results (element picker) route back to the owning view while `popup_root` access stays in the shell (spec §3 rule 1).

- [ ] **Step 1: Write the failing test for the factory + outcome default**

Append to `doc_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_tabs::{DocTab, TabKind};
    use crate::tree::TreeKind;

    fn tab(kind: TabKind, node_kind: TreeKind) -> DocTab {
        DocTab {
            id: LiveId::from_str("t"),
            key: "k".into(),
            title: "T".into(),
            kind,
            node_kind,
            preview: false,
        }
    }

    #[test]
    fn view_outcome_default_is_all_empty() {
        let o = ViewOutcome::default();
        assert!(o.ops.is_empty());
        assert!(o.open_preview.is_none());
        assert!(o.popup.is_none());
        assert!(o.promote_subject.is_none());
        assert!(!o.close_active);
        assert!(!o.statusbar_dirty);
    }

    #[test]
    fn make_view_dispatches_on_tab_kind() {
        let dv = make_view(&tab(TabKind::Diagram, TreeKind::Diagram));
        assert!(dv.wants_tooldock(), "diagram view drives the tool dock");
        let cv = make_view(&tab(TabKind::Classifier, TreeKind::Class));
        assert!(!cv.wants_tooldock(), "preview view has no tool dock");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor doc_view`
Expected: FAIL — `ViewOutcome`, `make_view`, `ClassDiagramView`, `ClassifierPreviewView` not defined.

- [ ] **Step 3: Add the trait, outcome, request, and factory**

Add to `doc_view.rs` (above the tests). The two concrete types are defined minimally here so the factory compiles; they gain real bodies in Tasks 3-4.

```rust
use waml::model::Model;
use waml::ops::Op;

use crate::doc_tabs::{DocTab, TabKind};
use crate::popup::base::PopupResult;
use crate::popup::select::SelectItem;

/// What a view hands back to the shell per interaction. The shell is the only
/// place that applies ops, opens tabs, and places popups (spec §3).
#[derive(Default)]
pub struct ViewOutcome {
    /// Edit intents the shell applies to `Model`. Empty in the seam migration —
    /// no `Op` is applied in the shell yet; this channel is forward-looking.
    pub ops: Vec<Op>,
    /// Ask the shell to open an element preview by key (spec §5). Unused this
    /// migration: the project tree (shell chrome) still drives previews.
    pub open_preview: Option<String>,
    /// A cross-tree popup the shell must place via `popup_root`.
    pub popup: Option<PopupRequest>,
    /// Ask the shell to promote (pin) the tab whose key matches this subject.
    pub promote_subject: Option<String>,
    /// Ask the shell to close the active tab.
    pub close_active: bool,
    /// Ask the shell to re-push the statusbar snapshot.
    pub statusbar_dirty: bool,
}

/// A popup a view wants placed. The view describes it; the shell computes window
/// bounds + anchor offset and calls `popup_root.show_at` (spec §3 rule 2).
pub enum PopupRequest {
    /// Node command wheel — items are always `node_radial_items()`.
    NodeRadial { center: DVec2 },
    /// Inspector element-picker flyout.
    ElementPicker {
        anchor_rect: Rect,
        min_width: f64,
        items: Vec<SelectItem>,
    },
}

/// One open document tab's behavior + live state. Shell-owned, one per tab.
pub trait DocView {
    /// Push this view's state into the shared body surface from a read-only
    /// `Model`. Imperative (plain `Cx`), like the shell's old `sync_active_tab`.
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model);

    /// Consume tab-routed actions; return intent upward.
    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        model: &Model,
    ) -> ViewOutcome;

    /// A document-scoped popup this view requested has closed; route its result
    /// back down. `popup_root` is read by the shell; only the result crosses.
    fn on_popup_result(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        tag: LiveId,
        result: PopupResult,
    ) -> ViewOutcome {
        let _ = (cx, body, model, tag, result);
        ViewOutcome::default()
    }

    /// Does this view drive the left tool dock? (diagram: yes, preview: no)
    fn wants_tooldock(&self) -> bool;

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        let _ = (cx, body);
    }
}

/// Create the view object for a tab, discriminating on `TabKind` (spec §5).
pub fn make_view(tab: &DocTab) -> Box<dyn DocView> {
    match tab.kind {
        TabKind::Diagram => Box::new(crate::class_diagram_view::ClassDiagramView::new()),
        TabKind::Classifier => {
            Box::new(crate::classifier_preview_view::ClassifierPreviewView::new(tab.key.clone()))
        }
    }
}
```

- [ ] **Step 4: Add minimal `ClassDiagramView` / `ClassifierPreviewView` stubs so the factory compiles**

Create `crates/waml-editor/src/class_diagram_view.rs`:

```rust
//! `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-
//! picker + tool dock + selection toolbar). Stub in Task 2; filled in Task 3.

use makepad_widgets::*;
use std::collections::HashSet;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};

#[derive(Default)]
pub struct ClassDiagramView {
    /// Node keys whose card body is expanded. Per-tab live state; moved off the
    /// shell in Task 3. Cleared when the diagram changes.
    expanded: HashSet<String>,
}

impl ClassDiagramView {
    pub fn new() -> ClassDiagramView {
        ClassDiagramView::default()
    }
}

impl DocView for ClassDiagramView {
    fn sync(&mut self, _cx: &mut Cx, _body: &BodyWidgets, _model: &Model) {}
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
        true
    }
}
```

Create `crates/waml-editor/src/classifier_preview_view.rs`:

```rust
//! `ClassifierPreviewView` — the single-element preview (focus canvas + inspector-
//! without-picker, no tool dock). Stub in Task 2; filled in Task 4.

use makepad_widgets::*;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};

pub struct ClassifierPreviewView {
    /// The previewed classifier/package key.
    key: String,
}

impl ClassifierPreviewView {
    pub fn new(key: String) -> ClassifierPreviewView {
        ClassifierPreviewView { key }
    }
}

impl DocView for ClassifierPreviewView {
    fn sync(&mut self, _cx: &mut Cx, _body: &BodyWidgets, _model: &Model) {}
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
}
```

Add both `mod` lines to `main.rs` now (after `mod doc_view;` from Task 1):

```rust
mod class_diagram_view;
mod classifier_preview_view;
```

- [ ] **Step 5: Add the (empty) registry field to `App` and keep the factory non-dead**

In `app.rs`, add to `struct App` (after the `nav_filter_ids` field, keeping the `#[rust]` attribute pattern):

```rust
    /// One live view object per open tab, keyed by `DocTab::id`. Populated /
    /// pruned by the shell as tabs open and close (Task 5). Empty until then.
    #[rust]
    views: std::collections::HashMap<LiveId, Box<dyn crate::doc_view::DocView>>,
```

The `dead_code` gate rejects a never-constructed factory. To keep `make_view` live without driving behavior yet, have `switch_diagram` / `open_dir` / the tab-open paths seed the map — but that is Task 5's job. For **this** task only, prevent the dead-code error by referencing the factory in a debug assertion inside `sync_active_tab` that does not change behavior:

```rust
// Seam scaffolding (Task 2): keep the factory reachable until Task 5 wires
// the registry for real. Builds a throwaway view and drops it — no surface
// touched, no state kept.
let _ = crate::doc_view::make_view(&active);
```

Place this immediately after `let Some(active) = self.tabs.active_tab().cloned() else { .. }` resolves a tab, before the `match active.kind`. It constructs and drops a view — zero behavioral effect. (This line is deleted in Task 5 when the registry drives sync for real.)

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p waml-editor doc_view`
Expected: PASS — `view_outcome_default_is_all_empty` and `make_view_dispatches_on_tab_kind` green.

Run: `cargo test -p waml-editor`
Expected: PASS, count = previous + 2.

Run: `cargo build -p waml-editor --bin waml-editor`
Expected: clean build, no `dead_code` error (registry field is read by the throwaway-view line; factory + types all referenced).

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(view-seam): add DocView trait, ViewOutcome, PopupRequest, factory + empty registry (dead)"
```

**Acceptance:** Compiles; two new unit tests green; total suite green; screen unchanged (no view drives the surface — `sync_active_tab` still runs its original `match` verbatim; the throwaway `make_view` line has no effect).

---

### Task 3: Move the Diagram arm into `ClassDiagramView`

**Files:**
- Modify: `crates/waml-editor/src/class_diagram_view.rs` (real `sync`, `handle`, `on_popup_result`)
- Modify: `crates/waml-editor/src/app.rs` (Diagram arm of `sync_active_tab` delegates to the view; the diagram-scoped branches of `handle_actions` delegate; `expanded` field removed from `App`, moved into the view)

**Why:** Spec §6 step 3. Port `build_scene`, inspector-with-picker, tool dock, selection toolbar, and the canvas/inspector/picker/dock/toolbar branches of `handle_actions` into the view; the shell delegates the Diagram case and relays its `ViewOutcome`.

**Interfaces:**
- Consumes: `BodyWidgets`, `DocView`, `ViewOutcome`, `PopupRequest` (Task 2); `crate::scene::build_scene`, `crate::inspector::{diagram_elements, Subject}`, `crate::canvas::{GraphCanvas, GraphCanvasAction}`, `crate::inspector_panel::Inspector`, `crate::tool_dock::{ToolDock, ToolDockAction}`, `crate::selection_toolbar::{SelectionToolbar, SelectionToolbarAction}`, `crate::popup::base::PopupResult`.
- Produces: `ClassDiagramView` implementing `DocView` with real bodies; `ClassDiagramView::set_diagram(&mut self)` clears `expanded` when the base tab's diagram changes (called by the shell's `switch_diagram`, replacing today's `self.expanded.clear()`).

**Behavior mapping (all verbatim moves from `app.rs`, re-homed onto `body`/`self.expanded`):**

| Shell source (app.rs) | Moves to |
|---|---|
| `sync_active_tab` Diagram arm (lines ~352-390): `build_scene` + `canvas.set_scene` + `sync_inspector_elements` + `inspector.set_subject(None)` + `toolbar.set_selection(None)` | `ClassDiagramView::sync` |
| `App::sync_inspector_elements` (lines 485-500) | private `ClassDiagramView` helper, driven off `body.inspector(cx)` |
| element-picker open (`inspector.take_open_request`, lines 1364-1392) | `handle` → `ViewOutcome.popup = Some(PopupRequest::ElementPicker{..})` |
| tool-dock action (lines 1397-1408) | `handle` → on `ModeChanged`, `ViewOutcome.statusbar_dirty = true`; other variants `log!` as today |
| canvas actions `NodeMenu`/`NodeSelect`/`NodeDeselect`/`ToggleExpand` (lines 1414-1488) | `handle` → `NodeMenu` sets `ViewOutcome.popup = Some(PopupRequest::NodeRadial{center: abs})`; `NodeSelect`/`NodeDeselect` repoint the inspector directly on `body`; `ToggleExpand` mutates `self.expanded` + `canvas.update_scene` |
| inspector `edited` (lines 1347-1359) | `handle` → `ViewOutcome.promote_subject = Some(key)` |
| selection-toolbar `Delete`/`NewDiagram` (lines 1584-1601) | `handle` → `Delete` is a no-op for a diagram tab (today it acts only when `active.kind == Classifier`); `NewDiagram` `log!` no-op |
| element-picker closed (lines 1141-1149) | `on_popup_result` for tag `element_picker` → `inspector.on_picker_closed` |

- [ ] **Step 1: Write `ClassDiagramView::sync` and the inspector-elements helper**

Replace the stub `sync` in `class_diagram_view.rs`. Move the Diagram arm body verbatim, re-homing widget lookups to `body`:

```rust
use crate::inspector::{diagram_elements, Subject};
use crate::scene::build_scene;

impl ClassDiagramView {
    pub fn new() -> ClassDiagramView {
        ClassDiagramView::default()
    }

    /// A new diagram is being shown in the base tab: drop stale expansion
    /// (keyed by node key, which may not exist in the new diagram).
    pub fn set_diagram(&mut self) {
        self.expanded.clear();
    }

    /// Feed the inspector's element-picker the current diagram's contents.
    fn sync_inspector_elements(
        &self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &Model,
        diagram_key: &str,
        diagram_title: &str,
        node_keys: &[String],
    ) {
        let rows = diagram_elements(model, diagram_key, diagram_title, node_keys);
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_diagram_elements(cx, model, rows);
        }
    }
}

impl DocView for ClassDiagramView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        // The active Diagram tab's key/title come from the shell; the shell
        // passes them by setting them on the view before sync. We resolve the
        // base diagram off the model by matching the tab the shell activated.
        // (The shell sets `self.active_key`/`self.active_title` — see Step 2.)
        let built = model
            .diagrams
            .iter()
            .find(|d| d.key == self.active_key)
            .map(|d| build_scene(model, d, &self.expanded));
        if let Some((scene, diags)) = built {
            for d in &diags {
                log!("diagnostic: {d:?}");
            }
            let node_keys: Vec<String> = scene.nodes.iter().map(|n| n.key.clone()).collect();
            if let Some(mut canvas) = body
                .canvas(cx)
                .borrow_mut::<crate::canvas::GraphCanvas>()
            {
                canvas.set_scene(cx, scene);
            }
            self.sync_inspector_elements(
                cx,
                body,
                model,
                &self.active_key,
                &self.active_title,
                &node_keys,
            );
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::None);
        }
        if let Some(mut toolbar) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        {
            toolbar.set_selection(cx, None);
        }
    }
    // handle + on_popup_result: Steps 3-4.
    fn handle(&mut self, cx: &mut Cx, body: &BodyWidgets, actions: &Actions, model: &Model) -> ViewOutcome {
        let _ = (cx, body, actions, model);
        ViewOutcome::default()
    }
    fn wants_tooldock(&self) -> bool {
        true
    }
}
```

Add the two fields the shell sets before `sync` to `ClassDiagramView` (the diagram base tab's identity):

```rust
#[derive(Default)]
pub struct ClassDiagramView {
    active_key: String,
    active_title: String,
    expanded: HashSet<String>,
}
```

and a shell-facing setter:

```rust
impl ClassDiagramView {
    pub fn set_active(&mut self, key: String, title: String) {
        self.active_key = key;
        self.active_title = title;
    }
}
```

- [ ] **Step 2: Shell delegates the Diagram arm of `sync_active_tab`**

In `app.rs`, remove the `expanded` field from `struct App`, and route the Diagram arm through the view held in the registry. Replace the `TabKind::Diagram => { .. }` arm body with:

```rust
TabKind::Diagram => {
    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    let view = self
        .views
        .entry(active.id)
        .or_insert_with(|| crate::doc_view::make_view(&active));
    if let Some(v) = view.downcast_diagram() {
        v.set_active(active.key.clone(), active.title.clone());
    }
    view.sync(cx, &body, &self.model);
    body.set_tool_dock_visible(cx, view.wants_tooldock());
}
```

`downcast_diagram` is a small helper on `dyn DocView` — add to `doc_view.rs`:

```rust
impl dyn DocView {
    /// Downcast helper so the shell can push the active diagram key/title before
    /// sync. Returns `None` for a preview view.
    pub fn downcast_diagram(&mut self) -> Option<&mut crate::class_diagram_view::ClassDiagramView> {
        // Enabled by adding `as_any_mut` to the trait (Step 2a).
        self.as_any_mut()
            .downcast_mut::<crate::class_diagram_view::ClassDiagramView>()
    }
}
```

**Step 2a:** add `fn as_any_mut(&mut self) -> &mut dyn std::any::Any;` to the `DocView` trait and implement it as `{ self }` on both concrete views. (This is the idiomatic Rust downcast seam; keeps `set_active` off the trait since only the diagram view has it.)

Replace every remaining `self.expanded` use in `app.rs` — `open_dir` (lines 573, 605-611), `switch_diagram` (line 453), and the `ToggleExpand` branch (moved in Step 3) — as follows: `switch_diagram`'s `self.expanded.clear()` becomes a call that clears the base tab's view expansion (the view is (re)created on the next sync, so simply drop any cached view for the base tab id: `self.views.remove(&base_id);` after `set_diagram_base`, before `sync_active_tab`). `open_dir`'s inline `build_scene(&self.model, diagram, &self.expanded)` uses a fresh empty set — replace `&self.expanded` with `&std::collections::HashSet::new()` (open_dir always starts a fresh model with cleared expansion, so an empty set is exactly equivalent to today's just-cleared `self.expanded`).

- [ ] **Step 3: Write `ClassDiagramView::handle`**

Replace the stub `handle`. Move the diagram-scoped branches from `app.rs::handle_actions` verbatim, re-homing to `body` and converting shell side-effects into `ViewOutcome`:

```rust
fn handle(
    &mut self,
    cx: &mut Cx,
    body: &BodyWidgets,
    actions: &Actions,
    model: &Model,
) -> ViewOutcome {
    let mut out = ViewOutcome::default();

    // Inline-edit commit: inspector emits `Edited(subject_key)`.
    if let Some(key) = body
        .inspector(cx)
        .borrow_mut::<crate::inspector_panel::Inspector>()
        .and_then(|inspector| inspector.edited(actions))
    {
        out.promote_subject = Some(key);
        return out;
    }

    // Element-picker: the SelectBox asked to open its flyout.
    if let Some((anchor_rect, min_width, items)) = body
        .inspector(cx)
        .borrow_mut::<crate::inspector_panel::Inspector>()
        .and_then(|inspector| inspector.take_open_request(cx, actions))
    {
        out.popup = Some(crate::doc_view::PopupRequest::ElementPicker {
            anchor_rect,
            min_width,
            items,
        });
        return out;
    }

    // Tool dock: mode clicks update their own highlight; ModeChanged re-snaps
    // the statusbar. Other actions stay mock `log!` no-ops.
    if let Some(action) = body
        .tool_dock(cx)
        .borrow_mut::<crate::tool_dock::ToolDock>()
        .and_then(|dock| dock.dock_action(actions))
    {
        match action {
            crate::tool_dock::ToolDockAction::ModeChanged(_) => out.statusbar_dirty = true,
            other => log!("tool dock: {other:?}"),
        }
        return out;
    }

    // Canvas pointer actions.
    let canvas_menu = body
        .canvas(cx)
        .borrow_mut::<crate::canvas::GraphCanvas>()
        .and_then(|c| c.canvas_action(actions));
    match canvas_menu {
        Some(crate::canvas::GraphCanvasAction::NodeMenu { abs, node: _ }) => {
            out.popup = Some(crate::doc_view::PopupRequest::NodeRadial { center: abs });
            return out;
        }
        Some(crate::canvas::GraphCanvasAction::NodeSelect { key }) => {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, model, Subject::Classifier(key));
            }
            return out;
        }
        Some(crate::canvas::GraphCanvasAction::NodeDeselect) => {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, model, Subject::None);
            }
            return out;
        }
        Some(crate::canvas::GraphCanvasAction::ToggleExpand { key }) => {
            if !self.expanded.remove(&key) {
                self.expanded.insert(key);
            }
            if let Some(diagram) = model.diagrams.iter().find(|d| d.key == self.active_key) {
                let (scene, diags) = build_scene(model, diagram, &self.expanded);
                for d in &diags {
                    log!("diagnostic: {d:?}");
                }
                if let Some(mut canvas) = body
                    .canvas(cx)
                    .borrow_mut::<crate::canvas::GraphCanvas>()
                {
                    canvas.update_scene(cx, scene);
                }
            }
            return out;
        }
        _ => {}
    }

    // Selection toolbar: Delete only acts on a classifier preview (no-op here);
    // New Diagram is a mock no-op.
    if let Some(action) = body
        .selection_toolbar(cx)
        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        .and_then(|toolbar| toolbar.toolbar_action(actions))
    {
        match action {
            crate::selection_toolbar::SelectionToolbarAction::Delete => {}
            crate::selection_toolbar::SelectionToolbarAction::NewDiagram => {
                log!("selection toolbar: New Diagram (mock no-op)");
            }
            _ => {}
        }
        return out;
    }

    out
}
```

- [ ] **Step 4: Write `ClassDiagramView::on_popup_result`**

```rust
fn on_popup_result(
    &mut self,
    cx: &mut Cx,
    body: &BodyWidgets,
    model: &Model,
    tag: LiveId,
    result: PopupResult,
) -> ViewOutcome {
    // Element-picker: any close clears the box's active state; a node commit
    // repoints the inspector (inspector-local — no tab, no canvas move).
    if tag == live_id!(element_picker) {
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.on_picker_closed(cx, model, result);
        }
    }
    // node_menu currently only `log!`s on commit — kept in the shell for now.
    ViewOutcome::default()
}
```

Add `use crate::popup::base::PopupResult;` to `class_diagram_view.rs`.

- [ ] **Step 5: Shell delegates the diagram branches of `handle_actions` and relays the outcome**

In `app.rs::handle_actions`, replace the moved branches (inspector `edited`, element-picker open, tool-dock action, canvas actions, selection-toolbar action — the blocks now living in the view) with a single delegation to the **active** view, placed at the point in `handle_actions` where those branches used to run (after the tree/switcher/chrome branches, before the shared doc-tab strip handling). Because Task 3 only migrates the Diagram case, guard on kind so the Classifier path stays on its existing code until Task 4:

```rust
if let Some(active) = self.tabs.active_tab().cloned() {
    if active.kind == TabKind::Diagram {
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        let view = self
            .views
            .entry(active.id)
            .or_insert_with(|| crate::doc_view::make_view(&active));
        let outcome = view.handle(cx, &body, actions, &self.model);
        if self.relay_outcome(cx, &active, outcome) {
            return;
        }
    }
}
```

Add the relay to `impl App` (this is the shell's single outcome-relay choke point — spec §3 rule 2):

```rust
/// Apply a view's `ViewOutcome`: place popups, relay tab-lifecycle intents,
/// re-snap the statusbar. Returns `true` if the shell consumed the event
/// (mirrors the old `return`-after-handling flow).
fn relay_outcome(
    &mut self,
    cx: &mut Cx,
    active: &crate::doc_tabs::DocTab,
    outcome: crate::doc_view::ViewOutcome,
) -> bool {
    let mut consumed = false;

    // ops: forward-looking, empty this migration. Applied here when it lands.
    for _op in &outcome.ops {
        // No shell Op application exists yet (out of scope, spec §2).
    }

    if let Some(req) = outcome.popup {
        let bounds = self.window_bounds(cx);
        if let Some(mut pr) = self.ui.widget(cx, ids!(popup_root)).borrow_mut::<PopupRoot>() {
            match req {
                crate::doc_view::PopupRequest::NodeRadial { center } => {
                    pr.show_at(
                        cx,
                        PopupSpec::Radial {
                            tag: live_id!(node_menu),
                            center,
                            bounds,
                            items: node_radial_items(),
                            open: RadialOpen::Marking,
                        },
                    );
                }
                crate::doc_view::PopupRequest::ElementPicker {
                    anchor_rect,
                    min_width,
                    items,
                } => {
                    let anchor = dvec2(
                        anchor_rect.pos.x,
                        anchor_rect.pos.y + anchor_rect.size.y + crate::popup::select::SELECT_GAP,
                    );
                    pr.show_at(
                        cx,
                        PopupSpec::Select {
                            tag: live_id!(element_picker),
                            anchor,
                            min_width,
                            bounds,
                            items,
                        },
                    );
                }
            }
        }
        consumed = true;
    }

    if let Some(key) = outcome.promote_subject {
        if let Some(tab) = self.tabs.tabs.iter().find(|t| t.key == key) {
            let id = tab.id;
            self.tabs.promote(id);
            self.refresh_doc_tabs(cx);
        }
        consumed = true;
    }

    if outcome.close_active {
        let id = active.id;
        self.tabs.close(id);
        self.views.remove(&id);
        self.refresh_doc_tabs(cx);
        self.sync_active_tab(cx);
        consumed = true;
    }

    if let Some(key) = outcome.open_preview {
        // Unused this migration (tree drives previews). Placeholder relay for
        // the forward-looking channel; resolves title/kind off the model.
        if let Some(node) = self.model.nodes.iter().find(|n| n.key == key) {
            let title = node.concept.title.clone().unwrap_or_else(|| node.key.clone());
            self.tabs.open_preview(key, title, crate::tree::kind_of(&node.ty));
            self.refresh_doc_tabs(cx);
            self.sync_active_tab(cx);
        }
        consumed = true;
    }

    if outcome.statusbar_dirty {
        self.sync_statusbar(cx);
    }

    consumed
}
```

Also route document-scoped popup results into the active view. In the popup-outcomes block (lines ~1141-1149 for `picker_closed`), replace the direct `inspector.on_picker_closed` call with delegation:

```rust
if let Some(result) = picker_closed {
    if let Some(active) = self.tabs.active_tab().cloned() {
        let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
        if let Some(view) = self.views.get_mut(&active.id) {
            let outcome =
                view.on_popup_result(cx, &body, &self.model, live_id!(element_picker), result);
            self.relay_outcome(cx, &active, outcome);
        }
    }
}
```

(Note: `relay_outcome` needs `&self.model` read plus `&mut self` — resolve the borrow by cloning `active` first, as shown, and by having `relay_outcome` take `active` by reference. `on_popup_result` returns `ViewOutcome::default()` for the picker today, so this relay is a no-op tail but keeps the seam correct.)

- [ ] **Step 6: Build + test**

Run: `cargo test -p waml-editor`
Expected: PASS at the same count as Task 2 (no new unit tests — the migrated logic is imperative UI with no new pure surface; the `OpenTabs`/`build_scene` tests already cover the state math).

Run: `cargo build -p waml-editor --bin waml-editor`
Expected: clean, no `dead_code` / unused-field warnings (`expanded` now lives in the view and is read by `sync`/`handle`).

- [ ] **Step 7: Optimized build + screenshot parity (diagram)**

Run (from the worktree root):

```powershell
$p = Start-Process -FilePath ./scripts/run-native.ps1 -ArgumentList '-Optimized' -PassThru
```

(Or launch the built exe directly with a fixture and capture `$p.Id`.) Then, in **one** PowerShell call, screenshot **that pid** and inspect: the class diagram must render identically — canvas nodes, inspector-with-picker on the right, tool dock on the left, selection toolbar at the bottom. Verify node select (inspector repoints), a right-press node radial (command wheel), the element-picker flyout opening, and a tool-dock mode change (statusbar label updates). Do NOT capture or `Stop-Process` `waml-editor` by name (that hits the user's editor — `MEMORY.md`). Close by `$p.Id`.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/app.rs
git commit -m "feat(view-seam): move Diagram behavior into ClassDiagramView; shell relays ViewOutcome"
```

**Acceptance:** Compiles; full suite green; `-Optimized` screenshot of the class diagram + node select + node radial + element picker + tool-dock mode change is pixel-identical to pre-task; `expanded` lives in the view (per-tab).

---

### Task 4: Move the Classifier arm into `ClassifierPreviewView`

**Files:**
- Modify: `crates/waml-editor/src/classifier_preview_view.rs` (real `sync`, `handle`)
- Modify: `crates/waml-editor/src/app.rs` (Classifier arm of `sync_active_tab` delegates; the Classifier `handle_actions` guard now delegates; the Classifier-specific `Delete` path relays `close_active`)

**Why:** Spec §6 step 4. Port `build_focus_scene` and the classifier-focus branches; the shell delegates the Classifier case and relays its `ViewOutcome`.

**Interfaces:**
- Consumes: `BodyWidgets`, `DocView`, `ViewOutcome` (Task 2); `crate::scene::build_focus_scene`, `crate::inspector::Subject`, `crate::canvas::{GraphCanvas, GraphCanvasAction}`, `crate::inspector_panel::Inspector`, `crate::selection_toolbar::{SelectionToolbar, SelectionToolbarAction}`.
- Produces: `ClassifierPreviewView` implementing `DocView` with real bodies; holds `key: String` (the previewed classifier/package).

**Behavior mapping (verbatim moves from `app.rs`):**

| Shell source (app.rs) | Moves to |
|---|---|
| `sync_active_tab` Classifier arm (lines 391-419): `build_focus_scene` + `canvas.set_focus` + `inspector.set_subject(Classifier)` + `inspector.set_picker_visible(false)` + `toolbar.set_selection(Some(1))` | `ClassifierPreviewView::sync` |
| inspector `edited` | `handle` → `ViewOutcome.promote_subject = Some(key)` |
| selection-toolbar `Delete` on a classifier (lines 1585-1595) | `handle` → `ViewOutcome.close_active = true` |
| canvas `NodeSelect`/`NodeDeselect` (repoint inspector) | `handle` → repoint the inspector directly on `body` |

- [ ] **Step 1: Write `ClassifierPreviewView::sync`**

Replace the stub in `classifier_preview_view.rs`:

```rust
use crate::inspector::Subject;
use crate::scene::build_focus_scene;

impl DocView for ClassifierPreviewView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        let scene = build_focus_scene(model, &self.key);
        if let Some(mut canvas) = body
            .canvas(cx)
            .borrow_mut::<crate::canvas::GraphCanvas>()
        {
            canvas.set_focus(cx, scene);
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::Classifier(self.key.clone()));
            // Previewing a classifier/package (not a diagram): no picker.
            inspector.set_picker_visible(cx, false);
        }
        if let Some(mut toolbar) = body
            .selection_toolbar(cx)
            .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        {
            // Single-classifier focus only in this mock -- always 1.
            toolbar.set_selection(cx, Some(1));
        }
    }
    // handle: Step 2.
    fn handle(&mut self, cx: &mut Cx, body: &BodyWidgets, actions: &Actions, model: &Model) -> ViewOutcome {
        let _ = (cx, body, actions, model);
        ViewOutcome::default()
    }
    fn wants_tooldock(&self) -> bool {
        false
    }
}
```

- [ ] **Step 2: Write `ClassifierPreviewView::handle`**

```rust
fn handle(
    &mut self,
    cx: &mut Cx,
    body: &BodyWidgets,
    actions: &Actions,
    model: &Model,
) -> ViewOutcome {
    let mut out = ViewOutcome::default();

    // Inline-edit commit: promote (pin) this preview tab.
    if let Some(key) = body
        .inspector(cx)
        .borrow_mut::<crate::inspector_panel::Inspector>()
        .and_then(|inspector| inspector.edited(actions))
    {
        out.promote_subject = Some(key);
        return out;
    }

    // Canvas select/deselect repoints the inspector (inspector-local).
    let canvas_action = body
        .canvas(cx)
        .borrow_mut::<crate::canvas::GraphCanvas>()
        .and_then(|c| c.canvas_action(actions));
    match canvas_action {
        Some(crate::canvas::GraphCanvasAction::NodeSelect { key }) => {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, model, Subject::Classifier(key));
            }
            return out;
        }
        Some(crate::canvas::GraphCanvasAction::NodeDeselect) => {
            if let Some(mut inspector) = body
                .inspector(cx)
                .borrow_mut::<crate::inspector_panel::Inspector>()
            {
                inspector.set_subject(cx, model, Subject::None);
            }
            return out;
        }
        _ => {}
    }

    // Selection toolbar: Delete closes this preview tab (in-memory only).
    if let Some(action) = body
        .selection_toolbar(cx)
        .borrow_mut::<crate::selection_toolbar::SelectionToolbar>()
        .and_then(|toolbar| toolbar.toolbar_action(actions))
    {
        match action {
            crate::selection_toolbar::SelectionToolbarAction::Delete => {
                out.close_active = true;
                return out;
            }
            crate::selection_toolbar::SelectionToolbarAction::NewDiagram => {
                log!("selection toolbar: New Diagram (mock no-op)");
                return out;
            }
            _ => {}
        }
    }

    out
}
```

- [ ] **Step 3: Shell delegates the Classifier arm of `sync_active_tab`**

Replace the `TabKind::Classifier => { .. }` arm in `app.rs::sync_active_tab` with:

```rust
TabKind::Classifier => {
    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    let view = self
        .views
        .entry(active.id)
        .or_insert_with(|| crate::doc_view::make_view(&active));
    view.sync(cx, &body, &self.model);
    body.set_tool_dock_visible(cx, view.wants_tooldock());
}
```

- [ ] **Step 4: Shell delegates the Classifier branch of `handle_actions`**

Widen the Task-3 delegation guard to cover both kinds (delete the `if active.kind == TabKind::Diagram` restriction — both cases now delegate through the same choke point):

```rust
if let Some(active) = self.tabs.active_tab().cloned() {
    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    let view = self
        .views
        .entry(active.id)
        .or_insert_with(|| crate::doc_view::make_view(&active));
    let outcome = view.handle(cx, &body, actions, &self.model);
    if self.relay_outcome(cx, &active, outcome) {
        return;
    }
}
```

Delete the old shell-side classifier `Delete` block (lines 1584-1601) if it still remains — its behavior now flows through `close_active` in `relay_outcome`.

- [ ] **Step 5: Build + test**

Run: `cargo test -p waml-editor`
Expected: PASS at the same count.

Run: `cargo build -p waml-editor --bin waml-editor`
Expected: clean, no warnings.

- [ ] **Step 6: Optimized build + screenshot parity (preview + Delete)**

Launch the worktree's own `-Optimized` build (capture pid as in Task 3 Step 7). Verify: single-click a class tree row opens a preview tab, the focus canvas renders the 1.5x node, the inspector points at that classifier with **no** element picker, the tool dock is **hidden**, the selection toolbar shows a selection of 1. Click Delete — the preview tab closes and the shell re-syncs to the base diagram tab (tool dock reappears). Capture by explicit pid only; close by pid.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/app.rs
git commit -m "feat(view-seam): move Classifier preview behavior into ClassifierPreviewView"
```

**Acceptance:** Compiles; full suite green; `-Optimized` screenshot of an opened element preview + Delete is pixel-identical; tool dock hidden on preview, picker hidden, selection=1.

---

### Task 5: Shell drives the registry; delete the dead monolith branches

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (`sync_active_tab` becomes registry-driven; view lifecycle on tab open/close; remove the Task-2 throwaway `make_view` line; delete now-dead helpers/branches; verify per-tab state persistence)

**Why:** Spec §6 step 5. Replace the `TabKind` `match` with "look up the active tab's view, delegate, relay outcome," and delete the now-dead monolith branches. After this the shell holds only chrome behavior; both document behaviors live in their views.

**Interfaces:**
- Consumes: everything from Tasks 1-4.
- Produces: `sync_active_tab` with no `TabKind` `match`; a `prune_views` reconcile helper: `fn reconcile_views(&mut self)` drops registry entries whose tab id is no longer open.

- [ ] **Step 1: Collapse `sync_active_tab` to registry lookup**

Replace the `match active.kind { .. }` in `sync_active_tab` (both arms now identical modulo the view) with a single delegation, and delete the Task-2 throwaway `let _ = crate::doc_view::make_view(&active);` line:

```rust
fn sync_active_tab(&mut self, cx: &mut Cx) {
    self.reconcile_views();
    let Some(active) = self.tabs.active_tab().cloned() else {
        if let Some(mut panel) = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_selected_key(cx, None);
        }
        return;
    };
    // Mirror the active tab onto the tree row highlight (single choke point).
    if let Some(mut panel) = self
        .ui
        .widget(cx, ids!(project_tree))
        .borrow_mut::<crate::tree_panel::ProjectTree>()
    {
        panel.set_selected_key(cx, Some(active.key.clone()));
    }

    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    let view = self
        .views
        .entry(active.id)
        .or_insert_with(|| crate::doc_view::make_view(&active));
    if let Some(v) = view.downcast_diagram() {
        v.set_active(active.key.clone(), active.title.clone());
    }
    view.sync(cx, &body, &self.model);
    body.set_tool_dock_visible(cx, view.wants_tooldock());

    self.sync_statusbar(cx);
}
```

- [ ] **Step 2: Add `reconcile_views` and prune on close**

```rust
/// Drop view objects for tabs that are no longer open. Keeps per-tab live
/// state (a diagram's `expanded`, a preview's key) alive across tab switches
/// but reclaims it when the tab closes.
fn reconcile_views(&mut self) {
    let open: std::collections::HashSet<LiveId> =
        self.tabs.tabs.iter().map(|t| t.id).collect();
    self.views.retain(|id, _| open.contains(id));
}
```

`reconcile_views` runs at the top of `sync_active_tab` (Step 1) — every activation source (tab click, tree click, switcher, keys, Delete relay) funnels through it, so a closed tab's view is dropped on the next sync. The `relay_outcome` `close_active` path already calls `self.views.remove(&id)` explicitly for immediacy; `reconcile_views` is the belt-and-suspenders sweep.

- [ ] **Step 3: Delete the now-dead shell helpers**

Delete from `app.rs`:
- `App::sync_inspector_elements` (lines 485-500) — moved into `ClassDiagramView`.
- Any residual `self.expanded` references (the field was removed in Task 3; confirm none remain — `switch_diagram` now does `self.views.remove(&base_id)`).
- The old inspector `edited` / element-picker-open / tool-dock / canvas-action / selection-toolbar blocks in `handle_actions` (lines 1347-1408, 1414-1488, 1579-1601) — moved into the views. **Keep** the tree/switcher/logo/burger/start-screen/shortcuts/doc-tab-strip branches: those are shell chrome and stay.

Verify `set_diagram_toolbars` is now unused (the views drive visibility via `body.set_tool_dock_visible` + `wants_tooldock`) and delete it; its remaining caller in `open_dir` (line 679) becomes an inline `crate::doc_view::BodyWidgets::new(cx, &self.ui).set_tool_dock_visible(cx, has_diagram);`. Confirm `open_dir` still seeds the view registry for the freshly-opened base tab — after `self.tabs = OpenTabs::diagram_base(..)` / `refresh_doc_tabs`, call `self.sync_active_tab(cx)` OR keep `open_dir`'s inline scene build but ensure the base view is created on the next activation (it is, via `entry().or_insert_with`). Simplest: leave `open_dir`'s inline build as-is (it is a fast-path that bypasses `sync_active_tab`); the first `sync_active_tab` after any interaction creates the view lazily. No behavior change.

- [ ] **Step 4: Build + test**

Run: `cargo test -p waml-editor`
Expected: PASS at the same count (no new tests; the deletion is covered by the existing `OpenTabs` suite + parity).

Run: `cargo build -p waml-editor --bin waml-editor`
Expected: clean, zero warnings — no dead code left (the `match TabKind` is gone; `set_diagram_toolbars`/`sync_inspector_elements` deleted).

- [ ] **Step 5: Optimized build + per-tab state persistence check**

Launch the worktree's own `-Optimized` build (capture pid). Verify the full seam:
1. Class diagram, preview open/close, node radial, element picker, tool-dock mode, statusbar — all pixel-identical to pre-refactor (re-run Task 3/4 checks).
2. **Per-tab state persistence (spec §7):** on the base diagram tab, pan the canvas and expand a node (`ToggleExpand`); switch to a classifier preview tab; switch back to the diagram tab — the pan **and** the expansion must be intact (camera lives in the canvas widget; `expanded` now lives in `ClassDiagramView`, held across the switch because the view object persists in the registry). Screenshot by explicit pid; close by pid. Never touch `waml-editor` by name.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/app.rs
git commit -m "refactor(view-seam): shell drives the DocView registry; delete dead TabKind branches"
```

**Acceptance:** Compiles; full suite green; the shell's `sync_active_tab` / `handle_actions` hold only chrome + delegation (no `TabKind` document `match`); `-Optimized` parity holds for diagram, preview, and tab-switching; per-tab camera + expansion survive a tab switch (state now in the view objects).

---

## Self-Review

**Spec coverage:**
- §2 in-scope `DocView` trait → Task 2. Two concrete views → Tasks 3, 4. View registry (`HashMap<TabId, Box<dyn DocView>>`) → Task 2 field, Task 5 driven. `make_view` factory on `TabKind` → Task 2. `ViewOutcome` (ops / open_preview / popup) → Task 2. Shell relays outcomes (ops to Model, previews via OpenTabs, popups via popup_root) → `relay_outcome`, Task 3. ✓
- §2 out-of-scope: no `Op::PlaceSet`, no Sequence/Activity/OKF, no dynamic subtree instancing, no visual change — honored (Global Constraints; `ops` channel drains empty). ✓
- §3 rules 1-3 (views read `&Model`, emit `ViewOutcome`, never touch Model/OpenTabs/popup_root): enforced by trait signature + `relay_outcome` as sole applier + `on_popup_result` keeping popup_root in the shell. ✓
- §4 object-per-tab, one shared surface: `BodyWidgets` is the single surface; views are plain Rust objects in the registry; no PortalList/TabBar. ✓ (Statusbar stays shell chrome — §6's port list omits it, unlike §4's looser note; documented in Task 3 as `statusbar_dirty` relay. This is the conservative, behavior-preserving reading.)
- §5 contract sketch: implemented with documented deviations (`&mut Cx` not `Cx2d`; `&BodyWidgets` params; extra `ViewOutcome` fields; `on_popup_result`). ✓
- §6 migration path steps 1-5 → Tasks 1-5, one-to-one, each behavior-preserving and independently landable. ✓
- §7 testing: existing suite green each task; `-Optimized` screenshot parity for diagram + preview + tab-switch by explicit pid; per-tab state persistence check → Task 5 Step 5. ✓
- §8 risks: borrow tangle (views never hold `&mut Model`; shell applies after `handle` returns) — `relay_outcome` design; popup routing (only the request origin moves; `popup_root.route` untouched in the shell) — Task 3 relay + `handle_event` `pr.route` left as-is; hidden shared state (`expanded` surfaced then moved; `nav_state` stays shell) — Task 1 bundle + Task 3 field move. ✓

**Placeholder scan:** No "TBD"/"handle edge cases"/"similar to Task N". The one forward-looking empty loop (`for _op in &outcome.ops`) is explicitly explained as the out-of-scope `Op`-application channel (spec §2). `open_preview` relay is fully coded though unused this migration. ✓

**Type consistency:** `ViewOutcome` fields (`ops`, `open_preview`, `popup`, `promote_subject`, `close_active`, `statusbar_dirty`) referenced identically across Tasks 2-5. `PopupRequest::{NodeRadial, ElementPicker}` consistent. `BodyWidgets` accessors (`canvas`/`inspector`/`tool_dock`/`selection_toolbar`/`set_tool_dock_visible`) consistent Tasks 1-4. `DocView` methods (`sync`/`handle`/`on_popup_result`/`wants_tooldock`/`as_any_mut`/`on_activate`/`on_deactivate`) consistent. `ClassDiagramView::set_active`/`set_diagram`, `ClassifierPreviewView::new(key)` consistent. Real symbols verified against source: `build_scene`/`build_focus_scene` (`scene.rs`), `Subject::{None,Classifier}` + `diagram_elements` (`inspector.rs`), `GraphCanvasAction::{NodeMenu,NodeSelect,NodeDeselect,ToggleExpand}` + `canvas.set_scene`/`set_focus`/`update_scene` (`canvas.rs`), `Inspector::{set_subject,set_picker_visible,set_diagram_elements,on_picker_closed,edited,take_open_request}` (`inspector_panel.rs`), `ToolDockAction::ModeChanged` (`tool_dock.rs`), `SelectionToolbarAction::{Delete,NewDiagram}` (`selection_toolbar.rs`), `PopupSpec::{Radial,Select}` + `RadialOpen::Marking` + `SELECT_GAP` (`popup`), `OpenTabs::{promote,close,open_preview,active_tab}` + `DocTab` (`doc_tabs.rs`), `Op` (`waml::ops`). ✓
