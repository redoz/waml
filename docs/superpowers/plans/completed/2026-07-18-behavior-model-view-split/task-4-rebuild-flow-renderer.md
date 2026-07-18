# Behavior Model/View Split — Task 4: Rebuild the flow renderer against the pool/view model (full gate)

> **Segment 4 of 4** of the **Behavior Model/View Split** plan. See [`README.md`](README.md) for the plan Goal, Architecture, Tech Stack, File Structure, and Notes/risks; full original monolithic plan preserved verbatim as [`_source.md`](_source.md).
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
## Task 4: Rebuild the flow renderer against the pool/view model (full gate)

Reshape the web renderer to resolve a `FlowDoc` view against the model pools, retype the flow node components, and bring the whole gate green.

**Files:**
- Modify: `packages/web/src/canvas/flowGraph.ts` — retype to `ActivityNode`; add `resolveFlow`; key layout by pool key.
- Modify: `packages/web/src/canvas/flowGraph.test.ts` — resolved-view fixtures + `resolveFlow` test.
- Modify: `packages/web/src/components/canvas/flow/FlowView.svelte` — `graph` prop + resolve.
- Modify: `packages/web/src/components/canvas/flow/FlowView.test.ts` — pass `graph`.
- Modify: `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, `FlowObjectNode.svelte` — `FlowNode` type → `ActivityNode`.
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` — pass `graph={$model}`.

**Interfaces:**
- Consumes: `ActivityNode`, `FlowEdge`, `FlowFlavor`, `FlowDoc`, `ModelGraph` from `@waml/okf`.
- Produces: `resolveFlow(doc: FlowDoc, graph: ModelGraph): { flavor: FlowFlavor; nodes: ActivityNode[]; edges: FlowEdge[] }`; `flowToRf(view: { flavor: FlowFlavor; nodes: ActivityNode[]; edges: FlowEdge[] }): { nodes: Node[]; edges: Edge[] }`.

Steps:

- [ ] **4.1 Rewrite `flowGraph.test.ts` against the resolved-view shape (RED).** Replace the whole file `packages/web/src/canvas/flowGraph.test.ts` with:
  ```ts
  import { describe, expect, it } from "vitest";
  import type { ActivityNode, FlowDoc, FlowEdge, FlowFlavor, ModelGraph } from "@waml/okf";
  import { flowToRf, resolveFlow, transitionLabel } from "./flowGraph";

  const B = "m/lifecycle";
  const k = (id: string) => `${B}#${id}`;
  const nodes: ActivityNode[] = [
    { key: k("initial"), id: "initial", behavior: B, kind: "initial" },
    { key: k("Draft"), id: "Draft", behavior: B, kind: "plain" },
    { key: k("Ready to ship?"), id: "Ready to ship?", behavior: B, kind: "decision" },
    { key: k("final"), id: "final", behavior: B, kind: "final" },
  ];
  const edges: FlowEdge[] = [
    { key: k("e0"), kind: "controlFlow", behavior: B, from: k("initial"), to: k("Draft") },
    { key: k("e1"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Ready to ship?"), trigger: "place", guard: "items > 0", effect: "reserve" },
    { key: k("e2"), kind: "controlFlow", behavior: B, from: k("Ready to ship?"), to: k("final"), else: true },
    { key: k("e3"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Missing") }, // unresolved target: not drawn, never errors
  ];
  const view = { flavor: "stateMachine" as FlowFlavor, nodes, edges };

  describe("transitionLabel", () => {
    it("renders UML 'trigger [guard] / effect' labels", () => {
      expect(transitionLabel(edges[1])).toBe("place [items > 0] / reserve");
      expect(transitionLabel(edges[2])).toBe("[else]");
      expect(transitionLabel(edges[0])).toBe("");
    });
  });

  describe("resolveFlow", () => {
    it("dereferences a view's node/edge keys against the model pools", () => {
      const graph = { activityNodes: nodes, flowEdges: edges } as unknown as ModelGraph;
      const doc: FlowDoc = { key: B, title: "T", flavor: "stateMachine", nodes: nodes.map((n) => n.key), edges: edges.map((e) => e.key) };
      const r = resolveFlow(doc, graph);
      expect(r.flavor).toBe("stateMachine");
      expect(r.nodes.map((n) => n.id)).toEqual(["initial", "Draft", "Ready to ship?", "final"]);
      expect(r.edges).toHaveLength(4);
    });
  });

  describe("flowToRf", () => {
    it("lays out every node and maps kinds to component types", () => {
      const { nodes: rf, edges: rfEdges } = flowToRf(view);
      expect(rf).toHaveLength(4);
      expect(rf.map((n) => n.type)).toEqual(["flowControl", "flowStep", "flowControl", "flowControl"]);
      // React node ids are pool keys; dagre TB puts initial above final.
      const y = (key: string) => rf.find((n) => n.id === key)!.position.y;
      expect(y(k("initial"))).toBeLessThan(y(k("final")));
      // the edge to a missing node is dropped, the rest are transitions
      expect(rfEdges).toHaveLength(3);
      expect(rfEdges.every((e) => e.type === "transition")).toBe(true);
    });

    it("carries the flavor and the source node's kind on each edge", () => {
      const { edges: rfEdges } = flowToRf(view);
      const data = (i: number) => rfEdges[i].data as { flavor: string; fromKind: string };
      expect(rfEdges.every((e) => (e.data as { flavor: string }).flavor === "stateMachine")).toBe(true);
      expect(data(0).fromKind).toBe("initial");
      expect(data(2).fromKind).toBe("decision");
    });
  });
  ```

- [ ] **4.2 Run it, verify it fails.** Run:
  ```
  pnpm --filter web test -- flowGraph
  ```
  Expected: FAIL — `resolveFlow` is not exported and `flowToRf` still expects a `FlowDoc` with inline `nodes`/`edges`.

- [ ] **4.3 Rewrite `flowGraph.ts`.** Replace the whole file `packages/web/src/canvas/flowGraph.ts` with:
  ```ts
  import dagre from "@dagrejs/dagre";
  import type { Edge, Node } from "@xyflow/svelte";
  import type { ActivityNode, FlowDoc, FlowEdge, FlowFlavor, ModelGraph } from "@waml/okf";

  // ── Flow substrate rendering (behavior model/view split) ─────────────────────
  // A behavior document is a VIEW: it references pooled activity nodes / flow
  // edges by key. `resolveFlow` dereferences those against the model pools; the
  // resolved graph is laid out at render time by dagre (relational, never stored).

  export function flowNodeSize(n: ActivityNode): { width: number; height: number } {
    switch (n.kind) {
      case "initial":
      case "final":
        return { width: 36, height: 36 };
      case "decision":
      case "merge":
        return { width: 56, height: 56 };
      case "fork":
      case "join":
        return { width: 120, height: 10 };
      case "object":
        return { width: 160, height: 48 };
      default: {
        const internals = [n.entry, n.do, n.exit].filter(Boolean).length;
        return { width: 180, height: 48 + internals * 18 + (n.refines ? 18 : 0) };
      }
    }
  }

  /** UML edge label: `trigger [guard] / effect`; a decision default is `[else]`. */
  export function transitionLabel(e: FlowEdge): string {
    const head = [e.trigger, e.guard ? `[${e.guard}]` : e.else ? "[else]" : undefined]
      .filter(Boolean)
      .join(" ");
    const eff = e.effect ? `/ ${e.effect}` : "";
    return [head, eff].filter(Boolean).join(" ").trim();
  }

  const KIND_TO_TYPE: Record<ActivityNode["kind"], string> = {
    plain: "flowStep",
    object: "flowObject",
    initial: "flowControl",
    final: "flowControl",
    decision: "flowControl",
    merge: "flowControl",
    fork: "flowControl",
    join: "flowControl",
  };

  export interface ResolvedFlow {
    flavor: FlowFlavor;
    nodes: ActivityNode[];
    edges: FlowEdge[];
  }

  /** Dereference a behavior VIEW's node/edge keys against the model-level pools. */
  export function resolveFlow(doc: FlowDoc, graph: ModelGraph): ResolvedFlow {
    const nodeByKey = new Map((graph.activityNodes ?? []).map((n) => [n.key, n]));
    const edgeByKey = new Map((graph.flowEdges ?? []).map((e) => [e.key, e]));
    const nodes = doc.nodes.map((key) => nodeByKey.get(key)).filter((n): n is ActivityNode => n != null);
    const edges = doc.edges.map((key) => edgeByKey.get(key)).filter((e): e is FlowEdge => e != null);
    return { flavor: doc.flavor, nodes, edges };
  }

  export function flowToRf(view: ResolvedFlow): { nodes: Node[]; edges: Edge[] } {
    const g = new dagre.graphlib.Graph();
    g.setDefaultEdgeLabel(() => ({}));
    g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
    for (const n of view.nodes) {
      const s = flowNodeSize(n);
      g.setNode(n.key, { width: s.width, height: s.height });
    }
    const local = new Set(view.nodes.map((n) => n.key));
    for (const e of view.edges) if (local.has(e.from) && local.has(e.to)) g.setEdge(e.from, e.to);
    dagre.layout(g);

    const nodes: Node[] = view.nodes.map((n) => {
      const s = flowNodeSize(n);
      const pos = g.node(n.key);
      return {
        id: n.key,
        type: KIND_TO_TYPE[n.kind],
        position: { x: (pos?.x ?? 0) - s.width / 2, y: (pos?.y ?? 0) - s.height / 2 },
        data: { node: n, flavor: view.flavor } as unknown as Record<string, unknown>,
        draggable: false,
        connectable: false,
        selectable: false,
      };
    });
    const kindByKey = new Map(view.nodes.map((n) => [n.key, n.kind]));
    const edges: Edge[] = view.edges
      .filter((e) => local.has(e.from) && local.has(e.to))
      .map((e) => ({
        id: e.key,
        source: e.from,
        target: e.to,
        type: "transition",
        // flavor picks the path shape; fromKind lets a decision source snap to a tip.
        data: { label: transitionLabel(e), carries: e.carries, flavor: view.flavor, fromKind: kindByKey.get(e.from) } as unknown as Record<string, unknown>,
        selectable: false,
      }));
    return { nodes, edges };
  }
  ```

- [ ] **4.4 Run the renderer test, verify green.** Run:
  ```
  pnpm --filter web test -- flowGraph
  ```
  Expected: `transitionLabel`, `resolveFlow`, and `flowToRf` suites all pass.

- [ ] **4.5 Retype the three flow node components.** In each of `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, and `FlowObjectNode.svelte`, change the import line
  ```ts
    import type { FlowFlavor, FlowNode } from "@waml/okf";
  ```
  to
  ```ts
    import type { ActivityNode, FlowFlavor } from "@waml/okf";
  ```
  and change the props line
  ```ts
    let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  ```
  to
  ```ts
    let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  ```
  (The component bodies read `n.id` / `n.entry` / `n.do` / `n.exit` / `n.refines` / `n.partition`, all of which exist on `ActivityNode` — no body changes.)

- [ ] **4.6 Update `FlowView.svelte` to resolve the view.** Replace the whole `<script>` block of `packages/web/src/components/canvas/flow/FlowView.svelte` with:
  ```svelte
  <script lang="ts">
    import { SvelteFlow, SvelteFlowProvider, Background, BackgroundVariant, Controls, type Edge, type Node } from "@xyflow/svelte";
    import type { FlowDoc, ModelGraph } from "@waml/okf";
    import { flowToRf, resolveFlow } from "../../../canvas/flowGraph";
    import FlowStepNode from "./FlowStepNode.svelte";
    import FlowControlNode from "./FlowControlNode.svelte";
    import FlowObjectNode from "./FlowObjectNode.svelte";
    import TransitionEdge from "./TransitionEdge.svelte";

    let { doc, graph }: { doc: FlowDoc; graph: ModelGraph } = $props();

    const nodeTypes = { flowStep: FlowStepNode, flowControl: FlowControlNode, flowObject: FlowObjectNode };
    const edgeTypes = { transition: TransitionEdge };

    let nodes = $state<Node[]>([]);
    let edges = $state<Edge[]>([]);
    $effect(() => {
      const rf = flowToRf(resolveFlow(doc, graph));
      nodes = rf.nodes;
      edges = rf.edges;
    });
  </script>
  ```
  (Leave the markup below `</script>` unchanged.)

- [ ] **4.7 Pass the graph from `CanvasInner.svelte`.** In `packages/web/src/components/canvas/CanvasInner.svelte`, change the `FlowView` usage (~line 729) from:
  ```svelte
          <FlowView doc={activeFlow} />
  ```
  to:
  ```svelte
          <FlowView doc={activeFlow} graph={$model} />
  ```

- [ ] **4.8 Update the `FlowView.test.ts` fixture to the view+pool shape.** Replace the whole file `packages/web/src/components/canvas/flow/FlowView.test.ts` with:
  ```ts
  import { describe, expect, it } from "vitest";
  import { render } from "@testing-library/svelte";
  import type { FlowDoc, ModelGraph } from "@waml/okf";
  import FlowView from "./FlowView.svelte";

  const B = "m/lifecycle";
  const DOC: FlowDoc = {
    key: B,
    title: "Order Lifecycle",
    flavor: "stateMachine",
    nodes: [`${B}#initial`, `${B}#Placed`, `${B}#final`],
    edges: [`${B}#e0`, `${B}#e1`],
  };
  const GRAPH = {
    activityNodes: [
      { key: `${B}#initial`, id: "initial", behavior: B, kind: "initial" },
      { key: `${B}#Placed`, id: "Placed", behavior: B, kind: "plain", entry: "reserveStock" },
      { key: `${B}#final`, id: "final", behavior: B, kind: "final" },
    ],
    flowEdges: [
      { key: `${B}#e0`, kind: "controlFlow", behavior: B, from: `${B}#initial`, to: `${B}#Placed` },
      { key: `${B}#e1`, kind: "controlFlow", behavior: B, from: `${B}#Placed`, to: `${B}#final`, trigger: "deliver" },
    ],
  } as unknown as ModelGraph;

  describe("FlowView", () => {
    it("renders every flow node with its internals", () => {
      const { getByText } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
      expect(getByText("Placed")).toBeTruthy();
      expect(getByText("entry / reserveStock")).toBeTruthy();
    });

    it("gives every node a source and target handle so edges survive", () => {
      // SvelteFlow drops an edge unless the source node has a source handle and
      // the target node a target handle. jsdom never lays the graph out, so we
      // assert the invariant that was missing and caused every flow edge to vanish.
      const { container } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
      expect(container.querySelectorAll(".svelte-flow__handle.source").length).toBe(DOC.nodes.length);
      expect(container.querySelectorAll(".svelte-flow__handle.target").length).toBe(DOC.nodes.length);
    });
  });
  ```

- [ ] **4.9 Run the full CI gate, verify green.** Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green end-to-end — Rust pools resolve, the wasm bindings match, `@waml/okf` and web typecheck against `ActivityNode` / `FlowEdgeKind`, and every flow test passes on the new shape.

- [ ] **4.10 Commit.** Run:
  ```
  git add packages/web/src/canvas/flowGraph.ts packages/web/src/canvas/flowGraph.test.ts packages/web/src/components/canvas/flow/FlowView.svelte packages/web/src/components/canvas/flow/FlowView.test.ts packages/web/src/components/canvas/flow/FlowStepNode.svelte packages/web/src/components/canvas/flow/FlowControlNode.svelte packages/web/src/components/canvas/flow/FlowObjectNode.svelte packages/web/src/components/canvas/CanvasInner.svelte
  git commit -m "feat(web): render behavior views by resolving activity-node + flow-edge pools"
  ```
