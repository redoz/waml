# Behavior Model/View Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reverse today's "behavior doc IS model AND view." Activity/state-machine flow *elements* become reusable model-level pool members (`ActivityNode`); transitions become typed model-level pool edges (`FlowEdge`, discriminated `ControlFlow` / `ObjectFlow`); and `FlowDoc` becomes a **view** that references those pool members by key — exactly as a class `Diagram` references pooled classifiers by `members`.

**Architecture:** Today `Model.flows: Vec<FlowDoc>` and each `FlowDoc` owns its `nodes: Vec<FlowNode>` / `edges: Vec<FlowEdge>` inline (model AND view). This plan introduces two model-level pools — `Model.activity_nodes: Vec<ActivityNode>` and `Model.flow_edges: Vec<FlowEdge>` — populated during `build_model`. `FlowDoc.nodes` / `FlowDoc.edges` become `Vec<String>` pool keys (`"{behavior}#{localId}"` for nodes, `"{behavior}#e{n}"` for edges). Cross-document transition semantics (`to_ref` = target behavior key, unresolved link labels not drawn) are preserved verbatim. The markdown storage format (grammar/syntax/serialize) is UNTOUCHED — this is a runtime-model reshape only (design spec §9: storage and runtime need not be 1:1). The two node-pool filters that special-cased `!Behavior(_)` are unified behind one honest predicate, `ElementType::is_view()`. The TypeScript renderers rebuild against the new pool/view shape in the same series.

**Tech Stack:** Rust (`waml` crate, `cargo`, serde, tsify-next/wasm-bindgen codegen), TypeScript monorepo (`packages/`, `pnpm`, `vitest`), `@dagrejs/dagre` + `@xyflow/svelte` flow renderer.

## Task index (execute in order)

This plan is segmented for implement-plan directory-plan mode. Each `task-N-*.md` is one committable green unit; execute them in ascending N. Every segment re-states the Global Constraints below so it can run standalone.

1. [Task 1 — Unify the node-pool filter behind `ElementType::is_view()`](task-1-unify-node-pool-filter.md)
2. [Task 2 — Split behavior model from view (Rust + wasm bindings)](task-2-split-behavior-model-wasm.md)
3. [Task 3 — Reshape `@waml/okf` types + core overlay plumbing](task-3-okf-types-core-overlay.md)
4. [Task 4 — Rebuild the flow renderer against the pool/view model (full gate)](task-4-rebuild-flow-renderer.md)

The full original monolithic plan is preserved verbatim as [`_source.md`](_source.md).

---

## Global Constraints

- **Storage format is frozen.** Do NOT touch `crates/waml/src/grammar.rs`, `crates/waml/src/syntax.rs`, or `crates/waml/src/serialize.rs`. The markdown ↔ AST round-trip (the `flow_document_serialize_is_a_semantic_fixpoint` test) must stay byte-identical. This slice reshapes only the RESOLVED runtime `Model` (design spec §9).
- **Do NOT touch the sequence/interaction substrate.** `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`, `FragmentKind`, `build_interactions`, and `Model.interactions` are a SEPARATE plan being written in parallel. Leave them exactly as they are.
- **`ops` is out of scope for flow.** `crates/waml/src/ops/mod.rs` has zero flow references (verified) — activity nodes/edges are derived-only, never mutated by `Op`s. Do not add flow ops.
- **`FlowEdgeKind` and `is_view()` matches MUST be exhaustive and explicit** — no `_ =>` catch-all where a metaclass/kind decision should be forced at compile time.
- **`is_view()` replaces both `!= Diagram && !Behavior(_)` filters** (in `parse.rs::build_model` and `validate.rs::link`). `is_view()` returns `true` for `Diagram` and every `Behavior(_)`, so the filter behavior is identical — this is a pure clarity refactor. Do NOT confuse `is_view()` with `is_classifier()` (which returns `true` for behaviors).
- **Wire naming:** new multi-word wire fields use camelCase via `#[serde(rename = ...)]` — `activityNodes`, `flowEdges`, `objectRef`, `toRef`, `controlFlow` / `objectFlow`. This matches the existing camelCase wire conventions (`objectRef`, `toRef` already exist on the old shapes).
- Idiomatic Rust: run `cargo fmt` on touched files before every commit; introduce no new `cargo clippy` warnings on the `waml` crate.
- Full CI gate (from `.github/workflows/ci.yml`), in order: `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`.
- **Cross-language atomicity note:** the runtime-model wire SHAPE changes, so Rust and TypeScript cannot both be green on the *full* gate at every intermediate commit. Each task runs the per-language gate that covers its own change (stated per task); the FULL gate is green only at the end of **Task 4**. This is expected for a feature branch and mirrors how a shape change lands.
- Do NOT edit files under `docs/` (historical specs/plans reference the old shapes — leave them).
- Frequent commits, one deliverable per task.

---

## File Structure

Modified files:

- `crates/waml/src/model.rs` — **Task 1**: add `ElementType::is_view()` + unit test. **Task 2**: add `ActivityNode` struct, `FlowEdgeKind` enum, reshape `FlowEdge` (add `key`/`kind`/`behavior`; `from`/`to` become pool keys), reshape `FlowDoc` (`nodes`/`edges` → `Vec<String>`), add `Model.activity_nodes` / `Model.flow_edges`.
- `crates/waml/src/parse.rs` — **Task 1**: unify the `build_model` classifier filter behind `is_view()`. **Task 2**: rewrite `build_flows` to return `(Vec<FlowDoc>, Vec<ActivityNode>, Vec<FlowEdge>)` and distribute into the new `Model` fields; rewrite the `builds_flow_doc_with_resolved_links_and_edges` test.
- `crates/waml/src/validate.rs` — **Task 1**: unify the `link` keyset filter behind `is_view()`.
- `packages/wasm/src/generated/waml_wasm.d.ts` — **Task 2**: REGENERATED by `pnpm build:wasm` (never hand-edited).
- `packages/wasm/src/index.ts` — **Task 2**: re-export list: `FlowNode` → `ActivityNode`, add `FlowEdgeKind`.
- `packages/okf/src/types.ts` — **Task 3**: re-export `ActivityNode`/`FlowEdgeKind` (drop `FlowNode`); add `activityNodes?`/`flowEdges?` to `ModelGraph`.
- `packages/core/src/state/overlay.ts` — **Task 3**: `toModelGraph` passes the two new pools through.
- `packages/core/src/state/overlay.test.ts` — **Task 3**: update the flow-passthrough fixture to the new view+pool shape.
- `packages/web/src/canvas/flowGraph.ts` — **Task 4**: `flowNodeSize`/`transitionLabel`/`KIND_TO_TYPE` retype to `ActivityNode`; add `resolveFlow(doc, graph)`; `flowToRf` takes a resolved view and keys nodes by pool key.
- `packages/web/src/canvas/flowGraph.test.ts` — **Task 4**: rebuild against the resolved-view shape; add a `resolveFlow` test.
- `packages/web/src/components/canvas/flow/FlowView.svelte` — **Task 4**: take a `graph` prop; resolve then render.
- `packages/web/src/components/canvas/flow/FlowView.test.ts` — **Task 4**: pass `graph`.
- `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, `FlowObjectNode.svelte` — **Task 4**: `FlowNode` type → `ActivityNode`.
- `packages/web/src/components/canvas/CanvasInner.svelte` — **Task 4**: pass `graph={$model}` to `FlowView`.

No files are created. No files are deleted.

---

## Notes / risks

- **Object-flow classification is derived, not authored.** `FlowEdgeKind` is computed at build time: `ObjectFlow` iff the edge `carries` a resolved type OR either endpoint is an `object` node; else `ControlFlow`. This matches design spec §3 (ObjectFlow carries a type; object nodes are its endpoints) without adding grammar. Cross-document targets can't be inspected for kind, so a cross-doc transition is `ObjectFlow` only when it `carries` or its source is an object node — acceptable, and the renderer drops cross-doc edges anyway.
- **Cross-document transitions are unchanged.** `to_ref` still resolves to the target *behavior document* key; the local `to` fallback (the link title) still fails the renderer's `local.has(e.to)` check and is not drawn — byte-for-byte the same visible behavior as today.
- **Pool keys use `#` deliberately.** Document slugs derive from file paths (slashes, no `#`), so `"{behavior}#{id}"` / `"{behavior}#e{n}"` can never collide with a classifier / package / diagram key. The sequence-slice plan must use the same `#` convention if it pools any interaction elements.
- **Storage is untouched.** No grammar/syntax/serialize change; the markdown fixpoint test proves it. The wire *runtime* model changes shape, which is explicitly allowed (design spec §9: storage and runtime need not be 1:1).
- **`is_view()` vs `is_classifier()`.** They are different predicates and both now exist on `ElementType`. `is_view()` (Diagram + behaviors) gates the classifier node pool; `is_classifier()` (structural + behavior classifiers) is the spec §3.1 predicate. Do not merge them.
</content>