# Incremental Markdown syntax platform — design

**Date:** 2026-07-31
**Status:** Approved in conversation; implemented
**Sequence:** 1 of 4

## Problem

WAML already has a Roslyn-style syntax platform: immutable source snapshots,
lossless green/red trees, incremental reparsing, exact source recovery,
diagnostics, and language-island reuse. The current `OkfMarkdownLanguage`,
however, recognizes only the outer WAML shell: frontmatter, headings, and opaque
Markdown regions. It does not model the complete block and inline structure
needed by an editable Markdown document.

The current native Source view therefore delegates reading to Makepad's
independent `Markdown` widget. Extending that path would create two authorities:
WAML's incremental syntax tree for domain analysis and Makepad's parser for
presentation.

## Goal

Make the WAML syntax platform the one lossless, incremental authority for the
entire Markdown document:

- conform to CommonMark 0.31.2;
- support the GitHub Flavored Markdown extensions for tables, task-list items,
  strikethrough, extended autolinks, and tag filtering;
- preserve every source byte and delimiter token;
- retain WAML frontmatter and domain-language islands as explicit extensions;
- serve the native editor, renderer, diagnostics, formatter, and LSP from the
  same revisioned snapshot.

## Non-goals

- Rendering or editing UI; those belong to specs 2 and 3.
- Executing raw HTML.
- Adding unrelated Markdown extensions such as footnotes, math, or Mermaid.
- Replacing the generic green/red-tree infrastructure.
- Keeping Makepad's Markdown parser as a runtime dependency.

## Standards and dialect

The base dialect is CommonMark 0.31.2. The named GFM extensions are enabled
together as WAML's default Markdown profile. If the published GFM conformance
suite embeds an older CommonMark-core expectation, CommonMark 0.31.2 wins for
core behavior and the GFM cases remain authoritative for the extension itself.
WAML adds two named extensions:

1. root frontmatter in the form already accepted by the repository;
2. recognized WAML sections whose contents may be parsed by a specialized
   language island.

The parser must not grow accidental product-specific syntax. A future extension
requires an explicit dialect flag, syntax kinds, conformance examples, and
round-trip tests.

## One outer syntax tree

Evolve the existing `OkfMarkdownLanguage` into the full outer document
language. Do not add a parallel CommonMark tree over the same source.

The outer tree owns:

- document and frontmatter;
- container blocks: block quotes, lists, and list items;
- leaf blocks: paragraphs, headings, thematic breaks, indented and fenced code,
  HTML blocks, link-reference definitions, and tables;
- inline structure: text, escapes, entities, code spans, emphasis, strong
  emphasis, strikethrough, links, images, autolinks, raw HTML, and line breaks;
- all marker, delimiter, whitespace, newline, and recovery tokens.

`MarkdownStructureMap` becomes a projection derived from this tree rather than
a second interpretation of raw regions. Existing WAML lowering consumes the
projection and recognized syntax nodes.

Specialized WAML/UML syntax remains an embedded analysis keyed by the source
range and identity of its owning Markdown section. It does not parse the
surrounding Markdown again.

## Parsing model

Follow CommonMark's two logical phases while producing one lossless tree:

1. Parse line-oriented block structure and build the link-reference map.
2. Parse inline children for paragraphs, headings, table cells, and other
   inline-bearing blocks.

Raw source is retained by tokens and trivia. Recovery nodes preserve malformed
or unfinished constructs without inventing or deleting characters. Any Unicode
string remains an editable Markdown document even when diagnostics are present.

Raw HTML is parsed structurally, including GFM tag-filter classification, but
the native renderer will not execute it.

## Incremental updates

The editor supplies exact `TextChange` values against a base document revision.
The syntax platform applies them to a new immutable `SourceText` and derives a
new snapshot from the previous snapshot.

Incremental processing:

1. Map changes through `ChangeMap`.
2. Select the smallest safe block reparse roots.
3. Reparse affected block structure.
4. Rebuild the reference-definition map if relevant definitions changed.
5. Reparse only inline-bearing blocks affected directly or through changed
   reference definitions.
6. Rebase unchanged source-independent green nodes.
7. Reparse only WAML language islands whose owning range or dependencies
   changed.

An unsafe structural boundary produces an explicit full-reparse outcome with a
reason. Full parsing is a correctness fallback, not the normal editing path.

Because reference definitions can affect non-contiguous blocks, the result
reports affected ranges rather than pretending every update is one interval.
The existing single `reparsed_range` remains available when the affected set is
contiguous.

## Snapshot API

Expose one immutable revisioned product conceptually shaped as:

```rust
pub struct MarkdownSyntaxSnapshot {
    pub revision: DocumentRevision,
    pub text: SourceText,
    pub tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    pub structure: Arc<MarkdownStructureMap>,
    pub diagnostics: Arc<[SyntaxDiagnostic]>,
}

pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}
```

The concrete API may reuse existing repository types, but it must preserve
these ownership rules:

- snapshots are immutable and revision-tagged;
- consumers never receive mutable syntax nodes;
- affected ranges are expressed in the new snapshot;
- stale updates cannot be mistaken for current analysis;
- presentation roles are syntax queries, not reparsing utilities.

## Presentation queries

Provide framework-free traversal/query APIs that classify exact source ranges
for consumers:

- content versus syntax marker;
- block and inline semantic role;
- heading level, list kind, and table alignment;
- link destination and image source;
- fenced-code info string and content range;
- WAML language-island ownership;
- diagnostics and recovery state.

The native editor and LSP consume these queries directly. Neither maintains a
regex-based Markdown classifier.

## Error handling

- Oversized source returns the existing typed source-size error.
- Malformed Markdown produces a lossless tree plus diagnostics.
- Internal source/tree mismatch remains a structural invariant error.
- An incremental safety failure falls back to a full parse and records why.
- A language-island failure does not invalidate unrelated Markdown syntax.
- Exact source recovery must succeed for every published snapshot.

## Testing

### Standards conformance

- Run the official CommonMark 0.31.2 examples against the syntax projection and
  renderer-neutral expected structure.
- Run the official GFM extension examples.
- Record the imported fixture source, revision, and license alongside fixtures.

### Losslessness and recovery

- Exact source recovery for every conformance example.
- BOM, CRLF, tabs, Unicode, combining characters, and mixed line endings.
- Malformed fences, links, emphasis delimiters, tables, HTML, and frontmatter.
- WAML section recognition inside and around protected Markdown containers.

### Incremental oracle

For every edit sequence, compare the incremental result with a clean full parse:

- exact recovered source;
- equivalent syntax structure and diagnostics;
- correct structure map and reference resolution;
- correct WAML island ownership;
- unchanged source-independent green-node reuse.

Use deterministic unit cases plus randomized insert, delete, replace, paste, and
Unicode-boundary edits. Every full fallback must expose an expected reason.

## Success criteria

- The complete document is represented by one lossless WAML-owned Markdown
  syntax tree.
- CommonMark and enabled GFM conformance suites pass.
- Normal local edits return incremental outcomes and reuse unchanged greens.
- Full parsing remains an explicit tested fallback.

## Implementation evidence

- Date: 2026-07-31.
- Implementation range: `89835eb..HEAD`.
- CommonMark 0.31.2: 652 passed; GFM: 24 passed; total: 676.
- `rtk cargo fuzz run parse_write -- -runs=10000`: DEFERRED, 0 Windows iterations; the sanitizer runtime failed to load with `STATUS_DLL_NOT_FOUND`.
- `rtk cargo fuzz run syntax_edits -- -runs=10000`: DEFERRED, 0 Windows iterations; the no-sanitizer retry cannot link because MSVC has no `sancov` section symbols.
- `rtk cargo fmt --all -- --check`: GREEN.
- `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`: 0 errors. Cargo reported two external residual warnings while discovering the pinned Makepad source at revision `c38f529984eda61e258ca69fb50c6712d85c74c1`: duplicate `bitflags v2.10.0` and `cfg-if v1.0.4` manifests under `libs/vulkan`, each skipped in favor of the canonical manifest under `libs`. Resolving them requires an upstream Makepad repository cleanup or dependency-revision migration, not a WAML manifest or lockfile change.
- `rtk cargo test --workspace --all-features`: 1,652 passed across 61 suite summaries; 0 failed.
- Existing WAML frontmatter and language-island behavior is preserved.
- Native UI and LSP can obtain semantic source ranges without parsing Markdown.
