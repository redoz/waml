# Folder View as a Middleware Chain — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-08-05-folder-view-middleware-design.md` (approved).
Supersedes the plan `docs/superpowers/plans/2026-08-02-folder-view.md` (deleted in this
branch; nothing from it had landed). Its model-layer material (frontmatter on `Index`,
`render_index`, `ProfileDef`, resolution queries, OKF-substrate concept ops) survives
here largely intact; its `ViewSpec` closed enum and `FolderIndex`-as-the-view-model do not.

**Goal:** A folder's view is a middleware chain over its contents. The terminal
stage — the root view — is the plain OKF listing; every other stage is a lens over
it. The chain answers both *what rows exist* and *which surface renders* a target.
Rows travel up; edit ops travel down to the row's owner.

**Architecture, non-negotiable boundary:** `Projection`, `Row`, `Chain`, `RowPath`,
`RowId`, capabilities, the runner, `CoreExtension`, profiles, and resolution all live
in `crates/waml` (headless — no editor dependency, no makepad, no window, no file I/O
beyond `SourceBundle`). `DocView`, `EditorExtension`, `SurfaceFactory`, widgets, and
the tree behavior live in `crates/waml-editor`. Phases A–C and E-core and F-core are
pure `waml` work, unit-tested with no editor. Do not blur this: it is the entire point
of the Extension split.

**Not in scope:**
- `UmlExtension` — deliberately deferred by the spec. One extension (`core`) only.
- Middleware beyond `index` and `hide`. No `kanban`, `gallery`, `group-by`, `inherit`.
- Bundle-supplied middleware code. Rust built-ins with frontmatter params only.
- The full profile system (legal element types, child templates, validation).
- `catch_unwind` around middleware. `Result` only; a panic is a bug caught by tests.
- Resolving the two open questions (carried forward below).

## Global constraints

- **Gate for every task, all of it, every time:**
  - `cargo test --workspace`
  - `cd editors/vscode && npm run build && npm run test && npm run lint` (build FIRST —
    a stale `dist/` produces phantom typecheck errors).
- **`docs/okf-spec.md` stays byte-identical.** Deviations go in
  `docs/specs/waml-okf-extensions.md`, one entry each, with strict-consumer degradation.
  This design adds **no new deviation family**: `view:` widening scalar→scalar-or-sequence
  is a value shape inside the existing non-root-index-frontmatter deviation
  (already read by `parse_authored_index`, `crates/waml/src/okf/shell.rs:434`).
- **Depth cap is user/workspace scope, never bundle scope.** Default 20. Any
  `max_view_depth` in bundle frontmatter is ignored (it survives in `extra` like any
  unknown key, but never reaches the runner's limits). Security invariant, tested.
- **Whole-chain failure granularity.** A failing stage discards earlier stages'
  output; the root view renders with a spanned diagnostic. Never a half-applied chain.
- **The fallback path IS the default path** — the same root-view object, not a
  parallel safe mode.
- **Visual verification is DEFERRED to a human-run pass, not waived.** An
  implementer subagent has no window and cannot screenshot a running editor, so
  tasks whose verification is visual (D1b, D2, D3's affordance, G3, G4) would
  otherwise never land. Those tasks ship **gate-green with the visual check
  outstanding**, and each such commit MUST carry a `Visual-Check: pending` trailer
  naming what a human has to look at. The claim "verified" is never made on the
  strength of a green gate. Every pending item is collected in "Outstanding visual
  verification" at the foot of this plan, and the plan is NOT signed off until that
  list is walked in a real window. Headless tasks are unaffected: they carry their
  usual burden of proof and land verified.
- **makepad widget rules (editor tasks D1a–D2, G3–G4):** every NEW widget must be
  imported BY NAME in `crates/waml-editor/src/app.rs` `script_mod!` (no glob — an
  unregistered widget is silently dropped: no draw, no hit-test, gate stays GREEN),
  must get a boot-list `script_mod(vm)` line, and a child widget must register BEFORE
  its consumer. New modules go in `crates/waml-editor/src/main.rs`'s `mod` list. The
  `script_mod` namespace is ONE object literal, never field-by-field. Inline
  `font_size`/`FontMember` is gate-banned — fonts come from
  `crates/waml-editor/src/fonts.rs`. Widget drawing changes are verified visually and
  the plan says so per task; a green gate is NOT evidence for a drawing change.

## Verified touch points

| Path | What is there today |
|---|---|
| `crates/waml/src/okf.rs:235` | `pub struct Index` — no `profile`/`view`/`extra`; `frontmatter_is_empty` at `:319`; `Bundle::index` at `:283` |
| `crates/waml/src/okf/shell.rs:434` | `parse_authored_index` — already reads frontmatter `title` (the one existing OKF deviation); synthetic `Index` at `:287` |
| `crates/waml/src/index_md.rs:42` | `render_index` — emits NO frontmatter today; `reindex_source` at `:74`; test asserting `!out.contains("---")` at `:199` |
| `crates/waml/src/frontmatter.rs:257` | `render_frontmatter` |
| `crates/waml/src/diagnostic.rs:16/131/151/169/193` | `DiagCode` enum; `Diagnostic`; `new`/`warn` take `(DiagCode, message, file, line)`; `with_provenance` |
| `crates/waml/src/okf/ops.rs`, `crates/waml/src/okf/lower.rs` | OKF substrate op enum + lowering; `update_authored_index` (`lower.rs:669`) edits index.md surgically, preserving frontmatter |
| `crates/waml-editor/src/tree_panel.rs:~1565` | folder row currently only folds/unfolds |
| `crates/waml-editor/src/source_toggle_view.rs` | `SourceToggleView` / `DocView` trait usage |
| `crates/waml-editor/src/script_gate.rs` | existing gate-check pattern for script_mod registration |
| `crates/waml-editor/src/project_settings.rs` | `.waml/settings.json` load/store, corrupt-json-backs-up-and-defaults pattern |

## Module layout introduced by this plan (headless side)

```
crates/waml/src/view.rs            // pub mod view; in lib.rs
crates/waml/src/view/row.rs        // RowPath, RowId, ViewId, Row, RowTarget, RowCaps, ChildCaps
crates/waml/src/view/projection.rs // Projection, ProjectionCtx, Next, RowOp, errors
crates/waml/src/view/chain.rs      // Chain, ChainLimits, the runner
crates/waml/src/view/root.rs       // the `index` root-view middleware
crates/waml/src/view/hide.rs       // the `hide` middleware
crates/waml/src/view/decl.rs       // ViewDecl frontmatter parse (scalar or sequence)
crates/waml/src/view/surface.rs    // SurfaceId, default resolution by document type
crates/waml/src/extension.rs       // CoreExtension, registry of middleware + profiles
crates/waml/src/profile.rs         // ProfileDef static table
crates/waml-editor/src/extension_editor.rs  // EditorExtension, SurfaceFactory, pairing
```

---

## Phase A — Frontmatter on `Index` (spec delivery step 1, headless)

### Task A1: `ViewDecl` — parse `view:` as scalar or sequence

**Files:**
- Create: `crates/waml/src/view/decl.rs` (and `crates/waml/src/view.rs` with `pub mod decl;`; add `pub mod view;` to `crates/waml/src/lib.rs`)
- Test: inline `#[cfg(test)] mod tests` in `decl.rs`

The declaration layer is deliberately dumb: it captures what the author wrote and
where, and does **not** validate names — an unknown middleware name is a
declaration-level *chain-build* failure (Task B5) with a span on the name, not a
parse-time silent skip and not a parse error. `markdown` and `member:<href>` are
surface-resolution entries, not middleware (spec: "The chain resolves surfaces too");
the parse layer records them as entries and Task E3 interprets them.

```rust
/// One entry of a `view:` declaration, verbatim, with the source line for spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewEntry {
    pub raw: String,
    pub line: usize,
}

/// A parsed `view:` declaration. A scalar is a one-element chain.
/// First entry is outermost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewDecl {
    pub entries: Vec<ViewEntry>,
}

/// Parse the frontmatter value. Accepts a plain scalar or a flow/blocked
/// sequence of scalars. Returns `None` for a shape that is neither (e.g. a
/// nested mapping) — the caller keeps the key in `extra` so a re-render never
/// erases what the author wrote.
pub fn parse_view_decl(value: &crate::frontmatter::FmValue, line: usize) -> Option<ViewDecl>;
```

**Tests:**
- `scalar_view_is_a_one_element_chain` — `view: outline` → one entry `"outline"`.
- `sequence_view_preserves_order_first_is_outermost` — `view: [hide-refs, group-by-tag]` → two entries in order.
- `member_and_markdown_parse_as_ordinary_entries` — `member:./orders`, `markdown` are captured verbatim; no interpretation here.
- `a_mapping_value_is_rejected_not_erased` — returns `None`; caller behavior (stays in `extra`) asserted in Task A2.
- `empty_sequence_parses_to_an_empty_decl` — empty decl later resolves to the root view alone.

- [ ] Write failing tests; `cargo test -p waml view::decl` fails to compile.
- [ ] Implement; targeted tests pass.
- [ ] Full gate. Commit: `feat(view): ViewDecl scalar-or-sequence parse for view: declarations`

### Task A2: `Index` parses `profile`, `view`, and unknown keys

**Files:**
- Modify: `crates/waml/src/okf.rs:235` (the `Index` struct), `crates/waml/src/okf/shell.rs:287` (synthetic index) and `:434` (`parse_authored_index`)
- Create: `docs/specs/waml-okf-extensions.md`
- Test: inline tests in `crates/waml/src/okf/shell.rs`

Carried from the superseded plan's Task 2, with `view: Option<ViewSpec>` replaced by
`view: Option<ViewDecl>`.

```rust
pub struct Index {
    // ...existing six fields unchanged...
    /// Locally declared profile. What is in EFFECT is `Bundle::resolved_profile`.
    pub profile: Option<String>,
    /// Locally declared view chain. What is in EFFECT is `Bundle::resolved_view`.
    pub view: Option<ViewDecl>,
    /// Producer keys with no dedicated field. Preserved verbatim on round-trip.
    pub extra: Frontmatter,
}
```

In `parse_authored_index`: promote `profile` (trimmed, non-empty) and `view` (via
`parse_view_decl`); everything not in `INDEX_KNOWN_KEYS = ["title", "profile", "view"]`
lands in `extra`. A `view` value `parse_view_decl` rejects (mapping shape) stays in
`extra` so the author's text survives a re-render. Synthetic index gets
`profile: None, view: None, extra: Frontmatter::default()`. Fix every other
`Index { .. }` literal the compiler finds.

**Tests** (spec Testing: "Index parse" bullets):
- `index_frontmatter_promotes_profile_and_view_and_keeps_unknown_keys` — fixture with
  `title/profile/view/generator`; asserts promotion and that promoted keys do NOT
  double up in `extra`.
- `view_sequence_in_index_frontmatter_parses_in_order` — `view: [hide-refs, group-by-tag]`.
- `an_index_without_frontmatter_parses_exactly_as_before`.
- `a_malformed_view_value_stays_in_extra` — mapping-shaped `view:` → `index.view == None`, key present in `extra`.
- `a_synthesized_index_declares_nothing`.
- `max_view_depth_in_bundle_frontmatter_is_just_an_unknown_key` — lands in `extra`;
  (the "and never reaches the runner" half is Task B4's test).

Write `docs/specs/waml-okf-extensions.md`: one entry — frontmatter in a non-root
`index.md` (`title`, `profile`, `view` scalar-or-sequence, unknown keys pass through),
strict-consumer degradation stated. `docs/okf-spec.md` untouched.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(okf): parse profile, view chain, and unknown keys from index frontmatter`

### Task A3: `render_index` emits frontmatter; round-trip; retitle through frontmatter

**Files:**
- Modify: `crates/waml/src/index_md.rs:42` (`render_index`), `:74` (`reindex_source`), `:199` (the `!out.contains("---")` test comment), `crates/waml/src/okf/lower.rs:669` (`update_authored_index`)
- Test: inline tests in `index_md.rs` and `lower.rs`

Without this, any write path that re-renders an index silently erases a folder's
declarations. Adopt the superseded plan's Task 3 wholesale, with `view` rendered via
the decl: a one-entry decl renders as a scalar (`view: outline`), a multi-entry decl
as a flow sequence (`view: [hide-refs, group-by-tag]`).

```rust
#[derive(Default)]
pub struct IndexFrontmatter<'a> {
    pub profile: Option<&'a str>,
    pub view: Option<&'a ViewDecl>,
    pub extra: Option<&'a crate::frontmatter::Frontmatter>,
}

pub fn render_index(
    dir: &str, title: Option<&str>, description: Option<&str>,
    members: &[IndexEntry], frontmatter: &IndexFrontmatter<'_>,
) -> String
```

`IndexFrontmatter::default()` emits no block — a caller with nothing to declare
renders exactly today's bytes; `title` enters frontmatter only when some other key is
present. Call sites: `reindex_source` passes the parsed index's declarations through;
the one op-path call at `crates/waml/src/okf/lower.rs:845` (index file does not exist
yet) passes `default()` — correct, that branch has no parsed index. Confirm with
`rg "render_index\(" crates/` that no third non-test site exists.

Also: `update_authored_index` must move a frontmatter `title:` when one is present,
not just the H1 — otherwise a retitle on a declaring folder appears to do nothing
(frontmatter wins at parse). Do not ADD a `title:` key to an index that lacks one.

**Tests** (spec Testing: "Round-trip" bullet):
- `render_index_emits_declared_frontmatter_before_the_heading` (scalar view).
- `render_index_emits_a_chain_as_a_flow_sequence` — `view: [hide-refs, group-by-tag]` round-trips.
- `render_index_without_declarations_is_byte_identical_to_today`.
- `index_frontmatter_survives_parse_render_reparse` — parse, `reindex_source`, reparse;
  assert `title/profile/view/extra/members` equal; second pass is byte-stable.
- `pkg_retitle_moves_a_frontmatter_title_not_just_the_h1` (in `lower.rs` tests).

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(okf): emit index frontmatter and retitle through it`

---

## Phase B — `Chain`, `Projection`, the runner (spec delivery step 2, headless)

### Task B1: `RowPath`, `ViewId`, `RowId` — identity types

**Files:**
- Create: `crates/waml/src/view/row.rs` (`pub mod row;` in `view.rs`)
- Test: inline tests

```rust
/// "/"-separated, non-empty segments. Syntactically transparent, semantically
/// owned: anyone may split it; only the owning middleware says what a segment means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowPath(String);

impl RowPath {
    /// Rejects empty paths, empty segments, and leading/trailing '/'.
    pub fn parse(text: &str) -> Result<RowPath, RowPathError>;
    pub fn segments(&self) -> impl Iterator<Item = &str>;
    pub fn parent(&self) -> Option<RowPath>;
    pub fn starts_with(&self, other: &RowPath) -> bool;
    pub fn join(&self, segment: &str) -> Result<RowPath, RowPathError>;
    pub fn as_str(&self) -> &str;
}

/// The emitting middleware's declared name, disambiguated when a name repeats
/// within one chain ("group-by-tag#2"). Folder-scoped; NOT chain position —
/// inserting a stage must not invalidate persisted RowIds below it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowId { pub owner: ViewId, pub path: RowPath }
```

**Tests:**
- `row_path_parses_segments_and_parent` — `"a/b/c"` → 3 segments, parent `"a/b"`, root parent `None`.
- `row_path_rejects_empty_and_empty_segments` — `""`, `"a//b"`, `"/a"`, `"a/"` all `Err`.
- `starts_with_is_segment_wise_not_string_prefix` — `"ab/c"` does NOT start with `"a"`.
- `view_id_disambiguates_repeats_within_one_chain` — helper that assigns `name`, `name#2`, `name#3`; stable across two runs.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): RowPath, ViewId, RowId identity types`

### Task B2: `Row`, `RowTarget`, capabilities, and the virtual-surface construction rule

**Files:**
- Modify: `crates/waml/src/view/row.rs`
- Create: `crates/waml/src/view/surface.rs` (`SurfaceId` newtype only, for now)
- Test: inline tests

```rust
/// A surface name contributed by an extension's editor half. One name table
/// with middleware — not a second namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowTarget {
    /// A real concept document, by href.
    Concept(String),
    /// A real child directory, by address string.
    Folder(String),
    /// No file behind it. The owner interprets the RowPath.
    Virtual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowCaps { pub rename: bool, pub delete: bool, pub move_out: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChildCaps { pub reorder: bool, pub insert: bool, pub accept_move_in: bool }

pub struct Row {
    pub id: RowId,
    pub label: String,
    pub blurb: Option<String>,
    pub target: RowTarget,
    /// None ⇒ default resolution by document type. Middleware may override.
    pub surface: Option<SurfaceId>,
    /// Folder rows only: the chain used when this row expands. Lazy.
    pub expand: Option<Chain>,
    pub caps: RowCaps,
    pub child_caps: ChildCaps,
}

impl Row {
    /// The only constructor. Enforces: a Virtual target MUST name a surface —
    /// there is no document to infer one from.
    pub fn new(id: RowId, label: String, target: RowTarget, surface: Option<SurfaceId>)
        -> Result<Row, RowConstructError>;
}
```

Keep `Row`'s fields public for reading but route construction through `Row::new` (or a
builder) so the virtual-surface invariant cannot be bypassed. `Chain` is a forward
declaration here (`view/chain.rs` stub with the type only) if needed for compile order.

**Tests** (spec Testing: "A virtual row with `surface: None` is rejected at construction"):
- `a_virtual_row_without_a_surface_is_rejected_at_construction`.
- `a_real_target_with_surface_none_constructs` — default resolution is legal for real targets.
- `caps_default_to_nothing` — under-declaring is the safe default.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): Row, RowTarget, SurfaceId, and capability declarations`

### Task B3: `Projection` trait, `ProjectionCtx`, `Next`, `RowOp`, error types

**Files:**
- Create: `crates/waml/src/view/projection.rs`
- Test: compile-level + a `PassThrough` test double

```rust
pub struct ProjectionCtx<'a> {
    pub dir: &'a okf::Directory,
    pub bundle: &'a okf::Bundle,
    /// This middleware's frontmatter params (the folder's index frontmatter).
    pub params: &'a Frontmatter,
    /// Default descent: resolve the child's own declared chain.
    pub descend: &'a dyn Fn(&okf::Directory) -> Chain,
}

/// The continuation. Calling it runs the rest of the chain.
pub struct Next<'a> { /* runner internals */ }
impl<'a> Next<'a> {
    pub fn project(self, ctx: &ProjectionCtx<'_>) -> Result<Vec<Row>, ProjectionError>;
    pub fn apply(self, ctx: &ProjectionCtx<'_>, path: &RowPath, op: RowOp)
        -> Result<Vec<okf::Op>, Unsupported>;
    pub fn surface(self, ctx: &ProjectionCtx<'_>) -> SurfaceId;
}

/// Ops a surface can address to a row. v1 set mirrors what the root view can
/// lower to OKF ops (Phase G); middleware forward or refuse.
pub enum RowOp {
    Rename { title: String },
    Delete,
    Reorder { before: Option<RowPath> },
    InsertConcept { after: Option<RowPath>, title: String },
    MoveIn { from: RowId },
    MoveOut,
}

pub struct Unsupported;      // op-level refusal, not a chain failure
pub struct Unresolved;       // path no longer resolves; NOT a chain failure
pub struct ProjectionError { pub message: String }  // stage failure → whole-chain fallback

pub trait Projection {
    fn project(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>)
        -> Result<Vec<Row>, ProjectionError>;
    /// Given a path this middleware minted, return the rows along it, labels
    /// included — from the directory alone, on a later run. Breadcrumbs, deep
    /// links, and session restore are the same call.
    fn resolve(&self, ctx: &ProjectionCtx<'_>, path: &RowPath)
        -> Result<Vec<Row>, Unresolved>;
    fn apply(&self, ctx: &ProjectionCtx<'_>, path: &RowPath, op: RowOp, next: Next<'_>)
        -> Result<Vec<okf::Op>, Unsupported>;
    /// Which surface renders this container's own tab. Decline by returning
    /// `next.surface(ctx)`.
    fn surface(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> SurfaceId;
}
```

Provide a `#[cfg(test)] PassThrough` middleware (returns `next(...)` for everything)
used by later tasks as the filter-shaped test double.

**Tests:**
- `pass_through_is_the_identity_on_every_method` — deferred to run against the real
  runner in B5; here, only shape/compile plus doc tests.

- [ ] Implement (this task is mostly definition; the test is in B5) → full gate.
- [ ] Commit: `feat(view): Projection trait, ProjectionCtx, Next, RowOp`

### Task B4: `ChainLimits` from settings — never from the bundle

**Files:**
- Create: `crates/waml/src/view/chain.rs` (limits half)
- Modify: `crates/waml-editor/src/project_settings.rs` (add optional `max_view_depth` field, default `None` → 20)
- Test: inline in `chain.rs`; settings round-trip in `project_settings.rs`

```rust
/// Runner bounds. Constructed by the HOST (editor from .waml/settings.json,
/// tests directly, LSP from its own config) and passed in. There is no
/// constructor that reads a bundle: bundle-supplied max_view_depth is
/// unreachable by construction, not by filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainLimits { pub max_depth: usize }
impl Default for ChainLimits { fn default() -> Self { ChainLimits { max_depth: 20 } } }
```

Editor side: `ProjectSettings` gains `max_view_depth: Option<usize>` following the
existing corrupt-json-backs-up-and-defaults pattern; a helper
`fn chain_limits(&self) -> ChainLimits` maps `None`/absent → default 20.

**Tests** (spec Testing: "A bundle-frontmatter `max_view_depth` is ignored"):
- `chain_limits_default_is_twenty`.
- `bundle_frontmatter_max_view_depth_never_reaches_the_runner` — build a bundle whose
  root and folder indexes both declare `max_view_depth: 3`; construct the runner with
  `ChainLimits::default()`; assert the effective depth is 20 (checked properly once
  B6's runner exists — this test lands here as the settings-side half asserting the
  key sits inert in `Index::extra` and `ChainLimits` has no bundle-reading path;
  extended in B6 with a descent-depth assertion).
- `project_settings_max_view_depth_round_trips` and `absent_field_yields_default_20`
  (editor crate, headless serde test — no window).

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): ChainLimits with settings-scoped depth cap`

### Task B5: `Chain` and the runner — build, run, whole-chain failure

**Files:**
- Modify: `crates/waml/src/view/chain.rs`
- Modify: `crates/waml/src/diagnostic.rs` (new `DiagCode` variants)
- Test: inline tests using `PassThrough` and purpose-built failing/counting doubles

```rust
/// A resolved middleware chain. Cheap to clone (Arc'd stages); the terminal
/// stage is always the root view.
#[derive(Clone)]
pub struct Chain { stages: Arc<[(ViewId, Arc<dyn Projection>)]> }

pub struct ChainOutcome {
    pub rows: Vec<Row>,
    /// The folder's own surface, chain-resolved.
    pub surface: SurfaceId,
    /// Non-empty when the declared chain failed and the ROOT VIEW rendered
    /// instead (whole-chain granularity), or when bounds tripped.
    pub diagnostics: Vec<Diagnostic>,
}

impl Chain {
    /// Build from a ViewDecl against a middleware registry. An unknown name is
    /// a declaration-level failure: returns the root-view-only chain plus a
    /// diagnostic spanned on the name in `view:`. Bad/missing params likewise,
    /// spanned on the param key.
    pub fn build(decl: &ViewDecl, registry: &MiddlewareRegistry, index: &okf::Index)
        -> (Chain, Vec<Diagnostic>);
    /// The one-element root-view chain — a folder with no view: declaration.
    pub fn root_only(registry: &MiddlewareRegistry) -> Chain;
    /// Run. A stage returning Err discards ALL stage output and re-runs the
    /// root view alone (the same object — the fallback path IS the default
    /// path), attaching a document-level diagnostic.
    pub fn run(&self, ctx: &ProjectionCtx<'_>, limits: ChainLimits) -> ChainOutcome;
    pub fn resolve(&self, ctx: &ProjectionCtx<'_>, id: &RowId) -> Result<Vec<Row>, Unresolved>;
    pub fn apply(&self, ctx: &ProjectionCtx<'_>, id: &RowId, op: RowOp)
        -> Result<Vec<okf::Op>, Unsupported>;
}
```

New `DiagCode` variants (each needs an `as_str` slug and a `severity` arm; all
`Error` except `UnknownSurface` which degrades and is a `Warning`):
`UnknownViewMiddleware` (`unknown-view-middleware`), `InvalidViewParams`
(`invalid-view-params`), `ViewStageFailed` (`view-stage-failed`),
`ViewDepthExceeded` (`view-depth-exceeded`), `ViewCycle` (`view-cycle`),
`UnknownSurface` (`unknown-surface`). Reuse `Diagnostic::new`/`warn` with the
folder's `index.md` as `file` and `with_span` on the offending value; no new
diagnostic channel. Update the `code_has_stable_slug_and_severity` test.

The `MiddlewareRegistry` here is a plain name→factory map; Task E1 makes
`CoreExtension` the thing that populates it — the chain looks names up exactly the
same before and after.

**Tests** (spec Testing bullets: unknown name, failing stage, ViewId stability):
- `an_unknown_middleware_name_yields_root_chain_plus_spanned_diagnostic` — chain has
  exactly the root view; diagnostic code `UnknownViewMiddleware`, file =
  `sales/index.md`, span covers the bad name.
- `a_failing_stage_discards_earlier_stages_output_and_yields_the_root_view` — chain
  `[renaming-double, failing-double]` where the first stage decorates labels; outcome
  rows are the PLAIN root listing (no decoration survives) + `ViewStageFailed`
  document-level diagnostic. This is the whole-chain granularity invariant.
- `pass_through_chain_equals_root_only_chain` — `PassThrough` in front changes nothing
  (rows, surface, ops).
- `row_id_is_stable_across_reprojection_with_unchanged_inputs` — run twice, assert
  `Vec<RowId>` equal.
- `repeated_names_in_one_chain_get_stable_disambiguated_view_ids` — `[hide, hide]` →
  owners `hide`, `hide#2`, same on the second run.

- [ ] Failing tests (doubles first) → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): Chain runner with whole-chain failure fallback`

### Task B6: Depth cap and cycle guard

**Files:**
- Modify: `crates/waml/src/view/chain.rs`
- Test: inline, with a self-descending synthesizing double

Descent is lazy: `expand` chains are only forced when a row expands, so the runner
counts **chain-descent depth** (times an `expand` chain has been entered from the
top-level run), not directory depth — synthesized folders can exceed real tree depth.
A visited-directory guard (set of `DirectoryAddress` seen on the current descent
path) runs alongside so a cycle trips on first revisit, not at level 20. On trip:
stop descending, emit a **diagnostic row** (a `Virtual` row owned by the runner's
reserved `ViewId`, labeled with the folder and the chain) plus a document-level
`ViewDepthExceeded`/`ViewCycle` diagnostic. Never silent truncation. Enforced by the
runner; middleware cannot opt out — the limit state lives in runner-owned descent
context, not in `ProjectionCtx` where a stage could reset it.

**Tests** (spec Testing: depth cap, cycle guard; completes B4's ignored-key test):
- `depth_cap_trips_at_the_configured_value` — self-descending synthesizer double
  (`expand: Some(self-chain)` on a virtual folder row) with `ChainLimits { max_depth: 3 }`;
  descent stops at 3; diagnostic row present and names the folder; run TERMINATES.
- `cycle_guard_trips_on_first_revisit` — two real directories whose chains govern each
  other (A's chain descends into B with A's chain, and vice versa); trip happens at the
  first revisit of A, depth « cap; `ViewCycle` diagnostic names the folder.
- `bundle_max_view_depth_does_not_change_the_trip_point` — bundle declares
  `max_view_depth: 50` in frontmatter; runner built with `max_depth: 3`; trips at 3.
  (Security invariant, completing Task B4.)
- `the_trip_is_a_diagnostic_row_not_a_missing_row` — the truncated folder's row list
  ends with the diagnostic row, not silently.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): chain depth cap and visited-directory cycle guard`

### Task B7: The root view (`index` middleware) — project + identity invariant

**Files:**
- Create: `crates/waml/src/view/root.rs`
- Test: inline tests against fixture bundles

The terminal stage. `project` ignores `next` (there is none below it) and emits one
row per member of `ctx.dir`, in **authored index member order with unlisted items
appended** — exactly the plain OKF listing. Concept rows: label from title, blurb
from frontmatter `description`, `RowTarget::Concept(href)`, `surface: None`
(default resolution). Child-directory rows: `RowTarget::Folder(addr)`,
`expand: Some((ctx.descend)(child))` — **honor the child** is the default descent
policy. `RowPath` for a root-view row is the real member href — one segment per
directory level is not needed; the href IS the path the owner resolves.
`surface(ctx, _next)` returns the folder-listing surface id (`"folder"`).
Caps in this task: all `false` (editing arrives in Phase G; under-declaring is legal
and hides affordances without breaking anything). `resolve`/`apply` are stubs
(`Unresolved`/`Unsupported`) until B8/G2.

**Tests** (spec Testing: "Identity chain equals the plain OKF listing, row for row,
including order" — THE ground-truth invariant):
- `identity_chain_reproduces_the_plain_okf_listing_row_for_row` — fixture with
  authored member order deliberately different from filename order plus one unlisted
  file; assert the projected `(label, target)` list equals the listing
  `okf::Directory` + `Index::members` yields, in order, no more, no less.
- `a_folder_with_no_view_declaration_gets_the_root_only_chain` — today's behavior
  expressed in the new model, not special-cased beside it.
- `every_root_view_row_has_a_real_target` — no `Virtual` rows; ownership totality's
  root-view half.
- `child_folder_rows_expand_with_the_childs_own_chain` — child declares
  `view: [hide]`; the parent's projection of that child row carries a 2-stage chain
  (honor-the-child).
- `root_view_surface_is_the_folder_listing` .

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): root view middleware — the plain OKF listing as terminal stage`

### Task B8: `resolve` on the root view + prefix fallback + mint/resolve invariant

**Files:**
- Modify: `crates/waml/src/view/root.rs`, `crates/waml/src/view/chain.rs` (chain-level `resolve` walking to the owner)
- Test: inline + a property-shaped sweep

Root-view `resolve`: the path is a member href; return the row for it (label
included), built from the directory alone. Chain-level `resolve(id)`: dispatch to
the stage whose `ViewId` matches `id.owner`; `Unresolved` from the owner falls back
to the **nearest resolvable prefix** (`path.parent()` loop), at worst the folder
itself — `Unresolved` is not a failure of the chain and produces no diagnostic.

**Tests** (spec Testing: mint/resolve, prefix fallback):
- `every_path_minted_by_project_resolves_through_resolve_on_a_later_run` — for EVERY
  fixture bundle in the test suite: run `project`, tear down, re-parse the bundle
  fresh, `resolve` each minted `RowId`; assert the resolved row's label and target
  equal the projected row's. This sweep is written as a helper
  (`assert_mint_resolve_roundtrip(bundle)`) that later middleware tasks (F1) MUST
  also call on their fixtures — it is the standing invariant for any synthesizing
  middleware: paths must be keyed on something stable in the model, never positional.
- `an_unresolvable_path_falls_back_to_its_nearest_resolvable_prefix` — delete the
  file behind a resolved path, re-parse, `resolve`; get the parent folder's rows,
  no diagnostic, no error surfaced as a chain failure.
- `a_deleted_everything_path_falls_back_to_the_folder_itself` — worst case.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): root-view resolve with nearest-prefix fallback`

---

## Phase C — Resolution (spec delivery step 3, headless)

### Task C1: `ProfileDef` static table

**Files:**
- Create: `crates/waml/src/profile.rs`; add `pub mod profile;` to `crates/waml/src/lib.rs`
- Test: inline

Adopted unchanged from the superseded plan's Task 4, except `default_view` is now
`Option<ViewDecl>`:

```rust
pub struct ProfileDef { pub name: &'static str, pub default_view: Option<ViewDecl> }
pub fn profile(name: &str) -> Option<&'static ProfileDef>;
```

Ships `uml-domain` and `okf`, both `default_view: None` — today's behavior preserved.
(Task E1 later makes `CoreExtension::profiles()` the thing that contributes these;
the lookup function does not change shape.)

**Tests:** `shipped_profiles_resolve_by_name_and_default_to_no_view`,
`unknown_profiles_resolve_to_none` (exact names, no case folding).

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(waml): ProfileDef static table for uml-domain and okf`

### Task C2: `resolved_profile` and `resolved_view` → `Chain`

**Files:**
- Modify: `crates/waml/src/okf.rs` (`impl Bundle`, after `directories()` ~:313) — or a `view`-side extension if `okf.rs`'s no-view-import direction is cleaner; keep the okf→view dependency one-way and note which way was chosen
- Test: inline

Adopted from the superseded plan's Task 5, with the return type changed per spec:

```rust
/// Nearest declaring ancestor, self first. Stops at the first index declaring
/// a profile — an explicit declaration beats an inherited one.
pub fn resolved_profile(&self, directory: &str) -> Option<&str>;

/// The chain in EFFECT: (1) the index's own `view:` decl, else (2) the
/// inherited profile's `default_view` decl, else (3) the root-only chain.
/// Chain::build handles unknown names; diagnostics ride along.
pub fn resolved_view(&self, directory: &str, registry: &MiddlewareRegistry)
    -> (Chain, Vec<Diagnostic>);
```

No auto-detection: a folder holding one diagram does not silently become `member:`.

**Tests** (spec Testing: `resolved_profile` and `resolved_view` bullets, all
retained from the old plan and re-shaped for `Chain`):
- `resolved_profile_prefers_self_then_nearest_ancestor_then_none` (three-level fixture).
- `resolved_profile_is_none_when_nothing_declares`.
- `resolved_view_walks_local_then_profile_default_then_root_only` — each step in
  isolation; the step-3 chain has exactly one stage (root view).
- `an_explicit_local_view_beats_an_inherited_profile_default` — inject a test-only
  profile with a non-None default into the registry to drive step 2 for real.
- `an_unknown_directory_resolves_to_the_root_only_chain`.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(okf): resolved_profile and chain-returning resolved_view`

---

## Phase D — Folder surface, read-only (spec delivery step 4, editor)

Folders open for the first time. GUI limits apply: the gate proves the plumbing
(view-model tests are headless in the editor crate where possible); the drawing is
**verified visually and stated as such** in each task below.

> **D1 is split into D1a and D1b.** The original single task required three new
> files plus five wiring points before anything was gate-provable, and two
> implementer generations exhausted their budget orienting without landing a
> commit. D1a lands the headless half (view-model + provider + tab identity) with
> real tests and no widget; D1b lands the widget that renders it. The deliverable
> and the constraints are unchanged — only the commit boundary moved.

### Task D1a: folder view-model, provider, and tab identity (headless)

**Files:**
- Create: `crates/waml-editor/src/folder_view.rs` (the `DocView` impl, holding the `ChainOutcome`, with the row view-model and the row→navigation mapping as plain functions), `crates/waml-editor/src/folder_documents.rs` (provider)
- Modify: `crates/waml-editor/src/main.rs` (two `mod` lines), `crates/waml-editor/src/doc_view.rs` (a `DocViewIdentity` variant), `crates/waml-editor/src/documents.rs` (provider chain entry)
- Test: headless view-model tests in `folder_view.rs`

Model this on `crates/waml-editor/src/generic_okf_view.rs` and `okf_documents.rs` —
the closest existing read-only `DocView` + provider pair. The view calls
`resolved_view(dir, registry)` then `chain.run(ctx, settings.chain_limits())` and
exposes `ChainOutcome.rows` as a row view-model: bullet, label, optional blurb, in
order. Clicking a concept row opens the concept; clicking a folder row opens that
folder's own view (its `expand` chain drives the nested case later; the tab-open path
re-resolves) — expressed here as an action enum, with no widget behind it yet.

`documents.rs` is keyed on concept-id strings today; the folder target needs its own
locator/tab-identity path. That is the substance of this task.

**Tests:**
- `folder_view_model_lists_projected_rows_in_order` — build the outcome from a fixture
  bundle, assert the row view-model. No window.
- `clicking_a_row_maps_to_the_right_navigation_target` — at the action-enum level.
- `a_folder_target_gets_its_own_tab_identity` — distinct from a concept-id tab.

- [ ] Failing headless tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(editor): folder view-model and document provider`

### Task D1b: `FolderListView` widget

**Files:**
- Create: `crates/waml-editor/src/folder_list.rs` (widget)
- Modify: `crates/waml-editor/src/main.rs` (one `mod` line), `crates/waml-editor/src/app.rs` (`script_mod!` import by name + boot-list registration — **the widget is silently dead and invisible without both**, gate stays green; register `FolderListView` BEFORE any consumer that embeds it), `crates/waml-editor/src/doc_view.rs` (`BodyWidgets` handle), `crates/waml-editor/src/folder_view.rs` (bind the view-model to the widget)
- Test: the D1a headless tests still pass; drawing verified visually

Renders the D1a row view-model: bullet, label, optional blurb, in order. A
`ChainOutcome` with diagnostics renders the header strip (Task D2). Widget name
`FolderListView` — verify absent from `crates/` before use; never reuse a makepad
widget name. Fonts from `fonts.rs` only, never an inline `font_size`.

**Tests:**
- **Visual verification required and stated:** open a fixture folder; the listing
  renders titles + blurbs in authored order; a concept row opens the document. A green
  gate is NOT evidence for this.

- [ ] Implement → full gate → visual verify.
- [ ] Commit: `feat(editor): folder view surface renders the projected chain`

### Task D2: Tree row-vs-chevron split, diagnostics strip, tree marker

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (~:1565 region — folder row currently only folds/unfolds; split hit-testing: chevron rect folds, row body emits open-folder), `crates/waml-editor/src/app/navigation.rs` (Directory target opens the folder view instead of folding only), `crates/waml-editor/src/folder_list.rs` (diagnostics header strip above fallback rows)
- Test: tree_panel's existing headless action tests extended; drawing verified visually

Diagnostics: the strip names the stage and the reason (from the `ChainOutcome`
diagnostics, message written for the document author); the tree panel draws a marker
on a folder row whose chain degraded, so a degraded folder inside a collapsed subtree
is not silent. **Hit-testing must gate on the same verdict as drawing** (chevron rect
recorded at draw time, used at event time, same coordinate space — beware `Size::Fill`
siblings shifting cached rects).

**Tests:**
- Headless: extend the existing folder-click test — chevron hit yields fold intent,
  row-body hit yields open intent, and neither does the other's job.
- Headless: `a_degraded_chain_outcome_flags_the_tree_row` at the view-model level.
- **Visual verification required and stated:** chevron folds without opening; row body
  opens without folding; the strip and the marker appear for a folder whose `view:`
  names an unknown middleware; the folder still shows its fallback rows (blast radius
  is one folder — the workspace, tab bar, and other documents stay live).

- [ ] Failing headless tests → implement → targeted pass → full gate → visual verify.
- [ ] Commit: `feat(editor): tree row opens the folder view; chain diagnostics surfaced`

### Task D3: The raw OKF layer

**Files:**
- Modify: `crates/waml/src/view/chain.rs` (`Chain::raw()` — pins the chain to
  `[index]`, bypassing every declared stage), `crates/waml-editor/src/app/navigation.rs`
  (open-raw route), `crates/waml-editor/src/folder_list.rs` (raw-mode banner)
- Test: inline headless; the affordance verified visually

Hidden means hidden through the chain (spec: "The raw OKF layer"). Reachability
is preserved by a *separate route*, not by leaking: opening a target raw builds
`Chain::raw()` for its directory rather than the declared chain.

Two callers: an explicit open-raw affordance on a folder, and search — a hit whose
path does not resolve through the declared chain must open raw, or clicking the
result silently does nothing. That second caller is the one that makes this a task
rather than a one-liner.

Raw mode is visibly labelled; a user must not mistake a raw listing for the folder's
configured view.

This is **not** a permission boundary and carries no access check. `hide` is
presentational; any reader can open raw. No code or comment may imply otherwise.

**Tests:**
- `chain_raw_equals_the_identity_listing` — `Chain::raw()` row-for-row equal to a
  folder with no `view:` declaration.
- `chain_raw_ignores_a_declared_chain` — a folder declaring `hide` still yields
  every row under `raw()`.
- `a_search_hit_on_a_hidden_path_routes_to_raw` — at the navigation view-model
  level, no widget.
- **Visual verification required and stated:** the raw affordance opens the full
  listing for a folder whose chain hides rows, and the raw-mode label is present.

- [ ] Failing tests → implement → targeted pass → full gate → visual verify.
- [ ] Commit: `feat(view): raw OKF layer bypasses the declared chain`

---

## Phase E — Extensions (spec delivery step 5)

### Task E1: `CoreExtension` (headless) + the `core` extension

**Files:**
- Create: `crates/waml/src/extension.rs`; `pub mod extension;` in `lib.rs`
- Modify: `crates/waml/src/view/chain.rs` (`MiddlewareRegistry::from_extensions`), `crates/waml/src/profile.rs` (profiles now contributed via the extension; lookup unchanged)
- Test: inline

```rust
pub trait CoreExtension {
    fn name(&self) -> &str;
    fn middleware(&self) -> Vec<(&'static str, Box<dyn Projection>)>;
    fn profiles(&self) -> Vec<ProfileDef>;
}

/// The one registered extension: `index`, `hide` (F1), the `okf` and
/// `uml-domain` profiles.
pub struct CoreExt;
```

Declaration only, no behavior — an extension returns lists; it must never grow
`on_open`/`handle_event`. Nothing resolves *through* it: the chain looks middleware
up by name exactly as before; the extension is only what put the name in the table.
One name table — no separate `SurfaceId` namespace. Compiled-in; no dynamic loading.

**Tests** (spec Testing: "CoreExtensions load and project rows with no
EditorExtension present"):
- `core_extension_loads_and_projects_with_no_editor_present` — build the registry
  from `CoreExt` alone in the `waml` crate (which cannot even name a `DocView`),
  run an identity chain, get rows. The core half stands alone by construction.
- `registry_from_extensions_matches_the_hand_built_registry` — Chain lookup is
  unchanged by the extension layer.
- `duplicate_middleware_names_across_extensions_are_a_build_error` — one name table.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(waml): CoreExtension trait and the core extension`

### Task E2: `EditorExtension`, `SurfaceFactory`, default resolution, degrade path

**Files:**
- Create: `crates/waml-editor/src/extension_editor.rs`
- Modify: `crates/waml/src/view/surface.rs` (default resolution by document type — headless: a pure `fn default_surface(target: &RowTarget, bundle: &Bundle) -> SurfaceId` mapping document type → `"markdown"` / `"canvas"` / `"source"`, folder → `"folder"`), `crates/waml-editor/src/documents.rs` (+ `folder_view.rs`) to open rows through the surface table
- Test: headless resolution tests in `waml`; table tests in `waml-editor`

```rust
// waml-editor
pub trait EditorExtension {
    fn name(&self) -> &str;   // matches its core half
    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)>;
}
/// A factory, not an instance — called when a row is OPENED. A DocView per
/// listed row would allocate widgets and fonts for rows nobody opens.
pub type SurfaceFactory = Box<dyn Fn(&OpenCtx, &RowId) -> Box<dyn DocView>>;
```

Register today's surfaces on the core editor half: markdown reading, source, canvas,
folder listing. No speculative format registry. An unknown `SurfaceId` at open time
**degrades to the document-type default and emits a `UnknownSurface` warning
diagnostic at the resolution site — never a blank tab**. Session restore: persist the
`RowId`, re-run `resolve(path)`, open through the owner's surface — no persisted
surface identity (covered by B8's mint/resolve invariant).

**Tests** (spec Testing: surface totality + unknown-surface bullets):
- Headless (`waml`): `surface_resolution_is_total_for_real_targets` — a row with
  `surface: None` and each real document type resolves to its type default; a folder
  target resolves to `"folder"`. Root view is total on BOTH axes: unclaimed rows AND
  default surface resolution.
- Headless (`waml`): `an_unknown_surface_id_degrades_to_the_type_default_with_a_diagnostic`
  — resolution helper returns `(default, Some(diagnostic))`, never panics, never
  yields nothing.
- Editor (headless table test): `todays_four_surfaces_are_registered_by_the_core_editor_half`.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(editor): EditorExtension surfaces with total default resolution`

### Task E3: `markdown` and `member:<href>` as surface resolutions

**Files:**
- Modify: `crates/waml/src/view/chain.rs` / `decl.rs` interpretation (Chain::build recognizes the two resolution forms), `crates/waml/src/view/root.rs` (folder-target surface resolution honors them)
- Test: inline in `waml`

These are NOT middleware — neither projects rows (spec: "The chain resolves surfaces
too"). `view: markdown` resolves the folder's own target to the markdown surface over
`index.md`. `view: member:./orders` resolves the folder's target to that member's
target at that member's resolved surface (`member:` followed by the href, no space —
a space makes it a YAML mapping and A1/A2 already reject that shape into `extra`).
`Chain::build` recognizes them as resolution outcomes attached to the chain, not as
stages; the row projection beneath is unchanged (the chain still projects rows for
the tree). A `member:` href that does not resolve to a member is a declaration-level
diagnostic spanned on the entry, falling back to default resolution.

**Tests:**
- `view_markdown_resolves_the_folder_target_to_the_markdown_surface` — rows unchanged
  vs identity chain; `ChainOutcome.surface == "markdown"`.
- `view_member_resolves_to_the_members_target_and_surface` — folder targeting
  `./orders` (a diagram) yields the diagram's surface; targeting a folder member
  yields that member's own resolved chain surface.
- `member_with_a_missing_href_degrades_with_a_spanned_diagnostic`.
- `no_auto_detection` — a folder holding exactly one diagram and no `view:` still
  resolves to the folder listing.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): markdown and member: as chain surface resolutions`

### Task E4: Extension name pairing gate-checked in `script_gate.rs`

**Files:**
- Modify: `crates/waml-editor/src/script_gate.rs` (follow the existing assertion pattern in that file), `crates/waml-editor/src/extension_editor.rs` (expose the registered pair lists)
- Test: the gate test itself

A `CoreExtension` whose middleware is reachable while its `EditorExtension` half is
absent yields rows that cannot be opened — the `script_mod!` failure mode exactly.
Assert at gate time, not runtime discovery:

**Tests** (spec Testing: "every reachable middleware name has an editor half when it
resolves a surface"):
- `every_core_extension_has_a_paired_editor_extension_by_name` — set equality on
  `name()` across the two registries.
- `every_surface_id_resolvable_by_a_registered_chain_has_a_registered_factory` —
  enumerate the surface ids the core side can mint (root view default set + E3
  resolutions) and assert each has a factory. A middleware resolving a surface with
  no factory fails the gate, not the user.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `test(editor): gate-check core/editor extension pairing`

---

## Phase F — `hide` (spec delivery step 6)

### Task F1: `hide` middleware

**Files:**
- Create: `crates/waml/src/view/hide.rs`
- Modify: `crates/waml/src/extension.rs` (register `hide` in `CoreExt`)
- Test: inline

Params: `hide: [glob, ...]` read from `ctx.params` (the folder's index frontmatter).
Missing or malformed `hide:` param when the middleware is named in `view:` is a
declaration-level `InvalidViewParams` diagnostic spanned on the param key →
whole-chain fallback. `project`: call `next`, drop rows whose target href/address
matches any glob. Paths map one-to-one onto inner paths, so `apply` forwards to the
inner stage unchanged for *surviving* rows — `hide` needs no edit code and must not
break editing beneath it. `surface`: `next.surface(ctx)` (declines).

**Resolution of hidden paths — decided.** `hide` does **not** forward `resolve`
for a hidden path. It returns `Unresolved`, and the runner falls back to the
nearest resolvable prefix. Hidden means hidden through the chain, without
exception; a middleware never leaks a row it declined to emit.

Reachability comes from the **raw OKF layer** instead (spec: "The raw OKF layer") —
the root view opened directly, chain pinned to `[index]`. A search hit whose path
does not resolve through the chain must open in raw mode, or clicking the result
silently does nothing.

`hide` is presentational, never a permission boundary. Add no code or comment
implying a hidden file is protected.

**Tests** (spec Testing: hide bullet; the first non-identity chain end to end):
- `hide_drops_exactly_the_matching_rows_and_nothing_else` — `hide: ["references/**"]`;
  row set = identity minus matches; order of survivors unchanged.
- `hide_forwards_every_op_unchanged` — with a counting/recording inner double,
  each `RowOp` variant passes through byte-identical (full verification against the
  real root-view `apply` lands in G3).
- `hide_declines_surface_resolution` — outcome surface equals the identity chain's.
- `hide_with_no_hide_param_is_a_declaration_failure` — `InvalidViewParams`, spanned
  on... the `view:` entry (no param key exists) — root fallback.
- `assert_mint_resolve_roundtrip` sweep (from B8) over a hidden-rows fixture,
  scoped to surviving rows.
- `a_hidden_path_does_not_resolve_through_the_chain` — `Unresolved`, and the
  runner returns the nearest resolvable prefix (at worst the folder itself).
- `the_raw_okf_layer_resolves_a_hidden_path` — same path, chain pinned to
  `[index]`, resolves to the real concept.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): hide middleware — first non-identity chain`

---

## Phase G — Editing (spec delivery step 7)

### Task G1: OKF-substrate concept ops (`ConceptNew`, `ConceptSet`)

**Files:**
- Modify: `crates/waml/src/okf/ops.rs` (Op enum + `lower_one_with_state`), `crates/waml/src/okf/lower.rs` (two `pub(crate) fn`s beside `op_pkg_move`), `crates/waml/src/ops/mod.rs` + `crates/waml/src/compat.rs` (legacy variants + mapping)
- Test: inline in `okf/ops.rs`

Adopted from the superseded plan's Task 6 unchanged in substance — it is model-layer
and still required: editing must work in a `profile: okf` or profileless folder, but
`Op::NodeNew`/`NodeSet` are UML-claim-gated. New ops live on the OKF substrate
(`okf` must never import a UML type; `ty` is the free-text OKF `type` frontmatter
string, NOT `ElementType`):

```rust
ConceptNew { directory: DirectoryAddress, slug: String, ty: String, title: String, description: Option<String> },
ConceptSet { id: String, title: Option<String>, description: Option<String> },
```

**Tests** (from the old plan, kept verbatim in intent):
`concept_new_writes_a_free_text_type_with_no_uml_involvement`,
`concept_new_accepts_an_empty_type_for_a_profileless_folder`,
`concept_new_refuses_to_overwrite_an_existing_document`,
`concept_set_retitles_a_concept_uml_does_not_claim`,
`concept_set_leaves_unmentioned_fields_alone`.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(okf): OKF-substrate ConceptNew and ConceptSet ops`

### Task G2: `apply` on the root view + capability declarations + the property test

**Files:**
- Modify: `crates/waml/src/view/root.rs` (real `apply`, real caps), `crates/waml/src/view/chain.rs` if needed
- Test: inline + the fixture-sweep property test

Root-view `apply`: path is the real member href; `RowOp` lowers to OKF ops —
`Rename` → `ConceptSet`/`PkgRetitle`; `InsertConcept` → `ConceptNew` + index
insertion; `Reorder` → `PkgReorder`; `MoveIn`/`MoveOut` → `PkgMove`/`ConceptMove`
composites; `Delete` → the existing removal op. Nothing bypasses the model to write
files. With `apply` real, the root view now DECLARES its caps truthfully
(`rename/delete/move_out` on concept rows, `reorder/insert/accept_move_in` in
`child_caps` on the folder and folder rows).

**Tests** (spec Testing: op-batch bullet + THE capability property test):
- `apply_rename_on_the_root_view_produces_the_expected_op_batch_and_consistent_indexes`
  — apply, lower through `apply_source`, re-parse, assert both affected `index.md`
  files consistent (title moved in frontmatter AND H1 per A3).
- One test per remaining `RowOp` variant asserting the lowered op batch shape.
- **Property test** `every_declared_capability_is_accepted_by_apply` — for every row
  of every fixture chain in the suite (identity, hide, failing-fallback, synthesized
  doubles): for each capability the row declares, construct the corresponding `RowOp`
  and assert `apply` does not return `Unsupported`. The converse (under-declare yet
  accept) is explicitly allowed and NOT asserted. Written as a shared helper so Phase
  F and future middleware fixtures are swept automatically. A declared capability
  yielding `Unsupported` is the drift this test exists to catch.
- `hide_forwards_apply_to_the_real_root_view` — completes F1's forwarding proof
  against the real implementation: rename through a `[hide]` chain edits the file.

- [ ] Failing tests → implement → targeted pass → full gate.
- [ ] Commit: `feat(view): root-view apply lowers RowOps to OKF ops; caps property test`

### Task G3: Editor editing gestures — Enter, retitle, Tab/Shift-Tab

**Files:**
- Modify: `crates/waml-editor/src/folder_list.rs` (row focus, `KeyCode::Return`, inline text entry, `KeyCode::Tab` ± shift when the row/child caps allow), `crates/waml-editor/src/folder_view.rs` (map gestures to `chain.apply` + op batches)
- Test: headless gesture→RowOp mapping tests; interaction verified visually

Affordances gate on **declared caps** (advisory, rendering only); `apply` remains
the authority — a refused op surfaces as a no-op with the row unchanged, never a
crash. Widget state must stay consistent on early-return paths; every armed edit
state clears on focus loss.

**Tests:**
- Headless: `enter_on_a_row_emits_insert_concept_at_that_position`;
  `typing_commits_a_rename_row_op`; `tab_emits_move_in_to_the_preceding_sibling_directory`;
  `shift_tab_emits_move_out`; `tab_with_no_preceding_sibling_directory_refuses`
  (INTERIM behavior — open question 1 is carried, not resolved; the refusal is the
  conservative placeholder and the test names it as such).
- **Visual verification required and stated:** Enter creates and focuses a new row;
  typing retitles live; Tab reparents. A green gate is NOT evidence for the
  focus/caret drawing.

- [ ] Failing headless tests → implement → targeted pass → full gate → visual verify.
- [ ] Commit: `feat(editor): keyboard editing on the folder view`

### Task G4: Editor editing gestures — drag-reorder and bullet-zoom

**Files:**
- Modify: `crates/waml-editor/src/folder_list.rs` (drag state over recorded row rects; bullet hit rect), `crates/waml-editor/src/folder_view.rs`
- Test: headless drop-index math; interaction verified visually

Drag arms on `FingerDown` over a row with `caps`-permitted reorder, commits
`RowOp::Reorder` on `FingerUp`, and EVERY armed drag clears on `FingerUp` including
the refused/out-of-bounds paths. Bullet-zoom: clicking a concept's bullet opens the
concept; a folder row's bullet opens that folder's view. Draw-time rects and
event-time positions must be the same coordinate space (beware `Size::Fill` siblings).

**Tests:**
- Headless: `drop_index_from_pointer_y_is_correct_at_boundaries` (first, last,
  between, on-self is a no-op); `a_refused_reorder_leaves_row_order_unchanged`.
- **Visual verification required and stated:** drag ghost tracks the pointer;
  drop reorders; bullet-zoom opens the right target; no stuck drag state after an
  aborted drag.

- [ ] Failing headless tests → implement → targeted pass → full gate → visual verify.
- [ ] Commit: `feat(editor): drag-reorder and bullet-zoom on the folder view`

---

## Spec test → task map (completeness check)

| Spec Testing bullet | Task |
|---|---|
| Index parse: promotion, `extra`, no-frontmatter unchanged | A2 |
| Round-trip incl. unknown keys | A3 |
| `resolved_profile` self/nearest/none | C2 |
| `resolved_view` steps in isolation; local beats profile default | C2 |
| Identity chain equals plain OKF listing, row for row, in order | B7 |
| `hide` drops exactly matches, forwards every op | F1 (+ G2 against real apply) |
| Depth cap trips; self-descender terminates with diagnostic row | B6 |
| Cycle guard trips on first revisit | B6 |
| Failing stage discards earlier output → root view + spanned diagnostic | B5 |
| Bundle `max_view_depth` ignored | A2 + B4 + B6 |
| `RowId` stable across re-projection | B5 |
| Every minted path resolves on a later run (unless excluded) | B8 (sweep reused in F1) |
| Unresolvable path → nearest resolvable prefix | B8 |
| `hide` does not forward `resolve` for a hidden path | F1 |
| Raw OKF layer resolves a path the chain does not | D3 (+ F1) |
| Declared capability never `Unsupported` (property test) | G2 |
| Surface resolution total for `surface: None` + real target | E2 |
| Unknown `SurfaceId` degrades with diagnostic, never blank | E2 |
| Virtual row with `surface: None` rejected at construction | B2 |
| `CoreExtension`s load with no `EditorExtension` | E1 |
| Gate: reachable middleware has an editor half | E4 |
| Root-view `apply` op batch + consistent index.md files | G2 |
| Visual: chevron vs row body | D2 |
| Visual: opaque folder shows no descendants | D2 (add to the visual checklist: a take-over double or a `hide: ["**"]` folder shows none) |
| Visual: diagnostics strip + tree marker on failing chain | D2 |

## Outstanding visual verification

Filled in as GUI tasks land gate-green with `Visual-Check: pending`. Walk this list
in a real window before signing the plan off; a green gate is not evidence for any
line here.

| Task | What a human must see |
|---|---|
| D1b | A fixture folder opens; titles + blurbs render in authored order; a concept row opens the document. |
| D2 | Chevron folds without opening; row body opens without folding; the diagnostics strip and the tree marker appear for a folder whose `view:` names an unknown middleware, and its fallback rows still render. An opaque folder (`hide: ["**"]`) shows no descendants. |
| D3 | The raw affordance opens the full listing for a folder whose chain hides rows, and the raw-mode label is present. |
| G3 | Enter creates and focuses a new row; typing retitles live; Tab reparents. |
| G4 | Drag ghost tracks the pointer; drop reorders; bullet-zoom opens the right target; no stuck drag state after an aborted drag. |

## Open questions (carried forward, NOT resolved here)

1. **Tab on a concept with no preceding sibling directory.** Promote `orders.md` to
   `orders/index.md` (structural change on a keystroke) vs refuse. **Affects Task G3
   only.** G3 ships the conservative refusal and names it interim; revisit before
   sign-off on Phase G.
2. **Middleware inspectability.** "Why is this row here / missing" cannot be answered
   from `index.md` once a chain runs. A per-folder debug listing (chain resolved,
   per-stage row delta) is the likely answer. **Not scoped here; it should land
   before the middleware set grows past `hide`** — if it is picked up, it would hang
   off the runner (Task B5's `ChainOutcome`) and a small editor surface next to D2.
