# Tree Double-Click Tab Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make classifiers and diagrams open as previews on a single tree click and become persistent on a double click, including when already open.

**Architecture:** `ProjectTree` retains Makepad's raw `tap_count` until the matching `FileTree` action and emits one document-open request containing a persistence flag. `OpenTabs` treats diagrams and classifiers uniformly through the shared preview slot, while `App` owns model lookup, stale preview-view cleanup, activation, promotion, and UI synchronization.

**Tech Stack:** Rust, Makepad widgets, existing `waml-editor` unit tests.

## Global Constraints

- The initially selected diagram is a preview.
- Classifiers and diagrams share one preview slot.
- Single click opens or focuses a preview; double click opens or focuses and persists it.
- Double-clicking an already open item is idempotent and must not duplicate or demote it.
- Diagram tab IDs are derived from diagram keys so multiple promoted diagrams can coexist.
- Folder behavior remains unchanged.
- Do not change Makepad or update the pinned Makepad dependency.
- Use test-driven development: observe each new regression test fail for the expected behavior before production edits.

---

### Task 1: Unify tree document preview and persistence behavior

**Files:**
- Modify: `crates/waml-editor/src/doc_tabs.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Reference: `docs/superpowers/specs/2026-07-26-tree-double-click-tab-persistence-design.md`

**Interfaces:**
- `OpenTabs::diagram_preview(key, title) -> OpenTabs` seeds one active preview diagram.
- `OpenTabs::open_preview(key, title, node_kind) -> LiveId` supports both classifier kinds and `TreeKind::Diagram`.
- `diagram_tab_id(key: &str) -> LiveId` provides stable per-diagram identity.
- `ProjectTreeAction::OpenDocument { key: String, node_kind: TreeKind, persistent: bool }` carries tree intent to `App`.
- `ProjectTree::open_document(actions) -> Option<(String, TreeKind, bool)>` reads that intent.
- `App` opens the requested item, removes a replaced preview's cached view, promotes when requested, and runs the existing tab/view synchronization.

- [ ] **Step 1: Add failing tab-model regression tests**

Replace permanent-base expectations and add focused tests in
`doc_tabs.rs` equivalent to:

```rust
#[test]
fn initial_diagram_is_a_preview() {
    let open = OpenTabs::diagram_preview("orders", "Orders");
    assert_eq!(open.tabs.len(), 1);
    assert_eq!(open.tabs[0].kind, TabKind::Diagram);
    assert!(open.tabs[0].preview);
}

#[test]
fn diagram_and_classifier_share_the_preview_slot() {
    let mut open = OpenTabs::diagram_preview("orders", "Orders");
    let customer = open.open_preview("customer", "Customer", TreeKind::Class);
    assert_eq!(open.tabs.len(), 1);
    assert_eq!(open.active, customer);
    assert_eq!(open.tabs[0].kind, TabKind::Classifier);
    assert!(open.tabs[0].preview);
}

#[test]
fn promoted_diagrams_have_distinct_stable_ids() {
    let mut open = OpenTabs::diagram_preview("orders", "Orders");
    let orders = open.active;
    open.promote(orders);
    let billing = open.open_preview("billing", "Billing", TreeKind::Diagram);
    open.promote(billing);

    assert_ne!(orders, billing);
    assert_eq!(open.tabs.len(), 2);
    assert!(open.tabs.iter().all(|tab| !tab.preview));

    open.open_preview("orders", "Orders", TreeKind::Diagram);
    assert_eq!(open.active, orders);
    assert_eq!(open.tabs.len(), 2);
}
```

Update existing tests that use `diagram_base` or assume a permanent first
tab so they describe the new shared-preview model.

- [ ] **Step 2: Run the tab tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor --lib doc_tabs::tests
```

Expected: compilation or assertion failures because `diagram_preview` and
per-key diagram preview behavior do not exist yet.

- [ ] **Step 3: Implement the minimal unified tab model**

In `doc_tabs.rs`:

```rust
pub fn diagram_preview(
    key: impl Into<String>,
    title: impl Into<String>,
) -> OpenTabs {
    let mut tabs = OpenTabs::default();
    tabs.open_preview(key, title, TreeKind::Diagram);
    tabs
}

pub fn diagram_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_diagram__{key}"))
}
```

Make `open_preview` choose both ID and `TabKind` from `node_kind`:

```rust
let (id, kind) = if node_kind == TreeKind::Diagram {
    (diagram_tab_id(&key), TabKind::Diagram)
} else {
    (classifier_tab_id(&key), TabKind::Classifier)
};
```

Use `kind` for the replacement and append branches. Preserve the current
"already open" early return so persisted tabs are focused without being
demoted. Remove `diagram_base`, `set_diagram_base`, and the constant diagram
ID behavior after their callers and tests are migrated.

- [ ] **Step 4: Run the tab tests and verify GREEN**

Run:

```powershell
rtk cargo test -p waml-editor --lib doc_tabs::tests
```

Expected: all `doc_tabs` tests pass.

- [ ] **Step 5: Add failing tree click-classification tests**

Extract a pure helper in `tree_panel.rs` that maps a file kind and tap count
to a project-tree action:

```rust
fn document_action(
    key: &str,
    node_kind: TreeKind,
    tap_count: u32,
) -> Option<ProjectTreeAction> {
    matches!(
        node_kind,
        TreeKind::Diagram
            | TreeKind::Class
            | TreeKind::Interface
            | TreeKind::Enum
            | TreeKind::DataType
    )
    .then(|| ProjectTreeAction::OpenDocument {
        key: key.to_owned(),
        node_kind,
        persistent: tap_count == 2,
    })
}
```

Add tests equivalent to:

```rust
#[test]
fn document_action_marks_only_second_tap_persistent() {
    for kind in [TreeKind::Diagram, TreeKind::Class] {
        assert!(matches!(
            document_action("item", kind, 1),
            Some(ProjectTreeAction::OpenDocument {
                persistent: false,
                ..
            })
        ));
        assert!(matches!(
            document_action("item", kind, 2),
            Some(ProjectTreeAction::OpenDocument {
                persistent: true,
                ..
            })
        ));
    }
}

#[test]
fn document_action_ignores_folders() {
    assert!(document_action("pkg", TreeKind::Package, 2).is_none());
}
```

- [ ] **Step 6: Run the tree tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor --lib tree_panel::tests
```

Expected: compilation failures because `OpenDocument` and
`document_action` do not exist.

- [ ] **Step 7: Implement tap-count capture and document actions**

In `ProjectTree` add:

```rust
#[rust]
pending_tap_count: u32,
```

On a primary `Hit::FingerDown(fe)` within the tree panel, retain
`fe.tap_count`. When `file_tree.file_clicked(actions)` returns an ID, consume
that retained value with `std::mem::take`, resolve its key and kind, call
`document_action`, and emit the returned action. Keep the existing folder
open/close behavior and right-click behavior unchanged.

Replace `SelectDiagram` and `FocusClassifier` with:

```rust
OpenDocument {
    key: String,
    node_kind: TreeKind,
    persistent: bool,
},
```

Add `ProjectTree::open_document` returning `(key, node_kind, persistent)`.

- [ ] **Step 8: Run the tree tests and verify GREEN**

Run:

```powershell
rtk cargo test -p waml-editor --lib tree_panel::tests
```

Expected: all `tree_panel` tests pass.

- [ ] **Step 9: Migrate application startup and open paths**

In `app.rs`, replace `OpenTabs::diagram_base` with
`OpenTabs::diagram_preview`.

Replace the separate classifier and diagram tree-action branches with one
`open_document` branch. Resolve titles from `model.diagrams` for
`TreeKind::Diagram` and from `model.nodes` for classifier kinds.

Before calling `open_preview`, record the current preview ID:

```rust
let old_preview = self
    .tabs
    .tabs
    .iter()
    .find(|tab| tab.preview)
    .map(|tab| tab.id);
let id = self.tabs.open_preview(key, title, node_kind);
if old_preview.is_some_and(|old| old != id) {
    self.views.remove(&old_preview.unwrap());
}
if persistent {
    self.tabs.promote(id);
}
```

Implement this once in a private `App` helper and reuse it from the tree and
diagram switcher paths. Avoid duplicating preview cleanup and synchronization
logic. The helper must refresh tabs, sync the active tab, and update the
diagram switcher when the active document is a diagram.

Update diagram-switcher current-key/title lookup to prefer the active diagram
tab, because multiple promoted diagram tabs may now coexist.

- [ ] **Step 10: Run all editor library tests**

Run:

```powershell
rtk cargo test -p waml-editor --lib
```

Expected: all tests pass with no warnings introduced by the changed code.

- [ ] **Step 11: Check formatting and the complete diff**

Run:

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml-editor --lib --tests -- -D warnings
rtk git diff --check
rtk git status --short
```

Expected: formatting, Clippy, and diff checks pass; only the planned files
and plan/spec documents are changed.

- [ ] **Step 12: Commit the implementation**

```powershell
rtk git add crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app.rs
rtk git commit -m "fix(editor): persist double-clicked tree tabs"
```

The implementation report must include the RED failure evidence, GREEN test
commands/results, final commit hash, and any concerns.
