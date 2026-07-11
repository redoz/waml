# UAML Language Server (`uaml lsp`)

**Date:** 2026-07-12
**Product:** UAML core (`crates/uaml`), UAML CLI (`crates/uaml-cli`), editor tooling
**Scope:** An LSP for UAML-shaped Markdown, shipped as a subcommand of the existing `uaml` binary. Phase 1 delivers live diagnostics with precise spans. Reaching that cleanly first requires reshaping the parser to **parse and report in one pass** — the enabling refactor is the larger half of this spec.

## Context

UAML documents are CommonMark Markdown carrying a UML profile: YAML frontmatter, a set of recognized `## sections`, and line-grammar bullets inside them. The Rust core already parses these into a `Document`/`Model` and produces `Diagnostic`s; the `uaml check` command is effectively a batch diagnostics pass.

Two problems block good editor support:

**1. The parser is lossy.** `parse_document` (`crates/uaml/src/parse.rs`) classifies each `## section` and runs the grammar line-parsers through `filter_map` (`parse.rs:26-41`). A line that fails to parse is **silently dropped** — the parser cannot report *what* failed or *where*.

**2. Validation is a second, independent scanner.** Because the parser drops errors, `validate` (`crates/uaml/src/validate.rs:75`) re-scans the raw text with its *own* hand-rolled walk: its own frontmatter detection, its own fence tracker (`validate.rs:102-116`), its own `## `-section state machine. It re-calls the same grammar line-parsers only to check whether they return `None`. The structural walk is duplicated between `parse.rs` and `validate.rs`, and the two notions of "what is a section / what is inside a fence" can drift.

Everything an LSP wants — spans, error recovery, the node under the cursor — is exactly what the lossy `filter_map` parser cannot supply. Diagnostics also carry only `line: usize` (`crates/uaml/src/diagnostic.rs:51`); there are no columns. Building the LSP on today's two-scanner foundation would cement the duplication.

## Goals

- **Parser parses *and* reports, in one pass.** `parse` returns the `Document` **and** its syntactic diagnostics, each with a span. No `filter_map` drops; a malformed line becomes an explicit error node.
- **One structural walk of the text, ever.** `validate`'s hand-rolled scanner is deleted. Cross-document (semantic) checks become a thin pass over the parsed `Document`s that reuses the parser's spans.
- **Precise column spans** for line-local diagnostics (LSP squiggles land on the token, not the whole line).
- **`uaml lsp`** — a stdio language server, a subcommand of the existing binary, reusing the core crate. Phase 1: live diagnostics in VS Code.
- Coexist with the built-in Markdown extension; contribute *only* UAML diagnostics, and only for UAML documents.

## Non-goals

- **No full-fidelity / lossless CST** (Roslyn / rowan red-green trees, trivia preservation, incremental reparse). Explicitly rejected for now — see *Fidelity decision*. Canonical `fmt` stays the source of truth, so byte-perfect round-trip has no consumer, and UAML docs are small enough that full reparse per keystroke is free.
- **No reimplementation of Markdown.** `pulldown-cmark` keeps doing CommonMark; UAML structure is parsed on top. The server never provides general Markdown features (formatting, preview, link-following) — the Markdown extension does.
- **No new file extension or language id.** Files stay `.md`; UAML-ness is detected by frontmatter `type:`.
- Nav (go-to-definition, references, rename) and completion are **later phases**, not Phase 1.

## Fidelity decision

Three points on the fidelity axis were considered:

| Option | What it is | Verdict |
| --- | --- | --- |
| Current | Lossy `filter_map`, second scanner for errors | Reject — the problem |
| **B (chosen)** | Spanned, error-recovering parser returning `(Document, diagnostics)`; abstract tree, no trivia | **Chosen** |
| Full CST | Roslyn/rowan: green-red trees, trivia, byte-perfect round-trip, incremental | Reject (for now) |

Full losslessness buys byte-perfect round-trip (unwanted — `fmt` normalizes), incremental reparse (unmeasurable on tiny docs), and format-preserving edits (the only real payoff, and only if the `ops` mutation layer should preserve author formatting — not a current goal). Its cost is large: every consumer (`build_model`, `ops`, `serialize`, CLI) would read through a typed CST facade over green/red nodes. Over a grammar this small, that ceremony is not justified.

**Patterns borrowed from Roslyn without the CST weight:**

- **Syntactic vs semantic diagnostic split.** The parser emits syntax diagnostics; a later phase emits semantic ones. Mirrors Roslyn's parse-vs-bind split.
- **Error / missing nodes, never bail.** A malformed line becomes an explicit error node with a span + message; `parse` always yields a tree.
- **Diagnostics ride with the parse result**, not a separate re-scan.
- **One immutable tree, many consumers** — the discipline, not the red/green machinery.

Dropped: red/green trees, trivia nodes, on-demand offset computation. Absolute byte spans are stored directly on nodes/diagnostics.

## Architecture

Two parts. Part 1 (the parser refactor) is a prerequisite for Part 2 (the server) and is where most of the work lives.

### Part 1 — Parser returns diagnostics

Target shape:

```
parse(text)          -> (Document, Vec<Diagnostic>)   // syntactic, spanned, per-doc
link(&[Document])    -> Vec<Diagnostic>               // semantic, spanned, cross-doc
```

Diagnostics split by what each pass can *know*:

**Syntactic — produced inside `parse` (one document, no bundle needed):**
`MalformedAttribute`, `MalformedRelationship`, `MalformedLayout`, `DroppableContent`, `FrontmatterNotClean`, `UnknownType`. The parser has the text and the offsets, so it emits these with real spans.

**Semantic — produced by `link` over the parsed `Document`s (needs the whole bundle):**
`UnresolvedTarget`, `DuplicateSlug`, `UnresolvedLayoutRef`, `LayoutCycle`. Resolving `./ghost.md` requires the keyset of all documents, so these cannot live in the parser. This pass **walks the parsed tree, never re-scans raw text**, and reuses spans the parser already recorded.

Required changes:

- **`Diagnostic` gains a span.** Add an optional intra-line span:
  ```rust
  pub struct Diagnostic {
      // ...existing: severity, code, message, file, line...
      pub span: Option<(usize, usize)>, // (col_start, col_end) BYTE offsets within `line`; None = whole line
  }
  ```
  Byte offsets in core; the LSP layer converts to UTF-16 (see *Span model*).
- **Grammar line-parsers change signature.** `parse_attribute_line`, `parse_relationship_line`, `parse_layout_line`, `parse_member_line`, `parse_value_line` move from `Option<T>` to an error-carrying result (a parsed node **or** an error node with a span-relative range + message). No more silent `None`.
- **`Section` gains error nodes.** A malformed line inside a bullet section is preserved as an explicit error node rather than dropped, so `parse` is no longer lossy for the cases `validate` currently re-derives. Concretely: each bullet section holds `Vec<Line<T>>` where `enum Line<T> { Parsed(T), Error(ErrorNode) }` and `ErrorNode { raw, line, span, code, message }` keeps the offending line verbatim; the syntactic diagnostics are *derived* by walking these `Line::Error` nodes (single source of truth), `build_model` skips them, and `serialize` re-emits their `raw` text unchanged.
- **`parse_document` threads per-line byte offsets.** It already consumes pulldown-cmark byte offsets for headings (`parse.rs:58-85`, `into_offset_iter`); extend that so each section's content lines carry their absolute offset, letting the grammar parsers report spans.
- **`validate`'s scanner is deleted.** `validate_doc` (`validate.rs:59-193`) — the frontmatter/fence/section re-scan — is removed. `validate` becomes `link`: cross-doc checks over `Document`s. The layout line-approximation (`validate.rs:299`, *"Line number is approximate"* — anchors to the `## Layout` heading) is fixed by tracking each layout statement's line during parse.
- **Consumers updated.** `build_model` already ignores unrecognized sections gracefully; it learns to ignore error nodes too. CLI `check`/`fmt` rendering (`crates/uaml-cli/src/commands.rs`) consumes the new `(Document, diagnostics)` return and the new span field.

`fmt`'s skip-on-error contract is preserved: a file with any Error-severity diagnostic is left byte-for-byte untouched (`plan_fmt`, `commands.rs:133-149`).

### Part 2 — `uaml lsp` server

- **Subcommand.** Add `Command::Lsp` to `crates/uaml-cli/src/main.rs`, launched as `uaml lsp --stdio`. Server code lives in a new `crates/uaml-cli/src/lsp/` module so the core crate stays LSP-free.
- **Dependencies.** `tower-lsp` + `tokio` (stdio transport, JSON-RPC, lifecycle).
- **Workspace/bundle model — the crux.** UAML diagnostics are cross-document, so the server is workspace-aware, not per-file:
  - On `initialize`: read the workspace root, glob `*.md`, build an in-memory bundle `HashMap<path, text>`.
  - On `didOpen` / `didChange`: overlay that file's live text over the disk copy.
  - Recompute: run `parse` per doc + `link` over the overlaid bundle; `publishDiagnostics` per file. Bundles are small (tens–hundreds of tiny files) → full revalidation per keystroke is cheap. Debounce ~150 ms.
- **UAML filter.** A `.md` file is treated as UAML only if its frontmatter carries a recognized `type:` (`uml.Class`, `uml.Enum`, `Diagram`, …). Non-UAML Markdown gets no diagnostics — no noise, clean coexistence with the Markdown extension.
- **Diagnostic → LSP mapping.** Map core `Diagnostic` to LSP `Diagnostic`: `range` from `line` + `span`, `code` = the stable slug (`unresolved-target`), `source = "uaml"`, severity mapped from `Severity`.

### VS Code client (thin)

A minimal extension whose only job is to spawn `uaml lsp --stdio` and wire a `LanguageClient` with a `markdown` document selector. No language features implemented client-side — the server provides them. Multiple language servers per document is standard and supported (VS Code runs each extension's `LanguageClient` independently and unions their diagnostics); the UAML server and the built-in Markdown extension coexist on the same `.md` without conflict.

## Span model

LSP `Range` is line + character in **UTF-16 code units**; core stores **byte** offsets. Conversion happens in the LSP layer, keeping the core byte-based. Per-code precision, tiered by difficulty:

- **Precise, whole-bullet** (easy — offset arithmetic on the line): `MalformedAttribute`, `MalformedRelationship`, `MalformedLayout`, `DroppableContent` — span the indent + `- ` through end of content.
- **Precise, single-token** (easy-ish — string search on the held line): `UnresolvedTarget` spans the `[Title](./slug.md)`; `UnknownType` spans the type value.
- **Line-level in Phase 1** (the one real gap): `UnresolvedLayoutRef`, `LayoutCycle`. The layout parser discards operand positions, so precise operand columns need offsets threaded through `parse_layout_line` — deferred. Phase 1 at least fixes the *line* to point at the offending statement (not the `## Layout` heading).
- **Whole-line / document-anchored**: `DuplicateSlug`, `FrontmatterNotClean` — `span: None`, LSP falls back to the whole line.

## Phasing

- **Phase 1 — Diagnostics.** The parser refactor (Part 1) + the diagnostics server (Part 2) + thin VS Code client. Ships live squiggles with precise columns for line-local codes.
- **Phase 2 — Navigation.** Go-to-definition on `[Title](./slug.md)` links, find-references, rename across the bundle. Reuses `build_model` resolution and `ops::referrers` / rename (already implemented for the CLI).
- **Phase 3 — Completion.** Section headings, relationship verbs, resolvable slugs, layout keywords.
- **Later — Precise layout spans.** Thread offsets through `parse_layout_line` for operand-level columns.

## Testing

- **Parser refactor:** the existing `validate` test suite (`validate.rs:370-602`) is the safety net — its assertions on codes and lines must keep passing after the checks move into `parse` + `link`, extended to assert spans. `parse` round-trip and error-node tests. `fmt` skip-on-error regression tests (`commands.rs`) must stay green.
- **Server:** unit tests over the bundle-overlay + republish logic (open → edit → diagnostics update; UAML filter excludes plain Markdown). An end-to-end test driving the server over stdio with a small bundle.
- **Byte→UTF-16:** a targeted test with non-ASCII in a `[Title]` to confirm columns land correctly.

## Editor support beyond VS Code

The server is editor-agnostic (stdio LSP). Neovim, Helix, and Zed all support multiple language servers per buffer, so a UAML client config there works the same way. Emacs `eglot` is the one weak spot (historically one primary server per buffer); `lsp-mode` add-on servers work. VS Code is the Phase 1 target; others are config, not code.

## Risks / open questions

- **Grammar signature churn.** Moving five line-parsers off `Option` ripples into every caller (`parse`, `validate`, tests). Contained but broad — the bulk of Part 1's diff.
- **Workspace discovery.** Which root(s) to glob when a file is opened outside a recognized bundle root, and how to handle files with no workspace folder (single-file edit). Phase 1 can scope to "workspace folder present."
- **Debounce vs freshness.** 150 ms is a starting point; tune against felt latency on real bundles.
