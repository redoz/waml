# Transition Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add lossless, typed, editable, diagnosable, indexed, and navigable `traces` relationships to activity and state-machine transitions.

**Architecture:** Extend the shared flow-transition syntax with a fixed trace container that accepts one-link inline and indented clauses. Preserve authored syntax in the declared tree, resolve targets into public transition metadata, expose that metadata through indexes and the transition inspector, and keep it out of flow solving and geometry.

**Tech Stack:** Rust, `waml-syntax` green/red trees, WAML UML analysis and edit lowering, Makepad editor widgets, Cargo tests.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\worktree-20260810`.
- Prefix shell commands with `rtk`.
- Use ASD-STE100 Simplified Technical English.
- Each `traces` clause contains exactly one Markdown link.
- Accept inline, repeated inline, indented, and mixed authored forms.
- Canonical formatting keeps one trace inline and puts two or more traces on separate indented lines.
- Preserve invalid and unresolved traces in typed syntax and declared data.
- Accept HTTPS targets without network validation.
- Do not change execution semantics, solver inputs, diagram layout, routing, or edge labels.

---

### Task 1: Lossless transition trace syntax

**Files:**
- Modify: `crates/waml/src/uml/syntax/kind.rs`
- Modify: `crates/waml/src/uml/syntax/ast.rs`
- Modify: `crates/waml/src/uml/syntax/mod.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Test: `crates/waml/tests/uml_behavior_syntax.rs`

**Interfaces:**
- Produces: `FlowTraceSyntax`, `FlowTracesSyntax`, `FlowTransitionSyntax::traces()`.
- Produces: fixed transition slots `TRACES_SLOT`, `RECOVERY_SLOT`, and `NEWLINE_SLOT`.
- Consumes: existing `behavior_link`, `behavior_bounds`, `flow_block`, and `FlowTransitionSyntax` accessors.

- [ ] **Step 1: Add failing losslessness and fixed-slot tests**

Add tests with this authored input and assert exact `write_to_string()` output:

```rust
let authored = "---\r\ntype: uml.StateMachine\r\n---\r\n# Sign in\r\n\r\n## Nodes\r\n### Idle\r\n- on `authenticated` transitions to SignedIn traces [AUTH-OIDC-004](./sign-in-behavior.md#auth-oidc-004)\r\n- on `retry` transitions to Idle traces [Retry](#retry) traces [Policy](https://example.com/policy)\r\n- on `fallback` transitions to SignedIn\r\n  traces [Local](#fallback)\r\n  traces [External](https://openid.net/specs/openid-connect-core-1_0.html)\r\n### final SignedIn\r\n";
```

Assert that every trace is a `FlowTraceSyntax`, that every transition has one
`FlowTracesSyntax` at `TRACES_SLOT`, and that existing target, carries, effect,
recovery, and newline occurrence indices stay stable relative to their named
constants.

- [ ] **Step 2: Run the syntax tests and confirm the red state**

Run:

```powershell
rtk cargo test -p waml --test uml_behavior_syntax transition_trace -- --nocapture
```

Expected: compilation fails because the trace syntax types and accessors do not exist.

- [ ] **Step 3: Add syntax kinds and typed accessors**

Add `FlowTraces` and `FlowTrace` node kinds. Define these interfaces:

```rust
pub struct FlowTracesSyntax(pub(crate) SyntaxNode<UmlLanguage>);
pub struct FlowTraceSyntax(pub(crate) SyntaxNode<UmlLanguage>);

impl FlowTransitionSyntax {
    pub const TRACES_SLOT: usize = 8;
    pub const RECOVERY_SLOT: usize = 9;
    pub const NEWLINE_SLOT: usize = 10;

    pub fn traces(&self) -> impl Iterator<Item = FlowTraceSyntax> + '_;
}

impl FlowTraceSyntax {
    pub const KEYWORD_SLOT: usize = 0;
    pub const LINK_SLOT: usize = 1;
    pub const RECOVERY_SLOT: usize = 2;
    pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>>;
}
```

- [ ] **Step 4: Group indented trace lines with their transition**

Change `flow_block` from a simple `for` loop to an indexed line walk. When a
line parses as a transition, collect immediately following non-bullet lines
whose trimmed content starts with `traces` and whose indentation is deeper
than the transition bullet. Pass those ranges to `flow_transition`.

Do not collect a trace after a blank line, another bullet, or a heading. Send
an orphan `traces` line through normal recovery with a `MalformedFlow`
diagnostic.

- [ ] **Step 5: Parse inline and indented trace clauses**

Before transition recovery, parse zero or more inline `traces` clauses. Each
clause must call `behavior_link` once and create one `FlowTrace` child. Put all
children in one fixed `FlowTraces` node.

For indented clauses, include the prior line ending and each clause's leading
indentation and ending in the `FlowTraces` subtree so `write_to_string()` is
byte-for-byte lossless. Keep the final fixed newline slot present or missing
according to the existing fixed-slot convention.

- [ ] **Step 6: Add recovery tests**

Cover a missing link, malformed link, empty href, orphan trace, and malformed
trace followed by a valid sibling transition. Assert that recovery stops at
the sibling boundary and that all authored text survives serialization.

- [ ] **Step 7: Run Task 1 tests**

```powershell
rtk cargo test -p waml --test uml_behavior_syntax
```

Expected: all `uml_behavior_syntax` tests pass.

- [ ] **Step 8: Commit Task 1**

```powershell
rtk git add crates/waml/src/uml/syntax crates/waml/tests/uml_behavior_syntax.rs
rtk git commit -m "feat(syntax): parse transition traces"
```

---

### Task 2: Declared traces, target resolution, and diagnostics

**Files:**
- Modify: `crates/waml/src/uml/declared.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/diagnostic.rs`
- Modify: `crates/waml/src/analysis.rs`
- Test: `crates/waml/tests/semantic_diagnostics.rs`
- Test: `crates/waml/tests/uml_behavior_syntax.rs`
- Test: `crates/waml/tests/serde_shape.rs`

**Interfaces:**
- Consumes: `FlowTransitionSyntax::traces()` from Task 1.
- Produces: `DeclaredFlowTrace`, `TransitionTrace`, `TraceTarget`, and `FlowEdge::traces`.
- Produces: reusable `resolve_trace_target` with no editor dependency.

- [ ] **Step 1: Add failing declared-model and resolution tests**

Test these targets in one bundle:

```text
./sign-in-behavior.md
./sign-in-behavior.md#auth-oidc-004
#local-claim
https://openid.net/specs/openid-connect-core-1_0.html
./missing.md
./sign-in-behavior.md#missing-fragment
mailto:owner@example.com
```

Assert ordered declared traces, exact labels and hrefs, resolved target kinds,
and exact diagnostic ranges. Assert that unresolved entries remain present.

- [ ] **Step 2: Run the semantic tests and confirm the red state**

```powershell
rtk cargo test -p waml --test semantic_diagnostics transition_trace -- --nocapture
rtk cargo test -p waml --test uml_behavior_syntax transition_trace -- --nocapture
```

Expected: compilation fails because trace model fields do not exist.

- [ ] **Step 3: Add declared and public types**

Add these data shapes, with the repository's existing serde conventions:

```rust
pub struct DeclaredFlowTrace {
    pub syntax: FlowTraceSyntax,
    pub label: DeclaredField<UmlLanguage, String>,
    pub href: DeclaredField<UmlLanguage, String>,
}

pub enum TraceTarget {
    InternalDocument { concept_id: String },
    InternalFragment { concept_id: String, fragment: String },
    Https { url: String },
    Unresolved { href: String },
    Invalid { href: String },
}

pub struct TransitionTrace {
    pub label: String,
    pub href: String,
    pub target: TraceTarget,
    pub source_path: String,
    pub source_range: TextRange,
}
```

Add `pub traces: Arc<[DeclaredFlowTrace]>` to `DeclaredFlowTransition` and
`pub traces: Vec<TransitionTrace>` to `FlowEdge`, with serde default and empty
skip behavior.

- [ ] **Step 4: Project trace links without dropping invalid entries**

In `declared_flow_transition`, use `link_parts` for every `FlowTraceSyntax`.
Represent missing and malformed pieces with `DeclaredField::Incomplete` or
`DeclaredField::Invalid`; do not use `filter_map` to remove them.

- [ ] **Step 5: Resolve internal and HTTPS targets**

Add a core resolver that:

```rust
fn resolve_trace_target(
    context: &DomainAnalysisContext<'_>,
    referring_path: &str,
    href: &str,
) -> Result<TraceTarget, TraceResolutionError>;
```

Use `okf::resolve_href` and the bundle's path-to-concept mapping for internal
documents. Validate fragments against headings and addressable WAML element
identities from the target document's Markdown snapshot. Reuse one shared
fragment normalization helper; move the LSP's `heading_slug` logic into core
if no shared helper exists.

Accept only a well-formed absolute `https` URL with a host. Return a typed
unsupported-scheme error for every other scheme. Do not perform network I/O.

- [ ] **Step 6: Add diagnostic codes and lower traces to flow edges**

Add distinct codes for missing trace documents, unresolved trace fragments,
malformed trace targets, and unsupported trace schemes. Anchor each diagnostic
to the href token range when available.

In `lower_flow_behavior`, resolve every declared trace and append its
`TransitionTrace` to the matching `FlowEdge`. Do not use traces when computing
edge kind, `from`, `to`, label fields, or solver input.

- [ ] **Step 7: Verify serde compatibility**

Add a serde shape test that accepts an absent `traces` field and emits no
field for an empty collection. Assert the exact JSON shape for internal,
external, and unresolved trace records.

- [ ] **Step 8: Run Task 2 tests**

```powershell
rtk cargo test -p waml --test semantic_diagnostics
rtk cargo test -p waml --test uml_behavior_syntax
rtk cargo test -p waml --test serde_shape
```

- [ ] **Step 9: Commit Task 2**

```powershell
rtk git add crates/waml/src crates/waml/tests
rtk git commit -m "feat(model): resolve transition traces"
```

---

### Task 3: Canonical formatting and structural trace operations

**Files:**
- Modify: `crates/waml/src/uml/format.rs`
- Modify: `crates/waml/src/uml/ops.rs`
- Modify: `crates/waml/src/uml/lower.rs`
- Modify: `crates/waml/src/uml/selector.rs`
- Modify: `crates/waml/src/uml/rename.rs`
- Test: `crates/waml/tests/formatter_actions.rs`
- Test: `crates/waml/tests/uml_lowering_order.rs`
- Test: `crates/waml/tests/uml_behavior_syntax.rs`

**Interfaces:**
- Consumes: trace syntax and declared trace ranges from Tasks 1 and 2.
- Produces: `TransitionSelector`, `TraceEdit`, and `Op::EditTransitionTraces`.

- [ ] **Step 1: Add failing formatter and edit-operation tests**

Cover:

- one indented trace formats inline;
- two inline traces format as indented clauses;
- mixed syntax formats as ordered indented clauses;
- formatting is idempotent;
- add, update, remove, and reorder preserve unrelated transition text;
- the selected occurrence changes when two parallel transitions share source
  and target;
- rename rewrites the document part and preserves `#fragment`.

- [ ] **Step 2: Run the focused tests and confirm the red state**

```powershell
rtk cargo test -p waml --test formatter_actions transition_trace -- --nocapture
rtk cargo test -p waml --test uml_lowering_order transition_trace -- --nocapture
```

- [ ] **Step 3: Canonicalize trace placement**

Extend `canonical_flow_lines` with transition grouping. Parse trace clauses
through typed analysis, not a Markdown Notes convention. Emit:

```rust
match traces.len() {
    0 => transition_line,
    1 => format!("{transition_line} traces {}", traces[0].authored_link()),
    _ => transition_line + &traces.iter()
        .map(|trace| format!("\n  traces {}", trace.authored_link()))
        .collect::<String>(),
}
```

Apply the existing `normalize_links` behavior to each link.

- [ ] **Step 4: Add occurrence-safe transition selectors and operations**

Define:

```rust
pub struct TransitionSelector {
    pub behavior: String,
    pub source_node: String,
    pub occurrence: usize,
}

pub enum TraceEdit {
    Insert { index: usize, label: String, href: String },
    Update { index: usize, label: String, href: String },
    Remove { index: usize },
    Move { from: usize, to: usize },
}
```

Add an `Op` variant that lowers one `TraceEdit` to an `EditBatch`. Validate
labels and hrefs before lowering. Insert the first trace inline, append later
traces as indented clauses, and use syntax ranges for update and removal.

- [ ] **Step 5: Extend rename and move rewriting**

Enumerate trace link nodes with other typed href referrers. Rewrite only the
document component through `rewritten_href`; preserve any exact fragment
suffix and unrelated authored spelling.

- [ ] **Step 6: Run Task 3 tests**

```powershell
rtk cargo test -p waml --test formatter_actions
rtk cargo test -p waml --test uml_lowering_order
rtk cargo test -p waml --test uml_behavior_syntax
```

- [ ] **Step 7: Commit Task 3**

```powershell
rtk git add crates/waml/src/uml crates/waml/tests
rtk git commit -m "feat(edit): manage transition traces"
```

---

### Task 4: Typed trace indexes and validator access

**Files:**
- Modify: `crates/waml/src/analysis.rs`
- Modify: `crates/waml/src/index_md.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Test: `crates/waml/tests/semantic_diagnostics.rs`
- Test: `crates/waml/tests/incremental_analysis.rs`

**Interfaces:**
- Consumes: `FlowEdge::traces` from Task 2.
- Produces: `TraceRecord`, `Analysis::traces_from`, and `Analysis::traces_to`.

- [ ] **Step 1: Add failing index tests**

Assert authored order, outgoing internal and HTTPS entries, reverse incoming
internal entries, and retained unresolved entries. Update a target document
incrementally and assert that fragment status and reverse indexes refresh.

- [ ] **Step 2: Run the index tests and confirm the red state**

```powershell
rtk cargo test -p waml --test incremental_analysis transition_trace -- --nocapture
```

- [ ] **Step 3: Add the typed query surface**

Define a record that contains the source behavior, flow-edge key, trace index,
label, href, target, and source range. Expose:

```rust
pub fn traces_from(&self, flow_edge_key: &str) -> &[TraceRecord];
pub fn traces_to(&self, concept_id: &str, fragment: Option<&str>) -> Vec<&TraceRecord>;
```

Store unresolved and external records only in the outgoing index. Store
resolved internal records in both indexes.

- [ ] **Step 4: Feed validators from typed records**

Move trace diagnostic iteration behind the same typed record surface. Tests
must prove that a validator can inspect trace kinds without parsing Markdown.

- [ ] **Step 5: Run Task 4 tests**

```powershell
rtk cargo test -p waml --test semantic_diagnostics
rtk cargo test -p waml --test incremental_analysis
```

- [ ] **Step 6: Commit Task 4**

```powershell
rtk git add crates/waml/src crates/waml/tests
rtk git commit -m "feat(index): expose transition traces"
```

---

### Task 5: Transition inspector read model and navigation

**Files:**
- Modify: `crates/waml-editor/src/inspector.rs`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs`
- Modify: `crates/waml-editor/src/navigation.rs`
- Test: inline tests in the same files

**Interfaces:**
- Consumes: `FlowEdge::traces` and typed targets from Task 2.
- Produces: `TraceRow`, `InspectorView::traces`, and a real transition inspector for `Subject::Edge(flow_edge_key)`.

- [ ] **Step 1: Add failing inspector projection tests**

Select a `BehaviorTarget::FlowEdge` and assert that `subject_for_target`
resolves to its flow-edge subject. Assert transition title, kind, all ordered
trace rows, target status, and parallel-edge identity.

- [ ] **Step 2: Run the focused editor tests and confirm the red state**

```powershell
rtk cargo test -p waml-editor inspector::tests::transition_trace -- --nocapture
rtk cargo test -p waml-editor behavior_doc_view::tests::transition -- --nocapture
```

- [ ] **Step 3: Add transition and trace read models**

Define:

```rust
pub enum TraceStatus {
    ResolvedInternal,
    ResolvedExternal,
    Unresolved,
    Invalid,
}

pub struct TraceRow {
    pub label: String,
    pub href: String,
    pub status: TraceStatus,
    pub navigation: Option<NavigationTarget>,
}

pub struct InspectorView {
    // existing fields
    pub traces: Vec<TraceRow>,
}
```

Extend `build_edge_view` to look for an exact `model.flow_edges[].key` before
it parses a class-edge key. Build a transition view from that edge and copy
its traces in order. Keep class-edge behavior unchanged.

- [ ] **Step 4: Reuse normal navigation target rules**

Convert resolved internal targets to `NavigationTarget::Document` with the
fragment and HTTPS targets to `NavigationTarget::ExternalUrl`. Do not add a
second href parser in the inspector.

- [ ] **Step 5: Run Task 5 tests**

```powershell
rtk cargo test -p waml-editor inspector::tests
rtk cargo test -p waml-editor behavior_doc_view::tests
rtk cargo test -p waml-editor navigation::tests
```

- [ ] **Step 6: Commit Task 5**

```powershell
rtk git add crates/waml-editor/src
rtk git commit -m "feat(inspector): show transition traces"
```

---

### Task 6: Inspector trace editing controls

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/editor_session.rs`
- Test: `crates/waml-editor/src/inspector_panel.rs`
- Test: `crates/waml-editor/src/editor_session/tests.rs`
- Test: `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `TraceRow`, `NavigationTarget`, and `Op::EditTransitionTraces`.
- Produces: `InspectorAction::{AddTrace, UpdateTrace, RemoveTrace, MoveTrace, OpenTrace}`.

- [ ] **Step 1: Add failing panel and session tests**

Test section visibility, ordered card content, add/edit/remove/reorder actions,
internal fragment navigation, HTTPS dispatch, unresolved disabled navigation,
and one undo step per trace edit.

- [ ] **Step 2: Run the focused tests and confirm the red state**

```powershell
rtk cargo test -p waml-editor inspector_panel::tests::trace -- --nocapture
rtk cargo test -p waml-editor editor_session::tests::trace -- --nocapture
```

- [ ] **Step 3: Add the Traces section and card controls**

Add a `TRACES` section below relationships. Each card shows label, destination,
status, open, edit, remove, move-up, and move-down controls. Add one section
button that opens an empty label and href editor.

Keep edit state explicit:

```rust
enum TraceEditorState {
    Closed,
    Adding { label: String, href: String },
    Editing { index: usize, label: String, href: String },
}
```

- [ ] **Step 4: Emit typed inspector actions**

Replace the string-only assumption in `InspectorAction` with typed variants.
Include the current `Subject::Edge` key and trace index in every mutating
action. The panel must not modify source text.

- [ ] **Step 5: Dispatch structural edits and navigation**

Map mutating actions to `Op::EditTransitionTraces` through `EditorSession`.
Refresh analysis and preserve editor history grouping. Map `OpenTrace` to the
normal internal navigation command or the existing external URL action.

- [ ] **Step 6: Run Task 6 tests**

```powershell
rtk cargo test -p waml-editor inspector_panel::tests
rtk cargo test -p waml-editor editor_session::tests
rtk cargo test -p waml-editor app::tests::navigation
```

- [ ] **Step 7: Commit Task 6**

```powershell
rtk git add crates/waml-editor/src
rtk git commit -m "feat(inspector): edit transition traces"
```

---

### Task 7: Isolation regressions and full verification

**Files:**
- Modify: `crates/waml/tests/flow_solver_golden.rs`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: evidence that traces do not affect behavior or geometry.

- [ ] **Step 1: Add execution and geometry equivalence tests**

Analyze two equivalent flows, one with traces and one without. Remove only
`FlowEdge::traces` before model comparison, then assert identical flow nodes,
edge semantics, solved routes, labels, and `FlowEdgeGeo` points.

- [ ] **Step 2: Run focused regression suites**

```powershell
rtk cargo test -p waml --test flow_solver_golden
rtk cargo test -p waml-editor behavior_doc_view::tests
```

- [ ] **Step 3: Run formatting and static checks**

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] **Step 4: Run the full workspace test suite**

```powershell
rtk cargo test --workspace --all-features
```

- [ ] **Step 5: Inspect the final diff and TokenSave impact**

```powershell
rtk git diff --check
rtk git status --short
```

Use `tokensave_affected` for the changed files and run any additional tests it
identifies.

- [ ] **Step 6: Commit final verification changes**

```powershell
rtk git add crates/waml crates/waml-editor
rtk git commit -m "test: verify transition trace isolation"
```

---

### Task 8: Review and local integration

**Files:**
- No planned source changes; review findings can modify Task 1-7 files.

- [ ] **Step 1: Review the complete branch**

Review the diff from the branch point for correctness, maintainability,
losslessness, diagnostic precision, parser recovery, edit identity, and UI
state handling. Fix every confirmed issue and rerun its focused test.

- [ ] **Step 2: Repeat final verification after review fixes**

```powershell
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk cargo test --workspace --all-features
rtk git status --short
```

- [ ] **Step 3: Commit review fixes if present**

Use a terse Conventional Commit message that describes the confirmed fix.

- [ ] **Step 4: Integrate with the `integrate` skill**

Fetch optimistically, fast-forward local `main` to `origin/main` when possible,
rebase the feature branch onto local `main`, and fast-forward local `main` to
the feature tip. Never force, never create a merge commit, and do not push.

- [ ] **Step 5: Verify integrated main**

Report the final local `main` SHA and subject, plus any remote divergence or
test limitation.
