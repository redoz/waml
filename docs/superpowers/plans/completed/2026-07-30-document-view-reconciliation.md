# Document View Reconciliation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve a compatible live document view across model revisions and use a complete lifecycle when reconciliation installs an incompatible view.

**Architecture:** `DocView` reports a small value that identifies its surface and fixed configuration. `DocumentHost` keeps an existing view when its tab ID and view identity match the prepared document. It replaces an incompatible view and distinguishes retained-active refresh from replaced-active activation and full synchronization.

**Tech Stack:** Rust 2024, Makepad widgets, existing `waml-editor` inline test harness, Cargo.

## Global Constraints

- Apply one reconciliation rule to all `DocView` implementations.
- Keep temporary per-tab state when the tab ID and view identity are unchanged.
- Update title and presentation from the prepared document.
- Call `after_session_change` only for a retained active view.
- For an active replacement, call old `on_deactivate`, new `on_activate`, and new `sync`.
- Preserve tab order, active selection, and preview or persistent state.
- Do not add automatic tab closure.
- Use test-driven development.
- Do not add dependencies.

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/waml-editor/src/doc_view.rs` | Define the uniform view-identity contract. |
| `crates/waml-editor/src/class_diagram_view.rs` | Identify class-diagram views. |
| `crates/waml-editor/src/behavior_doc_view.rs` | Distinguish flow and interaction views. |
| `crates/waml-editor/src/classifier_preview_view.rs` | Include the classifier category in view identity. |
| `crates/waml-editor/src/generic_okf_view.rs` | Identify generic OKF Markdown views. |
| `crates/waml-editor/src/source_view.rs` | Identify source Markdown views. |
| `crates/waml-editor/src/document_host.rs` | Retain compatible views and run the correct replacement lifecycle. |
| `crates/waml-editor/src/app.rs` | Add the two-edit application regression test and update test views. |

---

### Task 1: Preserve Compatible Document Views

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs:281-434`
- Modify: `crates/waml-editor/src/class_diagram_view.rs:317-877`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs:337-616`
- Modify: `crates/waml-editor/src/classifier_preview_view.rs:13-150`
- Modify: `crates/waml-editor/src/generic_okf_view.rs:10-91`
- Modify: `crates/waml-editor/src/source_view.rs:17-113`
- Modify: `crates/waml-editor/src/document_host.rs:234-333`
- Test: `crates/waml-editor/src/document_host.rs:384-620`
- Test: `crates/waml-editor/src/app.rs:2800-2990` and the application test section

**Interfaces:**
- Consumes: Existing `DocView::{sync, after_session_change, on_activate, on_deactivate}` lifecycle.
- Produces: `DocViewIdentity` and `DocView::identity() -> DocViewIdentity`.
- Produces: A reconciliation result that distinguishes a retained active view from a replaced active view and owns the removed active view until deactivation.

- [ ] **Step 1: Add the failing application regression test**

Add a small test helper in `app.rs` that creates an `App` with:

- one valid diagram source:

```rust
let source = waml::source::SourceBundle::try_from_pairs([(
    "orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\ndescription: Initial\n---\n# Orders\n",
)])
.unwrap();
```

- a mounted `ToolDock` at `ids!(tool_dock)`;
- a mounted `DiagramProperties` at `ids!(diagram_properties)`;
- a `View` wrapper at `ids!(diagram_properties_wrap)`;
- the diagram opened through `crate::documents::open` and `DocumentHost::transition`.

Use the existing `widget_action` helper. Open properties mode with:

```rust
widget_action(
    tool_dock_uid,
    crate::tool_dock::ToolDockAction::Triggered(
        crate::tool_dock::Tool::DiagramProps,
    ),
)
```

Then send two separate action batches:

```rust
crate::diagram_properties::DiagramPropertiesAction::DescriptionChanged(
    Some("First edit".into()),
)
```

and:

```rust
crate::diagram_properties::DiagramPropertiesAction::DescriptionChanged(
    Some("Second edit".into()),
)
```

Name the test:

```rust
#[test]
fn consecutive_diagram_property_edits_keep_the_live_properties_view()
```

Assert:

```rust
assert_eq!(app.session.revision(), initial_revision + 2);
assert_eq!(
    app.session
        .uml_analysis()
        .projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "orders")
        .and_then(|diagram| diagram.description.as_deref()),
    Some("Second edit"),
);
let text = app.session.source().documents()[0].text();
assert!(text.contains("description: Second edit"), "{text}");
assert!(
    !app.documents.active_chrome().tool_dock,
    "properties mode remains active after both edits",
);
```

- [ ] **Step 2: Run the regression test and verify the current failure**

Run:

```powershell
rtk cargo test -p waml-editor consecutive_diagram_property_edits_keep_the_live_properties_view
```

Expected: FAIL after the first edit. The session revision does not increase twice, or the second description does not reach the model and source.

- [ ] **Step 3: Add the uniform view-identity contract**

In `doc_view.rs`, add:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocViewIdentity {
    ClassDiagram,
    BehaviorFlow,
    BehaviorInteraction,
    ClassifierPreview(crate::document::NavCategory),
    GenericOkf,
    Source,
}
```

Add this required method to `DocView`:

```rust
fn identity(&self) -> DocViewIdentity;
```

Implement the exact mapping:

```rust
// ClassDiagramView
DocViewIdentity::ClassDiagram

// BehaviorDocView
match self.kind {
    BehaviorKind::Flow => DocViewIdentity::BehaviorFlow,
    BehaviorKind::Interaction => DocViewIdentity::BehaviorInteraction,
}

// ClassifierPreviewView
DocViewIdentity::ClassifierPreview(self.category)

// GenericOkfView
DocViewIdentity::GenericOkf

// SourceView
DocViewIdentity::Source
```

Update all test `DocView` implementations in `app.rs` and `document_host.rs`. Test-only views can return the closest production identity that matches their role.

- [ ] **Step 4: Add failing host reconciliation and lifecycle tests**

Extend `ProbeView` in `document_host.rs` with:

- a `DocViewIdentity`;
- separate counters for `sync`;
- `after_session_change`;
- `on_activate`;
- `on_deactivate`;
- existing `chrome` calls.

Add or revise tests to prove:

```rust
#[test]
fn compatible_prepared_document_keeps_the_live_view()
```

- The prepared tab ID and identity match.
- The title and presentation change.
- The old probe receives `after_session_change`.
- The prepared probe receives no calls.
- The old probe state remains available.

```rust
#[test]
fn incompatible_active_replacement_runs_full_lifecycle()
```

- The tab ID matches, but the identity differs.
- The old probe receives one `on_deactivate`.
- The new probe receives one `on_activate` and one `sync`.
- The new probe does not receive `after_session_change`.

Keep the provider-change and preview-state assertions in the current reconciliation tests.

- [ ] **Step 5: Implement compatible retention and replacement lifecycle**

In `document_host.rs`, add a private reconciliation result:

```rust
enum ActiveReconciliation {
    Retained,
    Replaced {
        old_view: Option<Box<dyn DocView>>,
    },
}
```

Change `reconcile_documents` to return `ActiveReconciliation`.

For a prepared document, retain the live view only when both checks are true:

```rust
let compatible = prepared.tab_id == current_id
    && self.views.get(&current_id).is_some_and(|current_view| {
        current_view.identity() == prepared.view.identity()
    });
```

For a compatible document:

```rust
self.tabs.tabs[index].title = prepared.title;
self.tabs.tabs[index].presentation = prepared.presentation;
```

Do not replace `self.views[current_id]`.

For an incompatible document, keep the existing replacement logic. If it replaces the active view, return the removed view in `ActiveReconciliation::Replaced`.

Update `after_session_change`:

```rust
let reconciliation = if change.okf_changed || change.uml_changed {
    self.reconcile_documents(prepared)
} else {
    ActiveReconciliation::Retained
};
let body = BodyWidgets::new(cx, ui);
match reconciliation {
    ActiveReconciliation::Retained => {
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.after_session_change(cx, &body, data(session), change);
        }
    }
    ActiveReconciliation::Replaced { mut old_view } => {
        if let Some(old_view) = old_view.as_mut() {
            old_view.on_deactivate(cx, &body);
        }
        if let Some(view) = self.views.get_mut(&self.tabs.active) {
            view.on_activate(cx, &body);
            view.sync(cx, &body, data(session));
        }
    }
}
```

Keep `refresh_tabs` after this match.

- [ ] **Step 6: Run focused tests**

Run:

```powershell
rtk cargo test -p waml-editor consecutive_diagram_property_edits_keep_the_live_properties_view
rtk cargo test -p waml-editor document_host::tests
rtk cargo test -p waml-editor class_diagram_view::tests
```

Expected: PASS.

- [ ] **Step 7: Run the complete verification gate**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test -p waml-editor
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected:

- Formatting check passes.
- All `waml-editor` tests pass.
- Clippy reports no warnings.

- [ ] **Step 8: Review and commit**

Review:

```powershell
rtk git diff --check
rtk git status --short
```

Commit:

```powershell
rtk git add crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/behavior_doc_view.rs crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/document_host.rs crates/waml-editor/src/app.rs
rtk git commit -m "fix(editor): preserve reconciled view state"
```
