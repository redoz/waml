# Sequence Flat Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flatten the runtime `SequenceDoc` into the unified nodes+edges shape — lifelines/fragments/operands become `SeqNode`s, messages become ordered, interaction-local `SeqEdge`s, and containment is preserved by id reference — without touching the on-disk (markdown) storage format.

**Architecture:** `SequenceDoc` is the *runtime* interaction model built by `build_interactions` (`crates/waml/src/parse.rs`) from the nested `## Lifelines`/`## Messages` *syntax* layer (`SeqItemSyntax`/`MessagesBlock`/`LifelineLine`). Storage (the syntax layer + grammar/serialize round-trip) is the source of truth and stays nested — this is exactly the spec's "storage and runtime are mostly, but not always, 1:1" case (§9, §6). Only the runtime model is reshaped: `Lifeline`/`SeqItem`/`SeqOperand` are replaced by `SeqNode` (tagged `lifeline`/`fragment`/`operand`), `SeqEdge` (a message), and `SeqChild` (an ordered `message`/`fragment` ref). The flat model is emitted verbatim to TypeScript through Tsify, so the wasm bindings regenerate and the Svelte sequence renderer rebuilds from the flat shape.

**Tech Stack:** Rust (`waml` crate; `cargo`, serde internally-tagged enums, `tsify_next`/`wasm-bindgen` codegen), TypeScript monorepo (`packages/`, `pnpm`, Svelte 5 runes, Vitest).

## Assumes the behavior slice has ALREADY landed

This plan is built SERIALLY, **on top of** the `2026-07-18-behavior-model-view-split` slice (its predecessor on `main`). It was originally drafted in parallel, blind to that slice; every task below has been rebased onto post-behavior state. Preconditions a fresh worker must take as given:

- **`ElementType::is_view()` exists** and is the single node-pool filter in `parse.rs::build_model` and `validate.rs::link` (it replaced both `!= Diagram && !matches!(Behavior(_))` filters). Do NOT re-add the old filter and do NOT revert `is_view()`. This slice does not touch that predicate or `validate.rs` at all.
- **The flow types are already reshaped in `model.rs`:** `FlowNode` is now `ActivityNode`, `FlowEdgeKind` exists, `FlowEdge`/`FlowDoc` are reshaped, and `Model` carries `activity_nodes: Vec<ActivityNode>` + `flow_edges: Vec<FlowEdge>` pools (wire `activityNodes` / `flowEdges`). `FlowDoc.nodes` / `.edges` are `Vec<String>` pool keys. Leave ALL of this intact — this slice reshapes only the sequence/interaction types and adds nothing to the shared `Model.nodes` / `Model.edges` structural pools.
- **The `parse.rs` `use crate::model::{…}` block already imports** `ActivityNode`, `FlowEdge`, `FlowEdgeKind` (behavior renamed `FlowNode`→`ActivityNode` and added `FlowEdgeKind` there). This slice swaps ONLY the sequence names (`Lifeline`/`SeqItem`/`SeqOperand` → `SeqChild`/`SeqEdge`/`SeqNode`) inside that same block, leaving the flow names untouched — see the corrected Step 1.4.
- **`ModelGraph` / `toModelGraph` (TS) already carry** `activityNodes?` / `flowEdges?` pools and a key-based `flows?` view; the flow re-export regions of `packages/wasm/src/index.ts` and `packages/okf/src/types.ts` already read `ActivityNode` / `FlowEdgeKind`. This slice adds/reshapes only the sequence region additively — no renames of, or collisions with, the behavior pools.
- **All quoted line numbers below predate the behavior slice.** The behavior split GROWS the flow-types section of `model.rs` and the `build_flows` function of `parse.rs` (both sit *above* this slice's edit sites), so every absolute line reference here is offset downward after behavior landed. Locate each edit by struct/enum/function NAME (given at every step), not by the stale line number. The edit sites themselves are disjoint from the behavior slice's, so the diffs apply cleanly on top.

## Task index (single atomic task)

This plan is segmented for implement-plan directory-plan mode. It is ONE atomic vertical — the Rust runtime shape, the wasm binding it regenerates, and the TS consumers that read it are coupled through the generated `waml_wasm.d.ts`, so they land in a single green commit. There is exactly one task segment:

1. [Task 1 — Flatten the sequence runtime model end-to-end](task-1-flatten-sequence-runtime-model.md)

The full rebased plan (this slice, rebased onto the already-landed behavior slice) is preserved verbatim as [`_source.md`](_source.md).

---

## Global Constraints

- **Full CI gate, in this order** (from `.github/workflows/ci.yml`): `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`. All must pass before the commit.
- **Storage format is NOT redesigned** (spec §9 non-goal). Do NOT touch the sequence *syntax* layer or round-trip: `crates/waml/src/syntax.rs` (`SeqItemSyntax`, `SeqOperandSyntax`, `LifelineLine`, `MessagesBlock`, `ParsedMessage`), `crates/waml/src/grammar.rs` (`parse_messages_block`/`render_messages_block`/`parse_lifeline_line`/`render_lifeline_line`), and `crates/waml/src/serialize.rs`. Source `.md` bytes round-trip identically before and after this change.
- **Message stays interaction-local and ordered** (spec §6): `SeqEdge` lives inside `SequenceDoc`, never in `Model.edges`; it is NOT a reusable pool edge and NOT an `Association`. Document order = time order — `edges` is emitted in document (pre-order) order and each container's ordered `items` fixes local time order.
- **`Lifeline.ref_` semantics are unchanged** (spec §6): it references a pool classifier (types-by). Do NOT widen the target to `InstanceSpecification` — that is slice §7.4, out of scope.
- **Do NOT touch the behavior/flow pools or view** — `FlowDoc`, `ActivityNode`, `FlowEdge`, `FlowEdgeKind`, the `Model.activity_nodes` / `Model.flow_edges` pools, or `ElementType::is_view()`. The behavior model/view split slice has ALREADY landed (serial predecessor). Do NOT revert `FlowNode`→`ActivityNode`, do NOT drop `FlowEdgeKind`, and do NOT re-add the old `!= Diagram && !matches!(Behavior(_))` node-pool filter.
- **Do NOT edit files under `docs/`.**
- **Do NOT hand-edit generated/auto files:** `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) is regenerated by `pnpm build:wasm`; `packages/core/src/templates/orders-checkout-sequence.bundle.ts` is `waml bundle` output (its markdown is unchanged by this slice anyway).
- **Idiomatic Rust:** `cargo fmt` touched files and introduce no new `cargo clippy` warnings before the commit. Exhaustive matches over the `SeqNode`/`SeqChild` variants — no `_ =>` catch-alls.


---

## File Structure

This slice is a single atomic vertical: the Rust runtime shape, the regenerated wasm binding it produces, and the TS consumers that read it are coupled through the generated `waml_wasm.d.ts`, so they land in one green commit. Files touched:

**Rust (runtime model + builder + tests):**
- `crates/waml/src/model.rs` — replace `Lifeline`, `SeqOperand`, `SeqItem`, and reshape `SequenceDoc` with `SeqChild`, `SeqEdge`, `SeqNode`, `SequenceDoc`. `MessageVerb` and `FragmentKind` are unchanged. (Pre-behavior these sat at ~`Lifeline` 508-525 / `SeqOperand` 527-539 / `SeqItem` 541-562 / `SequenceDoc` 564-579, with `MessageVerb` 441-475 / `FragmentKind` 477-505; the behavior slice grew the flow-types section above them, so all of these are offset downward — locate by name.)
- `crates/waml/src/parse.rs` — the `use crate::model::{…}` import block; rewrite `build_interactions`; rewrite the `builds_sequence_doc_with_resolved_lifelines_and_nested_messages` test. (Pre-behavior: import block 12-15, `build_interactions` 778-849, test 1427-1453; behavior's `build_flows` rewrite above `build_interactions` shifts these downward — locate by name.)
- `crates/waml/tests/serde_shape.rs` — rewrite the `sequence_doc_json_matches_ts_field_names` test (145-163) to the new wire shape.

**Regenerated binding (do not hand-edit):**
- `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) — regenerated by `pnpm build:wasm`.

**TS type barrels (mirror the regenerated names):**
- `packages/wasm/src/index.ts` — re-export list (46-51).
- `packages/okf/src/types.ts` — re-export list (18-30).

**TS consumers (rebuild from the flat shape):**
- `packages/web/src/canvas/sequenceLayout.ts` — full rewrite of `layoutSequence` + `SeqRow`.
- `packages/web/src/components/canvas/sequence/SequenceView.svelte` — read `row.edge.*` instead of `row.item.*`.
- `packages/web/src/canvas/sequenceLayout.test.ts` — flat fixtures.
- `packages/web/src/components/canvas/sequence/SequenceView.test.ts` — flat fixtures.
- `packages/core/src/state/overlay.test.ts` — flat seq fixture (195-203).
- `packages/core/src/state/diagrams.test.ts` — flat empty fixtures (36, 50).
- `packages/core/src/nav/tree.test.ts` — flat empty fixture (31).

**Verified NO change** (read only `.key`/`.title`, or pass through opaquely): `crates/waml/src/validate.rs` (walks the syntax layer, not the model), `crates/waml-editor/src/tree.rs`, `packages/core/src/state/overlay.ts`, `packages/core/src/state/diagrams.ts`, `packages/core/src/nav/tree.ts`, `packages/core/src/nav/search.ts`, `packages/core/src/templates/*`.

No files are created or deleted.


---

## Notes / risks

- **Why one atomic task:** the checked-in `packages/wasm/src/generated/waml_wasm.d.ts` couples the Rust runtime shape to the TS consumers. A Rust-only intermediate commit would drift that binding (CI runs `pnpm build:wasm`) and fail `pnpm build`, so the whole vertical lands in one green commit. This mirrors the element-pool-rename plan's atomic cross-crate commit.
- **Storage untouched = byte-stable round-trip:** the syntax layer (`SeqItemSyntax`/`MessagesBlock`) and `serialize.rs`/`grammar.rs` are not modified, so `.md` files parse→serialize identically. `validate.rs` walks the syntax layer (not `SequenceDoc`), so it needs no change; its sequence tests stay green.
- **Determinism / stability:** ids are structural (`m{n}` in document/time order, `f{n}` pre-order, `f{n}.o{j}`), so the emitted model is stable for a given source — important because `interactions` serializes into the model JSON.
- **Empty operands are fine:** containment is explicit (`operands` ids + `items` streams), so an `else` operand with no messages still round-trips as `{ node: "operand", id, items: [] }` — no position inference needed.
- **`pnpm build:wasm` needs the wasm toolchain** (`wasm-pack`, `wasm32-unknown-unknown`) — same as every change in this repo and part of the CI gate.


---

## Self-Review

**1. Spec coverage (§6 + task):**
- "lifeline / fragment / operand become nodes" → `SeqNode::{Lifeline, Fragment, Operand}` (Step 1.3). ✓
- "messages become ordered, interaction-local edges" → `SeqEdge` inside `SequenceDoc` (never `Model.edges`), `edges` emitted in document/time order (Steps 1.3, 1.5). ✓
- "containment preserved (fragments contain operands contain items)" → `Fragment.operands` ids + `Operand.items` stream + root `items` (Steps 1.3, 1.5); verified by the wire test and unit test (Steps 1.1, 1.6). ✓
- "Lifeline references a pool element via `ref_`; do NOT widen to instances" → `Lifeline.ref_` kept as-is; doc comment records §7.4 is out of scope (Step 1.3). ✓
- "message is its own edge kind, NOT a pool edge / NOT an Association, ORDERED" → dedicated `SeqEdge`; doc comment + local ordering (Step 1.3). ✓
- Combined fragments `alt`/`opt`/`loop` + guards keep nesting → `FragmentKind` unchanged; `Operand.guard` kept; nesting via ids (Steps 1.3, 1.5). ✓
- §9 non-goal (no storage redesign) → syntax/grammar/serialize untouched (Global Constraints, Notes). ✓

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to"/"write tests for the above". Every code step shows complete code. ✓

**3. Type consistency:** `SeqNode`/`SeqEdge`/`SeqChild`/`SequenceDoc` field and variant names are identical across model.rs (1.3), build_interactions (1.5), Rust tests (1.1, 1.6), regenerated binding (1.9), barrels (1.10, 1.11), and TS consumers/fixtures (1.12–1.18): `SeqChild = {item:"message",edge} | {item:"fragment",node}`; `SeqEdge{id,from,verb,to,signature?}`; `SeqNode` tag `node` with `lifeline{id,title,alias?,ref?}`/`fragment{id,kind,operands}`/`operand{id,guard?,items}`; `SequenceDoc{key,title,describes?,nodes,edges,items}`. The frontend reads `row.edge.*` consistently after the `SeqRow` message variant renamed `item`→`edge` (1.12, 1.13). ✓


---

## Cross-slice contract (relative to the already-landed behavior/flow slice)

The behavior slice and this sequence slice **deliberately diverge** on element promotion, and that divergence is spec-sanctioned (design spec §4 vs §6) — not an inconsistency to reconcile away:

- **Flows are pooled; the interaction stays inline — by design.** The behavior slice promotes activity/state-machine elements to model-level pools (`Model.activity_nodes` + typed `Model.flow_edges`) and makes `FlowDoc` a view of pool keys, per spec §4 (reusable activity elements live in the Element pool). This slice keeps `SeqNode`/`SeqEdge`/`SeqChild` **inline on `SequenceDoc`**, per spec §6: a Message is ordered + interaction-local (NOT a reusable pool edge, NOT an Association), and fragments/operands are inherently ordered-local. So flows go pool-level while the interaction stays document-local — the asymmetry is intentional and correct. (An earlier draft of this note wrongly claimed "the flow plan must leave `FlowNode`/`FlowEdge` out of the shared pool"; that was written blind to the behavior slice and is retracted.)
- **Neither slice touches the STRUCTURAL pool or the classifier pool.** Behavior's activity pools (`Model.activity_nodes` / `Model.flow_edges`) are their own model-level pools, separate from the structural `Model.nodes` / `Model.edges`; this slice adds nothing to any shared pool. Both behavior docs and interactions stay OUT of the classifier `Model.nodes` pool via `ElementType::is_view()` (the predicate the behavior slice added and this slice relies on unchanged). Messages stay interaction-local (§6).
- **Type names are substrate-specific and non-colliding:** `ActivityNode` / `FlowEdge` / `FlowEdgeKind` (flow, post-behavior — `FlowNode` was renamed to `ActivityNode`) vs `SeqNode` / `SeqEdge` / `SeqChild` (sequence). Do not introduce a shared `Node`/`Edge` behavior type that would collide with the structural `Model.nodes`/`Model.edges`. Shared "unified shape" vocabulary is only skin-deep: both expose `nodes` + `edges`, and `SequenceDoc` adds an `items` ordered-containment stream that flows lack.
- **`ElementType` / `is_classifier()` / `is_view()` are the shared predicates** both slices rely on unchanged; neither widens `BehaviorKind` or the metaclass set here.