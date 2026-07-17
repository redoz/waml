# Prose-solved Diagram Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render the `## Layout` solver's output on the real Diagram canvas — nodes at solved positions, titled `frame` hulls, `collapsed` chips, and inline solver diagnostics — as a drop-in for the imperative dagre pass, with drag left as a free override.

**Architecture:** A new pure `runSolveLayout` bridge in `layout.ts` calls the already-built `@waml/wasm` `solve()` and reshapes its result. `CanvasInner.svelte` gains a `layoutActiveView()` branch that solves for real Diagrams (backed by a doc) and falls back to `runDagreLayout` for the implicit "All"/behavior views; it runs on the exact imperative triggers dagre uses today plus one new one (Diagram-view activation, wired as an `untrack`ed effect keyed on the primitive `activeDiagramKey` so it can never re-enter itself). Solved positions flow through the existing store overlay; a `solveResult` `$state` drives group-hull pseudo-nodes, collapse flags, and a diagnostics banner. No Rust/wasm changes.

**Tech Stack:** TypeScript, Svelte 5 (runes: `$state`/`$derived`/`$effect`/`untrack`), `@xyflow/svelte` (SvelteFlow), Vitest + `@testing-library/svelte`, the `@waml/wasm` `solve()` bridge, `@waml/core` store + `erdAwareNodeSize`, `@waml/okf` types.

## Global Constraints

Copy these verbatim into every task's mental model — they are the spec's non-negotiable decisions and must not be re-litigated:

- **Solve is a drop-in for the IMPERATIVE dagre pass — never a reactive `$effect`.** It runs on the same explicit triggers dagre runs on today, plus Diagram-view activation. A reactive solve risks a `solve → updateNode → $model change → solve` loop.
- **Drag is a free override.** `onNodeDragStop` (`store.updateNode({ position })`) stays **unchanged**. A dragged node keeps its dropped position in the overlay until the next solve trigger overwrites it. Nothing becomes read-only.
- **Positions flow through the store overlay** via the existing `store.updateNode(key, { position })` path. The OKF format persists no coordinates; the overlay is ephemeral, recomputed per session.
- **Only `shape === "Frame"` draws chrome.** `Box` and `Shrink` shape the layout but render no node. NB: the generated `Shape` type is capitalized (`"Frame" | "Box" | "Shrink"`) — not the lowercase `frame` in the spec prose.
- **No Rust or wasm changes.** The `solve()` bridge is already built and parity-tested (`packages/wasm/src/solve.test.ts`, `crates/waml/tests/solver_golden.rs`).
- **`emphasize` is deferred to Spec 2.** No emphasis styling exists in `OkfNode`/`ClassifierBox` today, so the `emphasized` flag is read-through-but-unrendered here (recorded, not a blocker).
- **The prose round-trip (drag → written relation) is Spec 2 and OUT OF SCOPE.** So are: a prose-editing panel, the `diagram.layout` write op, measured node sizes, group dragging, per-view positions.
- **Per-task verification gate (repo convention):** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. This change is frontend-only (no crate touched), so lean on the `pnpm` side; run `cargo test --workspace` once at the end to confirm nothing regressed, but the per-task signal comes from `pnpm test && pnpm lint && pnpm build`.

### Key type shapes (from `@waml/wasm`, confirmed against `packages/wasm/src/generated/waml_wasm.d.ts`)

```ts
interface SolveResult { solved: Solved; diagnostics: Diagnostic[]; }
interface Solved { nodes: Record<string, Rect>; groups: SolvedGroup[]; flags: Record<string, FlagSet>; }
interface Rect { x: number; y: number; w: number; h: number; }   // x,y is the TOP-LEFT (no dagre-style centering fix-up)
interface SolvedGroup { rect: Rect; shape: Shape; title: string | undefined; depth: number; }
type Shape = "Frame" | "Box" | "Shrink";
interface FlagSet { emphasized: boolean; collapsed: boolean; }
interface Diagnostic { severity: "error" | "warning"; code: DiagCode; message: string; file: string; line: number; span: [number, number] | undefined; }
// solve signature (already exported from packages/wasm/src/index.ts):
function solve(bundle: [string, string][], diagramKey: string, sizes: Record<string, { w: number; h: number }>, cfg?: SolveConfig): SolveResult;
```

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `packages/web/src/canvas/layout.ts` | Layout passes | **Modify** — add `runSolveLayout` + exported `SolveLayout` type; `runDagreLayout` untouched |
| `packages/web/src/canvas/layout.test.ts` | layout unit tests | **Modify** — add `runSolveLayout` tests |
| `packages/web/src/components/canvas/nodes/GroupFrame.svelte` | Titled frame hull renderer | **Create** |
| `packages/web/src/components/canvas/nodes/GroupFrame.test.ts` | GroupFrame render test | **Create** |
| `packages/web/src/components/canvas/flowTypes.ts` | SvelteFlow node-type registry | **Modify** — register `"group-frame"` |
| `packages/web/src/components/canvas/flowTypes.test.ts` | registry test | **Create** |
| `packages/web/src/components/canvas/toRFNode.ts` | Model→RF node mappers | **Modify** — add pure `toGroupNode`; `toRFNode` untouched (already accepts `collapsed`) |
| `packages/web/src/components/canvas/toRFNode.test.ts` | mapper unit tests | **Modify** — add `toGroupNode` tests |
| `packages/web/src/components/canvas/CanvasInner.svelte` | Canvas orchestrator | **Modify** — `layoutActiveView()` branch, `solveResult` `$state`, reroute triggers, activation effect, group append + collapse-from-flags, diagnostics banner |
| `packages/web/src/components/canvas/Canvas.solve.test.ts` | integration tests | **Create** — solved positions, dagre fallback, diagnostics banner, drag free-override |

---

## Task 1: `runSolveLayout` bridge in `layout.ts`

**Files:**
- Modify: `packages/web/src/canvas/layout.ts`
- Test: `packages/web/src/canvas/layout.test.ts`

**Interfaces:**
- Consumes: `solve`, and types `SolvedGroup`, `FlagSet`, `Diagnostic` from `@waml/wasm`.
- Produces:
  ```ts
  export interface SolveLayout {
    positions: Map<string, { x: number; y: number }>;
    groups: SolvedGroup[];
    flags: Record<string, FlagSet>;
    diagnostics: Diagnostic[];
  }
  export function runSolveLayout(
    bundle: [string, string][],
    diagramKey: string,
    sizes: Record<string, { w: number; h: number }>,
  ): SolveLayout;
  ```

- [ ] **Step 1: Write the failing tests**

Append to `packages/web/src/canvas/layout.test.ts` (the existing `beforeAll(initWasm)` is reused):

```ts
import { runSolveLayout } from "./layout";

// Mirrors packages/wasm/src/solve.test.ts: a 3-class shop diagram whose
// `## Layout` prose frames the "Users" group and places it left of "Orders".
const solveBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];
const solveSizes = {
  "shop/customer": { w: 200, h: 90 },
  "shop/account": { w: 200, h: 90 },
  "shop/order": { w: 200, h: 90 },
};

test("runSolveLayout returns top-left positions, a Frame group, and no diagnostics", () => {
  const r = runSolveLayout(solveBundle, "shop/orders", solveSizes);
  expect(r.diagnostics).toEqual([]);
  // Rect.x/y is already the top-left — no centering fix-up like dagre.
  expect(r.positions.get("shop/customer")).toEqual({ x: 16, y: 16 });
  expect(r.positions.get("shop/account")).toEqual({ x: 16, y: 122 });
  expect(r.positions.get("shop/order")).toEqual({ x: 264, y: 69 });
  expect(r.groups.some((g) => g.shape === "Frame" && g.title === "Users")).toBe(true);
});

test("runSolveLayout maps a `collapsed` flag onto the node's key", () => {
  // `collapsed` is a real layout-prose flag (crates/waml/src/layout.rs L323, L673).
  const collapsing = solveBundle.map(
    ([p, t]) => (p === "shop/orders.md" ? [p, t + "- Order with collapsed\n"] : [p, t]) as [string, string],
  );
  const r = runSolveLayout(collapsing, "shop/orders", solveSizes);
  expect(Object.values(r.flags).some((f) => f.collapsed)).toBe(true);
});

test("runSolveLayout surfaces an unresolved-layout-ref diagnostic", () => {
  const bad = solveBundle.map(
    ([p, t]) => (p === "shop/orders.md" ? [p, t + "- Ghosts left of Orders\n"] : [p, t]) as [string, string],
  );
  const r = runSolveLayout(bad, "shop/orders", solveSizes);
  expect(r.diagnostics.some((d) => d.code === "unresolved-layout-ref")).toBe(true);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web run test src/canvas/layout.test.ts`
Expected: FAIL — `runSolveLayout` is not exported (`No known export 'runSolveLayout'` / type error).

- [ ] **Step 3: Implement `runSolveLayout`**

Edit `packages/web/src/canvas/layout.ts`. Add the import near the top (after the existing `@waml/core` import):

```ts
import { solve, type SolvedGroup, type FlagSet, type Diagnostic } from "@waml/wasm";
```

Append at the end of the file:

```ts
// ── Prose solver layout ──────────────────────────────────────────────────────
// A drop-in for runDagreLayout on REAL Diagram views (a diagram doc with a
// `## Layout` section). It calls the already-built @waml/wasm `solve()` bridge
// and reshapes SolveResult for the canvas: `positions` is `solved.nodes` reduced
// to each Rect's top-left {x,y} (a Rect's x,y is already the top-left, matching
// how the canvas positions nodes — no centering fix-up like dagre needs).
export interface SolveLayout {
  positions: Map<string, { x: number; y: number }>;
  groups: SolvedGroup[];
  flags: Record<string, FlagSet>;
  diagnostics: Diagnostic[];
}

export function runSolveLayout(
  bundle: [string, string][],
  diagramKey: string,
  sizes: Record<string, { w: number; h: number }>,
): SolveLayout {
  const { solved, diagnostics } = solve(bundle, diagramKey, sizes);
  const positions = new Map<string, { x: number; y: number }>();
  for (const [key, rect] of Object.entries(solved.nodes)) {
    positions.set(key, { x: rect.x, y: rect.y });
  }
  return { positions, groups: solved.groups, flags: solved.flags, diagnostics };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web run test src/canvas/layout.test.ts`
Expected: PASS (all `runSolveLayout` tests + the pre-existing dagre tests green).

> Note: if the `collapsed`-flag test fails on the exact prose, the `solve()` error text or an empty `flags` will say so — adjust the prose to the parser's accepted form (`- Order with collapsed`, per `crates/waml/src/layout.rs` L530/L673) and re-run. Do not change the assertion's intent.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/canvas/layout.ts packages/web/src/canvas/layout.test.ts
git commit -m "feat(web): add runSolveLayout bridge over the wasm solver"
```

---

## Task 2: `GroupFrame.svelte` node + registry

**Files:**
- Create: `packages/web/src/components/canvas/nodes/GroupFrame.svelte`
- Create: `packages/web/src/components/canvas/nodes/GroupFrame.test.ts`
- Modify: `packages/web/src/components/canvas/flowTypes.ts`
- Create: `packages/web/src/components/canvas/flowTypes.test.ts`

**Interfaces:**
- Consumes: SvelteFlow `NodeProps` (`data` carries `{ title?: string; width: number; height: number }`).
- Produces: default-exported `GroupFrame` component; `nodeTypes["group-frame"] === GroupFrame`.

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/components/canvas/nodes/GroupFrame.test.ts`:

```ts
import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import GroupFrame from "./GroupFrame.svelte";

test("renders a titled, sized frame hull", () => {
  const { container, getByText } = render(GroupFrame, {
    props: { data: { title: "Users", width: 232, height: 212 } },
  });
  expect(getByText("Users")).toBeTruthy();
  const root = container.querySelector("[data-group-frame]") as HTMLElement;
  expect(root).toBeTruthy();
  expect(root.getAttribute("style") ?? "").toContain("width: 232px");
  expect(root.getAttribute("style") ?? "").toContain("height: 212px");
});

test("renders no title when the group is untitled", () => {
  const { container } = render(GroupFrame, {
    props: { data: { title: undefined, width: 100, height: 100 } },
  });
  expect(container.querySelector("[data-group-frame-title]")).toBeNull();
});
```

Create `packages/web/src/components/canvas/flowTypes.test.ts`:

```ts
import { test, expect } from "vitest";
import { nodeTypes } from "./flowTypes";
import GroupFrame from "./nodes/GroupFrame.svelte";
import OkfNode from "./nodes/OkfNode.svelte";

test("registers the okf and group-frame node types", () => {
  expect(nodeTypes.okf).toBe(OkfNode);
  expect(nodeTypes["group-frame"]).toBe(GroupFrame);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web run test src/components/canvas/nodes/GroupFrame.test.ts src/components/canvas/flowTypes.test.ts`
Expected: FAIL — `GroupFrame.svelte` does not exist; `nodeTypes["group-frame"]` is undefined.

- [ ] **Step 3: Create `GroupFrame.svelte`**

Create `packages/web/src/components/canvas/nodes/GroupFrame.svelte`:

```svelte
<script lang="ts">
  import type { NodeProps } from "@xyflow/svelte";

  // A group hull for a `with frame` layout group: a titled, dashed bordered box
  // sized to the solver's rect. Only `shape === "Frame"` groups reach this
  // renderer (Box/Shrink shape the layout but draw nothing). It is a
  // non-interactive backdrop — selectable/draggable/deletable are set false on
  // the pseudo-node (see toGroupNode), so pointer events pass through.
  let { data }: NodeProps = $props();
  let group = $derived(data as unknown as { title?: string; width: number; height: number });
</script>

<div
  data-group-frame
  class="pointer-events-none relative h-full w-full rounded-lg border-2 border-dashed border-slate-300 bg-slate-50/40"
  style={`width:${group.width}px;height:${group.height}px;`}
>
  {#if group.title}
    <div
      data-group-frame-title
      class="absolute -top-[10px] left-3 bg-[#f7f8fa] px-2 text-[12px] font-semibold text-slate-500"
    >
      {group.title}
    </div>
  {/if}
</div>
```

- [ ] **Step 4: Register the node type**

Edit `packages/web/src/components/canvas/flowTypes.ts` to:

```ts
import type { NodeTypes, EdgeTypes } from "@xyflow/svelte";
import OkfNode from "./nodes/OkfNode.svelte";
import GroupFrame from "./nodes/GroupFrame.svelte";
import RelEdge from "./RelEdge.svelte";
import AnchorEdge from "./AnchorEdge.svelte";

export const nodeTypes: NodeTypes = { okf: OkfNode, "group-frame": GroupFrame };
export const edgeTypes: EdgeTypes = { rel: RelEdge, anchor: AnchorEdge };
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web run test src/components/canvas/nodes/GroupFrame.test.ts src/components/canvas/flowTypes.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/nodes/GroupFrame.svelte packages/web/src/components/canvas/nodes/GroupFrame.test.ts packages/web/src/components/canvas/flowTypes.ts packages/web/src/components/canvas/flowTypes.test.ts
git commit -m "feat(web): add GroupFrame node type for solved frame hulls"
```

---

## Task 3: pure `toGroupNode` mapper

**Files:**
- Modify: `packages/web/src/components/canvas/toRFNode.ts`
- Test: `packages/web/src/components/canvas/toRFNode.test.ts`

**Interfaces:**
- Consumes: `SolvedGroup` from `@waml/wasm`.
- Produces:
  ```ts
  export function toGroupNode(group: SolvedGroup, index: number): Node | null;
  ```
  Returns a `"group-frame"` SvelteFlow node for `shape === "Frame"`, else `null`. Pseudo-node id is `"__group__" + index` (never collides with a model node key, so selection/drag/delete handlers ignore it unchanged). `zIndex` is `depth - 1000` so hulls sit below members (default `zIndex` 0) and deeper/inner groups sit above shallower/outer ones.

- [ ] **Step 1: Write the failing tests**

Append to `packages/web/src/components/canvas/toRFNode.test.ts`:

```ts
import { toGroupNode } from "./toRFNode";
import type { SolvedGroup } from "@waml/wasm";

const frame: SolvedGroup = { rect: { x: 8, y: 8, w: 232, h: 212 }, shape: "Frame", title: "Users", depth: 1 };

test("toGroupNode maps a Frame group to a non-interactive group-frame pseudo-node", () => {
  const n = toGroupNode(frame, 0)!;
  expect(n).toMatchObject({
    id: "__group__0",
    type: "group-frame",
    position: { x: 8, y: 8 },
    selectable: false,
    draggable: false,
    deletable: false,
  });
  expect(n.data).toMatchObject({ title: "Users", width: 232, height: 212 });
  expect(n.zIndex).toBe(1 - 1000); // depth 1 → below members (0), above outer groups
});

test("toGroupNode returns null for Box and Shrink groups (they draw nothing)", () => {
  expect(toGroupNode({ ...frame, shape: "Box" }, 1)).toBeNull();
  expect(toGroupNode({ ...frame, shape: "Shrink" }, 2)).toBeNull();
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web run test src/components/canvas/toRFNode.test.ts`
Expected: FAIL — `No known export 'toGroupNode'`.

- [ ] **Step 3: Implement `toGroupNode`**

Edit `packages/web/src/components/canvas/toRFNode.ts`. Add to the imports:

```ts
import type { SolvedGroup } from "@waml/wasm";
```

Append at the end of the file:

```ts
// A solved layout group → a non-interactive SvelteFlow backdrop node. Only
// `shape === "Frame"` renders chrome; `Box`/`Shrink` shaped the layout but draw
// nothing, so they map to null. The `"__group__" + index` id never collides with
// a model node key, so the canvas's selection/drag/delete handlers ignore these
// pseudo-nodes without any special-casing.
export function toGroupNode(group: SolvedGroup, index: number): Node | null {
  if (group.shape !== "Frame") return null;
  return {
    id: "__group__" + index,
    type: "group-frame",
    position: { x: group.rect.x, y: group.rect.y },
    data: { title: group.title, width: group.rect.w, height: group.rect.h } as unknown as Record<string, unknown>,
    width: group.rect.w,
    height: group.rect.h,
    style: `width:${group.rect.w}px;height:${group.rect.h}px;`,
    selectable: false,
    draggable: false,
    deletable: false,
    zIndex: group.depth - 1000,
  } as Node;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web run test src/components/canvas/toRFNode.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/toRFNode.ts packages/web/src/components/canvas/toRFNode.test.ts
git commit -m "feat(web): add toGroupNode mapper for solved frame hulls"
```

---

## Task 4: `layoutActiveView` branch + solveResult wiring in `CanvasInner`

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Create: `packages/web/src/components/canvas/Canvas.solve.test.ts`

**Interfaces:**
- Consumes: `runSolveLayout`, `SolveLayout` (Task 1); `toGroupNode` (Task 3); `erdAwareNodeSize` (`@waml/core/canvas/layoutSize`); existing `store`, `runDagreLayout`, `activeDiagram`, `activeDisplay`, `activeDiagramKey`.
- Produces: `layoutActiveView()` (solve-or-dagre pass) replacing `layoutAll` at every trigger; `solveResult: SolveLayout | null` `$state`; a Diagram-activation `$effect`; `rfNodes` that appends group-frame pseudo-nodes and reads collapse from solver flags.

- [ ] **Step 1: Write the failing integration tests**

Create `packages/web/src/components/canvas/Canvas.solve.test.ts`:

```ts
import { test, expect, beforeAll, beforeEach } from "vitest";
import { render } from "@testing-library/svelte";
import { tick } from "svelte";
import { initWasm } from "@waml/wasm";
import Canvas from "./Canvas.svelte";
import { store } from "../../state/model.svelte";

// A real Diagram doc (has a `## Layout` section) + its three member classes.
const solvedBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];

beforeAll(async () => {
  await initWasm();
});
beforeEach(() => {
  // The store + persisted active-diagram key are module singletons shared across
  // tests; reset both so each test starts from a known view.
  localStorage.clear();
  store.load([]);
});

test("a real Diagram view solves: member positions come from the solver, not the origin", async () => {
  store.load(solvedBundle);
  render(Canvas);
  await tick();
  await tick();
  const g = store.get();
  const order = g.nodes.find((n) => n.key === "shop/order")!;
  // Solver top-left for Order (golden fixture) — not {0,0}.
  expect(order.position).toEqual({ x: 264, y: 69 });
});

test("the implicit All view falls back to dagre (no solve)", async () => {
  // No Diagram doc → effectiveDiagrams synthesizes the "All" view → dagre.
  store.load([
    ["a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"],
    ["b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"],
  ]);
  render(Canvas);
  await tick();
  await tick();
  const g = store.get();
  // dagre laid the two nodes out at distinct, non-origin positions.
  const a = g.nodes.find((n) => n.key === "a")!;
  const b = g.nodes.find((n) => n.key === "b")!;
  expect(a.position).not.toEqual(b.position);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: FAIL — the real-diagram case still shows unsolved positions (dagre or origin), because `layoutActiveView` does not exist yet.

- [ ] **Step 3: Add imports and the `solveResult` state**

Edit `packages/web/src/components/canvas/CanvasInner.svelte`.

Change the layout import (line ~22) to add `runSolveLayout`:

```ts
  import { runDagreLayout, runSolveLayout, type SolveLayout, NODE_W, NODE_H } from "../../canvas/layout";
```

Change the `toRFNode` import (line ~23) to add `toGroupNode`:

```ts
  import { toRFNode, toGroupNode } from "./toRFNode";
```

Add the `erdAwareNodeSize` import alongside the other `@waml/core` imports (near line ~65):

```ts
  import { erdAwareNodeSize } from "@waml/core/canvas/layoutSize";
```

Add the `solveResult` state next to `rfNodes`/`rfEdges` (after line ~144):

```ts
  // Latest solver output for the active REAL Diagram (null on All/behavior views,
  // which use dagre). Drives group-frame pseudo-nodes, collapse flags, and the
  // diagnostics banner. Written ONLY by the imperative layoutActiveView pass.
  let solveResult = $state<SolveLayout | null>(null);
```

- [ ] **Step 4: Replace `layoutAll` with `layoutActiveView`**

Replace the `layoutAll` function (lines ~528-535) with:

```ts
  // Lay out the active view and feed positions into the store overlay. For a REAL
  // Diagram (a doc in $model.diagrams, key ≠ ALL_DIAGRAM_KEY) this is the prose
  // solver; for the implicit "All"/behavior views it's dagre, exactly as before.
  // The OKF bundle carries no positions, so without this every freshly loaded node
  // piles up at the origin.
  function layoutActiveView() {
    const diag = activeDiagram;
    const g = store.get();
    const isRealDiagram = $model.diagrams.some((d) => d.key === diag.key);
    if (isRealDiagram) {
      const sizes: Record<string, { w: number; h: number }> = {};
      for (const n of g.nodes) {
        const s = erdAwareNodeSize(n, activeDisplay);
        sizes[n.key] = { w: s.width, h: s.height };
      }
      try {
        const result = runSolveLayout(store.getBundle(), diag.key, sizes);
        result.positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
        solveResult = result;
      } catch (e) {
        // A solve that throws (e.g. prose the parser rejects) must not escape a
        // handler — leave positions untouched and surface it as a diagnostic.
        solveResult = {
          positions: new Map(),
          groups: [],
          flags: {},
          diagnostics: [
            { severity: "error", code: "malformed-layout", message: String(e), file: diag.key, line: 0, span: undefined },
          ],
        };
      }
      return;
    }
    // Implicit "All" / behavior views have no backing doc → dagre.
    const positions = runDagreLayout(g.nodes, g.edges, activeDisplay);
    positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
    solveResult = null;
  }
```

- [ ] **Step 5: Reroute every trigger to `layoutActiveView`**

In `loadBundleWithLayout` (line ~544), `applyMergeWithLayout` (line ~558), and `handleNewPackageAdd` (line ~577), change each `layoutAll()` call to `layoutActiveView()`:

```ts
  // loadBundleWithLayout — line ~544
    activeDiagramKey = defaultDiagramKey(store.get());
    layoutActiveView();
```
```ts
  // applyMergeWithLayout — line ~558
    if (store.insertPackage("", name, bundle)) layoutActiveView();
```
```ts
  // handleNewPackageAdd — line ~577
      if (store.insertPackage(p.parentPath, slug, docs)) layoutActiveView();
```

Replace the `"layout"` tool button branch in `handleToolChange` (lines ~407-420) with:

```ts
    if (t === "layout") {
      // Turn on node transitions, re-solve/relayout the active view, then frame
      // the result — so the model visibly "organizes itself" instead of snapping.
      layoutAnimating = true;
      layoutActiveView();
      setTimeout(() => fitView({ duration: 500, padding: 0.18 }), 30);
      setTimeout(() => {
        layoutAnimating = false;
      }, 560);
      return;
    }
```

- [ ] **Step 6: Add the Diagram-activation effect**

Add this effect after effect #3 (persist active diagram key, line ~269). It is keyed on the **primitive** `activeDiagramKey` `$state` (never `$model` or the derived `activeDiagram`), and wraps the whole solve in `untrack()` so the solve's own `store.updateNode` writes can never retrigger it — the loop the spec forbids:

```ts
  // 3b) Diagram-view activation: re-solve when the active view switches to a real
  // Diagram, so its `## Layout` prose takes effect. Solve is an IMPERATIVE pass,
  // never reactive — so this effect depends ONLY on the primitive activeDiagramKey
  // (stable across $model emits), and untrack() confines the solve's $model/store
  // reads and writes so they can't re-enter it. Switching to All/behavior views
  // does nothing here (their existing overlay positions are kept).
  $effect(() => {
    const key = activeDiagramKey;
    untrack(() => {
      if ($model.diagrams.some((d) => d.key === key)) layoutActiveView();
    });
  });
```

- [ ] **Step 7: Append group nodes + read collapse from flags in the `rfNodes` effect**

Replace the `rfNodes` effect (effect #1, lines ~230-241) with:

```ts
  $effect(() => {
    const nodes = $model.nodes;
    const disp = activeDisplay;
    const diag = activeDiagram;
    const selNodes = selectionSet.nodes;
    const solved = solveResult;
    const memberNodes = nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => {
        // On a solved view, collapse comes from the solver flags (supersedes the
        // diagram's hand-authored `hints.collapse`); on dagre views, from hints.
        const collapsed = solved
          ? (solved.flags[n.key]?.collapsed ?? false)
          : (diag.hints?.collapse?.includes(n.key) ?? false);
        return {
          ...toRFNode(n, disp, diag.profile, collapsed),
          selected: selNodes.includes(n.key),
        };
      });
    // Append frame-group hull pseudo-nodes behind the members (toGroupNode drops
    // Box/Shrink groups, which draw nothing). Their ids never collide with model
    // node keys, so selection/drag/delete ignore them.
    const groupNodes = (solved?.groups ?? [])
      .map((grp, i) => toGroupNode(grp, i))
      .filter((n): n is Node => n !== null);
    rfNodes = [...groupNodes, ...memberNodes];
  });
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: PASS — the real Diagram shows the golden solved positions; the All view shows distinct dagre positions.

- [ ] **Step 9: Run the full web suite to catch regressions in existing Canvas tests**

Run: `pnpm --filter @waml/web run test`
Expected: PASS (existing `Canvas.test.ts`, `toRFNode.test.ts`, `layout.test.ts`, etc. still green).

- [ ] **Step 10: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.solve.test.ts
git commit -m "feat(web): solve real Diagram layouts via the imperative layoutActiveView pass"
```

---

## Task 5: solver diagnostics banner

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Modify: `packages/web/src/components/canvas/Canvas.solve.test.ts`

**Interfaces:**
- Consumes: `solveResult` (Task 4).
- Produces: a dismissible inline banner listing `solveResult.diagnostics` messages, scoped to the active diagram; `diagnosticsDismissed` `$state` reset whenever a new solve lands.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/Canvas.solve.test.ts`:

```ts
import { fireEvent } from "@testing-library/svelte";

const badRefBundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### All\n- [Customer](./customer.md)\n- [Order](./order.md)\n\n## Layout\n- Ghosts left of Order\n",
  ],
];

test("a diagram referencing a non-member surfaces the diagnostics banner, and it dismisses", async () => {
  store.load(badRefBundle);
  const { getByRole, queryByRole } = render(Canvas);
  await tick();
  await tick();
  const banner = getByRole("alert");
  expect(banner.textContent).toMatch(/Ghosts/i);
  await fireEvent.click(getByRole("button", { name: /dismiss layout warnings/i }));
  await tick();
  expect(queryByRole("alert")).toBeNull();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: FAIL — no element with `role="alert"` (the banner does not exist yet).

- [ ] **Step 3: Add the dismiss state + reset**

In `packages/web/src/components/canvas/CanvasInner.svelte`, add next to `solveResult` (after the `solveResult` `$state` line from Task 4):

```ts
  // The diagnostics banner is dismissible; each new solve (a fresh solveResult
  // object) un-dismisses it so new warnings always show on the next reload.
  let diagnosticsDismissed = $state(false);
```

Add an effect after the activation effect (3b) from Task 4:

```ts
  // 3c) Un-dismiss the diagnostics banner whenever a new solve lands.
  $effect(() => {
    void solveResult;
    diagnosticsDismissed = false;
  });
```

- [ ] **Step 4: Render the banner**

In the canvas wrapper, insert the banner just before the "Empty canvas CTA" block (before the `{#if $model.nodes.length === 0 && ...}` at line ~781):

```svelte
      <!-- Solver diagnostics: a lightweight dismissible strip listing each
           layout warning (conflicts, unresolved refs), so a mistyped name or a
           dumb layout is visible the moment you reload. -->
      {#if solveResult && solveResult.diagnostics.length > 0 && !diagnosticsDismissed}
        <div
          role="alert"
          class="absolute top-3 left-1/2 z-[5] max-w-[600px] -translate-x-1/2 rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-[13px] text-amber-900 shadow"
        >
          <div class="flex items-start gap-2">
            <div class="flex-1">
              {#each solveResult.diagnostics as d}
                <div>{d.message}</div>
              {/each}
            </div>
            <button
              class="text-amber-700 hover:text-amber-900"
              aria-label="Dismiss layout warnings"
              onclick={() => (diagnosticsDismissed = true)}
            >
              ×
            </button>
          </div>
        </div>
      {/if}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: PASS — the banner shows the `Ghosts` warning and disappears on dismiss.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.solve.test.ts
git commit -m "feat(web): surface solver diagnostics in a dismissible canvas banner"
```

---

## Task 6: drag free-override regression test

**Files:**
- Modify: `packages/web/src/components/canvas/Canvas.solve.test.ts`

**Interfaces:**
- Consumes: `store`, the mounted `Canvas` (Tasks 4-5). No production code changes — this task pins the invariant that `onNodeDragStop` (`store.updateNode({ position })`) stays a free override and no reactive effect re-solves it back.

- [ ] **Step 1: Write the failing/guarding test**

Append to `packages/web/src/components/canvas/Canvas.solve.test.ts`:

```ts
test("a dragged position is a free override: it persists until the next solve trigger", async () => {
  store.load(solvedBundle);
  render(Canvas);
  await tick();
  await tick();
  // Sanity: the solve placed Order at its golden spot.
  expect(store.get().nodes.find((n) => n.key === "shop/order")!.position).toEqual({ x: 264, y: 69 });

  // Simulate a drag drop write-back (what onNodeDragStop does).
  store.updateNode("shop/order", { position: { x: 999, y: 999 } });
  await tick();
  await tick();

  // No reactive effect re-solves it back — the override stands until an explicit
  // solve trigger (view switch / layout button / load) overwrites it.
  expect(store.get().nodes.find((n) => n.key === "shop/order")!.position).toEqual({ x: 999, y: 999 });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: PASS immediately — the design already makes drag a free override (no code change needed). If this FAILS (position snapped back to `{264,69}`), a reactive re-solve leaked in: fix by confirming the activation effect is keyed on the primitive `activeDiagramKey` and wrapped in `untrack()` (Task 4, Step 6), not on `$model`/`activeDiagram`.

- [ ] **Step 3: Commit**

```bash
git add packages/web/src/components/canvas/Canvas.solve.test.ts
git commit -m "test(web): pin drag as a free override on solved diagrams"
```

---

## Final verification

- [ ] **Step 1: Run the full repo gate**

Run: `pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. (Optionally `cargo test --workspace` to confirm no crate regressed — this change touches no Rust.)

- [ ] **Step 2: Manual smoke (dogfood the probe)**

Run: `pnpm dev`, then import/load an OKF bundle containing a Diagram doc with a `## Layout` section (e.g. the `shop/orders` fixture prose above). Confirm: members sit at solved positions, a titled frame is drawn behind the `with frame` group, a `with collapsed` node renders as a chip, a mistyped name shows the diagnostics banner, dragging a node moves it freely, and switching to the diagram again re-solves.

---

## Self-Review

**Spec coverage:**

- "Diagram views position nodes from `solve()` output instead of dagre" → Task 1 (bridge) + Task 4 (`layoutActiveView` branch).
- "Render group hulls: titled box for `frame`; box/shrink draw nothing" → Task 2 (`GroupFrame`) + Task 3 (`toGroupNode` drops Box/Shrink) + Task 4 (append).
- "`collapsed` renders a node as a chip; `emphasize` best-effort" → Task 4 (collapse from flags; `OkfNode` already renders the chip). `emphasize` deferred to Spec 2 per Global Constraints (no existing styling) — recorded gap below.
- "Surface solver diagnostics inline" → Task 5 (banner).
- "Keep drag live everywhere" → Task 6 (regression test; `onNodeDragStop` untouched).
- "Solve is a drop-in for the imperative dagre pass; not a reactive `$effect`" → Task 4 (`layoutActiveView` on imperative triggers; activation effect `untrack`ed and keyed on the primitive key).
- "Positions flow through the store overlay" → Task 4 (`store.updateNode`).
- "Defensive try/catch around solve → diagnostic, never throws out of a handler" → Task 4 (`layoutActiveView` catch).
- "One new trigger: Diagram-view activation" → Task 4 (activation effect).
- "No Rust/wasm changes" → honored; only `packages/web` touched.
- Testing section (unit layout / component / diagnostics / drag) → Tasks 1, 4, 5, 6.

**Type consistency:** `SolveLayout` (Task 1) is the single shape consumed by `CanvasInner` (Task 4). `toGroupNode(group, index)` (Task 3) is used exactly as declared in Task 4's `rfNodes` effect. `Shape` compared as `"Frame"` (capitalized) everywhere. `sizes` is `Record<string, { w: number; h: number }>` in both `solve()` and `runSolveLayout`. `store.getBundle()` returns `[string, string][]`, matching `solve()`'s `bundle` param.

**Placeholder scan:** No TBD/TODO/"add error handling"/"similar to Task N" — every code step carries full code.

## Assumptions / gaps for the human to sanity-check

- **`Shape` is capitalized** (`"Frame" | "Box" | "Shrink"`), not the lowercase `frame`/`box`/`shrink` in the spec prose. The plan uses `"Frame"`. Confirm the wasm-generated type hasn't since changed.
- **`collapsed` layout prose form.** The plan's fixtures use `- Order with collapsed` (backed by `crates/waml/src/layout.rs` L323/L530/L673). If the parser rejects that exact phrasing for a bare member subject, the fail-first step will reveal it; adjust the prose (not the assertion) to the accepted form.
- **`emphasize` is not rendered.** No emphasis styling exists in `OkfNode`/`ClassifierBox`, so per the spec's "best-effort... otherwise defer to Spec 2" the `emphasized` flag is read-through-but-unused. If a lightweight emphasis affordance is wanted now, it's a small add to `OkfNode` — flagged, not planned.
- **Sizes are built from all `store.get().nodes`, not just members.** `solve()` ignores sizes for non-member keys, so this is harmless and simpler; confirm that's acceptable vs. filtering to `memberSet`.
- **The activation effect also re-solves on mount and on every real-diagram switch**, recomputing any drag override — this is the spec's explicitly-accepted "known consequence" (single overlay position per node; re-solve on view entry). The Open Question of skip-if-already-solved is intentionally deferred (unconditional re-solve).
- **Group-hull `zIndex` uses `depth - 1000`.** This assumes member nodes keep the default `zIndex` (0) and no diagram legitimately nests groups more than ~1000 deep. Fine for a probe; revisit if deep nesting appears.
- **Component tests assert store overlay positions (reliable), not SvelteFlow-rendered node DOM** (which jsdom leaves unmeasured/hidden). Group-hull and collapse-chip *rendering* correctness is covered by the direct `GroupFrame` render test (Task 2) and the pure `toGroupNode`/`runSolveLayout` unit tests (Tasks 1, 3) rather than by asserting SvelteFlow internals.
