# WAML Parser Platform

**Status:** Draft — intentionally unfinished  
**Date:** 2026-07-26  
**Scope:** All WAML parsers, authoring operations, diagnostics, and editor surfaces

> This document records a direction for later design work. It is not approved
> for implementation and must not become an implicit prerequisite of native
> diagram properties.

## Motivation

WAML needs to remain useful when a document is incomplete or malformed. An
authoring UI should be able to say, for example, “this attribute is valid except
for its multiplicity” and offer a focused correction without discarding the
attribute or losing the original source.

The current parser already preserves some malformed constructs as raw
`Line::Error` values, but semantic model construction commonly filters those
lines out. This protects validated consumers, but leaves authoring surfaces
without structured access to partially valid declarations.

Solving this one field at a time with types such as `Authored<T>` risks spreading
parser recovery concerns throughout the semantic model. The longer-term direction
is instead a Roslyn-style parser and workspace platform shared by every WAML
language surface.

## Intended scope

The platform should eventually cover:

- Markdown document structure and frontmatter;
- classifier attributes, operations, slots, values, and relationships;
- diagram members, groups, and layout statements;
- state, activity, object, and sequence syntax;
- diagnostics and source navigation;
- structured authoring operations and code fixes;
- semantic projection, validation, solving, and rendering;
- native, web, CLI, WASM, and automation consumers.

It must not be designed around one editor control or one syntax production.

## Direction

The likely architecture has four explicit layers:

```text
Workspace snapshot
├── immutable source documents
├── lossless syntax trees
├── diagnostics and code actions
└── semantic models
```

### Workspace and document snapshots

An edit produces a new immutable workspace or document snapshot. A snapshot owns:

- a document identity;
- a monotonically changing revision identity;
- immutable source text;
- its syntax tree;
- syntax diagnostics;
- lazily or eagerly produced semantic information.

Source ranges are meaningful only inside their owning snapshot. Consumers must
not apply a range from one revision to another without translating or rejecting
it.

### Lossless syntax

The syntax tree must represent the complete source:

- tokens;
- whitespace and comments/trivia;
- valid nodes;
- malformed tokens;
- missing expected tokens;
- skipped text;
- source ranges.

Writing an untouched tree reproduces the original text, including malformed
content. Parsing must make progress for arbitrary input and must not discard
source merely because a production is invalid.

Partially parseable constructs remain structured. For example:

```text
- quantity: Integer {bananas}
```

should retain the attribute declaration, name, type, brace/token structure, and
an invalid multiplicity token. It should not collapse into an opaque failed
document or disappear from authoring APIs.

### Diagnostics and code actions

Diagnostics are snapshot-scoped values containing:

- a stable diagnostic code;
- severity;
- message arguments or a display message;
- a source range;
- related locations where necessary.

The platform should distinguish diagnostics from recovery representation: a bad
token exists in the syntax tree, while one or more diagnostics explain it.

Code actions describe edits against a specific snapshot revision. Applying a
stale action must fail or be explicitly translated; it must never silently edit
the same numeric range in different source text.

### Semantic models

Semantic models project syntax into concepts used by validation, solving,
rendering, and external APIs. They must make the validity boundary explicit.

Open design question: authoring consumers may need a tolerant declared-symbol
model alongside a strictly validated semantic model. The platform must avoid
forcing every solver and renderer to branch over malformed leaf values merely so
the editor can display them.

Possible split:

```text
Syntax tree
├── declared/authoring model (partial symbols + syntax identities)
└── validated semantic model (resolved values suitable for computation)
```

This split is directional, not yet approved.

## Roslyn concepts under consideration

The following are candidates, not commitments:

- immutable `SourceText`;
- green nodes containing immutable structure and full widths;
- red façades adding parents, absolute positions, and typed navigation;
- structural sharing between document revisions;
- incremental lexing and parsing from text change ranges;
- missing tokens rather than nullable required children;
- skipped tokens preserved as trivia or recovery nodes;
- syntax annotations or equivalent tracking identities;
- lazy semantic binding;
- versioned diagnostics and code actions.

The project should not copy Roslyn mechanically. Each mechanism must earn its
complexity against WAML document sizes, grammar shape, editing patterns, and
cross-target requirements.

## Initial implementation boundary

A first platform increment might provide:

1. Immutable document snapshots with revision identities.
2. Lossless source text and syntax trees for one representative grammar.
3. Bad/missing token recovery with exact round-tripping.
4. Snapshot-scoped syntax diagnostics.
5. A semantic projection matching today’s validated model.
6. Full-document reparse after each change.

Incremental subtree reuse, green/red layering, and lazy binding can follow only
after the public boundaries are proven and profiling demonstrates value.

The representative grammar is not yet chosen. Attributes are attractive because
they exercise absent, valid, invalid, and partially structured values, but a
platform design must be checked against the more complex flow and layout
grammars before its APIs stabilize.

## Migration principles

- Preserve existing behavior while each grammar migrates.
- Do not require a flag day across native, WASM, web, and CLI consumers.
- Keep validated semantic APIs available during transition.
- Make source/syntax identities explicit at authoring boundaries.
- Never equate a malformed value with an absent/default value.
- Never discard source that cannot be parsed.
- Reuse one diagnostic vocabulary across parser and editor surfaces.
- Prefer reparsing over premature incremental complexity.
- Measure memory before introducing interning or structural sharing.

## Relationship to diagram properties

Native diagram properties should continue with the narrow semantic requirement:

- valid attribute multiplicity is `Option<Multiplicity>`;
- `None` means the default was not authored;
- `Some(1)` means `{1}` was explicitly authored;
- malformed lines remain preserved by the current syntax recovery path;
- the current properties feature does not introduce a temporary generic
  pseudo-Roslyn error wrapper.

Once the parser platform exists, diagram properties can consume declared symbols
and targeted diagnostics to display and repair partially valid attributes.

## Unresolved questions

- What is the authoritative workspace/document identity model?
- Is the current `Bundle` replaced, wrapped, or retained as storage?
- Should source text use `Arc<str>`, a rope, or another representation?
- Do WAML document sizes justify a rope before source editing lands?
- Which parser/grammar is the representative first migration?
- Do we need green/red trees, or are immutable typed trees with ranges enough?
- Where is trivia attached?
- How are missing tokens represented?
- How do declared/authoring symbols differ from validated semantic symbols?
- Which semantic queries need lazy evaluation?
- How are cross-document references invalidated after one document changes?
- How do operations target syntax nodes across revisions?
- How are stale code actions rejected or translated?
- What stable IDs, if any, survive reparsing?
- How are generated TypeScript/WASM APIs exposed during migration?
- What memory and latency budgets define success?
- Which malformed examples must remain editable in each grammar?
- How do format/serialize commands treat recovered syntax?
- Can parser implementations share infrastructure without forcing unrelated
  grammars into one monolithic parser?

## Required design work before implementation

- Inventory every parser and syntax representation in the repository.
- Map all `Bundle`, syntax-tree, model, diagnostic, serialization, and operation
  consumers.
- Catalog current recovery and round-trip behavior with fixtures.
- Define representative malformed documents for every grammar family.
- Establish memory and interactive-latency baselines.
- Compare full Roslyn green/red trees with a smaller immutable-tree design.
- Define snapshot, identity, syntax, semantic, diagnostic, and edit APIs.
- Produce a staged migration plan with compatibility seams.
- Review the architecture against native, web/WASM, CLI, and automation needs.

This draft is complete only as a parking place. The next session must resume the
brainstorming/design process rather than treating the direction above as an
approved implementation plan.
