# Completion suggestions for incomplete WAML constructs

Date: 2026-08-10
Status: designed, not implemented

## Problem

A WAML author who writes a keyword and stops gets no help with what the keyword
asked for. `- [Source Bundle](../concepts/implementation/source-bundle.md) as`
is a lifeline whose alias was never written. Until 2026-08-09 the parser did not
even mark it; it now does, along with six sibling shapes in the classifier
sections. Marking the mistake is not the same as saying what belongs there.

Two things are wanted, and they are separate features that happen to share a
trigger:

1. A completion list at the positions where the grammar expects an operand.
2. Diagnostic text that names what was expected rather than only that something
   is wrong.

## What already exists

- **A Roslyn-shaped syntax tree.** `GreenFactory` builds fixed slots; an operand
  the author never wrote is a zero-width missing token carrying a real position;
  unparsable bytes land in `SkippedTokensSyntax`. Every completion position in
  this design is already a typed slot in that tree.
- **A quick-fix engine.** `waml::uml::repair_actions` (`crates/waml/src/uml/repair.rs`)
  emits `CodeAction`s with real `TextEdit`s. The native editor consumes it at
  `crates/waml-editor/src/documents.rs`. It is not exposed over the LSP.
- **An LSP.** `crates/waml-cli/src/lsp/server.rs` serves `documentSymbol`,
  `definition`, `documentLink` and `semanticTokens`. A test asserts
  `completion_provider.is_none()`.
- **A partial expectation vocabulary.** `ExpectedSyntax`
  (`crates/waml/src/uml/declared.rs`) has eight variants and is carried by
  `DeclaredField::Incomplete`. It is under-specified: a lifeline alias reports
  itself as `ExpectedSyntax::MessageTarget`.
- **Accept-set diagnostics.** `UnknownLifelineHandle`, `UnknownFlowTarget`,
  `UnknownCallIdentity`, `UnresolvedLayoutRef`, `SlotUnknownAttribute` and
  `UnresolvedTarget` each already compute the set of values a position accepts.

## Prior art: what Roslyn does, and why this design differs

Roslyn starts from the same tree: fixed green-node slots, `IsMissing` tokens with
positions, skipped-token trivia. Its completion nonetheless reads neither slot
kinds nor parser diagnostics. `CompletionService` runs a set of
`CompletionProvider`s; each request first computes a `CSharpSyntaxContext` of
roughly forty hand-written predicates (`IsStatementContext`,
`IsMemberDeclarationContext`, and so on), and keyword suggestions come from dozens
of individually hand-written recommender classes. Those walks key off
`FindTokenOnLeftOfPosition`, not the token under the cursor.

Roslyn had no alternative. A recursive-descent parser cannot enumerate its own
follow set, and in C# slot kind badly under-determines meaning: an identifier in
an expression slot may want locals, types, namespaces or extension methods
depending on semantics absent from the tree. For the semantic half Roslyn reaches
for speculative binding (`TryGetSpeculativeSemanticModel`).

The cost is a second description of the grammar, kept honest by a very large
recommender test suite.

WAML differs where it matters. The grammar is line-oriented, closed and
unambiguous, and each typed slot maps one-to-one onto a candidate source:
`LifelineAlias` wants a fresh handle, `MessageCallId` wants a declared call id,
`FlowIdentity` wants a node name. Roslyn could not collapse context onto slot
kind; WAML can, and therefore adds no second grammar description.

Three Roslyn lessons are adopted unchanged:

- Look **left** of the cursor. At completion time the construct is unfinished.
- Keep providers **separate and independently testable**, one per candidate
  family, rather than one branching function.
- Do **not** let completion consume diagnostics. Same underlying facts, different
  consumers; coupling them makes both harder to change.

## Design

### Entry point

A new `crates/waml/src/uml/complete.rs`, sibling to `repair.rs`:

```rust
pub fn completions(
    context: ActionContext<'_>,
    document: DocumentId,
    offset: TextSize,
) -> Result<Vec<Completion>, ActionError>
```

`uml::ActionContext` (`crates/waml/src/uml/format.rs`) already validates that the
OKF catalog and the UML analysis are the same `Arc` and that all three session
revisions agree, returning `MismatchedCatalog` or `MismatchedAnalysisRevision`.
Completions inherit that check, so a stale request fails loudly instead of
offering names from a document that has since changed.

`Completion { label, insert, kind, detail }` is a WAML type. `crates/waml` must
not gain an LSP dependency; `waml-cli` owns that. Two consumers map it:

- **LSP.** Advertise `completion_provider` in `server.rs`; add
  `completion(&self, physical, position)` to `lsp/query.rs` beside `definition`,
  which already converts a path and an LSP position into a document and offset.
- **Native editor.** Calls `completions` directly, as `documents.rs` already
  calls `repair_actions`.

The module has two halves that do not know about each other: a locator answering
*where am I*, and candidate providers answering *what goes here*.

### The locator

```rust
fn expectation_at(tree: &SyntaxTree<UmlLanguage>, offset: TextSize) -> Option<Expectation>

struct Expectation {
    slot: UmlSyntaxKind,
    node: SyntaxNode<UmlLanguage>,
    prefix: TextRange,
}
```

Find the token to the left of the offset, skipping trivia, then walk ancestors to
the innermost node whose kind is a completion slot. No grammar table is
introduced, because the parser already wrote one down as `UmlSyntaxKind`: a
fixed slot holding a missing token at a real position is exactly the statement
"an operand belongs here".

Three cases:

- **After the keyword.** `as |` — the left token is `AsToken`, the parent slot is
  `LifelineAlias`, the prefix is empty, and every candidate is offered.
- **Mid-word.** `- order calls b|` — the left token is the partially typed
  identifier inside `MessageTarget` and the prefix is `b`. Providers filter on
  the prefix; the returned range covers it so a client replaces rather than
  appends.
- **Nothing expected.** After a complete construct, inside prose, or inside a raw
  Markdown region such as `## Operations`, the result is `None` and the list is
  empty. Silence is a normal result, never an error.

### Candidate providers

One function per family, `fn(&Expectation, &ActionContext) -> Vec<Completion>`.
Adding a family is a new function and a match arm; the locator never changes.

**`fixed_vocabulary`** — message verbs, relationship kinds, flow node kinds,
layout directions and hints, frontmatter `type:` values. Requires no analysis.

These tables must be **moved** to one module that the parser, the formatter and
this provider all read — not retyped. `canonical_layout_keywords` in
`crates/waml/src/uml/format.rs` already keeps a layout keyword list separate from
the parser's, which is one copy too many. A third copy is the same failure that
produced the reported-keyword / silent-operand asymmetry this work began with.

**`in_document_refs`** — lifeline handles at message endpoints, including the
`outside` and `use@gate` forms; handles of the used interaction after
`bind … to`; flow node identities after `transitions to`; declared call ids in
the `MessageReturnCall` slot after `for`; member refs in `## Layout`. All read
from `analysis.declared.concept(id)`.

That slot name is worth dwelling on, because it is the evidence the whole
approach rests on. A call id appears in two places: after `as`, where the author
declares one, and after `for`, where the author references one. If both were the
same slot kind, the locator could not tell a declaration from a reference and
would need a keyword test bolted on beside it — the first crack through which a
second grammar description enters. They are not the same kind. `as` builds
`MessageCallId`/`CallIdToken`; `for` builds `MessageReturnCall`/`ReturnCallToken`.
The parser already distinguishes declaration sites from reference sites, so the
providers can be selected on slot kind alone.

Each of these positions already has a diagnostic that rejects wrong values. The
completion list is the inverse of the same accept-set, which the design pins as
an invariant:

> Every candidate a provider offers at a position must be a value the
> corresponding diagnostic accepts at that position.

That invariant is a property test over the fixture corpus. It is what keeps
provider and diagnostic from drifting, and it is the part of Roslyn's problem
this design does not have to solve by hand.

**`link_targets`** — inside `](…)`, drawn from the OKF document catalog and
filtered by what the enclosing section accepts: a lifeline wants a Class or an
Actor, a flow `object` node wants a Class. Insert the bundle-relative path, label
with the document title, reusing the resolution `UnresolvedTarget` already
performs so that paths round-trip.

**`type_driven`** — slot names from the target classifier's attributes, an
accept-set `SlotUnknownAttribute` already computes; and values after `set to`
from the attribute's type when it resolves to an Enum.

**`derived_names`** — the positions where the author invents a name rather than
referencing one: `LifelineAlias`, `InteractionUseAlias`, the inline-instance
name, and `MessageCallId`. From the link title, offer the kebab
form, the lowercased first word, and the initials, minus any name already taken
in the document, since duplicates are themselves diagnosed
(`DuplicateSequenceName`, `DuplicateCallIdentity`). With no link to derive from,
offer nothing rather than guess.

For the case that prompted this work,
`- [Source Bundle](../concepts/implementation/source-bundle.md) as ` offers
`source-bundle`, `source`, and `sb`.

### The diagnostic half

`ExpectedSyntax` gains `Alias`, `InstanceName`, `SlotName`, `SlotValue` and
`CallId`. This corrects a live defect: `declared_lifeline` in
`crates/waml/src/uml/analysis.rs` reports a missing alias as
`ExpectedSyntax::MessageTarget`, so every consumer of that field is told the
wrong thing today.

Parser diagnostic messages stay static. The parser cannot see the analysis and
should not learn to; the wording landed on 2026-08-09 (`expected a lifeline alias
after "as"` and siblings) is the final form. Completion never reads diagnostics.
Both read the same slot-kind table.

### Errors

Mirrors `repair_actions`. An unknown document is an error. A catalog or revision
mismatch is an error. Everything else — an offset past the end of the document,
an incomplete tree, a position in prose — returns an empty list. An unfinished
document is the expected input, not a failure.

## Testing

- **Locator table.** An authored line with a `|` cursor marker maps to an
  expected slot kind. Dozens of cases, no GUI, and it covers the piece most
  likely to regress.
- **Per-provider tests** against a small fixture bundle.
- **Silence test.** No candidates at a complete construct, in prose, or in a raw
  `## Operations` region. Same shape as the truncation audit that found the
  parser gaps this work began with.
- **Accept-set invariant** as a property test over the corpus.
- **LSP round-trip.** Capability advertised, `textDocument/completion` returns
  items. This reverses the existing assertion that `completion_provider.is_none()`;
  that test records a deliberate past decision, so it is updated with a comment
  naming this spec rather than deleted.
- **Native popup.** A visual check, explicitly deferred. A GUI check cannot be
  verified headlessly and will otherwise stall an implementer indefinitely.

## Staging

Each slice lands green on its own.

1. Locator, the `Completion` type, and `fixed_vocabulary`. No analysis
   dependency. Includes moving the keyword tables to one owner.
2. LSP wiring. VS Code has working completions at the end of this slice.
3. `in_document_refs` and the accept-set invariant test.
4. `link_targets`, then `type_driven` and `derived_names`.
5. Native editor popup, with the visual check deferred.

Stopping after slice 2 leaves a coherent, useful feature.

## Out of scope

- Exposing `repair_actions` over the LSP. Worth doing, unrelated to this.
- Inline ghost text in the native editor.
- Completion inside frontmatter beyond the `type:` value.
- Snippet-style completions that insert multi-token skeletons. Every candidate
  here inserts one operand.
