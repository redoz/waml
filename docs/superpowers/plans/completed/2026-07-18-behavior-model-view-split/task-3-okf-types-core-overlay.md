# Behavior Model/View Split — Task 3: Reshape @waml/okf types + core overlay plumbing

> **Segment 3 of 4** of the **Behavior Model/View Split** plan. See [`README.md`](README.md) for the plan Goal, Architecture, Tech Stack, File Structure, and Notes/risks; full original monolithic plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

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
## Task 3: Reshape `@waml/okf` types + core overlay plumbing

Propagate the new pool/view shape through the type re-export package and the `Rust Model → ModelGraph` fusion so the two pools reach the frontend graph.

**Files:**
- Modify: `packages/okf/src/types.ts` — re-export list + `ModelGraph` interface.
- Modify: `packages/core/src/state/overlay.ts` — `toModelGraph`.
- Modify: `packages/core/src/state/overlay.test.ts` — flow-passthrough fixture.

**Interfaces:**
- Consumes: `ActivityNode`, `FlowEdge`, `FlowEdgeKind`, `FlowDoc` from `@waml/wasm` (Task 2).
- Produces: `ModelGraph.activityNodes?: ActivityNode[]`, `ModelGraph.flowEdges?: FlowEdge[]`; `toModelGraph` fills both from `model.activityNodes` / `model.flowEdges`.

Steps:

- [ ] **3.1 Update the `@waml/okf` re-exports.** In `packages/okf/src/types.ts`, in the `export type { ... } from "@waml/wasm";` block (~lines 7-30), replace the line `  FlowNode,` with `  ActivityNode,` and add `  FlowEdgeKind,` immediately after `  FlowEdge,`. Then in the `import type { ... } from "@waml/wasm";` block just below it (~lines 32-40), add `  ActivityNode,` and `  FlowEdge,` to the imported names (they are needed to type the new `ModelGraph` pool fields).

- [ ] **3.2 Add the two pools to `ModelGraph`.** In `packages/okf/src/types.ts`, in `export interface ModelGraph`, immediately after the `flows?: FlowDoc[];` field (and its doc comment, ~line 174) add:
  ```ts
    /** Model-level pool of behavior flow elements, referenced by `FlowDoc.nodes` (design spec §3/§4). */
    activityNodes?: ActivityNode[];
    /** Model-level pool of typed control/object flow edges, referenced by `FlowDoc.edges`. */
    flowEdges?: FlowEdge[];
  ```

- [ ] **3.3 Update the overlay flow-passthrough test (RED).** In `packages/core/src/state/overlay.test.ts`, replace the `"passes flow docs through to the ModelGraph"` test body (~lines 180-192) with the new view+pool shape:
  ```ts
    it("passes flow views and their pools through to the ModelGraph", () => {
      const flow: FlowDoc = {
        key: "m/lifecycle",
        title: "Order Lifecycle",
        flavor: "stateMachine",
        describes: "m/order",
        nodes: ["m/lifecycle#initial", "m/lifecycle#Draft"],
        edges: ["m/lifecycle#e0"],
      };
      const activityNodes = [
        { key: "m/lifecycle#initial", id: "initial", behavior: "m/lifecycle", kind: "initial" },
        { key: "m/lifecycle#Draft", id: "Draft", behavior: "m/lifecycle", kind: "plain", entry: "reserveStock" },
      ];
      const flowEdges = [
        { key: "m/lifecycle#e0", kind: "controlFlow", behavior: "m/lifecycle", from: "m/lifecycle#initial", to: "m/lifecycle#Draft" },
      ];
      const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], flows: [flow], activityNodes, flowEdges };
      const g = toModelGraph(rust as never, emptyOverlay());
      expect(g.flows).toEqual([flow]);
      expect(g.activityNodes).toEqual(activityNodes);
      expect(g.flowEdges).toEqual(flowEdges);
    });
  ```

- [ ] **3.4 Run it, verify it fails.** Run:
  ```
  pnpm --filter @waml/core test -- overlay
  ```
  Expected: FAIL — `g.activityNodes` / `g.flowEdges` are `undefined` (`toModelGraph` does not yet forward them).

- [ ] **3.5 Forward the pools in `toModelGraph`.** In `packages/core/src/state/overlay.ts`, in the object returned by `toModelGraph` (~lines 136-144), immediately after `flows: model.flows ?? [],` add:
  ```ts
      activityNodes: model.activityNodes ?? [],
      flowEdges: model.flowEdges ?? [],
  ```
  (`RustModel` is the generated `Model` type from `@waml/wasm`, which now carries `activityNodes?` / `flowEdges?` — no manual `RustModel` edit is needed.)

- [ ] **3.6 Run the core tests, verify green.** Run:
  ```
  pnpm --filter @waml/okf build
  pnpm --filter @waml/core test
  ```
  Expected: `@waml/okf` builds (types resolve); the overlay test passes; the `nav/tree.test.ts` and `state/diagrams.test.ts` fixtures — which build `FlowDoc`s with empty `nodes: []` / `edges: []` — still typecheck (empty arrays satisfy `string[]`) and pass.

- [ ] **3.7 Commit.** Run:
  ```
  git add packages/okf/src/types.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
  git commit -m "feat(core): thread activity-node + flow-edge pools through ModelGraph"
  ```
