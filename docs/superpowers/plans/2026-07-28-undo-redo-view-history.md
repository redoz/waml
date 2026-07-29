# Undo, Redo, and View History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add operation-based global undo/redo and independent Back/Forward logical-view history, including preview replacement, manual tab switching, and undo/redo revealing the affected editor.

**Architecture:** Extend every semantic edit batch so a successful application returns an exact reciprocal edit, then let `EditorSession` own bounded model-history stacks and savepoint identity. Keep logical navigation in a separate bounded `ViewHistory` keyed by stable document locators and view-specific anchors. `App` remains the sole orchestrator: it applies model transitions, reveals undo/redo targets, records visible transitions, and asks `DocumentHost` to capture or restore views.

**Tech Stack:** Rust, Makepad widgets, WAML semantic edit/lowering pipeline, crate unit and integration tests.

## Global Constraints

- Execute [the document-header/navigation plan](./2026-07-28-document-header-logical-navigation.md) first. This plan extends its `navigation.rs`, `DocumentHeader`, centralized `App` navigation, and Markdown fragment APIs; it must not recreate competing paths.
- Preserve the two independent histories:
  - Undo/Redo changes the open model and then reveals the affected editor.
  - Back/Forward changes only the visible logical view.
- Revealing an editor for Undo/Redo is itself a recorded view transition, so Back returns to the view the user occupied before invoking Undo/Redo.
- Never use `LiveId` as persisted history identity. Use semantic document/control locators.
- Keep edits transactional. Parse, projection, or lowering failure must leave the source, analyses, revision, dirty state, undo stack, and redo stack unchanged.
- A successful new edit after Undo clears Redo. A successful new navigation after Back clears Forward.
- Preserve at least the newest 64 model-history entries atomically. Only older compatible text/continuous entries may coalesce.
- Bound model history to 1024 entries and view history to 256 entries.
- Save must not clear model history. Dirty state is `current_state != saved_state`.
- Do not coalesce across a savepoint, focus/selection/navigation boundary, document/control change, edit-kind change, non-contiguous span, or any structural/destructive edit.
- Back/Forward restores logical view and anchor only. It must not restore pinned/preview state, tab order, or closed-tab state.
- Keep unavailable history locations in the history. Traversal skips them and can reach them later after Undo restores their targets.
- All shell commands in this repository must be run through `rtk`, per `RTK.md`.
- Do not leave incomplete implementation markers or add speculative keyboard/mouse bindings for Back/Forward.

## File Map

### `C:\dev\waml`

| File | Responsibility |
|---|---|
| `crates/waml/src/edit.rs` | Change `EditBatch` from one-way lowering to reversible application; expose `AppliedEdit` and internal edit composition. |
| `crates/waml/src/edit/reversible.rs` | Implement exact per-document source deltas, reciprocal application, and sequence composition. |
| `crates/waml/src/source.rs` | Add crate-private, index-safe primitives for text splice, insert, remove, and rename restoration. |
| `crates/waml/src/okf/ops.rs` | Journal each OKF semantic step and return its reciprocal batch. |
| `crates/waml/src/uml/ops.rs` | Journal each UML semantic step and return its reciprocal batch. |
| `crates/waml/src/compat.rs` | Journal mixed legacy OKF/UML steps without weakening atomicity. |
| `crates/waml/src/lib.rs` | Export only the public reversible-edit surface needed by the editor. |
| `crates/waml/tests/reversible_edits.rs` | Round-trip and failure tests for reversible source deltas and composed batches. |
| `crates/waml/tests/okf_ops.rs` | Exact-source forward/inverse coverage for every OKF operation. |
| `crates/waml/tests/uml_ops.rs` | Exact-source forward/inverse coverage for every UML operation. |
| `crates/waml/tests/compat_ops.rs` | Mixed legacy batch round-trip and late-failure coverage. |
| `crates/waml-editor/src/navigation.rs` | Extend the prerequisite logical-navigation types with stable document locators and transition causes. |
| `crates/waml-editor/src/view_history.rs` | Define anchors, locations, and the pure bounded Back/Forward state machine. |
| `crates/waml-editor/src/editor_history.rs` | Define model-history state IDs, steps, merge policy, bounds, and undo/redo stack transitions. |
| `crates/waml-editor/src/editor_session.rs` | Own model history, run reversible edits transactionally, and compare current/saved state IDs. |
| `crates/waml-editor/src/document.rs` | Add stable document kind/locator, anchor delegation, and document transition support. |
| `crates/waml-editor/src/doc_tabs.rs` | Store semantic document kind and locate an existing tab without relying on history `LiveId`s. |
| `crates/waml-editor/src/documents.rs` | Open primary document locators. |
| `crates/waml-editor/src/okf_documents.rs` | Distinguish primary OKF views from source views when opening locators. |
| `crates/waml-editor/src/uml_documents.rs` | Open primary UML document locators. |
| `crates/waml-editor/src/doc_view.rs` | Add default anchor capture/restore methods to `DocView`. |
| `crates/waml-editor/src/class_diagram_view.rs` | Capture/restore selected subject and camera; attach edit labels and merge metadata. |
| `crates/waml-editor/src/class_diagram_surface.rs` | Provide history-safe selection/camera snapshot and restoration methods. |
| `crates/waml-editor/src/markdown_view.rs` and source-view implementation files | Capture/restore fragment and vertical scroll anchors. |
| `crates/waml-editor/src/document_header.rs` | Add always-mounted Back/Forward buttons and enabled-state presentation. |
| `crates/waml-editor/src/statusbar.rs` | Show history action/error status with defined precedence. |
| `crates/waml-editor/src/shortcuts.rs` | Parse platform-correct Undo/Redo chords and document them in the shortcut overlay. |
| `crates/waml-editor/src/app.rs` | Centralize edit completion, savepoint capture, undo/redo reveal, and view-history traversal. |
| `crates/waml-editor/src/app/actions.rs` | Route conflict-list edits and document actions through typed intents and transition causes. |
| `crates/waml-editor/src/lib.rs` | Register the new history modules. |
| `crates/waml-editor/tests/editor_history.rs` | Model-history stack, atomic tail, coalescing, limits, failure, and savepoint tests. |
| `crates/waml-editor/tests/view_history.rs` | Pure navigation-history branching, skipping, anchors, and limit tests. |
| `crates/waml-editor/tests/history_integration.rs` | App/session/document-host scenarios spanning both histories. |

### `C:\dev\makepad`

| File | Responsibility |
|---|---|
| `widgets/src/markdown.rs` | Expose vertical scroll capture/restore through `MarkdownRef`, building on the prerequisite fragment-navigation work. |
| `widgets/tests/markdown_scroll.rs` | Verify scroll round trips and clamping without exposing internal `ScrollBars`. |

### Dependency pin

| File | Responsibility |
|---|---|
| `C:\dev\waml\Cargo.toml` or the existing Makepad dependency manifest | Pin the published Makepad revision containing both prerequisite navigation and scroll-anchor APIs. |

---

## Task 1: Introduce the Reversible Edit Protocol

**Files:**

- Create: `crates/waml/src/edit/reversible.rs`
- Modify: `crates/waml/src/edit.rs`
- Modify: `crates/waml/src/source.rs`
- Modify: `crates/waml/src/lib.rs`
- Test: `crates/waml/tests/reversible_edits.rs`

- [ ] Write a failing test proving a single-document text change returns an inverse that reconstructs the original source byte-for-byte.

```rust
let before = fixture_bundle("Customer");
let applied = edit.apply_reversible(EditContext::from(&before))?;
assert_eq!(applied.source.document(path).text(), "Customerr");

let restored = applied
    .inverse
    .apply_reversible(EditContext::from(&applied.source))?;
assert_eq!(restored.source, before);
```

- [ ] Add failing tests for document insertion, deletion, rename, a multi-document change, and reciprocal application returning the original forward edit.

- [ ] Run the narrow test and confirm the expected compile failure because `AppliedEdit` and `apply_reversible` do not exist.

```powershell
rtk cargo test -p waml --test reversible_edits
```

Expected: compile failure naming the missing reversible edit API.

- [ ] Replace the one-way public contract with:

```rust
pub struct AppliedEdit {
    pub source: SourceBundle,
    pub inverse: PendingEdit,
}

pub trait EditBatch: sealed::Sealed {
    fn apply_reversible(
        &self,
        context: EditContext<'_>,
    ) -> Result<AppliedEdit, EditError>;
}
```

- [ ] Add crate-private `SourceDelta`, `DocumentDelta`, and `TextSplice` types in `edit/reversible.rs`.

```rust
pub(crate) struct SourceDelta {
    documents: Vec<DocumentDelta>,
}

struct DocumentDelta {
    before_path: Option<BundlePath>,
    after_path: Option<BundlePath>,
    before_index: Option<usize>,
    after_index: Option<usize>,
    text: Option<TextSplice>,
    whole_before: Option<SourceDocument>,
    whole_after: Option<SourceDocument>,
}

struct TextSplice {
    start: usize,
    removed: String,
    inserted: String,
}
```

- [ ] Compute modified-text deltas with the longest common prefix and suffix on UTF-8 character boundaries. Store only the changed middle; store a whole document only when insertion/deletion makes that irreducible.

- [ ] Add crate-private `SourceBundle` restoration primitives that preserve vector order and rebuild `by_path` after insert, remove, or rename. Reject invalid paths, indices, and splice preimages without partially mutating the candidate.

- [ ] Implement an internal `SourceDeltaBatch` whose application returns the reciprocal delta. Apply to a candidate clone and publish it only after every delta succeeds.

- [ ] Add `PendingEdit::sequence(Vec<PendingEdit>)`. Its application executes in order, collects reciprocals, and returns them in reverse order.

- [ ] Keep the current copy-on-write behavior: cloning the bundle shares all `Arc<String>` values, and only modified documents detach.

- [ ] Run the narrow test and confirm it passes.

```powershell
rtk cargo test -p waml --test reversible_edits
```

- [ ] Run existing source/edit tests to catch ordering or index regressions.

```powershell
rtk cargo test -p waml source
rtk cargo test -p waml edit
```

- [ ] Commit.

```powershell
rtk git add crates/waml/src/edit.rs crates/waml/src/edit/reversible.rs crates/waml/src/source.rs crates/waml/src/lib.rs crates/waml/tests/reversible_edits.rs
rtk git commit -m "feat(waml): add reversible edit protocol"
```

---

## Task 2: Make Every OKF Operation Reversible

**Files:**

- Modify: `crates/waml/src/okf/ops.rs`
- Test: `crates/waml/tests/okf_ops.rs`

- [ ] Add a table-driven round-trip test covering all OKF variants:
  `ConceptMove`, `DirectoryRename`, `DirectoryMove`, `DirectoryDelete`,
  `IndexReorder`, `IndexSort`, `IndexRetitle`, and `BundleImport`.

- [ ] For each case, assert:
  1. the expected authored source after the forward operation,
  2. byte-exact source and document-order restoration after inverse,
  3. reapplying the returned reciprocal reproduces the forward result.

- [ ] Add a failing multi-step test where the second step depends on the first and inverse application must run in reverse order.

- [ ] Add a failing late-error test proving no `AppliedEdit` escapes and the input bundle remains unchanged.

- [ ] Run the test and confirm it fails because `okf::Batch` still implements the removed one-way method.

```powershell
rtk cargo test -p waml --test okf_ops
```

- [ ] In `okf::Batch::apply_reversible`, clone the candidate cheaply before each semantic `lower_one`, compute `SourceDelta::between(before_step, after_step)`, and append the reciprocal delta.

- [ ] Return a reverse-ordered `PendingEdit::sequence` only after every OKF step, parse check, and lowering invariant succeeds.

- [ ] Do not add hand-written inverse logic to the eight public operations; keep their semantic lowering functions as the single forward mutation owners.

- [ ] Run the test and confirm all eight variants and the failure cases pass.

```powershell
rtk cargo test -p waml --test okf_ops
```

- [ ] Commit.

```powershell
rtk git add crates/waml/src/okf/ops.rs crates/waml/tests/okf_ops.rs
rtk git commit -m "feat(waml): make OKF edits reversible"
```

---

## Task 3: Make Every UML Operation Reversible

**Files:**

- Modify: `crates/waml/src/uml/ops.rs`
- Test: `crates/waml/tests/uml_ops.rs`

- [ ] Add table-driven forward/inverse/reciprocal tests for all UML variants:
  `AttributeAdd`, `AttributeSet`, `AttributeRemove`, `ValueAdd`, `ValueRemove`,
  `RelationshipAdd`, `RelationshipSet`, `RelationshipRemove`, `ClassifierNew`,
  `ClassifierSet`, `ClassifierRemove`, `ClassifierRename`, `DiagramSet`,
  `PlacementSet`, and `PlacementRemove`.

- [ ] Include exact-source assertions for rename referrers, cascading removal, placement edits, authored formatting, and document order.

- [ ] Add a mixed multi-step batch test that proves reciprocal steps are reversed.

- [ ] Add a late-failure test proving the candidate and reciprocal journal never become visible.

- [ ] Run the test and confirm it fails on the old one-way `EditBatch` implementation.

```powershell
rtk cargo test -p waml --test uml_ops
```

- [ ] Implement `uml::Batch::apply_reversible` using the same per-step source-delta journal as OKF.

- [ ] Preserve all existing semantic validation and lowering paths; the journal observes their exact source effects instead of duplicating them.

- [ ] Run the operation matrix and existing UML tests.

```powershell
rtk cargo test -p waml --test uml_ops
rtk cargo test -p waml uml
```

- [ ] Commit.

```powershell
rtk git add crates/waml/src/uml/ops.rs crates/waml/tests/uml_ops.rs
rtk git commit -m "feat(waml): make UML edits reversible"
```

---

## Task 4: Preserve Atomicity for Mixed Compatibility Batches

**Files:**

- Modify: `crates/waml/src/compat.rs`
- Test: `crates/waml/tests/compat_ops.rs`

- [ ] Add a failing test for a legacy batch that alternates OKF and UML steps, then undoes and redoes the complete batch exactly.

- [ ] Add a late-failure test where an early OKF step succeeds and a later UML step fails; assert no source or inverse is published.

- [ ] Run the narrow test and confirm the existing compatibility pipeline cannot satisfy the reversible contract.

```powershell
rtk cargo test -p waml --test compat_ops
```

- [ ] Journal each converted `Step::Okf` or `Step::Uml` against one candidate and return one reverse-ordered reciprocal batch. Do not nest separate public history transactions.

- [ ] Run the compatibility test and the full `waml` crate suite.

```powershell
rtk cargo test -p waml --test compat_ops
rtk cargo test -p waml
```

- [ ] Commit.

```powershell
rtk git add crates/waml/src/compat.rs crates/waml/tests/compat_ops.rs
rtk git commit -m "feat(waml): reverse mixed edit batches"
```

---

## Task 5: Expose Markdown Scroll Anchors in Makepad

**Prerequisite:** The document-header/navigation plan's Makepad `MarkdownRef::scroll_to_fragment` change is already present on this branch.

**Files:**

- Modify: `C:\dev\makepad\widgets\src\markdown.rs`
- Create: `C:\dev\makepad\widgets\tests\markdown_scroll.rs`
- Modify after publish: WAML's existing Makepad dependency pin

- [ ] Add a failing widget-level test for reading the vertical scroll position, setting it, and clamping a value beyond the content extent.

```rust
markdown.set_scroll_y(&mut cx, 120.0);
assert_eq!(markdown.scroll_y(), 120.0);
markdown.set_scroll_y(&mut cx, f64::MAX);
assert!(markdown.scroll_y().is_finite());
```

- [ ] Run the narrow Makepad test and confirm the methods are missing.

```powershell
rtk cargo test -p makepad-widgets --test markdown_scroll
```

- [ ] Add the minimal public reference API without exposing `ScrollBars`:

```rust
impl MarkdownRef {
    pub fn scroll_y(&self) -> f64;
    pub fn set_scroll_y(&self, cx: &mut Cx, y: f64);
}
```

- [ ] Delegate clamping to the existing scrollbar implementation, request redraw, and treat non-finite inputs as zero.

- [ ] Run the narrow and Makepad widget tests.

```powershell
rtk cargo test -p makepad-widgets --test markdown_scroll
rtk cargo test -p makepad-widgets markdown
```

- [ ] Commit the Makepad change.

```powershell
rtk git add widgets/src/markdown.rs widgets/tests/markdown_scroll.rs
rtk git commit -m "feat(markdown): expose scroll anchors"
```

- [ ] Publish/push the Makepad commit using the repository's established dependency workflow, then pin WAML to the exact revision. This is an external-state action: obtain user authorization at execution time if the revision is not already available remotely.

- [ ] Verify the WAML dependency resolves to the new commit.

```powershell
rtk cargo tree -p waml-editor | Select-String makepad-widgets
```

- [ ] Commit only the WAML pin.

```powershell
rtk git add Cargo.toml Cargo.lock
rtk git commit -m "build: pin Makepad history anchors"
```

---

## Task 6: Add Stable Document Locators and View Anchors

**Files:**

- Modify: `crates/waml-editor/src/navigation.rs`
- Create: `crates/waml-editor/src/view_history.rs`
- Modify: `crates/waml-editor/src/document.rs`
- Modify: `crates/waml-editor/src/doc_tabs.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/documents.rs`
- Modify: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/uml_documents.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/class_diagram_surface.rs`
- Modify: Markdown/source-view implementation files
- Modify: `crates/waml-editor/src/lib.rs`
- Test: `crates/waml-editor/tests/view_history.rs`

- [ ] Add failing tests proving primary and source views of the same concept produce distinct locators, and a locator reopens the correct view after its transient tab ID is gone.

- [ ] Define stable identity and anchor data:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    Primary,
    Source,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentLocator {
    pub concept_id: String,
    pub kind: DocumentKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewAnchor {
    None,
    Markdown {
        fragment: Option<String>,
        scroll_y: f64,
    },
    Diagram {
        selected_key: Option<String>,
        camera: Camera,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewLocation {
    pub document: DocumentLocator,
    pub anchor: ViewAnchor,
}
```

- [ ] If `Camera` cannot be used in this pure type without leaking rendering internals, introduce a value-only `DiagramCameraAnchor` with the same center/zoom fields and explicit conversions at `ClassDiagramSurface`.

- [ ] Add `DocumentKind` to `OpenDocument` and `DocTab`. Set `Primary` in normal OKF/UML openers and `Source` only in `open_source`.

- [ ] Add lookup/open APIs that match a `DocumentLocator` to an existing tab first and otherwise construct the correct `OpenDocument` from the current projections.

- [ ] Extend `DocView` with no-op defaults:

```rust
fn capture_anchor(&self, body: &BodyWidgets) -> ViewAnchor {
    ViewAnchor::None
}

fn restore_anchor(
    &mut self,
    cx: &mut Cx,
    body: &BodyWidgets,
    anchor: &ViewAnchor,
) -> bool {
    matches!(anchor, ViewAnchor::None)
}
```

- [ ] Add `DocumentHost::capture_active_location` and `DocumentHost::restore_active_anchor`, delegating through the active `DocView`.

- [ ] For Markdown/source views, capture both resolved fragment identity and `MarkdownRef::scroll_y`; restore fragment first when present, then the recorded scroll offset.

- [ ] For class diagrams, capture `SelectionState::selected_key()` and the camera from `ViewportController::snapshot()`. Restore the camera through a new clamped `restore_camera` path, then restore selection if the key still resolves.

- [ ] Treat missing selection/fragment as a partial anchor restoration, not a failed document activation. Return failure only when the document itself cannot be opened.

- [ ] Run locator and anchor tests.

```powershell
rtk cargo test -p waml-editor --test view_history locator
rtk cargo test -p waml-editor --test view_history anchor
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/navigation.rs crates/waml-editor/src/view_history.rs crates/waml-editor/src/document.rs crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/documents.rs crates/waml-editor/src/okf_documents.rs crates/waml-editor/src/uml_documents.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/class_diagram_surface.rs crates/waml-editor/src/lib.rs crates/waml-editor/tests/view_history.rs
rtk git commit -m "feat(editor): add stable view locations"
```

---

## Task 7: Implement the Pure View-History State Machine

**Files:**

- Modify: `crates/waml-editor/src/view_history.rs`
- Test: `crates/waml-editor/tests/view_history.rs`

- [ ] Write failing tests for:
  - initial location and repeat-current deduplication,
  - preview replacement and manual switching as ordinary transitions,
  - Back then new navigation clearing Forward,
  - Back/Forward traversal not recording another entry,
  - refreshing the departing entry's anchor,
  - skipping an unresolved deleted target without removing it,
  - reaching that target after it resolves again,
  - the 256-entry bound.

- [ ] Define the pure state machine:

```rust
pub const VIEW_HISTORY_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDirection {
    Back,
    Forward,
}

pub struct HistoryTarget {
    index: usize,
    pub location: ViewLocation,
}

pub struct ViewHistory {
    entries: VecDeque<ViewLocation>,
    cursor: Option<usize>,
}
```

- [ ] Implement:

```rust
pub fn reset(&mut self, initial: Option<ViewLocation>);
pub fn record_transition(&mut self, departing: ViewLocation, arriving: ViewLocation);
pub fn refresh_current(&mut self, current: ViewLocation);
pub fn target(
    &self,
    direction: HistoryDirection,
    resolves: impl FnMut(&ViewLocation) -> bool,
) -> Option<HistoryTarget>;
pub fn commit_traversal(&mut self, target: HistoryTarget);
pub fn can_traverse(
    &self,
    direction: HistoryDirection,
    resolves: impl FnMut(&ViewLocation) -> bool,
) -> bool;
```

- [ ] Make `target` scan without mutation. Move the cursor only in `commit_traversal`, after the UI transition succeeds.

- [ ] In `record_transition`, replace the cursor entry with the freshest departing anchor, truncate only the forward branch, append a non-duplicate arrival, and evict oldest entries while fixing the cursor.

- [ ] Run the pure tests.

```powershell
rtk cargo test -p waml-editor --test view_history
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/view_history.rs crates/waml-editor/tests/view_history.rs
rtk git commit -m "feat(editor): add bounded view history"
```

---

## Task 8: Implement Bounded Atomic Model History

**Files:**

- Create: `crates/waml-editor/src/editor_history.rs`
- Modify: `crates/waml-editor/src/lib.rs`
- Create: `crates/waml-editor/tests/editor_history.rs`

- [ ] Create reversible fake edits and write failing pure-history tests for:
  - one edit producing one Undo step,
  - Undo producing Redo and Redo producing Undo,
  - new edit after Undo clearing Redo,
  - a failed Undo/Redo restoring the popped entry unchanged,
  - target and reciprocal locations swapping correctly,
  - current and saved state identity,
  - the newest 64 entries never coalescing,
  - only compatible older contiguous edits coalescing,
  - savepoint, focus, selection, navigation, document, control, kind, span, and structural boundaries,
  - the 1024-entry bound.

- [ ] Define the model-history types:

```rust
pub const ATOMIC_TAIL: usize = 64;
pub const HISTORY_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryStateId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditMergeKind {
    Insert,
    Delete,
    Continuous,
    Structural,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditMergeKey {
    pub document: DocumentLocator,
    pub control: String,
    pub kind: EditMergeKind,
    pub span: Option<Range<usize>>,
}

pub struct EditHistoryStep {
    edit: PendingEdit,
    label: String,
    merge_key: Option<EditMergeKey>,
    source_state: HistoryStateId,
    target_state: HistoryStateId,
    target_location: ViewLocation,
    reciprocal_location: ViewLocation,
}
```

- [ ] Keep the stacks private inside:

```rust
pub struct EditorHistory {
    undo: Vec<EditHistoryStep>,
    redo: Vec<EditHistoryStep>,
    current_state: HistoryStateId,
    saved_state: HistoryStateId,
    next_state: u64,
}
```

- [ ] Make stack mutation explicitly two-phase: `prepare_undo`/`prepare_redo` yields the command without permanently removing it; `commit_*` installs the reciprocal only after session application succeeds; `abort_*` is a no-op because the original stack remains intact.

- [ ] Use `PendingEdit::sequence` to coalesce commands while preserving application order. Coalescing is allowed only outside the newest 64 entries, with matching keys, contiguous spans, and no registered boundary or savepoint between states.

- [ ] Represent focus/selection/navigation boundaries as an incrementing merge epoch stored with steps. `EditorHistory::break_merge_group()` increments it; do not infer boundaries from wall-clock timing.

- [ ] Never coalesce `Structural`, `None` merge keys, or entries touching `saved_state`.

- [ ] Enforce the 1024 cap across the reachable logical timeline. Evict the oldest undo-side entries first; never renumber state IDs.

- [ ] Run the pure model-history tests.

```powershell
rtk cargo test -p waml-editor --test editor_history
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/editor_history.rs crates/waml-editor/src/lib.rs crates/waml-editor/tests/editor_history.rs
rtk git commit -m "feat(editor): add model history state machine"
```

---

## Task 9: Integrate Model History with `EditorSession`

**Files:**

- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/document.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Test: `crates/waml-editor/tests/editor_history.rs`
- Test: existing `editor_session` tests

- [ ] Add failing session tests for successful edit/undo/redo, failed edit, failed Undo, failed Redo, stale save, save followed by Undo, Undo returning to saved state, and replace resetting both stacks.

- [ ] Introduce typed edit metadata:

```rust
pub struct EditIntent {
    pub edit: PendingEdit,
    pub label: String,
    pub merge_key: Option<EditMergeKey>,
    pub after_location: Option<ViewLocation>,
}

pub struct EditRequest {
    pub intent: EditIntent,
    pub before_location: ViewLocation,
}

pub struct HistoryEffect {
    pub change: SessionChange,
    pub label: String,
    pub location: ViewLocation,
}
```

- [ ] Change `ViewOutcome.edit` from `Option<PendingEdit>` to `Option<EditIntent>`.

- [ ] Label current producers and keep them non-coalescing:
  - diagram properties: `"Change diagram properties"`,
  - placement: `"Place {subject}"`,
  - conflict removal: `"Remove conflicting placement"`,
  - `merge_key: None`.

- [ ] Add `EditorHistory` to `EditorSession`, initialized with a fresh state and savepoint when a source is loaded or replaced.

- [ ] Refactor current apply logic into one private primitive:

```rust
fn apply_pending(
    &mut self,
    edit: &PendingEdit,
) -> Result<(SessionChange, PendingEdit), EditError>;
```

It must:
  1. apply reversibly to a candidate,
  2. parse OKF,
  3. project UML,
  4. publish source/analyses/revision only after all stages pass,
  5. return the reciprocal.

- [ ] Add the public session API:

```rust
pub fn apply_edit(&mut self, request: EditRequest)
    -> Result<SessionChange, EditError>;
pub fn undo(&mut self) -> Result<Option<HistoryEffect>, EditError>;
pub fn redo(&mut self) -> Result<Option<HistoryEffect>, EditError>;
pub fn can_undo(&self) -> bool;
pub fn can_redo(&self) -> bool;
pub fn history_state(&self) -> HistoryStateId;
pub fn break_edit_merge_group(&mut self);
```

- [ ] On a new edit, assign a fresh state ID only after `apply_pending` succeeds; then push the inverse onto Undo and clear Redo.

- [ ] On Undo/Redo, keep the source stack untouched until `apply_pending` succeeds, then commit the reciprocal to the opposite stack and set `current_state` to the entry's target state.

- [ ] Replace `dirty_revision.is_some()` with `history.current_state() != history.saved_state()`.

- [ ] Change save completion to require both captured revision and captured history state:

```rust
pub fn mark_saved(
    &mut self,
    revision: u64,
    state: HistoryStateId,
) -> bool;
```

Only a matching current revision/state updates `persisted_source` and `saved_state`.

- [ ] Preserve all existing `SessionChange` invalidation behavior for Undo/Redo; the caller must not special-case refreshed documents.

- [ ] Run session and history tests.

```powershell
rtk cargo test -p waml-editor editor_session
rtk cargo test -p waml-editor --test editor_history
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/editor_session.rs crates/waml-editor/src/document.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/tests/editor_history.rs
rtk git commit -m "feat(editor): integrate undo and redo"
```

---

## Task 10: Centralize Visible Transitions and Back/Forward Traversal

**Prerequisite:** Use the centralized `App::handle_navigation_intent`/`navigate_with` path produced by the document-header/navigation plan.

**Files:**

- Modify: `crates/waml-editor/src/navigation.rs`
- Modify: `crates/waml-editor/src/document.rs`
- Modify: `crates/waml-editor/src/doc_tabs.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Create: `crates/waml-editor/tests/history_integration.rs`

- [ ] Add failing integration tests for:
  - manual tab A → B → Back → A,
  - preview A replaced by B → Back reopens A into the preview slot,
  - Back then manual switch clearing Forward,
  - explicit breadcrumb/navigation transitions,
  - active close recording the newly active document,
  - inactive close not recording,
  - promote-only not recording,
  - repeat-current and passive reconciliation not recording,
  - deleted target skipped but retained,
  - Undo restoring the target and making it traversable,
  - Back/Forward restoring anchors without restoring tab order/pinning/closed state.

- [ ] Define transition causes:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransitionCause {
    UserNavigation,
    UndoRedoReveal,
    HistoryTraversal,
    PassiveReconciliation,
}
```

- [ ] Add `view_history: ViewHistory` to `App`. Reset it when the editor transitions between no active document and the first active document, or when a different model replaces the session.

- [ ] Introduce one visible-transition method:

```rust
fn transition_to_location(
    &mut self,
    cx: &mut Cx,
    location: ViewLocation,
    cause: TransitionCause,
) -> bool;
```

- [ ] Before every departure, capture the current location and freshest anchor from `DocumentHost`.

- [ ] For `UserNavigation` and `UndoRedoReveal`, activate an existing matching locator or reopen it into the preview slot, restore the target anchor, then call `record_transition(departing, arriving)` only after success.

- [ ] For `HistoryTraversal`, perform the same open/restore work but never append or truncate. Commit the selected history cursor only after success.

- [ ] For `PassiveReconciliation`, switch without recording and refresh the current history entry if it still identifies the same logical view.

- [ ] Route all existing visible document changes through this method:
  - tab clicks,
  - document switcher selection,
  - preview replacement,
  - prerequisite breadcrumb/fragment navigation,
  - active-tab close fallback,
  - direct source/diagram opens,
  - post-edit navigation.

- [ ] Keep tab promotion and inactive close on non-recording paths. Detect repeat-current actions by logical document/view identity and transition intent, not tab ID or incidental anchor drift. Caret, scroll, pan, and selection changes refresh the current anchor but never create a new entry; an explicit fragment/navigation command may create one.

- [ ] Implement Back/Forward as:
  1. ask `ViewHistory::target` to scan for a currently resolvable locator,
  2. call `transition_to_location(..., HistoryTraversal)`,
  3. commit traversal on success,
  4. retain all skipped entries.

- [ ] Add one shared edit-completion method so normal edit, Undo, and Redo all call `documents.after_session_change`, refresh badges/chrome, update status, and schedule save.

- [ ] For Undo/Redo, call the session first. On success, reveal `HistoryEffect.location` with `UndoRedoReveal`; all affected views refresh even if they are not active.

- [ ] If reveal fails after the model edit succeeds, keep the successful model history transition, leave the current document active, and show a precise warning naming the affected logical document. Do not roll back a valid model change because a view cannot open.

- [ ] Register `break_edit_merge_group()` on manual navigation, focus-changing document switches, explicit selection changes, and savepoint creation.

- [ ] Run integration tests.

```powershell
rtk cargo test -p waml-editor --test history_integration
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/navigation.rs crates/waml-editor/src/document.rs crates/waml-editor/src/doc_tabs.rs crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/tests/history_integration.rs
rtk git commit -m "feat(editor): integrate logical view history"
```

---

## Task 11: Add Header Controls, Shortcuts, and Status Feedback

**Files:**

- Modify: `crates/waml-editor/src/document_header.rs`
- Modify: `crates/waml-editor/src/shortcuts.rs`
- Modify: `crates/waml-editor/src/statusbar.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Test: relevant module tests
- Test: `crates/waml-editor/tests/history_integration.rs`

- [ ] Add failing layout/action tests proving:
  - Back and Forward buttons remain mounted for every active document,
  - disabled state tracks resolvable history targets,
  - the header remains exactly 30 px with or without breadcrumbs,
  - the start screen has no header,
  - button actions dispatch once.

- [ ] Extend the prerequisite header action:

```rust
pub enum DocumentHeaderAction {
    Back,
    Forward,
    Navigate(NavigationTarget),
    ToggleRightDock,
}
```

- [ ] Reserve a fixed leading region for the two buttons so breadcrumb changes do not shift the rest of the header. Expose:

```rust
pub fn set_history_enabled(
    &self,
    cx: &mut Cx,
    can_back: bool,
    can_forward: bool,
);
```

- [ ] Recompute enabled state after successful model edits, Undo/Redo, view transitions, close/reopen, and passive projection reconciliation, because previously unresolved locations may become resolvable.

- [ ] Add a pure, platform-parameterized shortcut parser and failing tests:

```rust
enum HistoryCommand {
    Undo,
    Redo,
}

fn history_command_for(
    key: KeyCode,
    modifiers: KeyModifiers,
    macos: bool,
) -> Option<HistoryCommand>;
```

Expected bindings:
  - macOS: `Cmd+Z`, `Cmd+Shift+Z`,
  - other platforms: `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+Y`.

- [ ] Dispatch Undo/Redo in `App::handle_event` before focused widgets receive the key event. Consume a recognized chord even when its stack is empty so the active editor cannot apply a competing local undo.

- [ ] Do not add Back/Forward keyboard or mouse bindings in this change.

- [ ] Update the shortcut overlay with the platform-appropriate Undo/Redo bindings.

- [ ] Add status messages for successful Undo/Redo labels, empty stacks, unavailable Back/Forward targets, and reveal failures. Preserve precedence:
  `save error > history error/warning > history success > navigation > normal`.

- [ ] Run narrow tests.

```powershell
rtk cargo test -p waml-editor document_header
rtk cargo test -p waml-editor shortcuts
rtk cargo test -p waml-editor statusbar
rtk cargo test -p waml-editor --test history_integration
```

- [ ] Commit.

```powershell
rtk git add crates/waml-editor/src/document_header.rs crates/waml-editor/src/shortcuts.rs crates/waml-editor/src/statusbar.rs crates/waml-editor/src/app.rs crates/waml-editor/tests/history_integration.rs
rtk git commit -m "feat(editor): expose history controls"
```

---

## Task 12: Verify the Complete Behavior and Document the Invariants

**Files:**

- Modify: `docs/superpowers/specs/2026-07-28-undo-redo-view-history-design.md` only if implementation reveals a necessary, user-approved correction
- Modify: nearby module documentation in history modules as needed
- Test: all affected suites

- [ ] Add or confirm end-to-end regressions for the approved user stories:
  - type `Customerr`; the newest atomic Undo removes only the last `r`,
  - make an edit in editor A, switch to B, Undo reveals A, Back returns to B,
  - Undo then edit clears Redo,
  - Back then navigate clears Forward,
  - Save, Undo, Redo preserves history and computes dirty state from state identity,
  - preview and pinned/manual tabs participate identically as logical transitions,
  - deletion makes a view unavailable; traversal skips it; Undo restores it.

- [ ] Confirm memory bounds with test-only history inspection:
  - newest 64 model entries remain separate,
  - total model entries never exceed 1024,
  - total view entries never exceed 256,
  - compatible older text entries coalesce without crossing the saved state.

- [ ] Run formatting.

```powershell
rtk cargo fmt --all -- --check
```

Expected: no diff.

- [ ] Run WAML core tests.

```powershell
rtk cargo test -p waml
```

Expected: all reversible OKF, UML, compatibility, and existing tests pass.

- [ ] Run editor tests.

```powershell
rtk cargo test -p waml-editor
```

Expected: all session, history, document, header, shortcut, status, and existing tests pass.

- [ ] Run linting with the repository-approved command.

```powershell
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: no warnings or errors.

- [ ] Build the editor.

```powershell
rtk cargo build -p waml-editor
```

Expected: successful build.

- [ ] Launch the editor and manually verify the seven user stories above. Capture the running window only if visual evidence is useful:

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out history-controls.png -Process waml-editor
```

- [ ] Inspect the final diff for unrelated files and generated artifacts. Preserve the user's pre-existing worktree changes.

```powershell
rtk git status --short
rtk git diff --stat main...
rtk git diff main... -- crates/waml crates/waml-editor docs
```

- [ ] Commit any final test/documentation-only adjustments.

```powershell
rtk git add crates/waml crates/waml-editor docs/superpowers
rtk git commit -m "test(editor): cover history workflows"
```

## Completion Criteria

- Every successful semantic edit returns an exact reciprocal operation; failed edits return none and publish no state.
- Undo/Redo is global per open model, bounded, savepoint-aware, and leaves at least the newest 64 actions atomic.
- Undo/Redo reveals the affected editor and records that reveal in view history.
- Back/Forward includes preview replacement, manual switching, explicit navigation, active close, and undo/redo reveals.
- Back/Forward never changes model state or reconstructs historical tab topology.
- Headers and shortcuts match the approved platform behavior.
- All automated checks pass, and manual verification demonstrates the approved scenarios without disturbing unrelated worktree changes.
