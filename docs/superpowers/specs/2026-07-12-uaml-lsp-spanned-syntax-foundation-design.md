# UAML LSP — Spanned Syntax Foundation + documentSymbol

**Date:** 2026-07-12
**Status:** Design approved

## Problem

The UAML language server today emits **only diagnostics**. Every
cursor-position LSP feature — hover, go-to-definition, find-references,
rename — needs to answer one question the server currently cannot: *"what
model element is at this source position?"*

That reverse lookup is impossible because the **semantic model carries no
source positions**. Positions live only in a split, line-relative form:
`Diagnostic.line` (1-based) plus an optional `span: (usize, usize)` that is a
byte range *within that line*. The syntax layer is partially positioned —
`ParsedRel`, `MemberLine`, `LayoutItem`, and `ErrorNode` carry `line` (and
some a line-relative span) — but attributes, values, notes, section headings,
and the document title carry **no position at all** (`model::Attribute` has no
line field; values/notes are bare `Line<String>`).

The parser already computes absolute byte offsets while walking
(`content_abs_start + line_start`, `line_at(src, abs_byte)` in `parse.rs`) and
then **discards** them, keeping only the line-relative form. So the
information needed for a proper positional tree is already computed and thrown
away.

This spec builds the foundation — a single absolute-offset span currency on
every syntax node, plus an offset↔position index — and proves it end-to-end
with **documentSymbol** (the document outline) as its first consumer.

## Goals

- One span currency: absolute byte offsets into the document source, on every
  syntax node, including the bullets that currently have none.
- An offset↔`(line, utf16-col)` index that becomes the single place byte
  offsets convert to LSP positions.
- Keep the semantic model position-free (syntax/semantic separation).
- Ship `textDocument/documentSymbol` as the proof consumer: a hierarchical
  outline built by walking the spanned syntax tree.
- Bound the blast radius: no rewrite of the `Diagnostic` type (see Non-goals).

## Design inspiration

Roslyn's transferable ideas, adopted:

- **`TextSpan`** — an absolute `(start, length)` offset range, the one span
  currency shared by every node.
- **Full-fidelity syntax tree** — the tree round-trips losslessly to source.
  UAML already proves this: `serialize_document(parse_document(t))` is a
  golden fixpoint and `ErrorNode.raw` preserves malformed bytes verbatim.
- **Syntax vs semantic separation** — a positional syntax tree underneath, a
  position-free semantic model on top. UAML already has this split; this spec
  strengthens the syntax half.

Explicitly **not** adopted (YAGNI for a line-oriented Markdown dialect at demo
scale): red/green lazy trees, incremental reparse, trivia objects. That
machinery pays off on C#-sized files with keystroke-latency reparse; here it
is cost with no payoff.

## Architecture

Two new primitives in the `uaml` core crate, spans threaded through the
existing syntax tree, and one new LSP handler consuming them.

```
                         crates/uaml (core)
  ┌───────────────────────────────────────────────────────────┐
  │  text.rs (new)                                             │
  │    TextSpan { start, len }         ← absolute byte offsets │
  │    LineIndex                       ← offset ↔ (line, col)  │
  │                                                            │
  │  syntax.rs (spans threaded in)                             │
  │    Spanned<T> { span, node }                               │
  │    Line::Parsed(Spanned<T>) | Error(ErrorNode{span,..})    │
  │    ParsedRel/MemberLine/LayoutItem  .span: TextSpan        │
  │    Section  { heading_span, .. }                           │
  │    Document { title_span, .. }                             │
  │                                                            │
  │  parse.rs  ← retain the absolute offsets already computed  │
  └───────────────────────────────────────────────────────────┘
                              │ spanned Document
                              ▼
                   crates/uaml-cli/src/lsp
  ┌───────────────────────────────────────────────────────────┐
  │  symbols.rs (new)  Document + LineIndex → DocumentSymbol[] │
  │  server.rs         document_symbol handler + capability    │
  └───────────────────────────────────────────────────────────┘
```

## Components

### 1. `TextSpan` (core `text.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize, // absolute byte offset into the document source
    pub len: usize,
}

impl TextSpan {
    pub fn new(start: usize, len: usize) -> Self { .. }
    pub fn from_range(start: usize, end: usize) -> Self { .. } // end exclusive
    pub fn end(&self) -> usize { self.start + self.len }
    pub fn contains(&self, offset: usize) -> bool { .. } // start <= o < end
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
```

Byte offsets, not char offsets: the parser and `serialize` already work in
bytes, and UTF-16 conversion happens once, at the LSP boundary, via
`LineIndex`.

### 2. `LineIndex` (core `text.rs`)

Precomputes line-start byte offsets for one document's source, then converts
in both directions:

```rust
pub struct LineIndex { line_starts: Vec<usize>, /* + source len */ }

impl LineIndex {
    pub fn new(src: &str) -> Self;             // scan '\n' once
    pub fn line_col(&self, offset: usize) -> (usize, usize); // 0-based line, byte col
    pub fn utf16_col(&self, src: &str, offset: usize) -> u32; // byte col → UTF-16 unit
}
```

This is the single offset→LSP-Position engine. It subsumes `parse.rs::line_at`
and replaces `map.rs`'s re-read-the-line trick with an index built once per
document. CRLF: `line_starts` are computed on `\n`; the UTF-16 column counts
code units of the line slice, so a trailing `\r` is handled the same way
`map.rs::utf16_col` handles it today.

### 3. Spanned syntax tree (core `syntax.rs`)

Add an absolute `span: TextSpan` to `ErrorNode`, `ParsedRel`, `MemberLine`,
and `LayoutItem`, **alongside** their existing `line` (and line-relative
`span` where present). The existing `line` fields are **kept**, not dropped:
`validate.rs` and the diagnostic pipeline still consume `line`, and the
`Diagnostic` migration that would retire it is deferred (see Non-goals).
`line` is derivable from `LineIndex` and can be removed later, together with
that migration — not in this phase. The line-relative `span` on `ParsedRel` /
`MemberLine` / `ErrorNode` likewise stays until diagnostics move to the
absolute currency.

Because those three nodes already own a line-relative field named `span`, the
new absolute field must **coexist under a distinct name** (e.g.
`text_span: TextSpan`) rather than shadow it; nodes without an existing `span`
(`LayoutItem`, `Spanned<T>`, `Section`, `Document`) use the plain `span` /
`*_span` name. The plan picks the exact field names; the invariant is: one
absolute `TextSpan` reachable on every syntax node, no collision with the
retained line-relative fields.

Make `Line<T>` the positioned wrapper so *every* bullet — including the
currently-position-less attributes, values, and notes — carries a span
**without touching `model::Attribute`**:

```rust
pub struct Spanned<T> { pub span: TextSpan, pub node: T }

pub enum Line<T> {
    Parsed(Spanned<T>),
    Error(ErrorNode), // already carries a span
}
```

`Section` gains a `heading_span: TextSpan` (the `## Heading` line) and an
overall `span`. `Document` gains a `title_span: TextSpan` — the parser's
`Head { heading_start, content_start }` already has the bytes.

**Blast-radius control.** The existing accessors keep the same return types:

```rust
impl<T> Line<T> {
    pub fn parsed(&self) -> Option<&T>      // -> Some(&spanned.node)
    pub fn parsed_mut(&mut self) -> Option<&mut T>
    pub fn span(&self) -> TextSpan          // new: Parsed | Error both answer
}
```

Every consumer that reads bullets through `.parsed()` / `.parsed_mut()`
(serialize, validate, model-building in `parse.rs::build_node`) is
**unchanged**. Only direct `Line::Parsed(t)` destructures migrate to
`Line::Parsed(Spanned { node: t, .. })` — a mechanical edit. The golden
serialize-fixpoint test is the guard that the reshape preserved round-trip.

### 4. `symbols.rs` (LSP) — the documentSymbol consumer

A pure function `document_symbols(doc: &Document, index: &LineIndex, src: &str)
-> Vec<DocumentSymbol>`:

- **Root** — one symbol per document = the node. `name` = title; `kind` from
  `ClassifierType` (`Uml(Class)`→`CLASS`, `Interface`→`INTERFACE`,
  `Enum`→`ENUM`, `DataType`→`STRUCT`, `Package`→`PACKAGE`, `Note`→`STRING`,
  `Unknown`→`OBJECT`); `range` = whole document; `selection_range` =
  `title_span`.
- **Children** (flat under the node, differentiated by `kind` — no
  intermediate "Attributes" grouping node, matching how outlines show class
  members directly):
  - Attributes → `FIELD`, `name` = attr name, `detail` = type (+
    multiplicity if present), `selection_range` = the attribute name span.
  - Relationships → `kind` `PROPERTY`, `name` = `"<kind> <target_title>"`.
  - Members (diagram) → `OBJECT`, `name` = member title. Nested
    `MemberGroup`s become nested symbols.
  - Values → `ENUM_MEMBER`.
  - Notes / Body / Unknown sections → omitted (no useful navigable symbol) or
    a single `STRING` symbol for the section; default: omitted.
- Malformed bullets (`Line::Error`) are **still surfaced** as a symbol using
  `ErrorNode.span` with the raw text as the name, so a broken line is still
  navigable in the outline (the outline is for moving around, not for
  correctness).

`TextSpan` → `lsp::Range` via `LineIndex.line_col` + `utf16_col`.

### 5. `server.rs` wiring

- `initialize` advertises `document_symbol_provider: Some(OneOf::Left(true))`
  alongside the existing text-sync capability.
- `textDocument/documentSymbol` handler: look up the request's document text
  from the `Workspace`, `parse_document(text)`, build a `LineIndex`, call
  `document_symbols`, return `DocumentSymbolResponse::Nested(..)`. Non-UAML
  documents (per `is_uaml`) return an empty list.

## Data flow (documentSymbol request)

```
client: textDocument/documentSymbol { uri }
  → server: text = Workspace.doc(path)          (open-buffer overlay or disk)
  → parse_document(text) -> spanned Document
  → LineIndex::new(text)
  → document_symbols(&doc, &index, text) -> Vec<DocumentSymbol>
  → each TextSpan -> lsp::Range via index
  → DocumentSymbolResponse::Nested
```

## Error handling / edge cases

- **Malformed / mid-edit buffers** — `parse_document` is total (never panics;
  malformed bullets become `Line::Error`). documentSymbol still returns the
  well-formed symbols plus error-node symbols. No request ever fails on a
  broken buffer.
- **Non-UAML markdown** — filtered by `is_uaml`; returns `[]`.
- **UTF-16 / non-ASCII** — all column math goes through `LineIndex.utf16_col`;
  reuse the existing `utf16_col` UTF-16 tests.
- **CRLF** — line starts split on `\n`; trailing `\r` handled as today.
- **Empty document / title-only** — root symbol only, no children.

## Testing

**Core (`crates/uaml`):**
- `TextSpan`: `end`/`contains`/`is_empty`, `from_range`.
- `LineIndex`: `line_col` at line starts / mid-line / EOF; `utf16_col` on
  non-ASCII (`é`, emoji); CRLF; empty file; no-trailing-newline.
- Parser span tests: each node kind (`ParsedRel`, `MemberLine`, `LayoutItem`,
  attribute/value/note `Spanned` bullet, section heading, document title)
  carries a `TextSpan` whose byte range slices back to the expected source
  substring.
- Golden serialize-fixpoint suite stays green (the `Line` reshape did not
  change round-trip output).

**LSP (`crates/uaml-cli`):**
- `symbols.rs` unit: a fixture document → expected `DocumentSymbol` tree
  (names, kinds, nesting, ranges).
- e2e (`lsp_e2e.rs`): initialize advertises `documentSymbolProvider`;
  `textDocument/documentSymbol` on a fixture returns the outline; a non-UAML
  doc returns `[]`; a malformed-bullet doc still returns the node + an error
  symbol.

## Non-goals (deferred, deliberately)

- **Migrating the `Diagnostic` type** from `line + relative-span` to absolute
  `TextSpan`. Diagnostics keep their current form and code paths; they are an
  independent output of the same parse and continue to work unchanged. The
  unified span currency reaches diagnostics in a later phase, when
  go-to-def/references make it worth touching all ~11 emission sites +
  `map.rs` + the diagnostic tests.
- **Reverse position→symbol lookup** (`node_at(offset)` descent) and its
  consumers (hover, definition, references, rename). This spec lays the span
  groundwork they need; they are their own later phase.
- **Formatting and lifecycle** (didClose/didSave) — a separate features spec;
  they need no part of this foundation.
- **semanticTokens, inlayHint, codeLens, documentLink** — later.

## Roadmap position

Phase 1a of the LSP build-up. Unblocks the cursor-feature phase
(hover / definition / references / rename), which all consume the reverse
lookup built atop this span foundation.
