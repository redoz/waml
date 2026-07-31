# Markdown Editor Application Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the native read-only Markdown source surface with the WAML-owned editor and make application snapshots, analysis, canvas, persistence, navigation, assets, and LSP consume one revision-checked Markdown syntax product.

**Architecture:** `SourceView` owns the live `MarkdownDocumentSession`, but the application `EditorSession` owns the current immutable workspace snapshot and canonical raw source. A local proposal is accepted only when its document revision, exact text changes, resulting text, and `MarkdownSyntaxUpdate` all match; the application promotes that same syntax snapshot without parsing the accepted revision again. Semantic, asset, external-reload, and save completions carry revision identity and are installed only when they still match the current immutable snapshot.

**Tech Stack:** Rust 2021 with MSRV 1.80, `waml-syntax`, `waml`, `waml-markdown-editor`, Makepad low-level widgets, `tower-lsp` 0.20, native atomic persistence, browser URL-fragment persistence, and Cargo tests.

## Global Constraints

- Raw Markdown source is authoritative. Syntax, semantic analysis, diagnostics, presentation, canvas projections, persistence, and LSP data are revisioned projections.
- `MarkdownDocumentSession` is widget-local. `waml_editor::EditorSession` is the application workspace authority.
- `SourceView` stores the live `MarkdownDocumentSession` for one immutable `DocumentId`; `App` and `DocumentHost` do not inspect editor internals.
- Every source edit carries `DocumentId`, `DocumentRevision`, exact `TextChange` values, and the exact `MarkdownSyntaxUpdate` produced by the local session.
- The accepted proposal's `MarkdownSyntaxUpdate::snapshot` is promoted by `Arc`; application code must not call `parse_markdown` or `reparse_markdown` for that accepted document revision.
- A stale source edit, syntax update, semantic completion, image event, external replacement, or save completion must not replace newer state or clear a newer dirty revision.
- Syntax and semantic diagnostics never roll back literal source or disable continued editing.
- Invalid source is persisted literally.
- Parser and application source positions are checked UTF-8 byte `TextSize` and `TextRange` values. LSP positions use UTF-16 and convert through the same document `LineIndex`.
- The in-app editor calls syntax and semantic APIs directly. It never calls its own LSP server.
- One production source-editor path exists after each task. Do not keep a second parser, regex Markdown classifier, or long-lived competing editor path.
- Normal click and drag edit the source. Ctrl/Cmd-click on a parsed link emits typed navigation.
- Initial load and accepted external replacement use `LayoutChangeCause::InitialLoad` or `LayoutChangeCause::ExternalReplacement` and cut motion to target geometry.
- Persistence and asset policy remain application responsibilities. The editor widget does not write files, update URLs, or fetch network resources.
- The final production path contains no Makepad `Markdown`, `MarkdownRef`, or `MarkdownAction`.
- Every shell command starts with `rtk`.
- Each task writes a failing focused test, verifies the expected failure, implements the minimum named behavior, reruns the focused tests, and commits only after they pass.

---

## File Structure

- Modify `crates/waml/src/edit.rs` to add an exact raw-document edit that participates in the existing reversible history transaction.
- Modify `crates/waml/src/analysis.rs` to accept promoted Markdown updates, expose document snapshots, report affected documents, and avoid duplicate parsing.
- Modify `crates/waml/tests/incremental_analysis.rs` to prove exact syntax `Arc` promotion, unchanged identity reuse, invalid-island fallback, and parser call counts.
- Modify `crates/waml-editor/Cargo.toml` to depend on `waml-markdown-editor`.
- Modify `crates/waml-editor/src/editor_session.rs` to publish `Arc<EditorSessionSnapshot>`, promote `ProposedSourceEdit`, guard semantic completion, and keep revision-safe dirty state.
- Create `crates/waml-editor/src/markdown_analysis.rs` for revision-tagged semantic requests/completions and affected-document installation.
- Create `crates/waml-editor/src/markdown_hosts.rs` for application implementations of `CodeHighlightHost` and `MarkdownAssetHost`.
- Modify `crates/waml-markdown-editor/src/session.rs` to synchronize a live local session from an already-parsed host snapshot without reparsing.
- Modify `crates/waml-markdown-editor/tests/document_ops.rs` to cover host acknowledgement, app history, stale host snapshots, and external replacement.
- Modify `crates/waml-editor/src/source_view.rs` to own the local session, presentation, layout, motion, assets, and typed actions.
- Modify `crates/waml-editor/src/doc_view.rs` to expose a generic active-view event/draw scope and typed source-edit outcomes.
- Modify `crates/waml-editor/src/document_host.rs` to preserve live views that declare view-local state and route the active view's UI scope.
- Modify `crates/waml-editor/src/documents.rs` and `crates/waml-editor/src/okf_documents.rs` to construct source views from immutable document identity.
- Modify `crates/waml-editor/src/app/actions.rs` to promote typed source edits and install only current semantic completions.
- Modify `crates/waml-editor/src/app.rs` to route events through the active view, schedule saves on source acceptance, pass the native/browser asset context, and handle external replacement conflicts.
- Modify `crates/waml-editor/src/view_history.rs` to capture revision-bound Markdown selection and scroll state.
- Modify `crates/waml-editor/src/navigation.rs` to add current-source-range navigation.
- Modify `crates/waml-editor/src/class_diagram_view.rs` and `crates/waml-editor/src/behavior_doc_view.rs` to refresh only affected projections and preserve unaffected viewport/selection state.
- Modify `crates/waml-editor/src/native_save.rs` to save an owned immutable snapshot ticket and return its exact revision/state identity.
- Modify `crates/waml-cli/src/lsp/bundle.rs` to expose the shared Markdown/document snapshots to LSP queries.
- Modify `crates/waml-cli/src/lsp/map.rs` to use shared `LineIndex` conversion for diagnostics, definitions, links, and semantic tokens.
- Create `crates/waml-cli/src/lsp/query.rs` for renderer-free snapshot queries used by LSP.
- Modify `crates/waml-cli/src/lsp/server.rs` to publish revision-checked Markdown diagnostics, document symbols, definitions, links, and semantic tokens.
- Modify `crates/waml-cli/src/lsp/mod.rs` to register the query module.
- Modify `crates/waml-cli/tests/lsp_e2e.rs` to test the shared-snapshot LSP surface and UTF-8/UTF-16 round trips.
- Modify `crates/waml-editor/src/generic_okf_view.rs` in the same mounting task as `SourceView`, using the WAML editor in read-only mode from the same syntax snapshot.
- Modify `crates/waml-editor/src/main.rs`, `crates/waml-editor/src/app.rs`, and `crates/waml-editor/src/doc_view.rs` in that mounting task to replace the one shared live node and remove the Makepad Markdown registration and live path.
- Delete `crates/waml-editor/src/markdown_surface.rs` in the mounting task; the final rollout task verifies that it does not return.
- Create `crates/waml-editor/tests/markdown_integration.rs` for mounted source-tab, history, navigation, missing-source, and external-reload behavior.
- Create `crates/waml-editor/tests/markdown_authority.rs` for forbidden parser/editor/runtime imports.

## Cross-Plan Interface Contract

The syntax plan supplies these exact exports:

```rust
pub struct DocumentRevision(u64);

impl DocumentRevision {
    pub const INITIAL: Self;
    pub const fn new(value: u64) -> Self;
    pub const fn get(self) -> u64;
    pub fn checked_next(self) -> Option<Self>;
}

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError>;

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}
```

The foundation plan supplies:

```rust
pub struct ProposedMarkdownEdit {
    pub edit: MarkdownEdit,
    pub snapshot: Arc<MarkdownDocumentSnapshot>,
    pub syntax_update: MarkdownSyntaxUpdate,
}

pub struct MarkdownDocumentSession;

pub struct MarkdownEditorScope<'a> {
    pub session: &'a mut MarkdownDocumentSession,
}

impl MarkdownEditorRef {
    pub fn set_read_only(&self, cx: &mut Cx, read_only: bool);
    pub fn set_reduced_motion(&self, cx: &mut Cx, reduced: bool);
    pub fn install_presentation(
        &self,
        cx: &mut Cx,
        presentation: Arc<InstalledPresentation>,
        cause: LayoutChangeCause,
    );
    pub fn clear_presentation(&self, cx: &mut Cx);
    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>>;
    pub fn frame_layout(&self) -> Option<Arc<LayoutSnapshot>>;
    pub fn proposed_edit(actions: &Actions) -> Option<ProposedMarkdownEdit>;
    pub fn navigation_request(actions: &Actions) -> Option<TextPosition>;
}
```

The presentation plan supplies `compile_presentation`, `build_layout_document`, `InstalledPresentation`, `LayoutChangeCause`, `LayoutDocument`, `LayoutSnapshot`, `HighlighterRegistry`, `CodeHighlightHost`, `EmbeddedAssets`, and `MarkdownAssetHost`. Its final compiler and asset-host signatures are:

```rust
pub fn compile_presentation(
    snapshot: &MarkdownSyntaxSnapshot,
    styles: &PresentationStyles,
    highlighters: &HighlighterRegistry,
) -> Result<Arc<PresentationPlan>, PresentationError>;

pub trait MarkdownAssetHost {
    fn request_image(&mut self, request: ImageAssetRequest);
    fn cancel_image(&mut self, request_id: AssetRequestId);
    fn drain_events(&mut self) -> Vec<ImageAssetEvent>;
}
```

`compile_presentation` performs one segmentation pass with syntax and validated code-highlight boundaries. `SourceView` calls it once per installed syntax revision. `InstalledPresentation` atomically binds that plan to same-revision styles, `LayoutDocument`, diagnostics, and embedded assets. The widget owns target/frame `LayoutSnapshot` values and `MotionController`; application code never installs raw geometry.

---

### Task 1: Add the Exact Raw-Source and Promoted-Syntax Analysis Seam

**Files:**
- Modify: `crates/waml/src/edit.rs`
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/tests/incremental_analysis.rs`

**Interfaces:**
- Consumes: `DocumentId`, `DocumentRevision`, `MarkdownSyntaxUpdate`, `SourceText`, `TextChange`, `PreviousAnalyses`, and the syntax plan's `OkfAnalysis` Markdown snapshot map.
- Produces:

```rust
#[derive(Clone, Debug)]
pub struct ExactSourceEdit {
    pub document: DocumentId,
    pub base_revision: DocumentRevision,
    pub changes: Arc<[TextChange]>,
    pub expected_text: SourceText,
}

#[derive(Clone)]
pub struct PromotedMarkdownUpdate {
    pub document: DocumentId,
    pub base_revision: DocumentRevision,
    pub update: MarkdownSyntaxUpdate,
}

pub fn prepare_candidate_with_markdown_updates(
    candidate_source: SourceBundle,
    previous: PreviousAnalyses<'_>,
    candidate_revision: u64,
    promoted: Arc<[PromotedMarkdownUpdate]>,
) -> Result<PreparedCandidate, AnalysisError>;

impl OkfAnalysis {
    pub fn markdown_snapshot(
        &self,
        document: DocumentId,
    ) -> Option<&Arc<MarkdownSyntaxSnapshot>>;
}
```

- [ ] **Step 1: Write failing exact-edit and promotion tests**

Add tests that create two documents, reparse one local one-character edit, and assert:

```rust
#[test]
fn promoted_markdown_update_is_installed_by_arc_without_a_second_parse() {
    let before = prepared_two_document_fixture(41);
    let id = before.okf().catalog.id_for_path(&path("order.md")).unwrap();
    let old = before.okf().markdown_snapshot(id).unwrap().clone();
    let changes: Arc<[TextChange]> = Arc::from([replace(2..7, "Purchase")]);
    let next_revision = old.revision().checked_next().unwrap();
    let next_text = apply_changes(old.text(), &changes).unwrap();
    let update = reparse_markdown(&old, next_revision, next_text.clone(), &changes).unwrap();
    let candidate = replace_bundle_text(before.source(), "order.md", next_text.shared().to_string());
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();

    let after = waml::analysis::test_support::prepare_candidate_with_promoted_probe(
        candidate,
        PreviousAnalyses {
            okf: before.okf(),
            uml: before.uml(),
        },
        42,
        Arc::from([PromotedMarkdownUpdate {
            document: id,
            base_revision: old.revision(),
            update: update.clone(),
        }]),
        &mut probe,
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        after.okf().markdown_snapshot(id).unwrap(),
        &update.snapshot
    ));
    assert_eq!(probe.markdown_parse_calls(id), 0);
    assert_eq!(probe.markdown_promotions(id), 1);
}
```

Also test wrong document id, stale base revision, non-successor update revision, mismatched resulting text, mismatched source path, overlapping changes, and unchanged-document `Arc` reuse. Each error must leave the prior `PreparedCandidate` usable and must report `AnalysisError::InvalidPromotedMarkdownUpdate`.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `rtk cargo test -p waml --test incremental_analysis promoted_markdown_update`

Expected: FAIL with unresolved `ExactSourceEdit`, `PromotedMarkdownUpdate`, and `prepare_candidate_with_markdown_updates`.

- [ ] **Step 3: Implement `ExactSourceEdit` through the existing reversible edit path**

Implement `EditBatch::lower` for `ExactSourceEdit`. Resolve `document` through `EditContext::okf_analysis.catalog`, compare its current `DocumentRevision` with `base_revision`, validate `changes` with `ChangeMap::checked`, apply them to the current `SourceText`, and require byte equality with `expected_text`. Replace only that `SourceDocument` through `waml::host::replace_document`.

Return typed `EditError` stages:

```rust
"source.document"
"source.base_revision"
"source.change_map"
"source.expected_text"
```

The existing `PendingEdit::apply_reversible` must derive the inverse `SourceDelta`; do not add a second history stack in `waml`.

- [ ] **Step 4: Validate and install promoted syntax before semantic lowering**

In `prepare_candidate_with_markdown_updates`, validate all promoted records before analysis:

1. each `DocumentId` exists in both prior and candidate catalogs;
2. the prior snapshot revision equals `base_revision`;
3. the update revision equals `base_revision.checked_next()`;
4. the update text equals the candidate document text;
5. at most one promoted record exists per document;
6. every `affected_range` is inside the new text.

Pass validated updates into the syntax phase. For a promoted document, insert `update.snapshot.clone()` directly. For an unchanged document, clone the old snapshot `Arc`. Only other changed documents call `reparse_markdown`; initial documents call `parse_markdown`.

Extend `PreparationProbe` with per-document `markdown_parse_calls`, `markdown_reparse_calls`, and `markdown_promotions`. These counters are test-only and increment at the three mutually exclusive syntax ingress points.

- [ ] **Step 5: Run analysis and domain tests**

Run: `rtk cargo test -p waml --test incremental_analysis promoted_markdown_update`

Expected: PASS. The promoted document records zero parse/reparse calls and one promotion.

Run: `rtk cargo test -p waml --tests`

Expected: PASS. Existing reversible edits, incremental reuse, lowerers, and diagnostics remain valid.

- [ ] **Step 6: Commit the analysis seam**

```powershell
rtk git add crates/waml/src/edit.rs crates/waml/src/analysis.rs crates/waml/tests/incremental_analysis.rs
rtk git commit -m "feat(markdown): promote exact source syntax updates"
```

---

### Task 2: Publish Immutable Application Snapshots and Promote Source Proposals

**Files:**
- Modify: `crates/waml-editor/src/editor_session.rs`
- Create: `crates/waml-editor/src/markdown_analysis.rs`
- Modify: `crates/waml-editor/src/main.rs`

**Interfaces:**
- Consumes: Task 1 promotion API and `ProposedMarkdownEdit`.
- Produces:

```rust
#[derive(Clone)]
pub struct EditorSessionSnapshot {
    pub revision: u64,
    pub source: Arc<SourceBundle>,
    pub persisted_source: Arc<SourceBundle>,
    pub markdown_snapshots: Arc<BTreeMap<DocumentId, Arc<MarkdownSyntaxSnapshot>>>,
    pub okf_analysis: Arc<OkfAnalysis>,
    pub uml_analysis: Arc<waml::uml::Analysis>,
    pub dirty_revision: Option<u64>,
    pub affected_documents: Arc<[DocumentId]>,
}

#[derive(Clone, Debug)]
pub struct ProposedSourceEdit {
    pub document: DocumentId,
    pub base_revision: DocumentRevision,
    pub changes: Arc<[TextChange]>,
    pub syntax_update: MarkdownSyntaxUpdate,
}

impl ProposedSourceEdit {
    pub fn from_local(
        document: DocumentId,
        local: ProposedMarkdownEdit,
    ) -> Self;
}

impl EditorSessionSnapshot {
    pub fn markdown_snapshot(
        &self,
        document: DocumentId,
    ) -> Option<&Arc<MarkdownSyntaxSnapshot>>;
}

#[derive(Clone)]
pub struct SemanticAnalysisRequest {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub previous: Arc<EditorSessionSnapshot>,
    pub promoted: Arc<[PromotedMarkdownUpdate]>,
}

#[derive(Clone)]
pub struct SemanticAnalysisCompletion {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub prepared: Arc<PreparedCandidate>,
}

pub enum CompletionInstall {
    Installed(SessionChange),
    IgnoredStale,
}

impl EditorSession {
    pub fn snapshot(&self) -> Arc<EditorSessionSnapshot>;
    pub fn promote_source_edit(
        &mut self,
        proposal: ProposedSourceEdit,
        before_location: ViewLocation,
    ) -> Result<(SessionChange, SemanticAnalysisRequest), SourceEditError>;
    pub fn install_semantic_completion(
        &mut self,
        completion: SemanticAnalysisCompletion,
    ) -> CompletionInstall;
}
```

- [ ] **Step 1: Write failing snapshot, promotion, and stale-completion tests**

Replace borrowed-snapshot tests with owned snapshot tests:

```rust
#[test]
fn accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc() {
    let mut session = source_session("# Order\n", 7);
    let before = session.snapshot();
    let id = document_id(&before, "order.md");
    let local = local_proposal(before.okf_analysis.markdown_snapshot(id).unwrap(), 2..7, "Purchase");
    let syntax = local.syntax_update.snapshot.clone();

    let (change, request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        )
        .unwrap();
    let completion = run_semantic_request(request).unwrap();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));

    let after = session.snapshot();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(change.revision, after.revision);
    assert!(Arc::ptr_eq(
        after.okf_analysis.markdown_snapshot(id).unwrap(),
        &syntax
    ));
    assert_eq!(after.dirty_revision, Some(after.revision));
}
```

Add tests for a stale base revision, mismatched change list, mismatched update text, same-revision completion after a newer source edit, completion carrying a different source `Arc`, semantic failure after source acceptance, and an older completion arriving after the newer completion.

- [ ] **Step 2: Run the session tests and verify they fail**

Run: `rtk cargo test -p waml-editor editor_session::tests --bin waml-editor`

Expected: FAIL because `EditorSessionSnapshot`, `ProposedSourceEdit`, and guarded completion installation do not exist.

- [ ] **Step 3: Store one current immutable snapshot**

Replace the parallel mutable `source`, `persisted_source`, `okf_analysis`, `uml`, and `revision` fields with:

```rust
pub struct EditorSession {
    current: Arc<EditorSessionSnapshot>,
    history: EditorHistory,
}
```

Build a new `Arc<EditorSessionSnapshot>` off to the side and swap `current` only after every fallible validation succeeds. Keep compatibility accessors (`source`, `okf_analysis`, `uml_analysis`, `revision`, `persisted_bundle`) as reads through `current` until call sites migrate.

`replace` publishes a clean snapshot, resets history, and records all replacement documents as affected. `mark_saved(revision, state)` clones the current snapshot with a new `persisted_source` only when both identifiers still match.

- [ ] **Step 4: Promote a proposal without duplicate parsing**

`ProposedSourceEdit::from_local` must copy `document`, `edit.base_revision`, `edit.changes`, and `syntax_update`; it must not copy a full string from the widget.

`promote_source_edit` must:

1. resolve the current document snapshot;
2. validate base revision before mutation;
3. require the proposal changes to produce `syntax_update.snapshot.text()`;
4. lower `ExactSourceEdit` through existing reversible history;
5. create one `PromotedMarkdownUpdate` with the same `MarkdownSyntaxUpdate`;
6. clone `markdown_snapshots`, replace only the edited document with `syntax_update.snapshot.clone()`, and publish source/syntax revision and dirty state once;
7. record the inverse in `EditorHistory`;
8. return a semantic request that owns the prior/current immutable inputs.

Do not call `parse_markdown`, `reparse_markdown`, or the LSP.

- [ ] **Step 5: Install only current semantic completions**

`install_semantic_completion` returns `IgnoredStale` unless both conditions hold:

```rust
completion.session_revision == self.current.revision
    && Arc::ptr_eq(&completion.source, &self.current.source)
```

An accepted completion clones the current source/syntax/dirty fields and replaces only semantic analyses and affected-document metadata. Before installation, require every prepared Markdown snapshot to be pointer-identical to the corresponding current `markdown_snapshots` entry. This prevents a semantic worker from returning a separately parsed syntax product for an already-published revision. A failed semantic request leaves source and syntax current, publishes an internal diagnostic, and retains the prior semantic projection only for failed islands.

- [ ] **Step 6: Run session, history, and savepoint tests**

Run: `rtk cargo test -p waml-editor editor_session::tests --bin waml-editor`

Expected: PASS, including the existing undo/redo and matching revision/state savepoint tests.

Run: `rtk cargo test -p waml-editor --test editor_history`

Expected: PASS. Source proposals participate in the application history stack.

- [ ] **Step 7: Commit immutable session snapshots**

```powershell
rtk git add crates/waml-editor/src/editor_session.rs crates/waml-editor/src/markdown_analysis.rs crates/waml-editor/src/main.rs
rtk git commit -m "refactor(editor): publish immutable session snapshots"
```

---

### Task 3: Synchronize the Widget-Local Session from Host Snapshots

**Files:**
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Modify: `crates/waml-markdown-editor/src/edit.rs`
- Modify: `crates/waml-markdown-editor/tests/document_ops.rs`

**Interfaces:**
- Consumes: `MarkdownDocumentSnapshot`, `MarkdownSyntaxSnapshot`, `TextChange`, and the local session history/selection APIs.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostSnapshotCause {
    InitialLoad,
    AcknowledgedLocalEdit,
    ApplicationHistory,
    ExternalReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostSyncOutcome {
    Installed,
    Acknowledged,
    IgnoredStale,
}

impl MarkdownDocumentSession {
    pub fn synchronize_from_host(
        &mut self,
        snapshot: Arc<MarkdownDocumentSnapshot>,
        changes: Option<&[TextChange]>,
        cause: HostSnapshotCause,
    ) -> Result<HostSyncOutcome, MarkdownEditError>;
    pub fn scroll_state(&self) -> &ScrollState;
    pub fn set_scroll_state(&mut self, scroll: ScrollState);
}
```

- [ ] **Step 1: Write failing host synchronization tests**

Add:

```rust
#[test]
fn acknowledging_the_same_local_snapshot_keeps_selection_scroll_and_history() {
    let mut session = session("# A\n", 10);
    let proposal = session
        .execute(EditCommand::Insert(Arc::from("x")), HistoryGroup::named(1))
        .unwrap()
        .proposal
        .unwrap();
    session.set_scroll_state(ScrollState { x: 0.0, y: 48.0 });
    let selection = session.selections().clone();

    assert_eq!(
        session
            .synchronize_from_host(
                proposal.snapshot.clone(),
                Some(&proposal.edit.changes),
                HostSnapshotCause::AcknowledgedLocalEdit,
            )
            .unwrap(),
        HostSyncOutcome::Acknowledged
    );
    assert_eq!(session.selections(), &selection);
    assert_eq!(session.scroll_state().y, 48.0);
    assert!(session.can_undo());
}
```

Also test initial install, stale host snapshot ignore, app undo resetting local history, external replacement translating selection through supplied changes, external replacement without a safe map selecting offset zero, and all external causes cutting motion.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops host_`

Expected: FAIL with unresolved `HostSnapshotCause` and `synchronize_from_host`.

- [ ] **Step 3: Implement synchronization without parsing**

For `AcknowledgedLocalEdit`, require both revision equality and:

```rust
Arc::ptr_eq(
    session.snapshot().syntax(),
    snapshot.syntax(),
)
```

Keep selection, scroll, preferred column, and local history.

For `ApplicationHistory` and `ExternalReplacement`, install the supplied already-parsed snapshot. If `changes` are supplied, translate selection and scroll anchor with `ChangeMap`; otherwise install a caret at offset zero. Clear local undo/redo and IME. Never call syntax parsing.

Return `IgnoredStale` when the host revision is older than the installed local revision. Return a typed mismatch error when equal revisions have different syntax snapshot identity or text.

- [ ] **Step 4: Run document, Unicode, IME, and motion tests**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: PASS.

Run: `rtk cargo test -p waml-markdown-editor --test unicode_ime && rtk cargo test -p waml-markdown-editor --test motion`

Expected: PASS. External replacement cancels IME and cuts motion.

- [ ] **Step 5: Commit host synchronization**

```powershell
rtk git add crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/src/edit.rs crates/waml-markdown-editor/tests/document_ops.rs
rtk git commit -m "feat(markdown): synchronize editor from host snapshots"
```

---

### Task 4: Mount the Editable Source View Through the Existing View Boundary

**Files:**
- Modify: `crates/waml-editor/Cargo.toml`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/documents.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/okf_documents.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/view_history.rs`
- Delete: `crates/waml-editor/src/markdown_surface.rs`
- Create: `crates/waml-editor/tests/markdown_integration.rs`
- Create: `crates/waml-editor/tests/markdown_authority.rs`

**Interfaces:**
- Consumes: Tasks 2-3, `compile_presentation`, `build_layout_document`, `InstalledPresentation`, `LayoutChangeCause`, and `MarkdownEditorRef`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewReconcilePolicy {
    Replace,
    RetainLiveState,
}

pub trait DocView {
    fn route_ui_event(
        &mut self,
        cx: &mut Cx,
        ui: &WidgetRef,
        event: &Event,
    );
    fn reconcile_policy(&self) -> ViewReconcilePolicy {
        ViewReconcilePolicy::Replace
    }
}

#[derive(Default)]
pub struct ViewOutcome {
    pub edit: Option<EditIntent>,
    pub source_edit: Option<ProposedSourceEdit>,
    pub popup: Option<PopupRequest>,
    pub promote_subject: Option<String>,
    pub close_active: bool,
    pub statusbar_dirty: bool,
    pub break_merge_group: bool,
    pub navigation: Option<NavigationIntent>,
    pub view_source: Option<String>,
}
```

- [ ] **Step 1: Write failing mounted SourceView tests**

Create integration tests that mount the production body and assert:

```rust
#[test]
fn typing_in_source_view_emits_exact_edit_and_keeps_the_live_view() {
    let (mut cx, mut app) = mounted_source_app("shop/order.md", "# Order\n");
    let source_tab = app.open_source_for_test(&mut cx, "shop/order");
    app.type_text_for_test(&mut cx, "X");

    let snapshot = app.session_snapshot_for_test();
    let id = snapshot
        .okf_analysis
        .catalog
        .id_for_path(&BundlePath::parse("shop/order.md").unwrap())
        .unwrap();
    assert_eq!(
        snapshot.okf_analysis.markdown_snapshot(id).unwrap().text().shared(),
        "X# Order\n"
    );
    assert_eq!(app.active_tab_id_for_test(), source_tab);
    assert_eq!(
        app.active_source_selection_for_test()
            .primary()
            .cursor
            .offset
            .to_usize(),
        1
    );
}
```

Add mounted tests for tab switching, closing, retained selection/scroll after semantic refresh, app undo/redo, normal click editing, Ctrl/Cmd-click navigation, nested bundle source lookup, and missing source clearing old content into an editable-disabled diagnostic state.

Create `tests/markdown_authority.rs` in this task:

```rust
#[test]
fn production_editor_has_one_markdown_authority() {
    let source = production_rust_sources("crates/waml-editor/src");
    for forbidden in [
        "MarkdownRef",
        "MarkdownAction",
        "as_markdown()",
        "makepad_widgets::Markdown",
        "pulldown_cmark::Parser",
        "regex::Regex",
    ] {
        assert!(
            !source.contains(forbidden),
            "production editor still contains forbidden authority: {forbidden}"
        );
    }
    assert!(!Path::new("crates/waml-editor/src/markdown_surface.rs").exists());
}
```

Implement `production_rust_sources` by recursively reading only `.rs` files under the exact production directory and including each matching path in assertion output. Before implementation the test must report the existing Makepad `MarkdownRef`, `MarkdownAction`, `as_markdown()`, and `markdown_surface.rs` paths; after the atomic Source/Generic migration it must pass.

- [ ] **Step 2: Run mounted tests and verify the read-only path fails**

Run: `rtk cargo test -p waml-editor --test markdown_integration`

Expected: FAIL because the source body still mounts Makepad `Markdown` and cannot emit `ProposedSourceEdit`.

- [ ] **Step 3: Add generic active-view scope routing**

Add `DocView::route_ui_event`. Its default implementation calls:

```rust
ui.handle_event(cx, event, &mut Scope::empty());
```

`SourceView` overrides it. When ready, it passes:

```rust
let mut scope = Scope::with_data(MarkdownEditorScope {
    session: &mut state.session,
});
ui.handle_event(cx, event, &mut scope);
```

`DocumentHost::route_ui_event` delegates to the active view. Replace the direct `self.ui.handle_event(cx, event, &mut Scope::empty())` call in `AppMain::handle_event`. This route handles both input and `Event::Draw`, so the widget sees the same external session for editing and geometry.

- [ ] **Step 4: Preserve live source views during document reconciliation**

`SourceView::reconcile_policy` returns `RetainLiveState`; other current views keep the default `Replace`.

When a prepared tab has the same tab id and the current view returns `RetainLiveState`, `DocumentHost::reconcile_documents` updates tab title/presentation but discards the replacement view. The retained view receives the new immutable snapshot through `after_session_change`.

Change the existing replacement test into two tests: revision-bound diagram views still replace, and a retaining probe keeps its original allocation, selection marker, and scroll marker.

- [ ] **Step 5: Initialize and synchronize `SourceView` by `DocumentId`**

Construct a source view with concept id only for tab identity. On first sync, resolve `DocumentId` and the exact `Arc<MarkdownSyntaxSnapshot>` through `EditorSessionSnapshot::markdown_snapshot`, create `MarkdownDocumentSnapshot`, and create `MarkdownDocumentSession`. Do not obtain live editor syntax from the possibly stale semantic `OkfAnalysis`.

State is explicit:

```rust
enum SourceViewState {
    Uninitialized,
    Ready {
        document: DocumentId,
        session: MarkdownDocumentSession,
        plan: Arc<PresentationPlan>,
        styles: Arc<PresentationStyles>,
        assets: EmbeddedAssets,
    },
    Missing {
        expected_concept: String,
        message: Arc<str>,
    },
}
```

On host acknowledgement, call `synchronize_from_host` with `AcknowledgedLocalEdit`. On application undo/redo, call it with `ApplicationHistory`. On initial load, call `compile_presentation(snapshot, styles, &HighlighterRegistry::default())`, build same-revision layout and an `InstalledPresentation`, and call `install_presentation(..., LayoutChangeCause::InitialLoad)`. The widget owns layout snapshots, motion, and frame scheduling. On `Missing`, drop `Ready`, clear the widget's installed presentation, set read-only, and show `No source for '<concept>'`; never retain prior-tab text.

Replace the one shared live node in this task:

```text
markdown_surface.md := Markdown
    ->
markdown_surface.editor := MarkdownEditor
```

Change `BodyWidgets` to store `MarkdownEditorRef` and expose only `show_markdown_editor`, `markdown_editor`, and editor-specific focus/presentation helpers. Migrate `GenericOkfView` in the same step: it owns a retained `MarkdownDocumentSession`, installs the same snapshot-derived presentation, sets the shared widget read-only, returns `RetainLiveState`, and never emits `source_edit`. Delete `markdown_surface.rs`, its module registration, upstream action helpers, and the Makepad `Markdown` live import now. A shared node cannot support old and new concrete widget references in different views, so this migration must be atomic.

- [ ] **Step 6: Route typed widget actions**

`SourceView::handle` calls `ProposedSourceEdit::from_local(stored_document_id, local)`. The constructor moves the exact `MarkdownSyntaxUpdate`, converts the edit's `Vec<TextChange>` into `Arc<[TextChange]>`, copies `edit.base_revision`, and does not copy or reconstruct source text. It returns the proposal through `ViewOutcome::source_edit`.

For `navigation_request`, query the current `PresentationPlan::links` at the requested source offset. Emit `NavigationIntent::MarkdownLink` with the parsed destination. Do not slice punctuation or run a regex.

`App::apply_view_outcome` calls `EditorSession::promote_source_edit`, immediately completes the source/syntax `SessionChange`, schedules persistence, runs the semantic request, and calls `install_semantic_completion`. A stale promotion logs a status diagnostic and calls `DocumentHost::sync_active` so the view rebases to current host state.

- [ ] **Step 7: Capture exact view-local history**

Replace the old renderer anchor with:

```rust
ViewAnchor::Markdown {
    fragment: Option<String>,
    revision: DocumentRevision,
    selection: SelectionSet,
    scroll: ScrollState,
}
```

Restore only when the source document identity matches. Translate the stored selection through known changes when the revision advanced; otherwise retain the current safe selection and scroll. Fragment navigation resolves through syntax heading/link metadata and then uses `LayoutSnapshot::source_to_point`.

- [ ] **Step 8: Run view, host, navigation, and history tests**

Run: `rtk cargo test -p waml-editor --test markdown_integration`

Expected: PASS.

Run: `rtk cargo test -p waml-editor --test markdown_authority`

Expected: PASS. No production view uses Makepad Markdown after the shared node changes type.

Run: `rtk cargo test -p waml-editor document_host::tests --bin waml-editor && rtk cargo test -p waml-editor source_view::tests --bin waml-editor`

Expected: PASS. Source views retain live state; diagram views retain their existing replacement policy.

- [ ] **Step 9: Commit the editable source surface**

```powershell
rtk git add crates/waml-editor/Cargo.toml crates/waml-editor/src/app.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/document_host.rs crates/waml-editor/src/documents.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/main.rs crates/waml-editor/src/okf_documents.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/view_history.rs crates/waml-editor/src/markdown_surface.rs crates/waml-editor/tests/markdown_integration.rs crates/waml-editor/tests/markdown_authority.rs
rtk git commit -m "feat(editor): mount editable markdown source view"
```

---

### Task 5: Refresh Revisioned Diagnostics and Only Affected Canvas Projections

**Files:**
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/tests/incremental_analysis.rs`
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/markdown_analysis.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/tests/markdown_integration.rs`

**Interfaces:**
- Consumes: syntax `affected_ranges`, WAML island identity, semantic dependency information, and existing canvas `update_scene` behavior.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionFreshness {
    Current,
    RetainedStale { failed_revision: DocumentRevision },
}

#[derive(Clone)]
pub struct RevisionedDiagnostic {
    pub document: DocumentId,
    pub revision: DocumentRevision,
    pub range: TextRange,
    pub source: DiagnosticSource,
    pub severity: Severity,
    pub code: Arc<str>,
    pub message: Arc<str>,
}

pub struct AffectedAnalysis {
    pub documents: Arc<[DocumentId]>,
    pub islands: Arc<[SyntaxIdentity]>,
    pub diagrams: Arc<[Arc<str>]>,
}
```

- [ ] **Step 1: Write failing affected-analysis and stale-island tests**

Extend incremental tests:

```rust
#[test]
fn invalid_edited_island_keeps_unrelated_projection_current() {
    let before = analyzed_two_diagram_fixture();
    let edit = break_only_order_island(&before);
    let after = analyze_promoted(edit, &before);

    assert_eq!(
        after.projection_freshness(order_island()),
        ProjectionFreshness::RetainedStale {
            failed_revision: after.order_revision()
        }
    );
    assert_eq!(
        after.projection_freshness(customer_island()),
        ProjectionFreshness::Current
    );
    assert!(Arc::ptr_eq(
        before.diagram("customer"),
        after.diagram("customer")
    ));
}
```

Add tests that syntax diagnostics update immediately, semantic diagnostics install only with current completion, returning to valid source replaces the stale projection, and diagnostic navigation maps/selects the current source range.

- [ ] **Step 2: Run focused analysis and editor tests**

Run: `rtk cargo test -p waml --test incremental_analysis invalid_edited_island`

Expected: FAIL because projection freshness and affected metadata are absent.

Run: `rtk cargo test -p waml-editor --test markdown_integration diagnostic_`

Expected: FAIL because source diagnostics are not revision-bound and cannot select a current range.

- [ ] **Step 3: Publish affected dependency metadata**

During analysis, start from promoted Markdown `affected_ranges`, map them to owning WAML islands, follow semantic dependencies, and return sorted unique document/island/diagram sets.

If one edited island cannot lower, keep its previous projection with `RetainedStale`; do not retain it silently. Attach one diagnostic to the current source revision. Unrelated island and diagram `Arc` values remain unchanged.

- [ ] **Step 4: Preserve unaffected canvas state**

`SessionChange` carries `affected_documents` and `affected_diagrams`. `ClassDiagramView::after_session_change`:

- does nothing when its key is not affected;
- rebuilds only its affected scene;
- calls existing `ClassDiagramSurface::update_scene`, not `set_scene`, to preserve viewport and stable node geometry;
- preserves selection when the selected key survives;
- marks retained stale nodes desaturated and adds an explicit stale badge.

Apply the same affected-key guard to `BehaviorDocView`; retain its viewport/selection for surviving identities.

- [ ] **Step 5: Draw revisioned diagnostics in SourceView**

Build `PresentedDiagnostic` values only when their `document` and `revision` match the installed `MarkdownDocumentSnapshot`. Syntax diagnostics come from the current syntax snapshot immediately. Semantic diagnostics come from the newest installed semantic snapshot.

Add `NavigationIntent::SourceRange { document, revision, range }`. The app activates the source tab, maps the range to the current revision through known changes, and asks `SourceView` to install the selection. If mapping fails, leave selection unchanged and publish a status diagnostic.

- [ ] **Step 6: Run analysis, canvas, and diagnostic tests**

Run: `rtk cargo test -p waml --test incremental_analysis`

Expected: PASS.

Run: `rtk cargo test -p waml-editor --test markdown_integration && rtk cargo test -p waml-editor class_diagram_view::tests --bin waml-editor && rtk cargo test -p waml-editor behavior_doc_view::tests --bin waml-editor`

Expected: PASS. Unaffected scenes retain identity/viewport, and invalid edited islands do not clear unrelated canvas content.

- [ ] **Step 7: Commit affected analysis and canvas refresh**

```powershell
rtk git add crates/waml/src/analysis.rs crates/waml/src/uml/analysis.rs crates/waml/tests/incremental_analysis.rs crates/waml-editor/src/editor_session.rs crates/waml-editor/src/markdown_analysis.rs crates/waml-editor/src/app/actions.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/behavior_doc_view.rs crates/waml-editor/src/source_view.rs crates/waml-editor/tests/markdown_integration.rs
rtk git commit -m "feat(editor): refresh affected markdown projections"
```

---

### Task 6: Make Save and External Replacement Revision-Safe

**Files:**
- Modify: `crates/waml-editor/src/editor_session.rs`
- Modify: `crates/waml-editor/src/native_save.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/tests/markdown_integration.rs`

**Interfaces:**
- Consumes: immutable `EditorSessionSnapshot`, existing three-second save debounce, native conflict detection, browser URL-fragment persistence, and `HostSnapshotCause::ExternalReplacement`.
- Produces:

```rust
#[derive(Clone)]
pub struct SaveTicket {
    pub snapshot: Arc<EditorSessionSnapshot>,
    pub revision: u64,
    pub history_state: HistoryStateId,
}

pub struct SaveCompletion {
    pub revision: u64,
    pub history_state: HistoryStateId,
    pub result: Result<(), String>,
}

pub enum ExternalReplacement {
    Installed(SessionChange),
    Conflict { dirty_revision: u64 },
    IgnoredStale,
}
```

- [ ] **Step 1: Write failing save-ticket and external-replacement tests**

Add:

```rust
#[test]
fn old_save_completion_cannot_clear_new_literal_source() {
    let mut session = edited_invalid_source_session();
    let old = session.save_ticket().unwrap();
    promote_one_more_character(&mut session);
    let current = session.snapshot();

    assert!(!session.finish_save(SaveCompletion {
        revision: old.revision,
        history_state: old.history_state,
        result: Ok(()),
    }));
    assert_eq!(session.snapshot().dirty_revision, Some(current.revision));
    assert!(session.snapshot().source.document(&path("order.md")).unwrap().text().contains("["));
}
```

Add native and browser tests for exact invalid source bytes, dirty external replacement conflict, clean external replacement install, stale external replacement ignore, and selection/scroll mapping after an accepted replacement.

- [ ] **Step 2: Run save and reload tests**

Run: `rtk cargo test -p waml-editor native_save::tests --bin waml-editor`

Expected: FAIL because saving still borrows a transient snapshot and has no completion ticket.

Run: `rtk cargo test -p waml-editor --test markdown_integration external_`

Expected: FAIL because external replacement has no revision/conflict result.

- [ ] **Step 3: Save an owned immutable ticket**

`EditorSession::save_ticket` returns `None` when clean. Otherwise it captures the current snapshot `Arc`, session revision, and history state.

Change native persistence to:

```rust
pub(crate) fn save_ticket_atomic(
    root: &Path,
    ticket: &SaveTicket,
) -> io::Result<SaveCompletion>;
```

Write `ticket.snapshot.source` against `ticket.snapshot.persisted_source`. Browser persistence encodes `ticket.snapshot.source`. Both return the ticket identifiers unchanged.

`finish_save` reports the error, but clears dirty state only when current revision and history state still equal the completion. An older successful completion cannot clone its source into the current persisted baseline.

- [ ] **Step 4: Route external replacement through existing conflict policy**

Add `EditorSession::replace_external(document, base_revision, text)`. Reject stale base revision. If the session is dirty, return `Conflict` and let the current save-before-replacement policy decide; never overwrite live text silently.

For a clean accepted replacement, create the next document revision, parse once at the application syntax ingress, publish source/syntax, and send `ExternalReplacement` to the live SourceView. The view translates state through the exact full replacement change and cuts motion.

Initial bundle open remains `InitialLoad` and clean. A persistence failure keeps dirty state and the existing visible save error.

- [ ] **Step 5: Run persistence, app, and integration tests**

Run: `rtk cargo test -p waml-editor native_save::tests --bin waml-editor`

Expected: PASS.

Run: `rtk cargo test -p waml-editor app::tests --bin waml-editor && rtk cargo test -p waml-editor --test markdown_integration`

Expected: PASS. Existing save-before-load and failed-save-blocks-close tests remain green.

- [ ] **Step 6: Commit revision-safe persistence**

```powershell
rtk git add crates/waml-editor/src/editor_session.rs crates/waml-editor/src/native_save.rs crates/waml-editor/src/app.rs crates/waml-editor/tests/markdown_integration.rs
rtk git commit -m "fix(editor): guard markdown save and reload revisions"
```

---

### Task 7: Connect Snapshot Code Highlighting and Revision-Bound Assets

**Files:**
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/tests/incremental_analysis.rs`
- Create: `crates/waml-editor/src/markdown_hosts.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/tests/markdown_integration.rs`

**Interfaces:**
- Consumes: the presentation plan's final three-argument `compile_presentation`, exact `CodeHighlightHost`, `CodeHighlightRequest`, `MarkdownAssetHost`, `ImageAssetRequest`, `ImageAssetEvent`, and `EmbeddedAssets`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WamlCodeRole {
    Keyword,
    Type,
    Property,
    String,
    Number,
    Comment,
    Punctuation,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WamlCodeSpan {
    pub range: TextRange,
    pub role: WamlCodeRole,
}

impl OkfAnalysis {
    pub fn code_spans(
        &self,
        owner: SyntaxIdentity,
        content_range: TextRange,
    ) -> Option<Arc<[WamlCodeSpan]>>;
}

pub struct WamlCodeHighlightHost {
    snapshot: Arc<EditorSessionSnapshot>,
}

pub type SharedMarkdownAssetHost = Rc<RefCell<EditorMarkdownAssetHost>>;

pub struct EditorMarkdownAssetHost {
    policy: MarkdownAssetPolicy,
    completed: Vec<ImageAssetEvent>,
}

#[derive(Clone)]
pub enum MarkdownAssetPolicy {
    Native { canonical_root: Arc<PathBuf> },
    BrowserBundle,
}
```

- [ ] **Step 1: Write failing host and composition tests**

Test WAML fenced content, unknown languages, failed hosts, stale image completion, path traversal, remote URL denial, and accepted local image:

```rust
#[test]
fn app_highlighter_uses_the_matching_immutable_island_snapshot() {
    let snapshot = analyzed_waml_fence_fixture();
    let request = request_for_only_waml_fence(&snapshot);
    let host = WamlCodeHighlightHost::new(snapshot.clone());
    let spans = host.highlight(&request).unwrap();

    assert!(spans.iter().all(|span| request.content_range.contains_range(span.range)));
    assert!(spans.iter().any(|span| span.role == CodeTokenRole::Keyword));
    assert_eq!(request.revision, snapshot.document_revision(request.owner).unwrap());
}
```

For assets, canonicalize the native root and candidate parent, reject candidates outside the root, and reject `http`, `https`, and protocol-relative destinations. Browser bundle policy returns a failed event because the current share payload contains Markdown documents only.

- [ ] **Step 2: Run focused host tests**

Run: `rtk cargo test -p waml-editor --test markdown_integration host_`

Expected: FAIL because application host implementations are absent.

- [ ] **Step 3: Implement one shared WAML code-role query and the editor adapter**

Implement `OkfAnalysis::code_spans` in the framework crate so both the in-app host and LSP depend inward on one renderer-free semantic query. Resolve `owner` through `MarkdownSyntaxQueries::island`, require exact content range, and traverse the matching immutable `SyntaxTree<UmlLanguage>` tokens. Map:

- keyword tokens to `Keyword`;
- `TypeToken` and `TypeReference` tokens to `Type`;
- identifiers in property/slot positions to `Property`;
- quoted/layout quote tokens to `String`;
- numeric margin/multiplicity tokens to `Number`;
- bad/recovery tokens to `Invalid`;
- punctuation/delimiters/arrows to `Punctuation`.

Return sorted, non-overlapping absolute document ranges clipped to `content_range`. `WamlCodeHighlightHost` requires exact request/snapshot revision and maps `WamlCodeRole` to `CodeTokenRole`. It does not traverse syntax independently, parse text, or call LSP. `SourceView` creates one registry for the installed snapshot, registers this host under `waml`, and calls the existing `compile_presentation(snapshot, styles, &registry)` once.

- [ ] **Step 4: Implement the application asset host and explicit completion ingress**

Resolve relative paths from the owning Markdown document's `BundlePath` directory. Native policy returns `ApprovedImageSource::CanonicalFile` only after containment validation. Remote targets and browser-local files return `ImageAssetEvent::Failed` with a stable message.

`App` constructs `SharedMarkdownAssetHost` from the bundle root/browser policy and injects a clone when it constructs `SourceView` or `GenericOkfView`; the views never construct policy from source text. `EditorMarkdownAssetHost` implements all three trait methods, including `drain_events`.

`SourceView` calls `EmbeddedAssets::reconcile` after every presentation plan, drains only `MarkdownAssetHost::drain_events`, and accepts only request/revision/item matches through `EmbeddedAssets::apply_event`. Map `PresentationItemId { owner, fragment_ordinal, .. }` to `LayoutElementId { owner, fragment_ordinal }`; a changed accepted measurement rebuilds `InstalledPresentation` and calls `install_presentation(..., LayoutChangeCause::ImageMeasurement(layout_id))`. The widget's shared motion controller owns the transition. `GenericOkfView` uses the same ingress in read-only mode.

- [ ] **Step 5: Run host, presentation, layout, and mounted tests**

Run: `rtk cargo test -p waml-markdown-editor --test highlighting && rtk cargo test -p waml-markdown-editor --test assets && rtk cargo test -p waml-markdown-editor --test presentation_layout`

Expected: PASS.

Run: `rtk cargo test -p waml-editor --test markdown_integration`

Expected: PASS. Stale image events cause no redraw/relayout, and source text is unchanged in loading, ready, and failed states.

- [ ] **Step 6: Commit application hosts**

```powershell
rtk git add crates/waml/src/analysis.rs crates/waml/tests/incremental_analysis.rs crates/waml-editor/src/markdown_hosts.rs crates/waml-editor/src/main.rs crates/waml-editor/src/source_view.rs crates/waml-editor/src/generic_okf_view.rs crates/waml-editor/src/app.rs crates/waml-editor/tests/markdown_integration.rs
rtk git commit -m "feat(editor): connect markdown highlight and asset hosts"
```

---

### Task 8: Move LSP Markdown Features onto Shared Snapshots

**Files:**
- Modify: `crates/waml-cli/src/lsp/bundle.rs`
- Modify: `crates/waml-cli/src/lsp/map.rs`
- Create: `crates/waml-cli/src/lsp/query.rs`
- Modify: `crates/waml-cli/src/lsp/server.rs`
- Modify: `crates/waml-cli/src/lsp/mod.rs`
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Consumes: `LspAnalysisState` immutable snapshots, `MarkdownSyntaxSnapshot::queries()`, `DocumentVersion::line_index`, and current UML navigation/diagnostics.
- Produces:

```rust
impl LspAnalysisState {
    pub fn document_symbols(&self, physical: &Path) -> Option<Vec<lsp::DocumentSymbol>>;
    pub fn semantic_tokens(&self, physical: &Path) -> Option<lsp::SemanticTokens>;
    pub fn document_links(&self, physical: &Path) -> Option<Vec<lsp::DocumentLink>>;
    pub fn definition(
        &self,
        physical: &Path,
        position: lsp::Position,
    ) -> Option<lsp::Location>;
}
```

- [ ] **Step 1: Write failing shared-snapshot LSP tests**

Add bundle tests that compare one in-app classification fixture with LSP semantic token ranges and roles. Add end-to-end requests for document symbols, definition, links, and semantic tokens.

Add an astral Unicode round trip:

```rust
#[test]
fn utf8_snapshot_range_round_trips_through_lsp_utf16_position() {
    let state = state_with("unicode.md", "# 😀 [目标](./next.md)\n");
    let document = state.document_for_test("unicode.md");
    let range = range_of(document.text(), "目标");
    let lsp_range = to_lsp_range(range, document.line_index()).unwrap();
    assert_eq!(
        from_lsp_range(lsp_range, document.line_index()).unwrap(),
        range
    );
}
```

- [ ] **Step 2: Run LSP tests and verify missing capabilities**

Run: `rtk cargo test -p waml-cli lsp:: --lib`

Expected: FAIL because only full-text synchronization and semantic diagnostics are currently exposed.

- [ ] **Step 3: Centralize position conversion**

In `map.rs`, implement checked:

```rust
pub fn to_lsp_position(
    offset: TextSize,
    index: &LineIndex,
) -> Result<lsp::Position, PositionError>;

pub fn from_lsp_position(
    position: lsp::Position,
    index: &LineIndex,
) -> Result<TextSize, PositionError>;

pub fn to_lsp_range(
    range: TextRange,
    index: &LineIndex,
) -> Result<lsp::Range, PositionError>;

pub fn from_lsp_range(
    range: lsp::Range,
    index: &LineIndex,
) -> Result<TextRange, PositionError>;
```

All diagnostics, links, definitions, and tokens use these functions. Invalid UTF-16 positions return no result and never round.

- [ ] **Step 4: Query syntax and semantic products without parsing**

In `query.rs`, build:

- document symbols from parsed headings and WAML section owners;
- document links from parsed Markdown link/image metadata;
- definitions from parsed WAML symbol links and existing navigation resolution;
- semantic token spans from `MarkdownSyntaxQueries` plus `OkfAnalysis::code_spans`; the CLI maps shared `WamlCodeRole` values to the LSP legend while the editor adapter maps them to `CodeTokenRole`.

Map marker, heading, link, code, string, comment, keyword, type, property, number, and invalid roles into a fixed semantic-token legend. Sort absolute ranges, split multiline spans by `LineIndex`, and delta-encode only after UTF-16 conversion.

- [ ] **Step 5: Publish only current LSP results**

Keep `LspAnalysisState` immutable and compare-and-swap installation. Capture the state `Arc` for each request. Before returning or publishing, require it still equals the server's current `Arc`; otherwise discard the response.

Advertise `document_symbol_provider`, `definition_provider`, `document_link_provider`, and `semantic_tokens_provider`. Keep `TextDocumentSyncKind::FULL` for this rollout because the client version and immutable full text already provide a correct ingress; internal syntax analysis remains incremental from the previous shared snapshot. Do not add completion until a concrete WAML completion producer exists.

- [ ] **Step 6: Run LSP unit and end-to-end tests**

Run: `rtk cargo test -p waml-cli lsp:: --lib`

Expected: PASS.

Run: `rtk cargo test -p waml-cli --test lsp_e2e`

Expected: PASS. Diagnostics, symbols, links, definitions, and tokens are revision-current and Unicode-exact.

- [ ] **Step 7: Commit shared-snapshot LSP features**

```powershell
rtk git add crates/waml-cli/src/lsp/bundle.rs crates/waml-cli/src/lsp/map.rs crates/waml-cli/src/lsp/query.rs crates/waml-cli/src/lsp/server.rs crates/waml-cli/src/lsp/mod.rs crates/waml-cli/tests/lsp_e2e.rs
rtk git commit -m "feat(lsp): query shared markdown snapshots"
```

---

### Task 9: Verify the Single Markdown Runtime and Full Rollout

**Files:**
- Modify: `crates/waml-editor/tests/markdown_authority.rs`
- Modify: `crates/waml-editor/tests/README.md`

**Interfaces:**
- Consumes: all prior tasks and `MarkdownDocumentSession` read-only mode.
- Produces: one production Markdown widget/parser authority and verification evidence; no new production API.

- [ ] **Step 1: Extend the authority test to the completed rollout**

Keep the production-editor scan created in Task 4 and add:

```rust
#[test]
fn one_shared_waml_widget_serves_source_and_read_only_generic_views() {
    let app = fs::read_to_string("crates/waml-editor/src/app.rs").unwrap();
    let source = fs::read_to_string("crates/waml-editor/src/source_view.rs").unwrap();
    let generic = fs::read_to_string("crates/waml-editor/src/generic_okf_view.rs").unwrap();
    assert_eq!(app.matches("editor := MarkdownEditor").count(), 1);
    assert!(source.contains("MarkdownDocumentSession"));
    assert!(generic.contains("MarkdownDocumentSession"));
    assert!(generic.contains("set_read_only"));
    assert!(!generic.contains("source_edit: Some"));
}
```

Also scan `crates/waml-markdown-editor/src` and reject `pulldown_cmark`, `regex`, `makepad_widgets::Markdown`, `MarkdownRef`, and `MarkdownAction`. The only Markdown parser production imports must remain under `crates/waml-syntax`.

- [ ] **Step 2: Run the completed authority test**

Run: `rtk cargo test -p waml-editor --test markdown_authority`

Expected: PASS. Task 4 already made the Source/Generic migration atomic; this task must not perform a second runtime cutover.

- [ ] **Step 3: Run focused workspace tests**

Run: `rtk cargo test -p waml-editor --test markdown_authority`

Expected: PASS.

Run: `rtk cargo test -p waml-markdown-editor && rtk cargo test -p waml-editor && rtk cargo test -p waml-cli`

Expected: PASS.

- [ ] **Step 4: Run formatting, workspace, and lint gates**

Run: `rtk cargo fmt --check`

Expected: PASS with no diff.

Run: `rtk cargo test --workspace`

Expected: PASS.

Run: `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: PASS with no warnings.

- [ ] **Step 5: Capture native HiDPI evidence**

Run the editor with the integration fixture, then capture:

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out target/markdown-editor-integration.png -Process waml-editor
```

Expected: `target/markdown-editor-integration.png` shows literal visible Markdown markers, a mixed-metric heading, current selection/caret, a diagnostic, a fenced WAML block, and an image placeholder without canvas bleed-through.

Record in `crates/waml-editor/tests/README.md` the command, fixture path, DPI scale, window size, reduced-motion state, and the successful workspace commands. Do not claim a screenshot case that was not captured.

- [ ] **Step 6: Commit rollout evidence**

```powershell
rtk git add crates/waml-editor/tests/markdown_authority.rs crates/waml-editor/tests/README.md
rtk git commit -m "test(editor): verify markdown rollout authority"
```

## Plan Self-Review Record

- Spec coverage: Tasks 1-3 cover exact local edits, immutable workspace snapshots, revision validation, and no duplicate parsing. Task 4 atomically migrates SourceView and GenericOkfView, removes the legacy runtime, and covers tab identity, selection/scroll, click behavior, and missing source. Task 5 covers current diagnostics, affected projections, stale-island fallback, canvas stability, and diagnostic navigation. Task 6 covers exact literal persistence, dirty revisions, stale save completion, external replacement, and conflict routing. Task 7 covers the shared code-role query plus application code-highlight and asset host ingress. Task 8 covers shared-snapshot LSP diagnostics, symbols, navigation, links, semantic tokens, and UTF-16 conversion. Task 9 verifies authority and records native/workspace evidence without a second cutover.
- Duplicate-parsing guard: Task 1 records mutually exclusive parse/reparse/promotion counters and requires exact syntax `Arc` identity. Task 2 forbids syntax parsing in application promotion. Task 3 forbids parsing during host synchronization. Tasks 4 and 9 scan production editor code for parser imports.
- Stale-promotion guard: Task 2 validates document/base/update/source identities and rejects old semantic completion. Task 6 requires revision plus history-state identity for save completion. Task 7 requires request/revision/item identity for image events. Task 8 compares the request/publication snapshot `Arc` with current LSP state.
- Type consistency: `DocumentId`, `DocumentRevision`, `TextChange`, `MarkdownSyntaxSnapshot`, `MarkdownSyntaxUpdate`, `ProposedMarkdownEdit`, `ProposedSourceEdit`, `MarkdownDocumentSession`, `LayoutDocument`, `LayoutSnapshot`, `CodeHighlightHost`, and `MarkdownAssetHost` keep the same ownership and names throughout.
- Placeholder scan: the plan contains no deferred implementation marker, unnamed validation work, or unspecified test step.
- Cross-plan alignment: the presentation plan's final `compile_presentation(snapshot, styles, highlighters)` signature is used directly. Task 7 supplies the application registry and host without a second segmentation pass.
