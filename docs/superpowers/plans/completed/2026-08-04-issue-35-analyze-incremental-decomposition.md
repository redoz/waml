# Issue 35 — Decompose `analyze` and `reparse_okf_markdown_with_structure`

## Context

Two long functions have grown into several distinct jobs each. Both are covered
only through golden/property suites that run the whole pipeline; their high
crap scores reflect untestable shape, not zero coverage. This plan is
**mechanical extraction only** — no behaviour change, no new diagnostics, no
signature changes visible outside the two crates.

### Sub-item 1 — `analyze` (crates/waml/src/uml/analysis.rs:248-700)

Verified at HEAD (file is 3534 lines; `analyze` spans exactly 248-700). The
function is four jobs in one body:

1. **Island reuse / island-tree recovery** (~:311-393): per-island reuse check
   against the previous analysis (`translate_unchanged` on the change map),
   falling back to `parse_authoritative_island`.
2. **Declared-bundle extraction** (~:394-404, :506-618): collects per-kind
   syntax items and maps them through the existing `declared_*` family
   (`declared_value`, `declared_slot`, `declared_relationship`, … fifteen
   sibling categories, all one-line `.map(declared_x)` calls).
3. **Inline attribute lowering** (:405-505): ~100 lines of open code building
   `DeclaredAttribute` (name/type/multiplicity/visibility fields) inline —
   the lone violation of the `declared_*` pattern that the same file
   establishes fifteen times over. The file contains both the pattern and its
   violation as competing precedents.
4. **Parser-diagnostic translation** (:619-667): maps
   `UmlSyntaxDiagnosticCode` → `DiagCode` with line/col resolution and span
   provenance.

`validate_declared_semantics` (:976) and `declared_projection` (:1476) already
exist as functions but are only exercised through `analyze` under golden
suites; extraction of the four jobs above makes `analyze` a short orchestrator
and unlocks isolated tests for the extracted pieces.

### Sub-item 2 — `reparse_okf_markdown_with_structure` (crates/waml-syntax/src/incremental.rs:667-1009)

Verified at HEAD (file is 1634 lines). The `full(reason)` fallback closure is
defined at :774 — **after** two hand-expanded copies of exactly its body:

- :686-699 (the `recover_exact_source` failure path, hardcoding
  `FullReparseReason::UnsafeSynchronization`)
- :703-717 (the `ChangeMap::checked` error path, using the returned `reason`)

Both copies call `parse_with_structure(new_text, dialect, new_structure.clone())`
and wrap the result in `ReparseOutcome::Full { tree, reason }` — the closure's
body verbatim minus the `new_text.clone()`. They will drift the first time the
full-reparse path changes. After the closure at :774, roughly 25 sites
`return full(FullReparseReason::…)` (e.g. :808, :825, :832, :835, :838, :841,
:850, …, :975), which is the shape `plan_window_reparse(...) ->
Result<WindowPlan, FullReparseReason>` turns into `?`.

## Verdict evidence

- Recent commits touching these files (`e5b4ccb0`, `258e6392`, `165470e2`,
  `72486dfb`, `d30af731`) restructured neither function; both cited shapes
  exist byte-for-byte at the cited lines.

## Ordering / conflict flags (do not fold into this plan)

- **issue-26 (approved draft, `issue-26-concept-path-index.md`)** changes the
  signatures of `validate_declared_semantics` and `declared_projection` and
  edits the call sites at analysis.rs:675-676 plus the index block at
  :266-272 — inside the region Task 1-4 restructures. **Land issue-26 (and
  issue-27, which also cites analysis.rs line numbers) before this plan, or
  rebase this plan's analysis.rs tasks over them.** The extractions here are
  mechanical either way, but line references below will drift.
- **issue-29 (`issue-29-panics-and-catchalls.md`) Task 2** edits
  `analysis.rs:651` (the `UmlSyntaxDiagnosticCode` catch-all) — which sits
  inside the diagnostic-translation block Task 2 below extracts, and **Task 1**
  edits `sequence.rs:1294`. Land **26 → 27 → 29 → this plan**.
- **issues 21/22** touch waml-syntax `reparse.rs`, `inline.rs`, `block.rs` —
  not `incremental.rs`. No file overlap with sub-item 2; safe in either order.
- **issue-28 and issue-29 DO touch `incremental.rs`.** Issue 29 Task 3 converts
  `rebuild` to return `Result<_, ParseError>`; issue 28 tasks A and D rewrite
  `frontmatter_fences` and strengthen the debug oracle at :981-1000 — the latter
  sits inside the region Task 6 extracts. **Land 29 (T3) → 28 → this plan's
  tasks 5-6**, so the largest restructure goes last.
- Within this plan, the two sub-items touch different crates and are fully
  independent; tasks 5-6 can land before or after tasks 1-4.

## Folded in from issue 36 sub-item 9

`issue-36-small-consolidations.md` deferred its sub-item 9 to this plan on the
grounds that this plan IS the "next touch" of `crates/waml/src/uml/analysis.rs`.
Address it here, as part of Task 4's read-through:

- `Analysis` (`analysis.rs:21-33`) mixes six `pub` fields with five
  getter-wrapped private ones and no rule distinguishing them. Pick one
  discipline and apply it to all eleven; state the rule in a doc comment on the
  struct.
- Unique-basename disambiguation exists in three partial variants —
  `analysis.rs:172-176`, `lower.rs:137-144`, and `lower.rs:1424-1428`. Extract
  one helper consumed by all three. If the three variants turn out to differ
  behaviourally rather than incidentally, do NOT force them together: record the
  difference in a comment at each site and leave them, the same way issue 36
  Task 5 handles the two minimal-diff implementations.

## Design decisions

- **Extraction only.** Each new function is a straight cut-and-paste of the
  existing block with the minimal parameter set the block already reads.
  No logic edits, no diagnostic wording changes, no new error variants.
- **Naming follows the file's own precedent**: the inline attribute block
  becomes `declared_attribute(...)` (singular, matching `declared_flow_node`,
  `declared_lifeline`, etc.), called via the same `.into_iter().map(...)`
  shape as its fifteen siblings where signatures allow (it needs `context` and
  `document`, so a closure adapter or explicit loop calling the free function
  is acceptable — pick whichever avoids borrow contortions).
- **Hoist before extract** in incremental.rs: first replace the two
  hand-expanded copies with the (moved-up) helper so there is exactly one
  full-fallback body, then extract the planning phase. Two commits, each
  independently green.
- `plan_window_reparse` returns `Result<WindowPlan, FullReparseReason>`;
  hard errors (`ParseError::StructuralInvariant`, `WidthOverflow`) stay in the
  caller or come back as `Result<Result<WindowPlan, FullReparseReason>, ParseError>`
  — prefer a small enum or nested Result over widening `FullReparseReason`,
  which is public API surface.
- Golden/property suites are the safety net: `cargo test --workspace` must be
  bit-identical green after every task; no snapshot re-blessing is acceptable
  in this plan.

## Tasks

### Task 1: Extract inline attribute lowering to `declared_attribute`

- File: `crates/waml/src/uml/analysis.rs`
- Cut :405-505 into
  `fn declared_attribute(context: &DomainAnalysisContext<'_>, document: &…, syntax: …) -> DeclaredAttribute`
  placed next to the other `declared_*` free functions (near :2517
  `declared_flow_node`). The body needs `context.okf` (concept resolution for
  type hrefs) and `document.path()`; pass exactly those if the full context
  borrow fights the loop.
- Replace the open-coded loop with a `.map`/loop over the new function,
  mirroring the sibling call shape at :553-614.
- Tests: add unit tests in the same file's test module (or the existing uml
  analysis test module) covering: valid attribute, empty name → `Incomplete`,
  missing type → `Incomplete`, malformed multiplicity → `Invalid` with
  `MalformedAttribute`, unresolvable href → `TypeRef { ref_: None }`.
  Build inputs by parsing a small island fixture, not by hand-building syntax.
- Gate: `cargo test --workspace` green, no golden churn.

### Task 2: Extract parser-diagnostic translation

- File: `crates/waml/src/uml/analysis.rs`
- Cut :619-667 into
  `fn translate_parser_diagnostics(document: &…, id: DocumentId, tree: &…, diagnostics: &mut Vec<Diagnostic>) -> Result<(), AnalysisError>`
  (name to taste; keep the `AnalysisError::CatalogInvariant` line-index error
  paths verbatim).
- Tests: unit test the `UmlSyntaxDiagnosticCode → DiagCode` mapping table and
  the same-line vs cross-line span choice (:659-663) directly.
- Gate: workspace green, no golden churn.

### Task 3: Extract island reuse into a helper

- File: `crates/waml/src/uml/analysis.rs`
- Cut the per-island loop body (~:321-393: reversed-range check, reuse filter
  chain, `parse_authoritative_island` fallback) into
  `fn recover_island_tree(...) -> Result<SyntaxTree<UmlLanguage>, AnalysisError>`
  (or a helper returning `(key, tree, snapshot-entry)` if the surrounding
  inserts make that cleaner). Keep the seam-invariant comment with the code
  it explains.
- Tests: reuse-hit vs reuse-miss (changed range) via two analyses of the same
  fixture with a text edit between, asserting `Arc::ptr_eq` on the reused
  island tree — this is the isolated test the island cache never had.
- Gate: workspace green.

### Task 4: Shrink `analyze` to an orchestrator

- File: `crates/waml/src/uml/analysis.rs`
- After tasks 1-3, `analyze` should read as: validate context → build indices
  → per-concept: recover islands, extract declared bundle, translate
  diagnostics → `validate_declared_semantics` → `declared_projection` →
  metadata. If any residual open-coded block over ~30 lines remains in the
  per-concept body, extract it the same way; otherwise this task is only a
  read-through plus doc comment on `analyze` naming the phases.
- Gate: workspace green; `analyze` body ideally under ~150 lines.

### Task 5: Hoist the `full` fallback in incremental.rs

- File: `crates/waml-syntax/src/incremental.rs`
- Move the closure at :774 up to just after `new_structure` is built (:681),
  or convert it to a local `fn full_reparse(new_text: &SourceText, dialect: …,
  new_structure: &Arc<MarkdownStructureMap>, reason: FullReparseReason) -> Result<…, ParseError>`
  — then replace the hand-expanded copies at :686-699 and :703-717 with calls
  to it. One fallback body remains in the function.
- Tests: existing incremental property suite covers both fallback paths
  (`UnsafeSynchronization` on unrecoverable source, checked-map errors);
  assert nothing new — the suite passing unchanged is the proof.
- Gate: `cargo test -p waml-syntax` then workspace green. Do not commit
  `proptest-regressions`.

### Task 6: Extract `plan_window_reparse`

- File: `crates/waml-syntax/src/incremental.rs`
- Introduce a private `struct WindowPlan { … }` carrying whatever the
  post-planning tail of `reparse_okf_markdown_with_structure` consumes
  (reparse window range, affected islands/nodes — read the tail :1009-… to
  fix the exact fields before cutting).
- Cut the planning phase (the region containing the ~25
  `return full(reason)` sites, :789-~1009) into
  `fn plan_window_reparse(...) -> Result<WindowPlan, FullReparseReason>`,
  turning each `return full(X)` into `return Err(X)` — most collapse to `?`
  or `.ok_or(...)?`. Hard `ParseError`s in that region (e.g. shell_map
  failures) either stay in the caller before/after the call, or the function
  returns `Result<Result<WindowPlan, FullReparseReason>, ParseError>`;
  document the choice at the definition.
- Caller becomes: `match plan_window_reparse(...) { Ok(plan) => …tail…, Err(reason) => full(reason) }`.
- Tests: the planning function is now testable without executing the reparse —
  add at least one direct test per boundary class
  (`IslandBoundaryChanged`, `FrontmatterBoundaryChanged`,
  `HeadingBoundaryChanged`, `MarkdownContainerBoundaryChanged`) asserting the
  returned reason, using small markdown fixtures and a single `TextChange`.
- Gate: full workspace green; run the incremental proptest suite explicitly.
