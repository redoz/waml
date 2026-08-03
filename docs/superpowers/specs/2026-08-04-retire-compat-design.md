# Retire `compat` — design

**Date:** 2026-08-04
**Status:** approved (design), plan pending

## Problem

`crates/waml/src/compat.rs` opens with "Deprecated mixed-domain adapter retained
for DTO, CLI, and LSP callers", and everything in it is `#[doc(hidden)]`. That
label is half right, and the half it is wrong about is load-bearing.

The file holds two unrelated things:

1. **A genuine legacy bridge.** `TryFrom<crate::ops::Op> for Step` and
   `step_from_legacy` convert the old flat `waml::ops::Op` enum into steps. The
   only callers are the `waml::ops::apply`/`apply_source` shims and their tests.
   No production code constructs a legacy `Op` anymore — `waml-ops-dto`,
   `waml-cli::run_batch`, and `waml-editor`'s `editor_session` all build
   `compat::Step`/`Batch` directly.

2. **The current edit engine's composition layer.** `Step { Okf(okf::Op),
   Uml(uml::Op) }`, `Batch`, `apply`, and `MixedLoweringCursor` — the machinery
   that applies an ordered, interleaved cross-domain batch while incrementally
   cross-propagating invalidations between `OkfLoweringState` and
   `UmlLoweringState` (renames, inserts, removals, text changes). This is not an
   adapter and has no substitute: `edit::SequenceBatch` explicitly documents
   itself as *not* rebuilding projection data between children (history/undo
   only), and real user actions are mixed batches — "rename classifier" is
   `Okf(IndexRetitle)` + `Uml(ClassifierRename)` + `Uml(PlacementSet)` applied
   atomically.

The result: the blessed edit path is invisible (hidden, undocumented, in a file
that calls itself deprecated), the dead path (`waml::ops`) still exports
~1470 lines of tests and the flat `Op` enum, and shared value types squat in the
dead module. This bit during `waml serve` planning: the plan was written against
`OpDto::to_op`, a function that does not exist, because the module layout
misrepresents which layer is current.

## Decision: the mixed-domain batch is permanent, not transitional

Settled in brainstorming. Every client of the edit system — DTO wire, CLI, LSP,
the editor, the future `waml serve` — delivers ordered batches whose steps span
both domains. Eliminating the mixed batch would require either merging okf and
uml back into one enum (reversing a deliberate split) or full re-analysis
between every step (the cost `SequenceBatch` refuses). Neither is wanted.

The layering that already exists is correct and merely unnamed:

```
okf::ops::Batch      uml::ops::Batch      — single-domain edits
        \                 /
      edit — EditBatch trait, EditContext, PendingEdit, undo
        + mixed Batch/Step + MixedLoweringCursor   (today misfiled as "compat")
```

`edit` is the composition/transaction layer over the domains, so the mixed
batch moves there as its permanent home.

**Propagation seam, built properly this time:** today the cross-domain
invalidation flow is hard-coded pairwise — `MixedLoweringCursor` owns one
`OkfLoweringState` and one `UmlLoweringState` field and matches on
`StepFamily` to decide which side hears about which event
(`propagate_from`/`propagate_to_okf`/`propagate_to_uml`). The move formalizes
that into a named seam instead of copying it:

- `edit::Invalidation` — the event enum (`TextChanged`, `Inserted`, `Removed`,
  `Renamed`), promoted from the private `CandidateInvalidation` with its
  claimed-id payloads intact.
- `edit::InvalidationSink` — a trait each domain lowering state implements:
  `fn absorb(&mut self, event: &Invalidation) -> Result<(), EditError>`. The
  existing bodies of `propagate_to_okf` and `propagate_to_uml` become the two
  impls, living with their domains (`okf::lower`, `uml::lower`).
- The cursor holds the domain states uniformly and routes each step's events to
  every *other* domain's sink (self-notification stays limited to
  `TextChanged`, exactly today's `propagate_from` rule, expressed once instead
  of per-pair).

Behaviour is identical — this is the same flow with the seam named, so a third
domain later means one new `InvalidationSink` impl, not a cursor redesign.

## Changes

### 1. Mixed batch moves to `edit`

`Step`, `Batch`, `apply`, `MixedLoweringCursor`, the invalidation machinery,
the `EditBatch` impl for `Batch`, and compat's tests move from `compat.rs` into
the `edit` module: `waml::edit::Step`, `waml::edit::Batch`, `waml::edit::apply`.
`#[doc(hidden)]` is dropped; real doc comments name this the public edit
surface. If `edit.rs` grows unwieldy the plan may split it into `edit/mod.rs` +
`edit/batch.rs` internally — public paths stay `waml::edit::*`.

### 2. Value types move to their domains

- `NameSpec`, `FieldEdit`, `DiagramDisplaySet` move into `uml` proper. `uml`
  already re-exports them today (`uml.rs:13`), so `waml::uml::NameSpec`
  importers do not move; direct `waml::ops::…` importers (`waml-ops-dto`,
  `waml-cli`, `waml-editor`) re-point.
- `OpError` moves into `edit` and becomes `EditError` — the alias direction
  flips (today `edit::EditError = ops::OpError`). The `OpError` name dies.
  Field shape (`index`, `op`, `selector`, `reason`) is unchanged.

### 3. Legacy layer deleted

- `waml::ops` module deleted entirely: the flat `Op` enum, the
  `apply`/`apply_source` shims, the `Bundle` alias, the `referrers` wrapper
  (callers re-point to its `uml` home), and the `ops::selector` re-export
  (`uml` owns selectors).
- In `compat.rs`: `TryFrom<ops::Op> for Step` and `step_from_legacy` deleted.
- `compat.rs` deleted; `lib.rs` loses `pub mod compat` and `pub mod ops`.

### 4. DTO seam renamed

`OpDto::to_compat_step` / `from_compat_step` become `to_step` / `from_step`,
returning `waml::edit::Step`. Callers (ops-dto internals and tests,
`waml-cli::run_batch`, LSP, `editor_session`) re-point mechanically. The wire
format is untouched — `OpDto`'s serde shape does not change, only Rust method
names and return types.

### 5. Tests ported, not dropped

- The ~1470 test lines in `ops/mod.rs` and `tests/ops_golden.rs` port
  mechanically: each legacy `Op` literal becomes `Step::Uml(…)`/`Step::Okf(…)`
  applied via `edit::apply`. Coverage 1:1; golden files untouched.
- `tests/compat_lowering_order.rs` re-points its imports to `edit`.

## Error handling

No behaviour change anywhere. This is a move/rename/delete; the full gate
(`cargo test --workspace`, clippy `-D warnings`, `cargo fmt --all --check`,
vscode extension checks) green after every unit is the proof.

## Out of scope

- Any semantic change to okf/uml ops or lowering.
- DTO wire-format changes.
- Adding any third edit domain. The `InvalidationSink` seam makes room for
  one; nothing here uses it.
- `waml serve` (re-specced separately once this lands; its Task 1 command
  surface, already on main, is unaffected).
