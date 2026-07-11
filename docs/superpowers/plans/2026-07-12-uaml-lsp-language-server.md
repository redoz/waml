# UAML Language Server (`uaml lsp`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reshape the UAML parser to parse-and-report in one pass (returning spanned diagnostics), then ship `uaml lsp` — a stdio language server with a thin VS Code client that delivers live diagnostics for UAML documents.

**Architecture:** Part 1 refactors `crates/uaml` so `parse(text) -> (Document, Vec<Diagnostic>)` emits syntactic diagnostics with byte spans, deletes `validate`'s duplicate scanner, and adds `link(&[Document]) -> Vec<Diagnostic>` for semantic (cross-document) diagnostics. Part 2 adds a `Command::Lsp` subcommand backed by a new `crates/uaml-cli/src/lsp/` module (tower-lsp + tokio) that globs a workspace bundle, overlays open-buffer text, re-runs `parse` + `link` on edit, and publishes diagnostics; a thin `@uaml/vscode` extension spawns it. Core stays byte-based; byte→UTF-16 conversion happens only in the LSP layer.

**Tech Stack:** Rust (edition 2021, rust 1.80), `pulldown-cmark` 0.12, `regex` 1, `clap` 4; `tower-lsp` + `tokio` for the server; TypeScript 5.6 / Node 22 / pnpm workspace + `vscode-languageclient` for the editor extension.

## Global Constraints

- **Rust workspace:** `edition = "2021"`, `rust-version = "1.80"`, `license = "Apache-2.0"` (do NOT change the license — the AGPL relicense is a separate, later decision).
- **Core crate (`crates/uaml`) stays LSP-free and dependency-frozen for Part 1:** only `regex = "1"` and `pulldown-cmark = { version = "0.12", default-features = false }`. No `tokio`/`tower-lsp` in `crates/uaml`.
- **Diagnostic spans are BYTE offsets, relative to the diagnostic's own `line`.** `span: Option<(usize, usize)>`; `None` means "whole line". Byte→UTF-16 conversion happens ONLY in the LSP layer (`crates/uaml-cli/src/lsp/`), never in the core.
- **Syntactic vs semantic split is load-bearing:** `MalformedAttribute`, `MalformedRelationship`, `MalformedLayout`, `DroppableContent`, `FrontmatterNotClean`, `UnknownType` are produced inside `parse` (one document). `UnresolvedTarget`, `DuplicateSlug`, `UnresolvedLayoutRef`, `LayoutCycle` are produced inside `link` (whole bundle), walking the parsed tree — never re-scanning raw text.
- **`fmt` skip-on-error contract:** any file with an Error-severity diagnostic is left byte-for-byte untouched (`plan_fmt`, `commands.rs`). This must stay true after the refactor.
- **The existing `validate` test suite (`validate.rs`) is the safety net:** its assertions on codes and lines must keep passing, extended to assert spans.
- **TypeScript:** pnpm workspace member under `packages/*`, `"type": "module"`, extends `tsconfig.base.json`, `license: Apache-2.0`, Node ≥ 20 (repo pins 22).
- **No `Co-Authored-By: Claude` / "Generated with Claude Code" trailer on any commit** (repo standing rule).

## Design decisions & assumptions (reviewer: please confirm)

These resolve ambiguities in the spec. They keep the diff bounded while honoring every stated invariant.

1. **`parse` is the new primary entry; `parse_document` is retained as a thin wrapper.** `pub fn parse(text) -> (Document, Vec<Diagnostic>)` is added; `pub fn parse_document(text) -> Document` becomes `parse(text).0`. This avoids churning the many `Document`-only callers (`ops/mod.rs`, `ops/rename.rs`, `serialize` tests, `golden.rs`).
2. **`validate(bundle) -> Vec<Diagnostic>` is retained as the combined (syntactic + semantic) entry** so `uaml check` and `plan_fmt` need only minimal edits. Internally it now does: parse each doc (collect syntactic diagnostics) + `link` over the parsed docs (collect semantic diagnostics). The old `validate_doc`/`validate_diagram_refs` raw re-scan is deleted.
3. **"Error nodes" are recorded as spanned diagnostics emitted by `parse`, not by converting `Section` item vecs into `Vec<Result<T, _>>`.** `Section::Attributes(Vec<Attribute>)` / `Values` keep holding only well-formed items; every dropped line now yields a diagnostic, so `parse` is no longer *silently* lossy. This satisfies "no `filter_map` silent drops" without rippling an enum through `build_model`/`serialize`. (`fmt` skips any error file anyway, so error lines are never serialized.)
4. **Only relationship / member / layout nodes gain position info,** because only their diagnostics (`UnresolvedTarget`, `UnresolvedLayoutRef`, `LayoutCycle`) are emitted later, in `link`, and must reuse the parser's recorded position. `ParsedRel` and `MemberLine` gain `line: usize` + `span: Option<(usize, usize)>`; `Section::Layout` becomes `Vec<LayoutItem>` where `LayoutItem { line: usize, stmt: LayoutStatement }`. Attributes/values need no position (no semantic code references them).
5. **`UnresolvedTarget` spans are computed by string-search of the reconstructed `[Title](./slug.md)` on the held line** (the spec's "single-token, string search" tier), done in `parse` when it holds the raw line. Layout refs stay line-level (`span: None`) in Phase 1, but point at the offending statement's line (not the `## Layout` heading).
6. **The VS Code client lives in `packages/vscode` (`@uaml/vscode`)**, a pnpm workspace member matching existing `packages/*` conventions.
7. **`tower-lsp` + `tokio` are added only to `crates/uaml-cli`.** Suggested pins: `tower-lsp = "0.20"`, `tokio = { version = "1", features = ["rt-multi-thread", "io-std", "macros"] }`. If newer majors are current at implementation time, prefer the latest stable and adjust the trait/API calls in Tasks 11–12 accordingly.

## File structure

**Part 1 (modify):**
- `crates/uaml/src/diagnostic.rs` — `Diagnostic` gains `span`; add `with_span` builder.
- `crates/uaml/src/grammar.rs` — five line-parsers move `Option<T>` → `Result<T, LineError>`; `rel_error_message`/`has_multiplicity_ends` move here from `validate.rs`; new `LineError` type + `bullet_range` helper.
- `crates/uaml/src/layout.rs` — `parse_layout_line` moves `Option` → `Result<LayoutStatement, LineError>`.
- `crates/uaml/src/syntax.rs` — `ParsedRel` and `MemberLine` gain `line`/`span`; new `LayoutItem`; `Section::Layout(Vec<LayoutItem>)`.
- `crates/uaml/src/parse.rs` — `classify`/`parse_document` become `parse(text) -> (Document, Vec<Diagnostic>)` with a fence-aware content-line walk that emits syntactic diagnostics; `build_model` reads the new fields.
- `crates/uaml/src/serialize.rs` — `render_section` reads `Section::Layout(Vec<LayoutItem>)`.
- `crates/uaml/src/validate.rs` — `validate_doc` deleted; add `link(&[Document]) -> Vec<Diagnostic>`; `validate(bundle)` re-expressed as parse + link.
- `crates/uaml-cli/src/commands.rs` — `plan_fmt` uses `parse` for the per-file skip decision; JSON DTO gains `span`.

**Part 2 (create unless noted):**
- `crates/uaml-cli/Cargo.toml`, root `Cargo.toml` — add `tower-lsp`, `tokio` (modify).
- `crates/uaml-cli/src/main.rs` — `Command::Lsp { stdio: bool }` + dispatch (modify).
- `crates/uaml-cli/src/lsp/mod.rs` — module root + `run()` stdio entrypoint.
- `crates/uaml-cli/src/lsp/map.rs` — pure Diagnostic→LSP mapping, byte→UTF-16, UAML filter.
- `crates/uaml-cli/src/lsp/bundle.rs` — in-memory bundle overlay + recompute.
- `crates/uaml-cli/src/lsp/server.rs` — `tower-lsp` `LanguageServer` impl.
- `crates/uaml-cli/tests/lsp_e2e.rs` — stdio end-to-end test.
- `packages/vscode/` — `package.json`, `tsconfig.json`, `src/extension.ts`, `.vscodeignore` (thin client).

---

## Part 1 — Parser returns diagnostics (independent PR)

### Task 1: `Diagnostic` gains a byte span

**Files:**
- Modify: `crates/uaml/src/diagnostic.rs`
- Test: `crates/uaml/src/diagnostic.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `Diagnostic { severity, code, message, file, line, span: Option<(usize, usize)> }`; `Diagnostic::new`/`warn` unchanged 4-arg signatures (set `span: None`); new builder `fn with_span(self, span: (usize, usize)) -> Diagnostic`.

- [ ] **Step 1: Write the failing test** — append to `mod tests` in `diagnostic.rs`:

```rust
    #[test]
    fn span_defaults_to_none_and_with_span_sets_it() {
        let d = Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 3);
        assert_eq!(d.span, None);
        let d = d.with_span((2, 10));
        assert_eq!(d.span, Some((2, 10)));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml span_defaults_to_none_and_with_span_sets_it`
Expected: FAIL — compile error, no field `span` / no method `with_span`.

- [ ] **Step 3: Add the field and builder.** In `diagnostic.rs`, add `pub span: Option<(usize, usize)>,` as the last field of `struct Diagnostic`; set `span: None` in both `new` and `warn`; add the builder:

```rust
    /// Attach an intra-line byte span `(col_start, col_end)` (relative to `line`).
    pub fn with_span(mut self, span: (usize, usize)) -> Diagnostic {
        self.span = Some(span);
        self
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml diagnostic`
Expected: PASS — all `diagnostic` tests green (existing `Diagnostic::new`/`warn` call sites still compile because they set `span: None`).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/diagnostic.rs
git commit -m "feat(uaml): add optional byte span to Diagnostic"
```

### Task 2: `LineError` type + convert grammar line-parsers to `Result`

Moves `parse_attribute_line`, `parse_value_line`, `parse_relationship_line`, `parse_member_line` off `Option<T>`, and relocates the relationship error-message logic (`rel_error_message`, `has_multiplicity_ends`) from `validate.rs` into `grammar.rs`. Each error carries a byte range relative to the (untrimmed) input line and a message.

**Files:**
- Modify: `crates/uaml/src/grammar.rs`
- Test: `crates/uaml/src/grammar.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `LineError` (defined here).
- Produces:
  - `pub struct LineError { pub range: (usize, usize), pub message: String }`
  - `pub fn parse_attribute_line(line: &str) -> Result<Attribute, LineError>`
  - `pub fn parse_value_line(line: &str) -> Result<String, LineError>`
  - `pub fn parse_relationship_line(line: &str) -> Result<ParsedRel, LineError>`
  - `pub fn parse_member_line(line: &str) -> Result<MemberLine, LineError>`
  - `pub fn bullet_range(line: &str) -> (usize, usize)` — first non-whitespace byte index to last non-whitespace byte index (the whole-bullet span).
  - `pub fn rel_error_message(line: &str) -> String` (moved from `validate.rs`).
  - Note: `parse_relationship_line`/`parse_member_line` still return `ParsedRel`/`MemberLine` with `line: 0, span: None` — position is filled later by `parse` (Task 5). This task does the signature move only.

- [ ] **Step 1: Write the failing tests** — append to `mod tests` in `grammar.rs`:

```rust
    #[test]
    fn attribute_error_carries_a_range_and_message() {
        let e = parse_attribute_line("- bad line without colon").unwrap_err();
        assert!(e.range.0 < e.range.1);
        assert!(!e.message.is_empty());
    }

    #[test]
    fn relationship_error_message_reports_missing_ends() {
        let e = parse_relationship_line("- composes [OrderLine](./order-line.md)").unwrap_err();
        assert!(e.message.contains("requires"), "got: {}", e.message);
    }

    #[test]
    fn bullet_range_spans_indent_to_content_end() {
        assert_eq!(bullet_range("- id: X"), (0, 7));
        assert_eq!(bullet_range("  - id: X  "), (2, 9));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml -- attribute_error_carries relationship_error_message bullet_range_spans`
Expected: FAIL — `unwrap_err` not available on `Option`; `bullet_range` undefined.

- [ ] **Step 3: Add `LineError` + `bullet_range`.** At the top of `grammar.rs` (after imports):

```rust
/// A line-parse failure: a byte range within the input line plus a message.
#[derive(Debug, Clone, PartialEq)]
pub struct LineError {
    pub range: (usize, usize),
    pub message: String,
}

/// Whole-bullet byte range: first to last non-whitespace byte of `line`.
pub fn bullet_range(line: &str) -> (usize, usize) {
    let start = line.find(|c: char| !c.is_whitespace()).unwrap_or(0);
    let end = line.trim_end().len();
    (start, end.max(start))
}
```

- [ ] **Step 4: Convert `parse_attribute_line` and `parse_value_line`.** Replace their bodies so every `return None` / `?`-on-`None` becomes `Err(LineError { range: bullet_range(line), message: "<specific>".into() })`. Compute `bullet_range` on the ORIGINAL `line` before any `trim`. Example shape for attributes:

```rust
pub fn parse_attribute_line(line: &str) -> Result<Attribute, LineError> {
    let err = |msg: &str| LineError { range: bullet_range(line), message: msg.to_string() };
    let trimmed = line.trim_end_matches('\r').trim();
    let caps = ATTR_RE.captures(trimmed).ok_or_else(|| err("malformed attribute line"))?;
    // ...unchanged capture extraction...
    // each former `?`/`return None` -> `.ok_or_else(|| err("malformed attribute line"))?`
    //                                   or `return Err(err("malformed attribute line"));`
    Ok(Attribute { name, ty, multiplicity, visibility, description: None })
}

pub fn parse_value_line(line: &str) -> Result<String, LineError> {
    let trimmed = line.trim_end_matches('\r').trim();
    VALUE_RE.captures(trimmed)
        .map(|c| c[1].trim().to_string())
        .ok_or_else(|| LineError { range: bullet_range(line), message: "malformed value line".into() })
}
```

- [ ] **Step 5: Move `has_multiplicity_ends` + `rel_error_message` into `grammar.rs`** (verbatim from `validate.rs:31-57`, made `pub` for `rel_error_message`), then convert `parse_relationship_line` to return `Result`, using `rel_error_message(line)` for the message and `bullet_range(line)` for the range on every failure path:

```rust
pub fn parse_relationship_line(line: &str) -> Result<ParsedRel, LineError> {
    let err = || LineError { range: bullet_range(line), message: rel_error_message(line) };
    let trimmed = line.trim_end_matches('\r').trim();
    let m = REL_RE.captures(trimmed).ok_or_else(err)?;
    let kind = RelationshipKind::parse(&m[1]).ok_or_else(err)?;
    let ends_raw = m.get(7).map(|x| x.as_str());
    if kind.is_ended() != ends_raw.is_some() { return Err(err()); }
    // ...unchanged name/ends extraction, each `?`/`return None` -> `.ok_or_else(err)?` / `return Err(err());`...
    Ok(ParsedRel { kind, target_title: m[2].to_string(), target_slug: basename(&m[3]).to_string(),
                   name, from_end, to_end, line: 0, span: None })
}
```

Note: the `ParsedRel { ..., line: 0, span: None }` fields are added in Task 4 (syntax.rs). Until Task 4 lands, keep this task's diff on a branch that includes Task 4, OR add the two fields to `ParsedRel` here as part of Step 5 (they are inert). To keep each task green, **do Task 4's `syntax.rs` field additions first if implementing strictly in isolation.** (See Task 4 ordering note.)

- [ ] **Step 6: Convert `parse_member_line`** to `Result<MemberLine, LineError>` (message `"malformed member line"`), constructing `MemberLine { title, slug, line: 0, span: None }`. Inside `parse_members_block`, the call `parse_member_line(t)` changes from `if let Some(m)` to `if let Ok(m)`.

- [ ] **Step 7: Fix grammar's own tests.** In `grammar.rs` `mod tests`, every `parse_*_line(..).is_none()` becomes `.is_err()`, and `.unwrap()` stays valid (now unwraps a `Result`). Update the `rejects_*` assertions accordingly.

- [ ] **Step 8: Run grammar tests**

Run: `cargo test -p uaml grammar`
Expected: PASS — all `grammar` tests green.

- [ ] **Step 9: Commit**

```bash
git add crates/uaml/src/grammar.rs crates/uaml/src/syntax.rs
git commit -m "refactor(uaml): grammar line-parsers return LineError instead of Option"
```

### Task 3: Convert `parse_layout_line` to `Result`

**Files:**
- Modify: `crates/uaml/src/layout.rs`
- Test: `crates/uaml/src/layout.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `LineError` (from Task 2, `crate::grammar::LineError`).
- Produces: `pub fn parse_layout_line(line: &str) -> Result<LayoutStatement, LineError>`. Internal helpers (`lex_layout`, `try_parse_placement`, `parse_operand`, …) keep returning `Option` — only the public entry changes; on any internal `None`, the public fn returns `Err(LineError { range: crate::grammar::bullet_range(line), message: "malformed layout statement".into() })`.

- [ ] **Step 1: Write the failing test** — append to `mod tests` in `layout.rs`:

```rust
    #[test]
    fn malformed_layout_line_is_an_err_with_range() {
        let e = parse_layout_line("- Users nonsense Orders").unwrap_err();
        assert!(e.range.0 < e.range.1);
        assert!(e.message.contains("layout"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml malformed_layout_line_is_an_err_with_range`
Expected: FAIL — `unwrap_err` not available on `Option`.

- [ ] **Step 3: Change the public signature.** Wrap the existing `Option` body:

```rust
pub fn parse_layout_line(line: &str) -> Result<LayoutStatement, crate::grammar::LineError> {
    parse_layout_line_opt(line).ok_or_else(|| crate::grammar::LineError {
        range: crate::grammar::bullet_range(line),
        message: "malformed layout statement".to_string(),
    })
}

/// The recursive-descent core (unchanged body of the former `parse_layout_line`).
fn parse_layout_line_opt(line: &str) -> Option<LayoutStatement> {
    let body = line.trim().strip_prefix("- ")?;
    // ...existing body verbatim...
}
```

- [ ] **Step 4: Fix layout's own tests.** Every `parse_layout_line(..).is_none()` becomes `.is_err()`; `.unwrap()` still works (now on `Result`). In `layout_lines_round_trip` and `reserved_keyword_bare_name_round_trips_quoted`, the `.unwrap_or_else(|| panic!(...))` calls now operate on `Result` — change to `.unwrap_or_else(|_| panic!(...))`.

- [ ] **Step 5: Run layout tests**

Run: `cargo test -p uaml layout`
Expected: PASS — all `layout` tests green.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml/src/layout.rs
git commit -m "refactor(uaml): parse_layout_line returns LineError instead of Option"
```

### Task 4: AST position fields — `LayoutItem`, `Section::Layout`, and node positions

Adds the position-carrying shapes the semantic `link` pass will read. `ParsedRel`/`MemberLine` already gained `line`/`span` in Task 2; this task adds the layout wrapper and updates `build_model` + `serialize` + affected tests to compile.

**Files:**
- Modify: `crates/uaml/src/syntax.rs`, `crates/uaml/src/parse.rs` (`build_model`, `classify`), `crates/uaml/src/serialize.rs`
- Test: `crates/uaml/src/syntax.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `LayoutStatement` (existing).
- Produces:
  - `pub struct LayoutItem { pub line: usize, pub stmt: LayoutStatement }`
  - `Section::Layout(Vec<LayoutItem>)` (was `Vec<LayoutStatement>`).
  - Confirms `ParsedRel { …, pub line: usize, pub span: Option<(usize, usize)> }` and `MemberLine { …, pub line: usize, pub span: Option<(usize, usize)> }` (added in Task 2).

- [ ] **Step 1: Write the failing test** — in `syntax.rs` `mod tests`:

```rust
    #[test]
    fn layout_item_wraps_a_statement_with_a_line() {
        let item = LayoutItem {
            line: 12,
            stmt: LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Bare("Orders".into())), axis: None, hints: vec![],
            }),
        };
        assert_eq!(item.line, 12);
        let _s = Section::Layout(vec![item]); // must typecheck
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml layout_item_wraps_a_statement_with_a_line`
Expected: FAIL — `LayoutItem` undefined; `Section::Layout(Vec<LayoutStatement>)` mismatch.

- [ ] **Step 3: Edit `syntax.rs`.** Add the struct and change the variant:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutItem {
    pub line: usize,
    pub stmt: LayoutStatement,
}
```

Change `Layout(Vec<LayoutStatement>)` → `Layout(Vec<LayoutItem>)` in `enum Section`. Confirm `ParsedRel` and `MemberLine` each carry `pub line: usize,` and `pub span: Option<(usize, usize)>,` (present from Task 2). Update the `document_is_constructible` test's `ParsedRel { … }` literal to include `line: 0, span: None`.

- [ ] **Step 4: Update `build_model` (`parse.rs`).** In `build_diagrams`, the `Section::Layout(stmts)` arm now yields `Vec<LayoutItem>`; strip to `Vec<LayoutStatement>` for the model:

```rust
                Section::Layout(items) => {
                    layout = items.iter().map(|it| it.stmt.clone()).collect();
                }
```

- [ ] **Step 5: Update `serialize.rs`.** In `render_section`, the `Section::Layout(stmts)` arm maps over `items`:

```rust
        Section::Layout(items) => {
            let body = items
                .iter()
                .map(|it| crate::layout::render_layout_line(&it.stmt))
                .collect::<Vec<_>>()
                .join("\n");
            if body.is_empty() { "## Layout".to_string() } else { format!("## Layout\n{body}") }
        }
```

- [ ] **Step 6: Update `classify` (`parse.rs`) temporarily** so the crate compiles before the Task 5 rewrite: wrap each parsed statement with `line: 0`:

```rust
        "layout" => Section::Layout(
            lines(content).iter()
                .filter_map(|l| crate::layout::parse_layout_line(l).ok())
                .map(|stmt| crate::syntax::LayoutItem { line: 0, stmt })
                .collect(),
        ),
```

Also, since Task 2/3 changed the other parsers to `Result`, update `classify`'s other arms to `.ok()` (`parse_attribute_line(l).ok()`, `parse_value_line(l).ok()`, `parse_relationship_line(l).ok()`). This keeps `parse_document` lossy-but-compiling until Task 5 replaces it.

- [ ] **Step 7: Run the whole core test suite**

Run: `cargo test -p uaml`
Expected: PASS — all existing `uaml` tests green (behavior unchanged; only shapes moved). `validate` tests still pass because `validate.rs` is unchanged and still owns the diagnostics.

- [ ] **Step 8: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/parse.rs crates/uaml/src/serialize.rs
git commit -m "refactor(uaml): add LayoutItem and position fields to layout/rel/member AST"
```

### Task 5: `parse(text) -> (Document, Vec<Diagnostic>)` — emit syntactic diagnostics with spans

The core of Part 1. `parse` keeps using pulldown-cmark for section boundaries (already fence-correct), then walks each section's content lines with a local fence tracker, calling the Task 2/3 `Result` parsers. Well-formed items go into the `Section`; every failure and every droppable line becomes a spanned `Diagnostic`. Frontmatter is scanned once for `UnknownType` and `FrontmatterNotClean`. `parse_document` is retained as `parse(text).0`.

**Files:**
- Modify: `crates/uaml/src/parse.rs`
- Test: `crates/uaml/src/parse.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `parse_attribute_line`/`parse_value_line`/`parse_relationship_line`/`parse_member_line`/`parse_layout_line` (all `Result`), `LineError`, `bullet_range`, `Diagnostic`, `DiagCode`, `LayoutItem`.
- Produces:
  - `pub fn parse(src: &str) -> (Document, Vec<Diagnostic>)` — the file argument on each `Diagnostic` is `""` here (the caller/`link` sets the path; see note). Diagnostics carry absolute `line` (1-based over `src`) and line-relative byte `span`.
  - `pub fn parse_document(src: &str) -> Document { parse(src).0 }`
  - Private helpers: `fn line_at(src: &str, byte: usize) -> usize` (1-based line of a byte offset); the content-walk that classifies + diagnoses.

- [ ] **Step 1: Write the failing tests** — append to `parse.rs` `mod tests`:

```rust
    #[test]
    fn parse_reports_malformed_attribute_with_span_and_line() {
        let src = "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- bad line without colon\n";
        let (_doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::MalformedAttribute).unwrap();
        assert_eq!(d.line, 8);
        let span = d.span.expect("malformed attribute must carry a span");
        assert!(span.0 < span.1);
    }

    #[test]
    fn parse_reports_unknown_type_on_frontmatter_line() {
        let src = "---\ntype: bpmn.Task\ntitle: X\n---\n# X\n";
        let (_doc, diags) = parse(src);
        let d = diags.iter().find(|d| d.code == DiagCode::UnknownType).unwrap();
        assert_eq!(d.line, 2);
        assert_eq!(d.severity, crate::diagnostic::Severity::Warning);
    }

    #[test]
    fn parse_of_a_clean_doc_has_no_diagnostics() {
        let src = "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n";
        let (_doc, diags) = parse(src);
        assert!(diags.is_empty(), "got: {diags:?}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml -- parse_reports_malformed_attribute parse_reports_unknown_type parse_of_a_clean_doc`
Expected: FAIL — `parse` undefined (only `parse_document` exists).

- [ ] **Step 3: Add the line-mapping helper.** In `parse.rs`:

```rust
/// 1-based line number of byte offset `byte` within `src`.
fn line_at(src: &str, byte: usize) -> usize {
    1 + src[..byte.min(src.len())].bytes().filter(|&b| b == b'\n').count()
}
```

- [ ] **Step 4: Rewrite `classify` into a diagnostic-emitting content walk.** Replace `classify` with a function that takes the section title, the section's content text, the content's absolute byte start in `src`, and `src`; returns `(Section, Vec<Diagnostic>)`. It:
  1. Splits `content` into lines, tracking each line's byte offset within `content` (so `abs = content_abs_start + line_offset`, `line_no = line_at(src, abs)`).
  2. Maintains a local fence tracker (the `Option<char>` logic moved from `validate.rs:102-116`): lines inside a fence are skipped for diagnostics AND for parsing.
  3. For the five bullet sections, calls the matching `Result` parser on each non-fenced line; `Ok(v)` pushes `v`, `Err(LineError { range, message })` pushes `Diagnostic::new(code, message, "", line_no).with_span(range)` with the section's code (`MalformedAttribute`/`MalformedRelationship`/`MalformedLayout`; value/notes use `MalformedAttribute`? — use a dedicated message but note: `Values`/`Notes` malformed lines are reported as `DroppableContent`, matching today's behavior where only attributes/relationships/members/layout have Malformed codes). For layout, wrap `Ok(stmt)` as `LayoutItem { line: line_no, stmt }`.
  4. Emits `DroppableContent` for non-blank, non-bullet lines inside bullet sections, and (for `members`) allows `###` group headings (the `is_member_group_heading` rule from `validate.rs:129`). Span = `bullet_range(line)`.
  5. Body/Notes/Unknown sections keep their current construction (no per-line diagnostics for free prose).

Reference the exact rules being moved: `validate.rs:123-144` (DroppableContent), `validate.rs:150-191` (per-section parser dispatch), `validate.rs:102-116` (fence tracker).

- [ ] **Step 5: Fill node positions for `link`.** When a relationship parses `Ok(rel)`, set `rel.line = line_no` and `rel.span = find_link_span(line, &rel)` where:

```rust
/// Byte range of `[Title](./slug.md)` within `line`, or the whole bullet.
fn find_link_span(line: &str, title: &str, slug: &str) -> (usize, usize) {
    let needle = format!("[{title}](./{slug}.md)");
    match line.find(&needle) {
        Some(s) => (s, s + needle.len()),
        None => crate::grammar::bullet_range(line),
    }
}
```

Do the same for member lines (`MemberLine.line`/`.span`). This is the "single-token, string-search" span tier.

- [ ] **Step 6: Rewrite `parse_document` into `parse`.** Keep the existing pulldown heading loop that builds `heads` (`parse.rs:46-93`). Then:
  1. Before the section loop, scan the frontmatter region of `src` for `UnknownType` (each `type:` line where `ClassifierType::parse(ty)` is `Unknown` and `ty != "Diagram"`, at its real line — reuse the logic from `validate.rs:89-99`) and `FrontmatterNotClean` (the `has_metadata_block` check from `validate.rs:60-67`; move `has_metadata_block` into `parse.rs`). `FrontmatterNotClean` gets `span: None`, `line: 1`.
  2. Emit `DroppableContent` for non-blank prose before the first `## ` section (excluding the H1 title line), per `validate.rs:127-143`.
  3. For each head, call the new content-walk, collecting `(Section, diags)`; accumulate all diagnostics.
  4. Return `(Document { frontmatter, title, sections }, diags)`.
  5. Add `pub fn parse_document(src: &str) -> Document { parse(src).0 }`.

- [ ] **Step 7: Update existing `parse.rs` tests** that referenced the old `Section::Layout(Vec<LayoutStatement>)` shape or `classify`: `builds_diagram_groups_and_layout` now matches `d.layout[0]` (model layer, unchanged) — no change needed there since `build_model` strips to `LayoutStatement`. The `parse_document` unit tests keep working via the wrapper.

- [ ] **Step 8: Run the core suite**

Run: `cargo test -p uaml parse`
Expected: PASS — the three new tests plus all existing `parse`/`model_tests` green.

- [ ] **Step 9: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): parse returns spanned syntactic diagnostics in one pass"
```

### Task 6: `link(&[Document]) -> Vec<Diagnostic>` and delete `validate`'s scanner

Deletes `validate_doc` and the raw-text re-scan in `validate_diagram_refs`; re-expresses `validate(bundle)` as: parse each doc (syntactic, stamping the file path) + `link` over the parsed docs (semantic). Semantic checks walk the parsed tree and reuse the positions recorded in Task 5.

**Files:**
- Modify: `crates/uaml/src/validate.rs`
- Test: `crates/uaml/src/validate.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `parse` (Task 5), `Document`, `Section`, `LayoutItem`, `ParsedRel`/`MemberLine` positions, existing helpers `collect_group_names`/`check_operand_refs`/`operand_key`/`has_cycle`.
- Produces:
  - `pub fn link(docs: &[(String, ClassifierType, Document)]) -> Vec<Diagnostic>` — semantic diagnostics: `DuplicateSlug`, `UnresolvedTarget` (relationships + members), `UnresolvedLayoutRef`, `LayoutCycle`. Uses each node's recorded `line`/`span`.
  - `pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic>` — unchanged signature; now = parse-per-doc (syntactic, with `d.file` set to the doc path) ++ `link(...)`.

- [ ] **Step 1: Write the failing test** (span assertion — the new guarantee) in `validate.rs` `mod tests`:

```rust
    #[test]
    fn unresolved_relationship_target_carries_a_span() {
        let b = vec![("a/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        let t = d.iter().find(|x| x.code == DiagCode::UnresolvedTarget).unwrap();
        assert_eq!(t.line, 8);
        let (s, e) = t.span.expect("unresolved target must span the link");
        assert!(s < e);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml unresolved_relationship_target_carries_a_span`
Expected: FAIL — `span` is `None` (today's `validate_doc` sets no span).

- [ ] **Step 3: Delete the scanner.** Remove `validate_doc` (`validate.rs:59-193`), `has_metadata_block` (moved to `parse.rs` in Task 5), `has_multiplicity_ends`/`rel_error_message` (moved to `grammar.rs` in Task 2), and the raw-text walk portion of `validate_diagram_refs`. Keep `collect_group_names`, `check_operand_refs`, `operand_key`, `has_cycle`, `slug_of`, `doc_type`.

- [ ] **Step 4: Implement `link`.** Iterate the parsed docs; build `keyset`/`slug_count` (classifiers only, as today). Then per doc:
  - `DuplicateSlug` when `slug_count[slug] > 1` (`line: 1`, `span: None`).
  - For each `Section::Relationships(rels)`: for each `rel` with `!keyset.contains(&rel.target_slug)`, push `Diagnostic::new(DiagCode::UnresolvedTarget, msg, path, rel.line)` then `.with_span(rel.span.unwrap_or(bullet_range))`. (Use `rel.span` if `Some`.)
  - For each `Section::Members`: walk groups; for each member with unresolved slug, `Diagnostic::warn(UnresolvedTarget, …, path, member.line).with_span(member.span…)`.
  - For each `Section::Layout(items)`: run `check_operand_refs` per `item.stmt`, reporting at `item.line` (span `None` — Phase 1). Build the horizontal/vertical graphs from `item.stmt` and, on `has_cycle`, report `LayoutCycle` at the first participating `item.line` (fallback: first layout item's line).

- [ ] **Step 5: Re-express `validate`.**

```rust
pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut docs = Vec::new();
    for (path, text) in bundle {
        let (doc, mut syn) = crate::parse::parse(text);
        for d in &mut syn { d.file = path.clone(); }
        diags.append(&mut syn);
        let ty = ClassifierType::parse(doc.frontmatter.get_str("type").unwrap_or("uml.Class"));
        docs.push((path.clone(), ty, doc));
    }
    diags.extend(link(&docs));
    diags
}
```

- [ ] **Step 6: Update semantic-diagnostic line expectations.** The moved-code tests keep their assertions; the layout-ref/cycle tests (`unknown_layout_ref_is_a_warning`, `contradictory_placement_is_a_cycle_error`) now assert the statement's real line instead of the `## Layout` heading line — update those `assert_eq!(line, …)` if present (they currently assert only the code, so likely no change). Keep every existing assertion on codes/severities green.

- [ ] **Step 7: Run the full `validate` suite + whole crate**

Run: `cargo test -p uaml`
Expected: PASS — the entire `validate` suite (`validate.rs` `mod tests`) green, plus the new span test.

- [ ] **Step 8: Commit**

```bash
git add crates/uaml/src/validate.rs
git commit -m "refactor(uaml): delete validate scanner; add semantic link pass over parsed docs"
```

### Task 7: CLI consumes spans; `fmt` skip-on-error preserved via `parse`

Surfaces the new `span` in `check --format json`, and switches `plan_fmt`'s per-file skip decision to `parse` (so it reads the syntactic diagnostics directly). Confirms the byte-for-byte skip contract still holds.

**Files:**
- Modify: `crates/uaml-cli/src/commands.rs`
- Test: `crates/uaml-cli/src/commands.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `uaml::parse::parse`, `uaml::validate::validate`, `Diagnostic { span }`.
- Produces: `DiagDto` gains `span: Option<(usize, usize)>`; `plan_fmt` unchanged public signature/behavior.

- [ ] **Step 1: Write the failing tests** in `commands.rs` `mod tests`:

```rust
    #[test]
    fn json_output_includes_span_when_present() {
        let diags = vec![
            Diagnostic::new(DiagCode::MalformedAttribute, "bad", "a.md", 8).with_span((2, 20)),
        ];
        let out = render_json(&diags);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["span"][0], 2);
        assert_eq!(v[0]["span"][1], 20);
    }

    #[test]
    fn plan_fmt_still_skips_error_files_byte_for_byte() {
        let original = "---\ntype: uml.Class\ntitle: A\n---\n# A\n\nDo not lose this sentence.\n\n## Attributes\n- id: AId\n";
        let files = vec![("x/a.md".to_string(), original.to_string())];
        let plan = plan_fmt(&files);
        assert!(plan[0].skipped);
        assert_eq!(plan[0].formatted, original);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml-cli -- json_output_includes_span plan_fmt_still_skips`
Expected: FAIL — `DiagDto` has no `span`; `with_span` fine (Task 1). (`plan_fmt` test may already pass — keep it as a regression guard.)

- [ ] **Step 3: Add `span` to the DTO.** In `commands.rs`:

```rust
#[derive(Serialize)]
struct DiagDto<'a> {
    severity: &'a str,
    code: &'a str,
    message: &'a str,
    file: &'a str,
    line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<(usize, usize)>,
}
```

Set `span: d.span` in the `render_json` mapping.

- [ ] **Step 4: Point `plan_fmt`'s skip decision at `parse`.** Keep `validate(files)` for cross-file behavior, but compute the per-file error flag from `parse` so it reads syntactic diagnostics without re-scanning:

```rust
pub fn plan_fmt(files: &[(String, String)]) -> Vec<FmtResult> {
    let bundle_diags = validate(files); // includes semantic (link) errors, e.g. duplicate-slug
    let mut out = Vec::new();
    for (path, text) in files {
        let (_doc, syn) = parse(text);
        let has_error = syn.iter().any(|d| d.severity == Severity::Error)
            || bundle_diags.iter().any(|d| d.file == *path && d.severity == Severity::Error);
        if has_error {
            out.push(FmtResult { path: path.clone(), formatted: text.clone(), changed: false, skipped: true });
            continue;
        }
        let formatted = serialize_document(&parse_document(text));
        let changed = formatted != *text;
        out.push(FmtResult { path: path.clone(), formatted, changed, skipped: false });
    }
    out
}
```

Add `use uaml::parse::parse;` to the imports (alongside the existing `parse_document`).

- [ ] **Step 5: Run the CLI suite + full workspace gate**

Run: `cargo test --workspace`
Expected: PASS — all `uaml` and `uaml-cli` tests green.

- [ ] **Step 6: Confirm the fmt regression manually (evidence for the invariant).**

Run: `cargo test -p uaml-cli -- skips_a_file_with_errors skips_a_file_with_pre_section_prose plan_fmt_still_skips`
Expected: PASS — error files are skipped and left byte-for-byte untouched.

- [ ] **Step 7: Commit (ends Part 1 — this is the PR boundary)**

```bash
git add crates/uaml-cli/src/commands.rs
git commit -m "feat(uaml-cli): surface diagnostic spans in check json; fmt skip via parse"
```

---

## Part 2 — `uaml lsp` server + thin VS Code client (independent PR, depends on Part 1)

### Task 8: Add `tower-lsp`/`tokio` deps and the `uaml lsp` subcommand skeleton

**Files:**
- Modify: root `Cargo.toml`, `crates/uaml-cli/Cargo.toml`, `crates/uaml-cli/src/main.rs`
- Create: `crates/uaml-cli/src/lsp/mod.rs`
- Test: `crates/uaml-cli/src/main.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing (scaffolding).
- Produces: `Command::Lsp { stdio: bool }` variant; `mod lsp;` with `pub fn run() -> i32` (stub that will be implemented in Task 11).

- [ ] **Step 1: Write the failing test** in `main.rs` `mod tests`:

```rust
    #[test]
    fn parses_lsp_stdio_subcommand() {
        let cli = Cli::try_parse_from(["uaml", "lsp", "--stdio"]).unwrap();
        assert!(matches!(cli.command, Command::Lsp { stdio: true }));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml-cli parses_lsp_stdio_subcommand`
Expected: FAIL — no `Command::Lsp` variant.

- [ ] **Step 3: Add workspace deps.** In root `Cargo.toml` `[workspace.dependencies]`:

```toml
tokio = { version = "1", features = ["rt-multi-thread", "io-std", "macros"] }
tower-lsp = "0.20"
```

In `crates/uaml-cli/Cargo.toml` `[dependencies]`:

```toml
tokio = { workspace = true }
tower-lsp = { workspace = true }
```

- [ ] **Step 4: Add the subcommand + module.** In `main.rs`: add `mod lsp;` near the other `mod` lines; add the variant to `enum Command`:

```rust
    /// Run the UAML language server (stdio LSP).
    Lsp {
        /// Use stdio transport (the only supported transport in Phase 1).
        #[arg(long)]
        stdio: bool,
    },
```

In `fn main`'s `match cli.command`, add:

```rust
        Command::Lsp { stdio: _ } => lsp::run(),
```

Create `crates/uaml-cli/src/lsp/mod.rs`:

```rust
//! `uaml lsp` — stdio language server. Server code lives here so the core
//! crate (`uaml`) stays LSP-free.

pub mod bundle;
pub mod map;
mod server;

/// Entry point for `uaml lsp --stdio`. Implemented in Task 11.
pub fn run() -> i32 {
    server::serve_stdio();
    0
}
```

Create empty-ish `crates/uaml-cli/src/lsp/server.rs` with a placeholder `pub fn serve_stdio() {}` (filled in Task 11), and empty `map.rs`/`bundle.rs` (filled in Tasks 9–10) so the module tree compiles.

- [ ] **Step 5: Run the CLI suite**

Run: `cargo test -p uaml-cli parses_lsp_stdio_subcommand && cargo build -p uaml-cli`
Expected: PASS + clean build (deps resolve).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/uaml-cli/Cargo.toml crates/uaml-cli/src/main.rs crates/uaml-cli/src/lsp/
git commit -m "feat(uaml-cli): add uaml lsp subcommand skeleton and tower-lsp deps"
```

### Task 9: Pure Diagnostic→LSP mapping, byte→UTF-16, and the UAML filter

All pure functions — fully unit-testable without a running server. This is where byte offsets become UTF-16 code units (and nowhere else).

**Files:**
- Modify: `crates/uaml-cli/src/lsp/map.rs`
- Test: `crates/uaml-cli/src/lsp/map.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `uaml::diagnostic::{Diagnostic, DiagCode, Severity}`, `tower_lsp::lsp_types`.
- Produces:
  - `pub fn is_uaml(text: &str) -> bool` — true iff frontmatter `type:` is a recognized UAML type (`uml.*`, `Diagram`, …). Reuse `uaml::model::ClassifierType::parse` (`!= Unknown`) OR `== "Diagram"`.
  - `pub fn utf16_col(line_text: &str, byte_col: usize) -> u32` — UTF-16 code-unit offset of `byte_col` within `line_text`.
  - `pub fn to_lsp_diagnostic(d: &Diagnostic, line_text: &str) -> tower_lsp::lsp_types::Diagnostic` — `range` from `d.line`/`d.span` (whole line when `span` is `None`), `source = "uaml"`, `code` = `d.code.as_str()`, severity mapped.

- [ ] **Step 1: Write the failing tests** in `map.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_uaml_detects_recognized_types_only() {
        assert!(is_uaml("---\ntype: uml.Class\n---\n# X\n"));
        assert!(is_uaml("---\ntype: Diagram\n---\n# X\n"));
        assert!(!is_uaml("# just markdown\n"));
        assert!(!is_uaml("---\ntype: bpmn.Task\n---\n# X\n"));
    }

    #[test]
    fn utf16_col_counts_code_units_not_bytes() {
        // "héllo": 'é' is 2 bytes but 1 UTF-16 unit.
        let line = "héllo";
        assert_eq!(utf16_col(line, 0), 0);
        assert_eq!(utf16_col(line, 3), 2); // after "hé" (1 + 2 bytes) -> 2 units
    }

    #[test]
    fn non_ascii_link_span_maps_to_correct_utf16_range() {
        // A `[Café](./cafe.md)` link: the byte span must convert to UTF-16 units.
        let line = "- depends [Café](./cafe.md)";
        let byte_start = line.find("[Café]").unwrap();
        let u = utf16_col(line, byte_start);
        assert_eq!(u as usize, line[..byte_start].chars().map(char::len_utf16).sum::<usize>());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml-cli -- is_uaml_detects utf16_col_counts non_ascii_link_span`
Expected: FAIL — functions undefined.

- [ ] **Step 3: Implement.** In `map.rs`:

```rust
use tower_lsp::lsp_types as lsp;
use uaml::diagnostic::{Diagnostic, Severity};
use uaml::frontmatter::parse_frontmatter;
use uaml::model::ClassifierType;

pub fn is_uaml(text: &str) -> bool {
    let ty = parse_frontmatter(text).0.get_str("type").map(str::to_string);
    match ty {
        Some(t) => t == "Diagram" || !matches!(ClassifierType::parse(&t), ClassifierType::Unknown(_)),
        None => false,
    }
}

pub fn utf16_col(line_text: &str, byte_col: usize) -> u32 {
    line_text[..byte_col.min(line_text.len())]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum()
}

fn severity(s: Severity) -> lsp::DiagnosticSeverity {
    match s {
        Severity::Error => lsp::DiagnosticSeverity::ERROR,
        Severity::Warning => lsp::DiagnosticSeverity::WARNING,
    }
}

pub fn to_lsp_diagnostic(d: &Diagnostic, line_text: &str) -> lsp::Diagnostic {
    let line = (d.line.saturating_sub(1)) as u32; // LSP is 0-based
    let (start_ch, end_ch) = match d.span {
        Some((s, e)) => (utf16_col(line_text, s), utf16_col(line_text, e)),
        None => (0, utf16_col(line_text, line_text.len())),
    };
    lsp::Diagnostic {
        range: lsp::Range {
            start: lsp::Position { line, character: start_ch },
            end: lsp::Position { line, character: end_ch },
        },
        severity: Some(severity(d.severity)),
        code: Some(lsp::NumberOrString::String(d.code.as_str().to_string())),
        source: Some("uaml".to_string()),
        message: d.message.clone(),
        ..Default::default()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml-cli -- is_uaml_detects utf16_col_counts non_ascii_link_span`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml-cli/src/lsp/map.rs
git commit -m "feat(uaml-cli): LSP diagnostic mapping with byte to UTF-16 conversion"
```

### Task 10: Bundle overlay + recompute (workspace model)

The server is workspace-aware: an in-memory `HashMap<path, text>` seeded from disk, with open-buffer text overlaid, revalidated as a whole on each edit. This task is the pure state model + recompute — no async, fully unit-testable.

**Files:**
- Modify: `crates/uaml-cli/src/lsp/bundle.rs`
- Test: `crates/uaml-cli/src/lsp/bundle.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `uaml::validate::validate`, `map::to_lsp_diagnostic`, `map::is_uaml`.
- Produces:
  - `pub struct Workspace { docs: std::collections::HashMap<String, String> }`
  - `pub fn new() -> Workspace`; `pub fn seed_from_glob(&mut self, root: &std::path::Path)` (globs `**/*.md` via `std::fs` walk — no extra crate; a small recursive `read_dir`).
  - `pub fn overlay(&mut self, path: String, text: String)` — insert/replace one file's live text.
  - `pub fn diagnostics(&self) -> Vec<(String, Vec<lsp::Diagnostic>)>` — run `validate` over the whole bundle, group by file, map each with the correct line's text; skip non-UAML files (no entry, so their diagnostics are cleared).

- [ ] **Step 1: Write the failing tests** in `bundle.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_edit_updates_diagnostics() {
        let mut ws = Workspace::new();
        ws.overlay("a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId\n".into());
        let clean = ws.diagnostics();
        assert!(clean.iter().all(|(_, ds)| ds.is_empty()));

        ws.overlay("a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- broken line\n".into());
        let dirty = ws.diagnostics();
        let (_, ds) = dirty.iter().find(|(p, _)| p == "a.md").unwrap();
        assert!(ds.iter().any(|d| d.message.contains("attribute")));
    }

    #[test]
    fn plain_markdown_is_filtered_out() {
        let mut ws = Workspace::new();
        ws.overlay("notes.md".into(), "# just notes\n\nnot uaml at all\n".into());
        let diags = ws.diagnostics();
        assert!(diags.iter().find(|(p, _)| p == "notes.md").map(|(_, d)| d.is_empty()).unwrap_or(true));
    }

    #[test]
    fn cross_document_unresolved_target_is_reported() {
        let mut ws = Workspace::new();
        ws.overlay("order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into());
        let (_, ds) = ws.diagnostics().into_iter().find(|(p, _)| p == "order.md").unwrap();
        assert!(ds.iter().any(|d| matches!(&d.code, Some(lsp::NumberOrString::String(s)) if s == "unresolved-target")));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p uaml-cli -- overlay_edit_updates plain_markdown_is_filtered cross_document_unresolved`
Expected: FAIL — `Workspace` undefined.

- [ ] **Step 3: Implement.** In `bundle.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;
use tower_lsp::lsp_types as lsp;
use crate::lsp::map::{is_uaml, to_lsp_diagnostic};

#[derive(Default)]
pub struct Workspace {
    docs: HashMap<String, String>,
}

impl Workspace {
    pub fn new() -> Self { Workspace::default() }

    pub fn overlay(&mut self, path: String, text: String) {
        self.docs.insert(path, text);
    }

    pub fn seed_from_glob(&mut self, root: &Path) {
        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() { walk(&p, out); }
                    else if p.extension().and_then(|x| x.to_str()) == Some("md") { out.push(p); }
                }
            }
        }
        let mut files = Vec::new();
        walk(root, &mut files);
        for f in files {
            if let Ok(text) = std::fs::read_to_string(&f) {
                let rel = f.strip_prefix(root).unwrap_or(&f).to_string_lossy().replace('\\', "/");
                self.docs.entry(rel).or_insert(text);
            }
        }
    }

    /// Per-file LSP diagnostics for the whole bundle. Non-UAML files get an
    /// empty vec (so the client clears any stale squiggles).
    pub fn diagnostics(&self) -> Vec<(String, Vec<lsp::Diagnostic>)> {
        let bundle: Vec<(String, String)> =
            self.docs.iter().map(|(p, t)| (p.clone(), t.clone())).collect();
        let all = uaml::validate::validate(&bundle);
        let mut out: Vec<(String, Vec<lsp::Diagnostic>)> = Vec::new();
        for (path, text) in &bundle {
            let mut ds = Vec::new();
            if is_uaml(text) {
                let lines: Vec<&str> = text.lines().collect();
                for d in all.iter().filter(|d| d.file == *path) {
                    let line_text = lines.get(d.line.saturating_sub(1)).copied().unwrap_or("");
                    ds.push(to_lsp_diagnostic(d, line_text));
                }
            }
            out.push((path.clone(), ds));
        }
        out
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p uaml-cli -- overlay_edit_updates plain_markdown_is_filtered cross_document_unresolved`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml-cli/src/lsp/bundle.rs
git commit -m "feat(uaml-cli): in-memory workspace bundle overlay and per-file diagnostics"
```

### Task 11: `tower-lsp` server — lifecycle, didOpen/didChange, publish, debounce

Wires the pure model (Tasks 9–10) into a `tower-lsp` `LanguageServer` over stdio.

**Files:**
- Modify: `crates/uaml-cli/src/lsp/server.rs`
- Test: covered by the end-to-end test in Task 12 (async lifecycle is not unit-tested here).

**Interfaces:**
- Consumes: `bundle::Workspace`, `tower_lsp::{LspService, Server, Client, LanguageServer}`.
- Produces: `pub fn serve_stdio()` — builds a tokio runtime, constructs the service, serves over stdin/stdout.

- [ ] **Step 1: Implement the server.** In `server.rs`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::lsp::bundle::Workspace;

struct Backend {
    client: Client,
    ws: Arc<Mutex<Workspace>>,
}

impl Backend {
    async fn publish_all(&self) {
        let snapshot = { self.ws.lock().await.diagnostics() };
        for (path, diags) in snapshot {
            if let Ok(uri) = Url::from_file_path(std::path::Path::new(&path)) {
                self.client.publish_diagnostics(uri, diags, None).await;
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(folder) = params.workspace_folders.and_then(|f| f.into_iter().next()) {
            if let Ok(root) = folder.uri.to_file_path() {
                self.ws.lock().await.seed_from_glob(&root);
            }
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) { self.publish_all().await; }

    async fn did_open(&self, p: DidOpenTextDocumentParams) {
        let path = p.text_document.uri.to_file_path()
            .map(|x| x.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        self.ws.lock().await.overlay(path, p.text_document.text);
        self.publish_all().await;
    }

    async fn did_change(&self, p: DidChangeTextDocumentParams) {
        // FULL sync: the last content change is the whole document.
        if let Some(change) = p.content_changes.into_iter().last() {
            let path = p.text_document.uri.to_file_path()
                .map(|x| x.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            self.ws.lock().await.overlay(path, change.text);
            self.publish_all().await;
        }
    }

    async fn shutdown(&self) -> Result<()> { Ok(()) }
}

pub fn serve_stdio() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(|client| Backend {
            client,
            ws: Arc::new(Mutex::new(Workspace::new())),
        });
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
```

- [ ] **Step 2: Debounce (note).** Phase-1 debounce (~150 ms) can be added by coalescing rapid `did_change` events; a minimal implementation is acceptable to defer to a follow-up since bundles are tiny. If added now, gate `publish_all` behind a `tokio::time::sleep` + generation counter so only the latest edit publishes. Keep it out of the correctness path.

- [ ] **Step 3: Build**

Run: `cargo build -p uaml-cli`
Expected: clean build. (Adjust trait method signatures if the resolved `tower-lsp` version differs from 0.20 — see Decision 7.)

- [ ] **Step 4: Commit**

```bash
git add crates/uaml-cli/src/lsp/server.rs crates/uaml-cli/src/lsp/mod.rs
git commit -m "feat(uaml-cli): tower-lsp server with didOpen/didChange diagnostics"
```

### Task 12: End-to-end stdio test

Drives the compiled server over stdio with a small bundle and asserts a `publishDiagnostics` notification arrives.

**Files:**
- Create: `crates/uaml-cli/tests/lsp_e2e.rs`

**Interfaces:**
- Consumes: the `uaml` binary (`uaml lsp --stdio`) via `std::process`, or the in-process `LspService` via `tower_lsp` test helpers.

- [ ] **Step 1: Write the end-to-end test.** Spawn the built binary, send a framed `initialize` → `initialized` → `didOpen` (a doc with an unresolved target), and read until a `textDocument/publishDiagnostics` with a non-empty `diagnostics` array is received. Sketch:

```rust
use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

#[test]
fn publishes_diagnostics_for_unresolved_target_over_stdio() {
    let exe = env!("CARGO_BIN_EXE_uaml");
    let mut child = Command::new(exe).args(["lsp", "--stdio"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().expect("spawn uaml lsp");

    let mut stdin = child.stdin.take().unwrap();
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#;
    let inited = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///tmp/order.md","languageId":"markdown","version":1,"text":"---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n"}}}"#;
    for msg in [init, inited, open] { stdin.write_all(frame(msg).as_bytes()).unwrap(); }
    stdin.flush().unwrap();

    let mut out = String::new();
    let mut stdout = child.stdout.take().unwrap();
    // Read a bounded amount; assert the notification appears.
    let mut buf = [0u8; 8192];
    for _ in 0..20 {
        if let Ok(n) = stdout.read(&mut buf) {
            if n == 0 { break; }
            out.push_str(&String::from_utf8_lossy(&buf[..n]));
            if out.contains("publishDiagnostics") && out.contains("unresolved-target") { break; }
        }
    }
    let _ = child.kill();
    assert!(out.contains("publishDiagnostics"), "no publishDiagnostics seen; got: {out}");
    assert!(out.contains("unresolved-target"), "expected unresolved-target; got: {out}");
}
```

Note: `didOpen` alone reproduces the bundle (single overlaid file); no workspace folder needed for this test. If read timing is flaky on CI, wrap the read loop with a short per-read timeout thread or switch to the in-process `LspService` request/response harness.

- [ ] **Step 2: Run the test**

Run: `cargo test -p uaml-cli --test lsp_e2e`
Expected: PASS — the notification with `unresolved-target` is observed.

- [ ] **Step 3: Commit**

```bash
git add crates/uaml-cli/tests/lsp_e2e.rs
git commit -m "test(uaml-cli): end-to-end stdio LSP diagnostics test"
```

### Task 13: Thin VS Code client (`@uaml/vscode`)

A minimal extension whose only job is to spawn `uaml lsp --stdio` and wire a `LanguageClient` with a `markdown` document selector. No language features are implemented client-side.

**Files:**
- Create: `packages/vscode/package.json`, `packages/vscode/tsconfig.json`, `packages/vscode/src/extension.ts`, `packages/vscode/.vscodeignore`

**Interfaces:**
- Consumes: the `uaml` binary on `PATH` (or a configured path); `vscode-languageclient/node`.
- Produces: an activatable VS Code extension.

- [ ] **Step 1: Create `packages/vscode/package.json`.**

```json
{
  "name": "@uaml/vscode",
  "private": true,
  "version": "0.0.0",
  "license": "Apache-2.0",
  "type": "commonjs",
  "displayName": "UAML",
  "description": "Live UAML diagnostics for Markdown documents.",
  "engines": { "vscode": "^1.90.0" },
  "categories": ["Programming Languages", "Linters"],
  "activationEvents": ["onLanguage:markdown"],
  "main": "./dist/extension.js",
  "contributes": {
    "configuration": {
      "title": "UAML",
      "properties": {
        "uaml.serverPath": {
          "type": "string",
          "default": "uaml",
          "description": "Path to the uaml executable that provides the language server."
        }
      }
    }
  },
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "test": "echo \"no tests\" && exit 0"
  },
  "dependencies": { "vscode-languageclient": "^9.0.1" },
  "devDependencies": { "@types/vscode": "^1.90.0", "typescript": "^5.6.0" }
}
```

- [ ] **Step 2: Create `packages/vscode/tsconfig.json`.**

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "module": "CommonJS",
    "moduleResolution": "Node",
    "outDir": "dist",
    "rootDir": "src",
    "lib": ["ES2022"]
  },
  "include": ["src"]
}
```

- [ ] **Step 3: Create `packages/vscode/src/extension.ts`.**

```typescript
import { workspace, ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(_context: ExtensionContext): void {
  const command = workspace.getConfiguration("uaml").get<string>("serverPath", "uaml");
  const serverOptions: ServerOptions = {
    command,
    args: ["lsp", "--stdio"],
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "markdown" }],
  };
  client = new LanguageClient("uaml", "UAML", serverOptions, clientOptions);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
```

- [ ] **Step 4: Create `packages/vscode/.vscodeignore`.**

```
src/**
tsconfig.json
**/*.ts
!dist/**
```

- [ ] **Step 5: Install + typecheck.**

Run: `pnpm install && pnpm --filter @uaml/vscode build`
Expected: `tsc` compiles `src/extension.ts` → `dist/extension.js` with no errors.

- [ ] **Step 6: Confirm the workspace lint/format is clean.**

Run: `pnpm lint`
Expected: PASS (no new eslint errors from the added package).

- [ ] **Step 7: Commit (ends Part 2 — this is the PR boundary)**

```bash
git add packages/vscode/ pnpm-lock.yaml
git commit -m "feat(vscode): thin UAML language client spawning uaml lsp --stdio"
```

---

## Self-review — spec coverage

| Spec requirement | Task(s) |
| --- | --- |
| `parse` parses *and* reports in one pass, `(Document, Vec<Diagnostic>)` | Task 5 |
| No `filter_map` silent drops — malformed line → diagnostic | Tasks 2, 3, 5 |
| One structural walk; `validate`'s scanner deleted | Task 6 (delete) + Task 5 (walk owner) |
| Precise column spans (byte, relative to line) | Tasks 1, 2, 5 |
| Syntactic vs semantic split | Task 5 (syntactic) + Task 6 (semantic `link`) |
| `Diagnostic` gains span | Task 1 |
| Grammar line-parsers off `Option` | Tasks 2, 3 |
| Per-line byte offsets threaded through `parse` | Task 5 |
| Layout line no longer approximated to `## Layout` heading | Tasks 4 (`LayoutItem.line`) + 6 |
| `build_model` ignores error nodes / new shapes | Task 4 |
| `fmt` skip-on-error byte-for-byte | Task 7 (+ Task 6 keeps `validate`) |
| `Command::Lsp` subcommand, core stays LSP-free | Task 8 |
| tower-lsp + tokio deps | Task 8 |
| Workspace/bundle overlay + recompute + debounce | Tasks 10, 11 |
| UAML filter (frontmatter `type:`) | Tasks 9, 10 |
| Diagnostic → LSP mapping (range/code/source/severity) | Task 9 |
| byte → UTF-16 only in LSP layer, non-ASCII test | Task 9 |
| Thin VS Code client, markdown selector | Task 13 |
| Existing `validate` suite green + span assertions | Tasks 6, 7 |
| `parse` round-trip / error-node tests | Task 5 |
| Server unit tests (open→edit→update; filter) | Task 10 |
| End-to-end stdio test | Task 12 |

**PR boundaries:** Part 1 = Tasks 1–7 (one PR; ends at Task 7). Part 2 = Tasks 8–13 (one PR; depends on Part 1; ends at Task 13).

**Deferred by the spec (not in this plan):** navigation/go-to-definition (Phase 2), completion (Phase 3), precise layout operand spans (Later), incremental sync / lossless CST (rejected).

