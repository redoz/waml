# Transition Traces — Design

**Date:** 2026-08-10  
**Status:** Approved for planning

## Goal

Add a typed `traces` relationship to state-machine and activity-diagram
transitions. A transition can trace to one or more WAML documents, document
fragments, model elements, or HTTPS resources. WAML must parse, preserve,
format, edit, resolve, diagnose, index, validate, display, and navigate these
relationships.

`traces` is neutral traceability metadata. It does not mean that a transition
satisfies, verifies, or realizes its target.

## Non-goals

- Do not change transition execution.
- Do not change solver input, diagram layout, routing, or rendered edge ink.
- Do not check whether an HTTPS resource exists or is reachable.
- Do not add trace relationships to other WAML element types in this change.
- Do not infer stronger requirement semantics from a trace.

## Authored syntax

Each `traces` clause contains exactly one Markdown link. A transition can have
zero or more clauses.

One trace can be inline:

```waml
- on `authenticated` transitions to SignedIn traces [AUTH-OIDC-004](./sign-in-behavior.md#auth-oidc-004)
```

Traces can also be indented children of the transition:

```waml
- on `authenticated` transitions to SignedIn
  traces [AUTH-OIDC-004](./sign-in-behavior.md#auth-oidc-004)
  traces [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)
```

The parser also accepts repeated inline clauses and a mix of inline and
indented clauses. Each clause remains a separate typed syntax node.

Ordinary syntax serialization preserves the authored placement, whitespace,
link spelling, order, and line endings. Canonical formatting uses these rules:

- Keep one trace inline.
- Put two or more traces on separate indented lines.
- Keep trace order.
- Normalize the keyword to `traces` while applying the existing link-format
  rules.

## Syntax tree

`FlowTransitionSyntax` gains one fixed `TRACES_SLOT`. The slot contains a
`FlowTraces` node, which contains zero or more `FlowTrace` nodes and the trivia
needed for lossless serialization. Existing transition slots keep their
indices.

Each `FlowTrace` owns:

- one present or missing `traces` keyword token;
- one Markdown `Link` node, including its label and authored href;
- inline or indented placement trivia;
- recovery syntax for malformed or incomplete input.

The transition parser must recover at the next trace clause, transition,
flow-node heading, or behavior section. A malformed trace must not consume its
sibling transition or node.

## Declared and projected models

`DeclaredFlowTransition` gains an ordered collection of declared trace fields.
Each field keeps its syntax node even when its value is invalid.

The public transition metadata exposes an ordered `TransitionTrace`
collection. Each trace contains:

- its display label;
- its exact authored href;
- its source syntax identity and range;
- its classified target kind;
- its resolution status and resolved destination, when available.

Target kinds are:

- internal document or model element;
- internal document fragment;
- HTTPS resource;
- unresolved internal target;
- invalid or unsupported target.

The trace collection is available to editor projections, validators, and
indexers. The execution and layout projections do not consume it.

## Resolution

Trace resolution reuses WAML's ordinary href and navigation rules.

- `./file.md` resolves relative to the referring document.
- `./file.md#fragment` resolves the document and then its fragment.
- `#fragment` resolves in the referring document.
- `https://...` is a valid external target.

Fragment resolution uses the same anchor index as normal navigation. This
includes Markdown document fragments and addressable WAML model elements. The
resolver must not introduce a second fragment-normalization algorithm.

The parser stores a general Markdown href. URI policy remains in semantic
resolution, so support for another scheme does not require a grammar change.

## Diagnostics

Diagnostics attach to the smallest useful trace range. Prefer the href range
for target failures and the trace clause range for structural failures.

Required diagnostics are:

- missing internal file;
- existing file with an unresolved fragment;
- malformed or missing link;
- empty href;
- unsupported URI scheme.

HTTPS targets do not receive file or fragment diagnostics. WAML does not make
network requests during validation.

An invalid or unresolved trace remains in the syntax and declared model. It is
available to formatters, editors, validators, and indexers with its failure
status.

## Structural editing

Add structural operations to insert, update, remove, and reorder transition
traces. Operations identify the selected transition by its syntax identity or
its source-node-scoped occurrence identity. They must not identify a
transition only by source and target names because parallel transitions can
share both.

Editing rules are:

- Adding the first trace can author it inline after the existing transition
  fields.
- Adding another trace inserts an indented clause after the last trace.
- Updating a trace changes only that trace's label or href.
- Removing a trace does not rewrite unrelated transition fields.
- Reordering traces preserves every trace value and changes only their order.
- Formatting can later convert any valid authored placement to the canonical
  one-trace or multi-trace form.

All editor actions must use structural operations. They must not use ad hoc
source-string replacement.

## Inspector

When a user selects a state-machine or activity-diagram transition, the
inspector shows a **Traces** section. It lists trace cards in authored order.
Each card shows:

- the link label;
- the document, fragment, or HTTPS destination;
- resolved, unresolved, or invalid status;
- controls to open, edit, remove, or reorder the trace.

The section has an add action. The add and edit controls accept one label and
one href. Applying an edit dispatches the structural operation and refreshes
analysis from the updated document.

Opening a resolved internal document or fragment uses normal WAML navigation.
Opening an HTTPS trace uses the editor's external-link action. An unresolved
target stays editable and shows its diagnostic state; it does not navigate to
an unrelated fallback.

Inspector selection and editing must distinguish parallel transitions.

## Indexing and validator access

The index records every authored trace as a typed outgoing `traces` edge.
Resolved internal targets also receive reverse incoming-trace entries. HTTPS
targets have outgoing entries only. Invalid and unresolved entries remain
queryable with their status and source range.

Validators consume the same public transition trace records. They must not
reparse Markdown or inspect Notes blocks to find traceability data.

Rename and move operations must treat resolved internal trace hrefs as typed
references and rewrite them with the same relative-href rules as other WAML
links. The fragment suffix must be preserved.

## Execution and layout isolation

Trace metadata stops before flow compilation. The solver, rank assignment,
edge routing, geometry, hit testing, and rendered labels receive the same
inputs with or without traces.

Tests must compare the compiled flow and diagram geometry for equivalent
documents with and without traces.

## Verification

### Syntax and losslessness

- Parse inline, indented, repeated, and mixed trace forms.
- Preserve ordering, Unicode labels, href spelling, CRLF, whitespace, and
  malformed input during ordinary serialization.
- Verify fixed occurrence indices for existing transition slots.
- Verify recovery at each valid sibling boundary.

### Formatting

- Format one trace inline.
- Format multiple traces as ordered indented clauses.
- Verify idempotence.

### Resolution and diagnostics

- Resolve relative documents, same-document fragments, cross-document
  fragments, and HTTPS targets.
- Diagnose missing files, unresolved fragments, malformed links, empty hrefs,
  and unsupported schemes at exact ranges.
- Verify that HTTPS resolution performs no network access.

### Editing

- Add, update, remove, and reorder traces through structural operations.
- Preserve unrelated fields and trivia.
- Edit the correct member of parallel transitions.
- Rewrite internal trace hrefs during document rename or move while preserving
  fragments.

### Inspector, indexing, and validation

- Project all trace states into inspector cards.
- Add, edit, remove, reorder, and navigate from the inspector.
- Open HTTPS targets through the external-link action.
- Expose outgoing, reverse incoming, unresolved, and invalid index entries.
- Verify that validators use typed trace records.

### Semantic and visual regression

- Confirm that compiled flow behavior is unchanged.
- Confirm that diagram node and edge geometry is unchanged.
- Run targeted crate tests, then the full workspace test suite.

## Expected implementation areas

- `crates/waml/src/uml/syntax`: syntax kinds, AST accessors, and parsing
- `crates/waml/src/uml`: declaration, analysis, formatting, lowering, and ops
- `crates/waml/src`: public trace model, href resolution, diagnostics, and index
- `crates/waml-editor/src`: transition selection, inspector projection,
  inspector controls, navigation, and external-link dispatch
- `crates/waml/tests` and `crates/waml-editor` tests: end-to-end verification

