# Task 21 Parser Authority Boundary Addendum

**Status:** Approved direction; implementation addendum

**Date:** 2026-07-29

**Applies to:** Task 21, “Delete the Final Legacy Parser and Serializer
Authority,” in `../plans/2026-07-28-parser-platform-implementation.md`

**Builds on:** `2026-07-26-parser-platform-design.md`

> This addendum supersedes only Task 21's overgrown enforcement
> implementation and its associated evidence. It does not supersede the
> parser-platform design, alter the architecture approved there, or change the
> scope or acceptance criteria of any other implementation-plan task.

## Context and problem

Task 21 successfully removed the legacy parser and serializer, but its
architecture test grew into a second language implementation. The current
`authority_guard.rs` is a roughly 5,950-line partial Rust interpreter that
models `syn` syntax, names, types, calls, taint, control flow, modules, and
selected macros. Its adversarial fixtures test that interpretation rather than
the WAML parser platform. Generic bounds and local macros still bypass the
analysis, so the size and complexity do not produce the claimed guarantee.

The guard also affected unrelated production code. Ordinary `Vec` expressions
in the editor were qualified as `std::vec::Vec` to accommodate the interpreter,
and a useful lowering error lost the requested concept name. These are signs
that the enforcement mechanism is shaping production behavior instead of
checking a stable boundary.

The parser platform itself does not need redesign. Independent review found the
crate split, source authority, analysis pipeline, exact writer, formatter, and
semantic model boundaries sound. The correction is to enforce only properties
that Rust and Cargo can state honestly, and to leave review conventions to
review.

## Goals

Task 21 will:

- retain one domain-neutral shell parser and exact green-tree writer in
  `waml-syntax`;
- retain UML grammar and recovery in `waml::uml::syntax`;
- make raw UML parser implementation details inaccessible outside their
  defining syntax module;
- keep one narrow crate-internal full-parse facade for legitimate UML analysis
  and lowering;
- keep `SourceBundle` mutation crate-private and `prepare_candidate` as the
  public analysis choke point;
- remove the legacy parser/serializer files, exports, and exact retired
  symbols;
- enforce the workspace dependency direction through Cargo metadata;
- enforce parser visibility through Rust compile failure; and
- restore ordinary runtime code and the target-bearing lowering diagnostic.

## Non-goals

Task 21 will not:

- interpret Rust source, infer types, expand macros, propagate taint, or prove a
  semantic property over arbitrary Rust programs;
- claim that deliberate construction of a new parser inside already-authorized
  `waml-syntax` or `waml::uml::syntax` code is machine-prevented;
- merge exact syntax writing, canonical UML formatting, or semantic `Model`
  serialization;
- introduce another source store, mutation API, preparation path, or revision
  authority;
- change the current crate split or make UML a runtime plugin;
- add incremental reparsing, change maps, reuse windows, retention logic, or
  any other Task 22 behavior; or
- broaden Task 21 into unrelated parser, editor, CLI, LSP, or model changes.

Creating a new parser inside code that already owns parsing remains a deliberate
architecture change caught by normal review. The automated checks do not claim
otherwise.

## Architecture and ownership

The approved ownership remains:

```text
waml-syntax
  domain-neutral Markdown/frontmatter shell parser
  shared text and green/red syntax infrastructure
  exact SyntaxTree writing

waml
  SourceBundle and document identity
  OKF analysis and static specialization composition
  public candidate preparation
  semantic model/value types and domain lowering

waml::uml::syntax
  UML syntax kinds, typed wrappers, grammar, recovery, and full parser

waml::uml
  recognition, declared analysis, validation, Projection, Formatter, and
  crate-internal consumers of the full-parse facade
```

`SourceBundle` remains the sole source/document authority. Its public
constructors remain the host-ingress boundary, while raw mutation helpers stay
`pub(crate)` for the approved Lowerer paths. Syntax and analysis remain
immutable derived views.
`prepare_candidate` remains the public function that turns candidate source
into a coherent OKF/UML analysis result. No raw UML parse function becomes a
public alternative.

The three output concepts remain separate:

1. `SyntaxTree` exact writing reproduces authored bytes, including malformed
   input and recovery.
2. `uml::Formatter` deliberately produces canonical WAML text.
3. Semantic `Model` serialization represents the model contract and is not a
   source serializer.

Task 21 removes the legacy source serializer without conflating any of these
retained responsibilities.

## APIs and visibility

`waml::uml::syntax::parser` becomes a private child module:

```rust
mod parser;

pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}
```

The implementation function in `parser.rs` is `pub(super)`, which is only
visible far enough for its parent `syntax` module to call it. The module itself
is not `pub(in crate::uml)`, and neither `parser::parse` nor `parse_full` is
externally public. Consequently:

- code outside `waml` cannot name the parser module or the facade;
- code elsewhere in `waml::uml` can use only `syntax::parse_full`;
- UML parser internals remain free to change without creating a public
  compatibility contract; and
- legitimate full-parse callers have one stable entry point.

All current direct callers in UML analysis, lowering, and rename logic route
through `syntax::parse_full`. No compatibility alias preserves the old raw
module path.

## Data flow

The full preparation path remains:

```text
SourceBundle
  -> waml-syntax full shell parse
  -> OKF analysis and DomainAnalysisContext
  -> UML recognition
  -> waml::uml::syntax::parse_full
  -> declared UML and validated Projection
  -> disjoint-claim validation
  -> PreparedCandidate
```

Lowerers continue to clone a candidate `SourceBundle`, apply crate-private
source changes, and reparse touched UML syntax when needed. Those reparses use
the same shell parser and `syntax::parse_full` facade before the candidate
re-enters normal validation/preparation. They do not expose source mutation or
raw parsing to consumers.

Recovery remains non-panicking and diagnostic-producing as defined by the
parser-platform design. The facade does not create a second error policy: text
and shell construction errors are handled by their existing callers, while the
UML parser returns a recovered tree with syntax diagnostics.

## Error handling and runtime restoration

Removing the source interpreter also removes its production accommodations:

- editor code returns to ordinary `Vec::new` and `Vec::with_capacity`
  expressions; and
- UML lowering reports `no claimed concept '{target}'` when lookup of the
  requested target fails.

These are restorations, not parser behavior changes. The target-bearing message
is required because it identifies the failed operation input. Existing error
types, operation codes, atomicity, and propagation remain unchanged.

## Testing and enforcement

The replacement guard has three focused layers.

### 1. Legacy-removal assertions

`no_legacy_authority` asserts that:

- `src/grammar.rs`, `src/parse.rs`, `src/syntax.rs`, and `src/serialize.rs` do
  not exist;
- `lib.rs` does not export the corresponding root modules; and
- the exact retired authority names are not defined or exported:
  `Document`, `Section`, `Line`, `ErrorNode`, `parse_document`, `build_model`,
  `build_model_from_source`, `project_okf`, and `serialize_document`.

The check is intentionally finite and literal. It proves removal of the named
legacy surface; it does not classify arbitrary functions as parsers or
serializers.

### 2. Cargo dependency assertion

A test invokes `cargo metadata --format-version 1` and examines workspace
package dependency declarations. Among workspace packages, the complete set of
packages with a direct normal, build, or development dependency named
`waml-syntax` must be exactly `{waml}`.

This lets Cargo define packages and dependencies instead of reconstructing
manifests. Transitive use through `waml` is allowed. A new direct dependency
from the editor, CLI, DTO crate, or another workspace package fails with the
offending package name.

### 3. External compile-fail assertion

A minimal temporary crate depending only on `waml` attempts to name the raw UML
parser path and the crate-internal full-parse facade. `cargo check` must fail
because those items are private. The fixture uses a separate target directory
and verifies a privacy failure rather than accepting an unrelated manifest or
resolution failure.

Rust visibility is therefore the authority for parser access. Existing API
tests continue to cover crate-private `SourceBundle` mutation, and preparation
tests continue to cover `prepare_candidate`; Task 21 does not duplicate the
compiler with source analysis.

The AST/taint interpreter, its `syn` dependency when no longer used elsewhere,
and the adversarial `authority-guard` fixture maze are deleted. The remaining
test should be small enough that each assertion maps directly to one claimed
property.

## Migration and removal scope

Task 21 implementation consists of:

- deleting the four legacy authority files and their root exports;
- completing the already-planned migration of retained semantic consumers away
  from the retired symbols;
- changing `uml::syntax::parser` from crate-UML-visible to private;
- adding `syntax::parse_full` and routing every legitimate internal caller
  through it;
- replacing the interpreter-based guard with the three focused checks above;
- deleting interpreter-only fixtures and development dependencies;
- restoring ordinary editor `Vec` expressions; and
- restoring the target-bearing UML lowering diagnostic.

No production refactor is justified solely to make a text scanner understand
Rust. Any concurrent Task 21 migration needed to compile against the already
approved parser platform is preserved; unrelated work is not reverted.

## Compatibility constraints

Task 21 preserves:

- the `waml-syntax` crate and its public domain-neutral syntax API;
- public `waml` analysis, formatter, projection, validation, Lowerer, and model
  contracts retained by the parser-platform design;
- generic OKF handling for arbitrary, missing, and unknown `uml.*` types;
- Index/Log separation and static specialization composition;
- exact syntax round-tripping and existing recovery diagnostics;
- atomic editor, CLI, and LSP preparation behavior through
  `prepare_candidate`; and
- Rust 2021 and the workspace MSRV.

The removed legacy parser/serializer API is intentionally incompatible and is
not reintroduced under a new name. Raw UML parser access was never a supported
external contract and becomes compiler-enforced private.

## Task 22 boundary

`syntax::parse_full` is retained as Task 22's exactness and semantic oracle.
Task 22 may compare incremental results with this full parse and may use it for
named fallback cases. Keeping the facade stable avoids coupling Task 22 to raw
parser-module layout.

Task 21 adds no incremental entry point, `TextChange`, `ChangeMap`, safe-window
selection, green reuse, annotation remapping, weak-reference retention gate, or
previous-snapshot integration. Those remain exclusively Task 22 work. Full
parsing is the only behavior behind the facade at the end of Task 21.

## Acceptance criteria

Task 21 is complete when:

1. the four legacy authority files, root exports, and exact retired symbols are
   absent;
2. `waml::uml::syntax::parser` is private and all legitimate UML callers use
   the single `pub(in crate::uml)` `syntax::parse_full` facade;
3. an external crate fails to compile when naming either the raw parser module
   or the full-parse facade for the expected privacy reason;
4. Cargo metadata shows that `waml` is the only workspace package with a direct
   dependency on `waml-syntax`;
5. `SourceBundle` mutation remains crate-private and `prepare_candidate`
   remains the public analysis choke point;
6. the AST/taint interpreter, adversarial fixture maze, and interpreter-only
   dependency are absent;
7. exact `SyntaxTree` writing, canonical `uml::Formatter` output, and semantic
   `Model` serialization remain distinct;
8. the lowering error includes the requested target and the editor uses
   ordinary `Vec` expressions;
9. focused authority tests and the full workspace tests/checks pass; and
10. no Task 22 incremental implementation is included.
