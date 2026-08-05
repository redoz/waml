# Issue 27 — UML validation rules: one verdict function per rule

## Context

UML validation verdicts currently live in three layers that each re-implement the
same rules: `declared_projection`'s admission filters (silently drop invalid
declarations from the model), the diagnostic pass (reports to the user), and
`sequence::lower`'s inline checks. Where two copies of a rule exist they must
agree byte-for-byte or the model and the diagnostics diverge — an admitted
element with no diagnostic, or a diagnosed element that was silently dropped.

**Ownership rule adopted by this plan:** for every validation rule there is
exactly ONE verdict function, placed in the crate module that owns the concept.
Projection *admits or drops* by consuming the verdict; the diagnostic pass
*reports* by consuming the same verdict. Neither re-derives it. `sequence::lower`
never invents a rule of its own.

## Verdict evidence (verified 2026-08-04, worktree HEAD after d56ff738)

1. **Interaction-use binding checks — two full copies.**
   - Silent copy: `crates/waml/src/uml/sequence.rs:250-337`
     (`interaction_use_graph`) — dup-binding, unknown-endpoint, classifier
     mismatch, missing-participating checks collapse into a bare `valid` bool.
   - Diagnosed copy: `crates/waml/src/uml/sequence.rs:581-642` (inside `lower`) —
     the same four checks, each emitting `DiagCode::InvalidInteractionUse`.
   - Cross-check that only works while the copies agree:
     `sequence.rs:644-649` (`is_graph_link` demotes `valid_use` when the graph
     copy disagreed).

2. **Relationship-end validity — admission and diagnostic disagree in coverage.**
   - Admission: `crates/waml/src/uml/analysis.rs:1749-1775` (`ends_valid`
     match on `RelationshipKind`), silent `continue` at `:1776-1784`.
   - Diagnostic: `analysis.rs:1019-1046` only diagnoses the
     `Associates`-with-absent-ends-between-classifiers case. A one-ended
     `composes`/`aggregates` is dropped by admission with **no message at all**.

3. **`describes` link parsed two ways with different tolerance.**
   - Hand-split: `crates/waml/src/uml/sequence.rs:966-972`
     (`.split("](").nth(1)` + `strip_suffix(')')` — no `[` prefix check, no trim).
   - Proper parser: `crates/waml/src/uml/analysis.rs:1894-1913`
     (`resolve_describes` / `parse_link_ref` — trims, requires `[`, builds
     `LinkRef`). Inputs like ` [t](x)` or `t](x)` produce different results.

4. **Branch-join open-set lattice hand-coded three times in sequence.rs.**
   - `walk_return_items` `:1041`, `repeated_deletes` `:1349`, `walk` `:1460` —
     each re-implements the fragment recursion with per-operand snapshot/merge
     of an open set. This is also why the file carries four
     `#[allow(clippy::too_many_arguments)]` (`:380, :1040/1147, :1348, :1459`).

**VERDICT: APPROVE — all four items confirmed at the lines above.**

## Design decisions

- Verdict functions are pure: take declared data, return a verdict enum/struct;
  no `diagnostics: &mut Vec<_>` parameter. The *caller* decides silence vs
  report. This keeps them unit-testable in the headless crate (testability rule).
- Do not change behaviour where the copies currently agree; where they disagree
  (one-ended composes, malformed describes) the *diagnostic-bearing* behaviour
  wins: admission still drops, but a diagnostic is now emitted (observability
  rule: no silent drop of user content).
- The fragment-walk skeleton is a private generic in `sequence.rs`, not a new
  module — it only has one client file.

### Task 1: shared interaction-use binding verdict

- Add `enum UseBindingIssue { DuplicateBinding, UnknownEndpoint, ClassifierMismatch, MissingParticipant }`
  and `fn validate_use_bindings(...) -> Vec<(UseBindingIssue, /*binding index*/ Option<usize>)>`
  in `crates/waml/src/uml/sequence.rs`, extracted from the `lower` copy
  (`:581-642`), keyed so the caller can map an issue back to the offending
  `DeclaredBinding` syntax node for the span.
- Rewrite `interaction_use_graph` (`:250-337`) to call it and treat
  `is_empty()` as valid; rewrite the `lower` block to call it and map each
  issue to its existing `DiagCode::InvalidInteractionUse` message verbatim.
- Delete the `is_graph_link` demotion (`:644-649`) only if the two call sites
  provably use identical inputs; otherwise keep it and add a
  `debug_assert!` that it never fires.
- Tests: unit tests in `crates/waml/src/uml/` (or existing sequence test
  module) covering each `UseBindingIssue` variant, asserting the graph drops
  the use AND a diagnostic with the existing message text is emitted —
  one fixture per variant. Run `cargo test -p waml`.

### Task 2: shared relationship-end verdict

- Add `enum EndVerdict { Ok, EndsRequired, EndsForbidden, OneEnded }` and
  `fn relationship_end_verdict(kind: RelationshipKind, from: &DeclaredField<..>, to: &DeclaredField<..>, source_ty/target_ty context) -> EndVerdict`
  in `crates/waml/src/uml/analysis.rs`.
- Admission (`:1749-1784`) consumes it (drop on non-`Ok`), diagnostic pass
  (`:1019-1046`) consumes it and now emits `DiagCode::MalformedRelationship`
  for the previously-silent cases (one-ended composes/aggregates, forbidden
  ends), with spans from `relationship.syntax`.
- Tests: fixtures for one-ended `composes`, ended `extends` (forbidden), and
  the existing associates case; assert both the drop and the new diagnostics.

### Task 3: one describes/link-ref parser

- Make `parse_link_ref` (`analysis.rs:1905-1914`) and a
  `resolve_describes`-shaped helper `pub(crate)` (or move to a small shared
  spot in `uml/mod.rs`), and replace the hand-split at `sequence.rs:966-972`
  with it.
- Tests: table test over ` [t](x) `, `t](x)`, `[t](x`, `[t](x)` asserting
  both call paths agree.

### Task 4: generic fragment fold skeleton

- Add in `sequence.rs` a private
  `fn fold_fragment<S>(items: &[SeqChild], ctx: &FoldCtx<'_>, state: &mut S, on_message: impl FnMut(...), merge: impl Fn(kind, incoming: S, branches: Vec<S>) -> S)`
  (exact shape driven by the three clients) that owns the recursion,
  per-operand snapshot, and lattice merge currently copied in
  `walk_return_items` (`:1041`), `repeated_deletes` (`:1349`), `walk` (`:1460`).
- Bundle the shared read-only parameters (`edge_by_id`, `node_by_id`,
  `context`, `concept`, `path`, `diagnostics` handle) into a struct so the
  four `too_many_arguments` allows at `:380/:1040/:1147/:1348/:1459` can be
  removed where they fall below the threshold.
- Tests: the existing sequence lifetime/return proptests and fixtures must
  pass unchanged (`cargo test -p waml`); no behaviour change intended, so no
  new fixtures unless a divergence between the three copies is discovered —
  if one is, record it as its own diagnosed decision, don't paper over it.

### Task 5: gate and doc note

- Full gate: `cargo test --workspace` plus the editors/vscode lint/build.
- Add a short "ownership rule" comment at the top of
  `crates/waml/src/uml/analysis.rs` and `sequence.rs`: projection admits or
  drops, validate diagnoses, both consume the shared verdict functions —
  new UML rules must follow this shape.
