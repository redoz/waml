# Issue 30 — Move UML syntax-highlighting classification out of core analysis.rs

## Context

`crates/waml/src/analysis.rs` (1,916 lines at HEAD 258e6392) is the core
catalog/session module, yet it hosts a UML-specific highlighting subsystem
that is parameterized over `crate::uml::syntax::UmlLanguage`:

- `WamlCodeSyntaxSnapshot` — analysis.rs:331 (holds `Arc<SyntaxTree<UmlLanguage>>`)
- `parse_fenced_waml_syntax` — analysis.rs:355
- `collect_waml_code_spans` — analysis.rs:371
- `token_content_range` — analysis.rs:403
- `waml_code_role` — analysis.rs:424 (~110 lines of `UmlSyntaxKind` matching, through ~line 540)
- `OkfAnalysis::attach_code_syntax` — analysis.rs:234 (builds the snapshot map from `crate::uml::Analysis`)

The module also carries plugin scaffolding sized for N specializations while
hard-coding the single one:

- `AnalysisStage::Specialization(&'static str)` — analysis.rs:598, only ever
  constructed as `Specialization("uml")` (analysis.rs:897, tests at 1054, 1066, 1796, 1817, 1869)
- `validate_disjoint_claims([("uml", &uml.claims)])` — analysis.rs:910, a
  disjointness check over a one-element list

Public consumers of the role types go through `waml::analysis`:
`WamlCodeRole` / `MarkdownTokenRole` in `crates/waml-cli/src/lsp/query.rs:8,200-207`
and `crates/waml/tests/incremental_analysis.rs:8`.

## Verdict evidence (APPROVE)

All cited symbols still exist at the lines above; recent commits
(258e6392 rustfmt, 43c69f28 / f9144554 fenced-WAML role work) grew the
UML-classification block rather than relocating it. `waml_code_role` imports
`crate::uml::syntax::UmlSyntaxKind` directly into core analysis — the exact
layering the specialization stages exist to prevent.

## Ordering / conflict flags

- **HARD COLLISION with issue 34 Task 4.**
  `issue-34-hot-path-costs.md` Task 4 adds a precomputed
  `spans: Arc<[WamlCodeSpan]>` field to `WamlCodeSyntaxSnapshot` and rewrites
  `OkfAnalysis::code_spans` — while THIS plan moves `WamlCodeSyntaxSnapshot`
  out of `analysis.rs` into `uml/highlight.rs`. **Land this plan first**, then
  issue 34 Task 4 targets the type in its new home (`uml/highlight.rs`) and
  `OkfAnalysis::code_spans` in `analysis.rs`. Running them concurrently
  guarantees a conflict on every hunk.
- **issue 31 Task 1** (`Display for AnalysisError`, `analysis.rs:660`) is a
  disjoint region of the same file — safe in either order, but do not run
  concurrently.

## Design decisions

1. **Move the mechanism, keep the vocabulary.** `WamlCodeRole`,
   `WamlCodeSpan`, `MarkdownTokenRole`, `MarkdownTokenSpan` are
   language-neutral span vocabulary consumed by the LSP; they stay in
   `analysis.rs` (no downstream churn in waml-cli or tests).
2. **New module `crates/waml/src/uml/highlight.rs`.** Everything typed over
   `UmlLanguage` moves there: `WamlCodeSyntaxSnapshot`,
   `parse_fenced_waml_syntax`, `collect_waml_code_spans`,
   `token_content_range`, `waml_code_role`. Visibility `pub(crate)` — no new
   public API.
3. **`attach_code_syntax` becomes a thin delegate.** The body that walks
   islands + fenced blocks moves to
   `uml::highlight::build_code_syntax(markdown: &MarkdownSyntaxSet, uml: &uml::Analysis) -> BTreeMap<SyntaxIdentity, WamlCodeSyntaxSnapshot>`;
   `OkfAnalysis` keeps its `code_syntax` field and its query methods
   (`code_spans`, `document_code_spans`) since those are catalog-level reads.
4. **Do NOT generalize the claims machinery.** Per the issue's suggested fix,
   `AnalysisStage::Specialization`, `ClaimSet`, `validate_disjoint_claims`
   stay exactly as they are — no registry, no trait, no second plugin — until
   a second specialization is real. No code change there.
5. **Pure move, no behavior change.** The gate (existing tests
   `crates/waml/tests/incremental_analysis.rs` and the in-module analysis
   tests) is the correctness oracle; no test rewrites beyond import paths.

### Task 1: Create uml/highlight.rs and move the classification quintet

- Create `crates/waml/src/uml/highlight.rs`.
- Move from `crates/waml/src/analysis.rs` verbatim (adjusting paths
  `crate::uml::syntax::X` -> `super::syntax::X`):
  `WamlCodeSyntaxSnapshot` (struct + `code_spans` impl),
  `parse_fenced_waml_syntax`, `collect_waml_code_spans`,
  `token_content_range`, `waml_code_role`.
- Mark items `pub(crate)` (struct fields too, or add a constructor).
- Register `pub(crate) mod highlight;` in `crates/waml/src/uml.rs`.
- Fix imports in `analysis.rs` (`use crate::uml::highlight::WamlCodeSyntaxSnapshot;`);
  remove now-unused `SyntaxNode`/`SyntaxToken`/`SyntaxElement` imports if
  nothing else in the file uses them.
- Test: `cargo test -p waml` green, specifically `incremental_analysis.rs`
  fenced/island role assertions unchanged.

### Task 2: Move the snapshot-building walk into highlight::build_code_syntax

- Extract the body of `OkfAnalysis::attach_code_syntax` (analysis.rs:234-295)
  into `pub(crate) fn build_code_syntax(markdown: &MarkdownSyntaxSet, uml: &crate::uml::Analysis) -> BTreeMap<SyntaxIdentity, WamlCodeSyntaxSnapshot>`
  in `uml/highlight.rs`.
- `attach_code_syntax` shrinks to
  `self.code_syntax = Arc::new(crate::uml::highlight::build_code_syntax(&self.markdown, uml));`
  (or is inlined at its single call site in the analyze pipeline — prefer
  whichever leaves analysis.rs smaller; keep the method if it has >1 caller).
- `parse_fenced_waml_syntax` becomes private to `highlight.rs` again.
- Test: full `cargo test --workspace` gate; confirm waml-cli LSP semantic
  token tests still pass (they consume `WamlCodeRole` via `waml::analysis`).

### Task 3: Verify the boundary and record the non-extension of claims

- Grep check: `analysis.rs` no longer mentions `UmlLanguage`,
  `UmlSyntaxKind`, or `uml::syntax` except via the single
  `build_code_syntax` call and the existing `Specialization("uml")` stage
  hook. Expected size drop: roughly 300 lines (snapshot struct + walk +
  classifier + fence parser).
- Confirm no `pub` surface was added to `waml::uml` (module is
  `pub(crate)`), and `waml::analysis` re-exports are unchanged.
- Leave `ClaimSet` / `validate_disjoint_claims` / `AnalysisStage` untouched;
  add no registry. If a reviewer asks, cite this plan's design decision 4.
- Gate: `cargo test --workspace` plus the vscode extension test/lint/build.
