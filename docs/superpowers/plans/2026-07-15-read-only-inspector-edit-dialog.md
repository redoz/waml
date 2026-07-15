# Read-Only Docked Inspector + Element Edit Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the docked `InspectorPanel` a read-only summary of the selected element, and move editing into the centered `CentralEditPanel` dialog (nodes and edges), with a live cropped preview of the element above the edit fields.

**Architecture:** The docked panel keeps its chrome but swaps its editable body for read-only presentational variants and gains an Edit button that opens `CentralEditPanelHost`. The host grows a new `"edge"` state kind (mirroring `"element"`), passes a `fullHeight` sizing flag to `CentralEditPanel`, and renders a new `ElementPreview` (a second, isolated read-only `SvelteFlow` fed a filtered node/edge subset) at the top of the dialog body. All model→xyflow conversion reuses the existing `toRFNode` / `buildRfEdges` / `flowTypes` helpers; the subset-filtering logic is a pure, unit-tested module.

**Tech Stack:** Svelte 5 (runes: `$props`, `$state`, `$derived`, `$effect`, snippets), TypeScript, `@xyflow/svelte` v1, Tailwind CSS, Vitest + `@testing-library/svelte`, pnpm workspace.

## Global Constraints

- Package under change: `@waml/web` (`packages/web`). Run tests with `pnpm --filter @waml/web test <path>`.
- Commit messages: Conventional Commits. **Never** add a `Co-Authored-By: Claude` trailer or any Claude/AI footer (repo policy).
- `CentralEditPanel`'s diagram-properties sizing is unchanged: `max-w-[560px]`, `max-h-[85vh]`, `p-8` scrim inset. The `fullHeight` variant keeps width at `max-w-[560px]` and only raises height (`max-h-[95vh]`) and reduces scrim inset (`p-4`).
- Neighbor/edge dimming in the preview reuses the app's established dim convention: opacity `0.4` (the same value as the Tailwind `opacity-40` used by the pinned Inspector and disabled `DiagramPropertiesBody` rows). Do **not** invent a new value.
- Do not remove or edit `packages/web/src/components/inspector/Inspector.svelte` or its test — it becomes unreferenced after Task 9 but its removal is explicitly out of scope.
- Read-only body reuses the same underlying model fields as the editable inspectors; it renders no `<input>`, `<textarea>`, or editable `<select>`.
- Model types (from `@waml/okf`, do not redefine): `ModelNode { key; type; concept: { id; type; title?; description?; body }; stereotypes: string[]; abstract?: boolean; attributes: Attribute[]; values?: string[] }`, `Attribute { name; type: { name; ref? }; multiplicity: string; visibility?: "+"|"-"|"#"|"~"; description? }`, `ModelEdge { id; kind: RelationshipKind; from; to; fromEnd: RelEnd; toEnd: RelEnd; bidirectional: boolean; name? }`, `RelEnd { multiplicity?; role?; navigable? }`. `ENDED_KINDS` (a `Set<RelationshipKind>`) is exported from `@waml/okf`.

---

## File Structure

**Created:**
- `packages/web/src/components/central/previewSubset.ts` — pure functions that compute the filtered node/edge subset (focal element + context) for the preview. No Svelte, no xyflow.
- `packages/web/src/components/central/previewSubset.test.ts` — unit tests for the above.
- `packages/web/src/components/central/ElementPreviewCanvas.svelte` — the inner read-only `SvelteFlow` (must live inside a provider; calls `useSvelteFlow().fitView`).
- `packages/web/src/components/central/ElementPreview.svelte` — wraps `ElementPreviewCanvas` in its own `SvelteFlowProvider` (isolates flow context from the main canvas) and applies the subset filter.
- `packages/web/src/components/central/ElementPreview.test.ts` — smoke render test.
- `packages/web/src/components/inspector/ObjectInspectorReadonly.svelte` — read-only node field summary.
- `packages/web/src/components/inspector/ObjectInspectorReadonly.test.ts`
- `packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte` — read-only edge field summary.
- `packages/web/src/components/inspector/RelationshipInspectorReadonly.test.ts`
- `packages/web/src/components/inspector/InspectorReadonly.svelte` — dispatcher: node→`ObjectInspectorReadonly`, edge→`RelationshipInspectorReadonly` (replaces `Inspector.svelte` as the docked body).
- `packages/web/src/components/inspector/InspectorReadonly.test.ts`

**Modified:**
- `packages/web/src/components/central/CentralEditPanel.svelte` — add `fullHeight` + optional `preview` snippet props.
- `packages/web/src/components/central/CentralEditPanel.test.ts` — cover `fullHeight` classes + `preview` slot.
- `packages/web/src/components/central/CentralEditPanelHost.svelte` — add `"edge"` state kind, `edges`/`onUpdateEdge`/`showPreview` props, edge branch, `fullHeight`, and `ElementPreview` wiring.
- `packages/web/src/components/central/CentralEditPanelHost.test.ts` — add `edges`/`onUpdateEdge` to props, cover the edge branch + since-deleted guard.
- `packages/web/src/components/inspector/InspectorPanel.svelte` — add an `onEdit` prop + header Edit button.
- `packages/web/src/components/inspector/InspectorPanel.test.ts` — cover the Edit button.
- `packages/web/src/components/canvas/CanvasInner.svelte` — swap `Inspector`→`InspectorReadonly` body, add `onEdit` wiring, extend the host props.
- `packages/web/src/components/canvas/Canvas.test.ts` — update the one assertion that expected an editable Title input in the docked panel; add an Edit-opens-dialog test.

---

### Task 1: Preview subset helper

**Files:**
- Create: `packages/web/src/components/central/previewSubset.ts`
- Test: `packages/web/src/components/central/previewSubset.test.ts`

**Interfaces:**
- Consumes: `ModelNode`, `ModelEdge` from `@waml/okf`.
- Produces:
  - `interface PreviewSubset { nodes: ModelNode[]; edges: ModelEdge[]; focalKeys: Set<string> }`
  - `function nodePreviewSubset(focalKey: string, nodes: ModelNode[], edges: ModelEdge[]): PreviewSubset`
  - `function edgePreviewSubset(focalEdgeId: string, nodes: ModelNode[], edges: ModelEdge[]): PreviewSubset`
  - Node mode: `focalKeys = {focalKey}`; `nodes` = focal + directly-connected neighbors; `edges` = every model edge whose **both** endpoints are in that node set. Edge mode: `focalKeys = {edge.from, edge.to}`; `nodes` = both endpoints; `edges` = just that edge (empty result if the id is missing).

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/central/previewSubset.test.ts
import { test, expect } from "vitest";
import type { ModelNode, ModelEdge } from "@waml/okf";
import { nodePreviewSubset, edgePreviewSubset } from "./previewSubset";

const node = (key: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title: key, body: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as ModelNode;

const edge = (id: string, from: string, to: string): ModelEdge =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: {}, bidirectional: false });

// a—b, a—c, c—d. From a: keep {a,b,c}; d is reachable only via c, not directly.
const NODES = [node("a"), node("b"), node("c"), node("d")];
const EDGES = [edge("ab", "a", "b"), edge("ac", "a", "c"), edge("cd", "c", "d")];

test("node subset keeps focal + direct neighbours, drops distant nodes", () => {
  const s = nodePreviewSubset("a", NODES, EDGES);
  expect(s.nodes.map((n) => n.key).sort()).toEqual(["a", "b", "c"]);
  expect([...s.focalKeys]).toEqual(["a"]);
});

test("node subset keeps only edges with both endpoints in the kept set", () => {
  const s = nodePreviewSubset("a", NODES, EDGES);
  expect(s.edges.map((e) => e.id).sort()).toEqual(["ab", "ac"]); // cd excluded (d dropped)
});

test("edge subset keeps the edge and both endpoint nodes as focal", () => {
  const s = edgePreviewSubset("ac", NODES, EDGES);
  expect(s.nodes.map((n) => n.key).sort()).toEqual(["a", "c"]);
  expect(s.edges.map((e) => e.id)).toEqual(["ac"]);
  expect([...s.focalKeys].sort()).toEqual(["a", "c"]);
});

test("edge subset for a missing id is empty", () => {
  const s = edgePreviewSubset("nope", NODES, EDGES);
  expect(s.nodes).toEqual([]);
  expect(s.edges).toEqual([]);
  expect(s.focalKeys.size).toBe(0);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/central/previewSubset.test.ts`
Expected: FAIL — `Failed to resolve import "./previewSubset"` / "nodePreviewSubset is not a function".

- [ ] **Step 3: Write the implementation**

```ts
// packages/web/src/components/central/previewSubset.ts
import type { ModelNode, ModelEdge } from "@waml/okf";

/** The filtered slice of the model rendered in the dialog's live preview. */
export interface PreviewSubset {
  /** Model nodes to render. */
  nodes: ModelNode[];
  /** Model edges to render (endpoints all present in `nodes`). */
  edges: ModelEdge[];
  /** Keys of nodes shown at full opacity; every other node/edge is dimmed. */
  focalKeys: Set<string>;
}

/**
 * Node edit: the focal node plus its directly-connected neighbours, and every
 * model edge whose BOTH endpoints are in that set. Only the focal node is
 * full-opacity (its context neighbours + connecting edges render dimmed).
 */
export function nodePreviewSubset(
  focalKey: string,
  nodes: ModelNode[],
  edges: ModelEdge[],
): PreviewSubset {
  const keep = new Set<string>([focalKey]);
  for (const e of edges) {
    if (e.from === focalKey) keep.add(e.to);
    if (e.to === focalKey) keep.add(e.from);
  }
  return {
    nodes: nodes.filter((n) => keep.has(n.key)),
    edges: edges.filter((e) => keep.has(e.from) && keep.has(e.to)),
    focalKeys: new Set([focalKey]),
  };
}

/** Edge edit: the edge plus both endpoint nodes, both endpoints full-opacity. */
export function edgePreviewSubset(
  focalEdgeId: string,
  nodes: ModelNode[],
  edges: ModelEdge[],
): PreviewSubset {
  const edge = edges.find((e) => e.id === focalEdgeId);
  if (!edge) return { nodes: [], edges: [], focalKeys: new Set() };
  const keep = new Set<string>([edge.from, edge.to]);
  return {
    nodes: nodes.filter((n) => keep.has(n.key)),
    edges: [edge],
    focalKeys: keep,
  };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/central/previewSubset.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/central/previewSubset.ts packages/web/src/components/central/previewSubset.test.ts
git commit -m "feat(web): add element-preview subset helper"
```

---

### Task 2: CentralEditPanel `fullHeight` + `preview` slot

**Files:**
- Modify: `packages/web/src/components/central/CentralEditPanel.svelte`
- Test: `packages/web/src/components/central/CentralEditPanel.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces: `CentralEditPanel` props become `{ title: string; onClose: () => void; fullHeight?: boolean; preview?: Snippet; children: Snippet }`. `fullHeight` defaults `false`. When `preview` is provided it renders full-bleed (no body padding) above the padded, scrolling `children` body.

- [ ] **Step 1: Write the failing test (append to the existing file)**

Add these tests to the end of `packages/web/src/components/central/CentralEditPanel.test.ts` (the `createRawSnippet` import already exists at the top of that file):

```ts
test("default sizing caps at 85vh with an 8-unit scrim inset", () => {
  render(CentralEditPanel, { props: props() });
  const card = screen.getByRole("dialog");
  expect(card.className).toContain("max-h-[85vh]");
  expect(card.className).not.toContain("max-h-[95vh]");
  expect(screen.getByTestId("central-scrim").className).toContain("p-8");
});

test("fullHeight raises the cap to 95vh and reduces the scrim inset", () => {
  render(CentralEditPanel, { props: props({ fullHeight: true }) });
  const card = screen.getByRole("dialog");
  expect(card.className).toContain("max-h-[95vh]");
  expect(card.className).not.toContain("max-h-[85vh]");
  expect(screen.getByTestId("central-scrim").className).toContain("p-4");
});

test("renders a preview snippet above the body", () => {
  const preview = createRawSnippet(() => ({
    render: () => `<div data-testid="preview-slot">preview</div>`,
  }));
  render(CentralEditPanel, { props: props({ preview }) });
  expect(screen.getByTestId("preview-slot")).toBeTruthy();
  expect(screen.getByLabelText("field")).toBeTruthy(); // body still renders
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/central/CentralEditPanel.test.ts`
Expected: FAIL — `fullHeight` test finds `max-h-[85vh]` (prop ignored); preview test can't find `preview-slot`.

- [ ] **Step 3: Implement — update the props block**

In `packages/web/src/components/central/CentralEditPanel.svelte`, replace:

```svelte
  let { title, onClose, children }: {
    title: string;
    onClose: () => void;
    children: Snippet;
  } = $props();
```

with:

```svelte
  let { title, onClose, fullHeight = false, preview, children }: {
    title: string;
    onClose: () => void;
    fullHeight?: boolean;
    preview?: Snippet;
    children: Snippet;
  } = $props();
```

- [ ] **Step 4: Implement — make the scrim inset conditional**

Replace:

```svelte
  class="fixed inset-0 z-[60] bg-slate-900/30 flex items-center justify-center p-8"
```

with:

```svelte
  class={`fixed inset-0 z-[60] bg-slate-900/30 flex items-center justify-center ${fullHeight ? "p-4" : "p-8"}`}
```

- [ ] **Step 5: Implement — make the card height conditional**

Replace:

```svelte
    class="w-full max-w-[560px] max-h-[85vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)]"
```

with:

```svelte
    class={`w-full max-w-[560px] ${fullHeight ? "h-[95vh] max-h-[95vh]" : "max-h-[85vh]"} flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)]`}
```

- [ ] **Step 6: Implement — render the preview above the body**

Replace:

```svelte
    <div class="px-5 py-5 overflow-y-auto flex-1 min-h-0">
      {@render children()}
    </div>
```

with:

```svelte
    {#if preview}
      {@render preview()}
    {/if}
    <div class="px-5 py-5 overflow-y-auto flex-1 min-h-0">
      {@render children()}
    </div>
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web test src/components/central/CentralEditPanel.test.ts`
Expected: PASS (all original tests + the 3 new ones).

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/central/CentralEditPanel.svelte packages/web/src/components/central/CentralEditPanel.test.ts
git commit -m "feat(web): add fullHeight + preview slot to CentralEditPanel"
```

---

### Task 3: Live `ElementPreview` component

**Files:**
- Create: `packages/web/src/components/central/ElementPreviewCanvas.svelte`
- Create: `packages/web/src/components/central/ElementPreview.svelte`
- Test: `packages/web/src/components/central/ElementPreview.test.ts`

**Interfaces:**
- Consumes: `nodePreviewSubset` / `edgePreviewSubset` / `PreviewSubset` from `./previewSubset` (Task 1); `toRFNode` from `../canvas/toRFNode`; `buildRfEdges` from `../canvas/edges`; `nodeTypes`, `edgeTypes` from `../canvas/flowTypes`; `DiagramDisplay` from `@waml/okf`.
- Produces: `ElementPreview` props `{ mode: "node" | "edge"; focalKey: string; nodes: ModelNode[]; edges: ModelEdge[]; display: DiagramDisplay; profileName: string }`. Renders a fixed 220px-high, bottom-bordered region containing an isolated, non-interactive `SvelteFlow` cropped (via `fitView`) to the subset. `focalKey` is a node key in `"node"` mode and an edge id in `"edge"` mode. Root element carries `data-testid="element-preview"`.
- **Critical:** `ElementPreview` MUST wrap its `SvelteFlow` in its own `<SvelteFlowProvider>`. Without it, the preview would attach to the main canvas's flow context and corrupt the real canvas.

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/central/ElementPreview.test.ts
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { DEFAULT_DISPLAY, type ModelNode, type ModelEdge } from "@waml/okf";
import ElementPreview from "./ElementPreview.svelte";

const node = (key: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title: key, body: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as ModelNode;

const edge: ModelEdge = {
  id: "ab",
  kind: "associates",
  from: "a",
  to: "b",
  fromEnd: {},
  toEnd: {},
  bidirectional: false,
};

test("renders the fixed-height preview region for a node", () => {
  render(ElementPreview, {
    props: {
      mode: "node",
      focalKey: "a",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  const region = screen.getByTestId("element-preview");
  expect(region).toBeTruthy();
  expect(region.className).toContain("h-[220px]");
});

test("renders the preview region for an edge", () => {
  render(ElementPreview, {
    props: {
      mode: "edge",
      focalKey: "ab",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  expect(screen.getByTestId("element-preview")).toBeTruthy();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/central/ElementPreview.test.ts`
Expected: FAIL — `Failed to resolve import "./ElementPreview"`.

- [ ] **Step 3: Implement the inner canvas**

```svelte
<!-- packages/web/src/components/central/ElementPreviewCanvas.svelte -->
<script lang="ts">
  // The read-only inner flow. Must be mounted inside a SvelteFlowProvider (see
  // ElementPreview.svelte) so useSvelteFlow()/fitView bind to an ISOLATED flow
  // context, never the real canvas's.
  import { SvelteFlow, useSvelteFlow, type Node, type Edge } from "@xyflow/svelte";
  import { tick } from "svelte";
  import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
  import { toRFNode } from "../canvas/toRFNode";
  import { buildRfEdges } from "../canvas/edges";
  import { nodeTypes, edgeTypes } from "../canvas/flowTypes";

  // Context neighbours + their connecting edges render dimmed. Reuse the app's
  // established opacity-40 dim convention (0.4) rather than inventing a value.
  const DIM = "opacity:0.4";

  let { nodes, edges, focalKeys, display, profileName }: {
    nodes: ModelNode[];
    edges: ModelEdge[];
    focalKeys: Set<string>;
    display: DiagramDisplay;
    profileName: string;
  } = $props();

  // SvelteFlow binds these; we rebuild them from props on every subset change
  // (mirrors CanvasInner's rfNodes/rfEdges effect pattern).
  let rfNodes = $state<Node[]>([]);
  let rfEdges = $state<Edge[]>([]);

  $effect(() => {
    rfNodes = nodes.map((n) => ({
      ...toRFNode(n, display, profileName, false),
      selectable: false,
      draggable: false,
      style: focalKeys.has(n.key) ? undefined : DIM,
    }));
    rfEdges = buildRfEdges(edges, nodes, display).map((e) => ({
      ...e,
      selectable: false,
      // Full opacity only when BOTH endpoints are focal. In edge mode focalKeys is
      // {from,to}, so the edge being edited stays crisp; in node mode focalKeys is
      // the single focal node, so every kept context edge (touching ≤1 focal node)
      // dims. Do NOT dim unconditionally — that would fade the very edge under edit.
      style: focalKeys.has(e.source) && focalKeys.has(e.target) ? undefined : DIM,
    }));
  });

  const { fitView } = useSvelteFlow();

  // Re-crop to the rendered set on mount and whenever its geometry changes.
  $effect(() => {
    void rfNodes;
    void rfEdges;
    void tick().then(() => fitView({ padding: 0.2, duration: 0 }));
  });
</script>

<SvelteFlow
  bind:nodes={rfNodes}
  bind:edges={rfEdges}
  {nodeTypes}
  {edgeTypes}
  fitView
  nodesDraggable={false}
  nodesConnectable={false}
  elementsSelectable={false}
  panOnDrag={false}
  panOnScroll={false}
  zoomOnScroll={false}
  zoomOnDoubleClick={false}
  minZoom={0.2}
  maxZoom={2}
/>
```

- [ ] **Step 4: Implement the provider wrapper**

```svelte
<!-- packages/web/src/components/central/ElementPreview.svelte -->
<script lang="ts">
  // A static, live-updating cropped render of the edited element in context.
  // No pan/zoom/click/drag — purely a view. Wraps its own SvelteFlowProvider so
  // its flow context is isolated from the real canvas behind the dialog.
  import { SvelteFlowProvider } from "@xyflow/svelte";
  import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
  import ElementPreviewCanvas from "./ElementPreviewCanvas.svelte";
  import { nodePreviewSubset, edgePreviewSubset } from "./previewSubset";

  let { mode, focalKey, nodes, edges, display, profileName }: {
    mode: "node" | "edge";
    focalKey: string;
    nodes: ModelNode[];
    edges: ModelEdge[];
    display: DiagramDisplay;
    profileName: string;
  } = $props();

  const subset = $derived(
    mode === "node"
      ? nodePreviewSubset(focalKey, nodes, edges)
      : edgePreviewSubset(focalKey, nodes, edges),
  );
</script>

<div
  class="h-[220px] shrink-0 border-b border-[#d8dee8] bg-[#f7f8fa]"
  data-testid="element-preview"
>
  <SvelteFlowProvider>
    <ElementPreviewCanvas
      nodes={subset.nodes}
      edges={subset.edges}
      focalKeys={subset.focalKeys}
      {display}
      {profileName}
    />
  </SvelteFlowProvider>
</div>
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/central/ElementPreview.test.ts`
Expected: PASS (2 tests). (SvelteFlow-in-jsdom is already exercised by `Canvas.test.ts`, so rendering works even though real layout/measurement is inert.)

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/central/ElementPreviewCanvas.svelte packages/web/src/components/central/ElementPreview.svelte packages/web/src/components/central/ElementPreview.test.ts
git commit -m "feat(web): add live ElementPreview component"
```

---

### Task 4: Read-only `ObjectInspector` variant

**Files:**
- Create: `packages/web/src/components/inspector/ObjectInspectorReadonly.svelte`
- Test: `packages/web/src/components/inspector/ObjectInspectorReadonly.test.ts`

**Interfaces:**
- Consumes: `ModelNode` from `@waml/okf`.
- Produces: `ObjectInspectorReadonly` prop `{ node: ModelNode }`. Renders Title, Description, Type, an `abstract` badge (only when `node.abstract`), Stereotypes (as `«…»`), and either Values (for `uml.Enum`) or Attributes — all as static text, no form controls.

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/inspector/ObjectInspectorReadonly.test.ts
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import type { ModelNode } from "@waml/okf";
import ObjectInspectorReadonly from "./ObjectInspectorReadonly.svelte";

const node: ModelNode = {
  key: "order",
  type: "uml.Class",
  concept: { id: "order", type: "uml.Class", title: "Order", description: "A placed order", body: "" },
  stereotypes: ["aggregateRoot"],
  attributes: [{ name: "total", type: { name: "Money" }, multiplicity: "1" }],
  position: { x: 0, y: 0 },
};

test("renders node fields as static text with no editable controls", () => {
  const { container } = render(ObjectInspectorReadonly, { props: { node } });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByText("A placed order")).toBeTruthy();
  expect(screen.getByText("uml.Class")).toBeTruthy();
  expect(screen.getByText("«aggregateRoot»")).toBeTruthy();
  expect(container.querySelector("input")).toBeNull();
  expect(container.querySelector("textarea")).toBeNull();
  expect(container.querySelector("select")).toBeNull();
});

test("shows an abstract badge only when the node is abstract", () => {
  const { rerender } = render(ObjectInspectorReadonly, { props: { node } });
  expect(screen.queryByText("abstract")).toBeNull();
  rerender({ node: { ...node, abstract: true } });
  expect(screen.getByText("abstract")).toBeTruthy();
});

test("shows the enum values list for uml.Enum", () => {
  render(ObjectInspectorReadonly, {
    props: { node: { ...node, type: "uml.Enum", values: ["NEW", "PAID"] } },
  });
  expect(screen.getByText("NEW")).toBeTruthy();
  expect(screen.getByText("PAID")).toBeTruthy();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/inspector/ObjectInspectorReadonly.test.ts`
Expected: FAIL — `Failed to resolve import "./ObjectInspectorReadonly"`.

- [ ] **Step 3: Implement**

```svelte
<!-- packages/web/src/components/inspector/ObjectInspectorReadonly.svelte -->
<script lang="ts">
  import type { ModelNode } from "@waml/okf";

  let { node }: { node: ModelNode } = $props();

  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
  const valueCls = "text-[13px] text-slate-900 whitespace-pre-wrap break-words";
  const emptyCls = "text-[13px] text-slate-400 italic";

  const isEnum = $derived(node.type === "uml.Enum");
</script>

<div class="flex flex-col gap-[15px]">
  <div>
    <span class={labelCls}>Title</span>
    {#if node.concept.title?.trim()}
      <div class={valueCls}>{node.concept.title}</div>
    {:else}
      <div class={emptyCls}>Untitled</div>
    {/if}
  </div>
  <div>
    <span class={labelCls}>Description</span>
    {#if node.concept.description?.trim()}
      <div class={valueCls}>{node.concept.description}</div>
    {:else}
      <div class={emptyCls}>No description</div>
    {/if}
  </div>
  <div class="flex gap-[10px] items-start">
    <div class="flex-1">
      <span class={labelCls}>Type</span>
      <div class={valueCls}>{node.type}</div>
    </div>
    {#if node.abstract}
      <span class="text-[12px] font-semibold text-[#1e88e5] bg-[#e6f1fb] rounded px-2 py-1">abstract</span>
    {/if}
  </div>
  <div>
    <span class={labelCls}>Stereotypes</span>
    {#if node.stereotypes.length > 0}
      <div class={valueCls}>{node.stereotypes.map((s) => `«${s}»`).join(" ")}</div>
    {:else}
      <div class={emptyCls}>None</div>
    {/if}
  </div>
  {#if isEnum}
    <div>
      <span class={labelCls}>Values</span>
      {#if (node.values ?? []).length > 0}
        <ul class="text-[13px] text-slate-900 list-disc pl-5">
          {#each node.values ?? [] as v (v)}
            <li>{v}</li>
          {/each}
        </ul>
      {:else}
        <div class={emptyCls}>No values</div>
      {/if}
    </div>
  {:else}
    <div>
      <span class={labelCls}>Attributes</span>
      {#if node.attributes.length > 0}
        <ul class="flex flex-col gap-[4px]">
          {#each node.attributes as a, i (i)}
            <li class="text-[13px] text-slate-900 font-mono break-words">
              {a.visibility ?? ""}{a.name}: {a.type.name}{a.multiplicity && a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}
            </li>
          {/each}
        </ul>
      {:else}
        <div class={emptyCls}>No attributes</div>
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/inspector/ObjectInspectorReadonly.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/inspector/ObjectInspectorReadonly.svelte packages/web/src/components/inspector/ObjectInspectorReadonly.test.ts
git commit -m "feat(web): add read-only ObjectInspector variant"
```

---

### Task 5: Read-only `RelationshipInspector` variant

**Files:**
- Create: `packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte`
- Test: `packages/web/src/components/inspector/RelationshipInspectorReadonly.test.ts`

**Interfaces:**
- Consumes: `ModelEdge`, `ModelNode`, `ENDED_KINDS` from `@waml/okf`.
- Produces: `RelationshipInspectorReadonly` props `{ edge: ModelEdge; fromNode?: ModelNode; toNode?: ModelNode }`. Renders the `from → to` heading, Kind, and (only for `ENDED_KINDS`) each end's multiplicity + role as static text, plus a "Bidirectional" line for a bidirectional `associates` edge. No form controls.

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/inspector/RelationshipInspectorReadonly.test.ts
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import type { ModelEdge, ModelNode } from "@waml/okf";
import RelationshipInspectorReadonly from "./RelationshipInspectorReadonly.svelte";

const node = (key: string, title: string): ModelNode =>
  ({ key, type: "uml.Class", concept: { id: key, type: "uml.Class", title, body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

const edge: ModelEdge = {
  id: "e1",
  kind: "associates",
  from: "a",
  to: "b",
  fromEnd: { multiplicity: "1" },
  toEnd: { multiplicity: "*", navigable: true },
  bidirectional: false,
};

test("renders endpoints, kind, and multiplicities as static text", () => {
  const { container } = render(RelationshipInspectorReadonly, {
    props: { edge, fromNode: node("a", "Order"), toNode: node("b", "OrderLine") },
  });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByText("OrderLine")).toBeTruthy();
  expect(screen.getByText("associates")).toBeTruthy();
  expect(screen.getByText("Order multiplicity")).toBeTruthy();
  expect(container.querySelector("input")).toBeNull();
  expect(container.querySelector("select")).toBeNull();
});

test("hides end fields for specializes (no ended kinds)", () => {
  render(RelationshipInspectorReadonly, {
    props: {
      edge: { ...edge, kind: "specializes", fromEnd: {}, toEnd: {} },
      fromNode: node("a", "Child"),
      toNode: node("b", "Parent"),
    },
  });
  expect(screen.queryByText("Child multiplicity")).toBeNull();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/inspector/RelationshipInspectorReadonly.test.ts`
Expected: FAIL — `Failed to resolve import "./RelationshipInspectorReadonly"`.

- [ ] **Step 3: Implement**

```svelte
<!-- packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte -->
<script lang="ts">
  import type { ModelEdge, ModelNode } from "@waml/okf";
  import { ENDED_KINDS } from "@waml/okf";

  let { edge, fromNode, toNode }: {
    edge: ModelEdge;
    fromNode?: ModelNode;
    toNode?: ModelNode;
  } = $props();

  const fromTitle = $derived(fromNode?.concept.title?.trim() || "Source");
  const toTitle = $derived(toNode?.concept.title?.trim() || "Target");
  const hasEnds = $derived(ENDED_KINDS.has(edge.kind));

  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
  const valueCls = "text-[13px] text-slate-900";
</script>

<div class="flex flex-col gap-[15px]">
  <div class="text-[13px] text-slate-500">
    <strong class="text-slate-900">{fromTitle}</strong> → <strong class="text-slate-900">{toTitle}</strong>
  </div>
  <div>
    <span class={labelCls}>Kind</span>
    <div class={valueCls}>{edge.kind}</div>
  </div>
  {#if hasEnds}
    <div class="flex flex-col gap-[10px]">
      <div class="flex gap-[6px]">
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{fromTitle} multiplicity</span>
          <div class={valueCls}>{edge.fromEnd.multiplicity ?? "—"}</div>
        </div>
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{fromTitle} role</span>
          <div class={valueCls}>{edge.fromEnd.role ?? "—"}</div>
        </div>
      </div>
      <div class="flex gap-[6px]">
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{toTitle} multiplicity</span>
          <div class={valueCls}>{edge.toEnd.multiplicity ?? "—"}</div>
        </div>
        <div class="flex-1">
          <span class="text-[11px] text-slate-500">{toTitle} role</span>
          <div class={valueCls}>{edge.toEnd.role ?? "—"}</div>
        </div>
      </div>
    </div>
  {/if}
  {#if edge.kind === "associates" && edge.bidirectional}
    <div class={valueCls}>Bidirectional</div>
  {/if}
</div>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/inspector/RelationshipInspectorReadonly.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte packages/web/src/components/inspector/RelationshipInspectorReadonly.test.ts
git commit -m "feat(web): add read-only RelationshipInspector variant"
```

---

### Task 6: Read-only inspector dispatcher

**Files:**
- Create: `packages/web/src/components/inspector/InspectorReadonly.svelte`
- Test: `packages/web/src/components/inspector/InspectorReadonly.test.ts`

**Interfaces:**
- Consumes: `Selection` from `../canvas/selection`; `ObjectInspectorReadonly` (Task 4); `RelationshipInspectorReadonly` (Task 5); `ModelNode`, `ModelEdge` from `@waml/okf`; `Snippet` from `svelte`.
- Produces: `InspectorReadonly` props `{ selection: Selection; nodes: ModelNode[]; edges: ModelEdge[]; externalRefs?: Snippet }`. Node selection → `ObjectInspectorReadonly` + the `externalRefs` snippet; edge selection → `RelationshipInspectorReadonly` (with resolved `fromNode`/`toNode`); null → renders nothing (the docked panel supplies its own empty hint).

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/inspector/InspectorReadonly.test.ts
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import type { ModelNode, ModelEdge } from "@waml/okf";
import InspectorReadonly from "./InspectorReadonly.svelte";

const nodes: ModelNode[] = [
  { key: "a", type: "uml.Class", concept: { id: "a", type: "uml.Class", title: "Order", body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
  { key: "b", type: "uml.Class", concept: { id: "b", type: "uml.Class", title: "OrderLine", body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
];
const edges: ModelEdge[] = [
  { id: "e1", kind: "associates", from: "a", to: "b", fromEnd: {}, toEnd: {}, bidirectional: false },
];

test("node selection renders the read-only object body plus externalRefs", () => {
  const externalRefs = createRawSnippet(() => ({ render: () => `<div data-testid="ext">refs</div>` }));
  render(InspectorReadonly, { props: { selection: { type: "node", id: "a" }, nodes, edges, externalRefs } });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByTestId("ext")).toBeTruthy();
});

test("edge selection renders the read-only relationship body", () => {
  render(InspectorReadonly, { props: { selection: { type: "edge", id: "e1" }, nodes, edges } });
  expect(screen.getByText("associates")).toBeTruthy();
});

test("null selection renders no editable controls", () => {
  const { container } = render(InspectorReadonly, { props: { selection: null, nodes, edges } });
  expect(container.querySelector("input")).toBeNull();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/inspector/InspectorReadonly.test.ts`
Expected: FAIL — `Failed to resolve import "./InspectorReadonly"`.

- [ ] **Step 3: Implement**

```svelte
<!-- packages/web/src/components/inspector/InspectorReadonly.svelte -->
<script lang="ts">
  // Read-only docked-panel body. Mirrors Inspector.svelte's embedded dispatch,
  // but shows static field summaries instead of editable inputs — editing moves
  // to the CentralEditPanel dialog (opened via the panel's Edit button).
  import type { Snippet } from "svelte";
  import type { ModelNode, ModelEdge } from "@waml/okf";
  import type { Selection } from "../canvas/selection";
  import ObjectInspectorReadonly from "./ObjectInspectorReadonly.svelte";
  import RelationshipInspectorReadonly from "./RelationshipInspectorReadonly.svelte";

  let { selection, nodes, edges, externalRefs }: {
    selection: Selection;
    nodes: ModelNode[];
    edges: ModelEdge[];
    externalRefs?: Snippet;
  } = $props();

  const selectedNode = $derived(
    selection?.type === "node" ? nodes.find((n) => n.key === selection.id) : undefined,
  );
  const selectedEdge = $derived(
    selection?.type === "edge" ? edges.find((e) => e.id === selection.id) : undefined,
  );
</script>

{#if selectedNode}
  <ObjectInspectorReadonly node={selectedNode} />
  {@render externalRefs?.()}
{:else if selectedEdge}
  <RelationshipInspectorReadonly
    edge={selectedEdge}
    fromNode={nodes.find((n) => n.key === selectedEdge.from)}
    toNode={nodes.find((n) => n.key === selectedEdge.to)}
  />
{/if}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/inspector/InspectorReadonly.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/inspector/InspectorReadonly.svelte packages/web/src/components/inspector/InspectorReadonly.test.ts
git commit -m "feat(web): add read-only inspector dispatcher"
```

---

### Task 7: Edit button on the docked `InspectorPanel`

**Files:**
- Modify: `packages/web/src/components/inspector/InspectorPanel.svelte`
- Test: `packages/web/src/components/inspector/InspectorPanel.test.ts`

**Interfaces:**
- Consumes: `Pencil` icon from `lucide-svelte`.
- Produces: `InspectorPanel` gains an optional prop `onEdit?: () => void`. A header Edit button (icon, `aria-label="Edit element"`) renders whenever `hasSelection` is true and calls `onEdit` on click. All other props unchanged.

- [ ] **Step 1: Write the failing test (append to the existing describe block)**

Add to `packages/web/src/components/inspector/InspectorPanel.test.ts`, inside the `describe("InspectorPanel", …)` block:

```ts
  it("shows an Edit button only when an element is focused, and fires onEdit", async () => {
    const onEdit = vi.fn();
    // Nothing focused → no Edit button.
    const { unmount } = setup({ selectedKey: null, focusedKind: undefined, onEdit });
    expect(screen.queryByRole("button", { name: "Edit element" })).toBeNull();
    unmount();
    // Node focused → Edit button present and wired.
    setup({ focusedKind: "node", onEdit });
    await fireEvent.click(screen.getByRole("button", { name: "Edit element" }));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/inspector/InspectorPanel.test.ts`
Expected: FAIL — no button named "Edit element" found.

- [ ] **Step 3: Implement — import the icon**

In `packages/web/src/components/inspector/InspectorPanel.svelte`, replace:

```svelte
  import { Pin, PinOff, ChevronUp, Box, Spline } from "lucide-svelte";
```

with:

```svelte
  import { Pin, PinOff, ChevronUp, Box, Spline, Pencil } from "lucide-svelte";
```

- [ ] **Step 4: Implement — add the `onEdit` prop**

Replace:

```svelte
  let {
    options,
    selectedKey,
    focusedKind,
    onSelect,
    pinned = false,
    onTogglePin,
    hideDelay = 250,
    width = $bindable(380),
    children,
  }: {
    options: { key: string; label: string }[];
    selectedKey: string | null;
    focusedKind: "node" | "edge" | undefined;
    onSelect: (key: string | null) => void;
    pinned?: boolean;
    onTogglePin: () => void;
    /** Delay (ms) before re-dimming after pointer leaves — avoids flicker. */
    hideDelay?: number;
    width?: number;
    children?: Snippet;
  } = $props();
```

with:

```svelte
  let {
    options,
    selectedKey,
    focusedKind,
    onSelect,
    pinned = false,
    onTogglePin,
    onEdit,
    hideDelay = 250,
    width = $bindable(380),
    children,
  }: {
    options: { key: string; label: string }[];
    selectedKey: string | null;
    focusedKind: "node" | "edge" | undefined;
    onSelect: (key: string | null) => void;
    pinned?: boolean;
    onTogglePin: () => void;
    /** Opens the edit dialog for the currently-focused element. */
    onEdit?: () => void;
    /** Delay (ms) before re-dimming after pointer leaves — avoids flicker. */
    hideDelay?: number;
    width?: number;
    children?: Snippet;
  } = $props();
```

- [ ] **Step 5: Implement — add the Edit button to the header**

In the header, the `<select>…</select>` block is immediately followed by `{#if hasSelection}` for the collapse button. Insert the Edit button between them. Replace:

```svelte
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
```

with:

```svelte
    {#if hasSelection}
      <button
        onclick={onEdit}
        aria-label="Edit element"
        title="Edit element"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <Pencil size={15} />
      </button>
    {/if}
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/inspector/InspectorPanel.test.ts`
Expected: PASS (all original tests + the new one).

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/inspector/InspectorPanel.svelte packages/web/src/components/inspector/InspectorPanel.test.ts
git commit -m "feat(web): add Edit button to docked InspectorPanel"
```

---

### Task 8: Edge branch + preview in `CentralEditPanelHost`

**Files:**
- Modify: `packages/web/src/components/central/CentralEditPanelHost.svelte`
- Test: `packages/web/src/components/central/CentralEditPanelHost.test.ts`

**Interfaces:**
- Consumes: `CentralEditPanel` (Task 2, `fullHeight` + `preview`); `ElementPreview` (Task 3); `RelationshipInspector` (existing); `ModelEdge` from `@waml/okf`.
- Produces:
  - `CentralPanelState` gains `{ kind: "edge"; edgeKey: string }`.
  - Host props become `{ state; nodes: ModelNode[]; edges: ModelEdge[]; display; profileName?; showPreview?: boolean; onUpdateNode; onUpdateEdge: (id, patch) => void; onDisplayChange; onClose }`. `showPreview` defaults `false` (omit the preview — matches the "no active diagram" case); callers with a live diagram pass `showPreview`.
  - Element + edge branches use `fullHeight` and render `ElementPreview` (node/edge mode) via the `preview` snippet only when `showPreview` is true. Diagram branch unchanged. Edge resolves by `edgeKey`; a since-deleted edge closes the panel via an `$effect` mirroring the node guard.

- [ ] **Step 1: Write the failing test — update imports + props helper + add edge tests**

First, in `packages/web/src/components/central/CentralEditPanelHost.test.ts`, add `ModelEdge` to the existing top-of-file okf import. Replace:

```ts
import { DEFAULT_DISPLAY, type ModelNode } from "@waml/okf";
```

with:

```ts
import { DEFAULT_DISPLAY, type ModelNode, type ModelEdge } from "@waml/okf";
```

Next, add an `edge` builder immediately after the existing `node` builder (the `const node = …` block):

```ts
const edge = (id: string, from: string, to: string): ModelEdge =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: {}, bidirectional: false });
```

Then update the `props` helper to include the new props. Replace:

```ts
const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer")],
  display: { ...DEFAULT_DISPLAY },
  profileName: "uml-domain",
  onUpdateNode: vi.fn(),
  onDisplayChange: vi.fn(),
  onClose: vi.fn(),
  ...over,
});
```

with:

```ts
const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer"), node("order", "Order")],
  edges: [edge("e1", "customer", "order")],
  display: { ...DEFAULT_DISPLAY },
  profileName: "uml-domain",
  showPreview: false,
  onUpdateNode: vi.fn(),
  onUpdateEdge: vi.fn(),
  onDisplayChange: vi.fn(),
  onClose: vi.fn(),
  ...over,
});
```

Then append these tests:

```ts
test("edge state mounts the RelationshipInspector titled Relationship", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "e1" } }),
  });
  expect(screen.getByRole("heading", { name: "Relationship" })).toBeTruthy();
  // RelationshipInspector's Kind control is present inside the host.
  expect(screen.getByLabelText("Kind")).toBeTruthy();
});

test("editing an edge calls onUpdateEdge with the edge id", async () => {
  const onUpdateEdge = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "e1" }, onUpdateEdge }),
  });
  await fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "composes" } });
  expect(onUpdateEdge).toHaveBeenCalledWith("e1", { kind: "composes" });
});

test("a since-deleted edge closes the panel", () => {
  const onClose = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "gone" }, onClose }),
  });
  expect(onClose).toHaveBeenCalled();
});

test("showPreview renders the preview region for an element", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, showPreview: true }),
  });
  expect(screen.getByTestId("element-preview")).toBeTruthy();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/central/CentralEditPanelHost.test.ts`
Expected: FAIL — no "Relationship" heading / no "Kind" control (edge branch missing); `element-preview` not found.

- [ ] **Step 3: Implement — rewrite the host**

Replace the entire contents of `packages/web/src/components/central/CentralEditPanelHost.svelte` with:

```svelte
<script module lang="ts">
  // What the central panel is currently editing. `null` means the panel is
  // closed. An element edits one model node's fields; an edge edits one model
  // relationship's fields; a diagram edits the active diagram's display settings.
  export type CentralPanelState =
    | { kind: "element"; nodeKey: string }
    | { kind: "edge"; edgeKey: string }
    | { kind: "diagram" };
</script>

<script lang="ts">
  import type { DiagramDisplay, ModelNode, ModelEdge } from "@waml/okf";
  import CentralEditPanel from "./CentralEditPanel.svelte";
  import ElementPreview from "./ElementPreview.svelte";
  import ObjectInspector from "../inspector/ObjectInspector.svelte";
  import RelationshipInspector from "../inspector/RelationshipInspector.svelte";
  import DiagramPropertiesBody from "../canvas/DiagramPropertiesBody.svelte";

  let {
    state,
    nodes,
    edges,
    display,
    profileName,
    showPreview = false,
    onUpdateNode,
    onUpdateEdge,
    onDisplayChange,
    onClose,
  }: {
    state: CentralPanelState | null;
    nodes: ModelNode[];
    edges: ModelEdge[];
    display: DiagramDisplay;
    profileName?: string;
    /** Render the live cropped preview above the fields. Omit when there is no
     *  active diagram behind the dialog (Navigator's out-of-diagram context). */
    showPreview?: boolean;
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onUpdateEdge: (id: string, patch: Partial<ModelEdge>) => void;
    onDisplayChange: (patch: Partial<DiagramDisplay>) => void;
    onClose: () => void;
  } = $props();

  // Resolve the edited node/edge; a since-deleted key resolves to undefined.
  const node = $derived(
    state?.kind === "element" ? nodes.find((n) => n.key === state.nodeKey) : undefined,
  );
  const edge = $derived(
    state?.kind === "edge" ? edges.find((e) => e.id === state.edgeKey) : undefined,
  );

  // Pointing at a since-deleted key: close instead of showing an empty shell.
  $effect(() => {
    if (state?.kind === "element" && !node) onClose();
  });
  $effect(() => {
    if (state?.kind === "edge" && !edge) onClose();
  });
</script>

{#if state?.kind === "element" && node}
  <CentralEditPanel title={node.concept.title?.trim() || "Untitled"} fullHeight {onClose}>
    {#snippet preview()}
      {#if showPreview}
        <ElementPreview mode="node" focalKey={node.key} {nodes} {edges} {display} profileName={profileName ?? ""} />
      {/if}
    {/snippet}
    <ObjectInspector
      {node}
      onUpdate={(patch) => onUpdateNode(node.key, patch)}
      {profileName}
    />
  </CentralEditPanel>
{:else if state?.kind === "edge" && edge}
  <CentralEditPanel title="Relationship" fullHeight {onClose}>
    {#snippet preview()}
      {#if showPreview}
        <ElementPreview mode="edge" focalKey={edge.id} {nodes} {edges} {display} profileName={profileName ?? ""} />
      {/if}
    {/snippet}
    <RelationshipInspector
      {edge}
      fromNode={nodes.find((n) => n.key === edge.from)}
      toNode={nodes.find((n) => n.key === edge.to)}
      onUpdate={(patch) => onUpdateEdge(edge.id, patch)}
    />
  </CentralEditPanel>
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" {onClose}>
    <DiagramPropertiesBody {display} onChange={onDisplayChange} />
  </CentralEditPanel>
{/if}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/central/CentralEditPanelHost.test.ts`
Expected: PASS (all original tests + the 4 new ones).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/central/CentralEditPanelHost.svelte packages/web/src/components/central/CentralEditPanelHost.test.ts
git commit -m "feat(web): route edge edits + preview through CentralEditPanelHost"
```

---

### Task 9: Wire read-only inspector + edit dialog into the canvas

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Test: `packages/web/src/components/canvas/Canvas.test.ts`

**Interfaces:**
- Consumes: `InspectorReadonly` (Task 6); `InspectorPanel.onEdit` (Task 7); host props `edges`/`onUpdateEdge`/`showPreview` (Task 8); the `focused` derived value + `centralPanel` state (existing).
- Produces: the docked panel shows the read-only body and its Edit button opens the element/edge dialog; the host receives the model edges + `showPreview`.

- [ ] **Step 1: Write/adjust the failing test**

In `packages/web/src/components/canvas/Canvas.test.ts`, replace this block (end of the "picker lists active-diagram member nodes…" test):

```ts
    // Selection round-tripped: the combobox reflects the chosen node, the hint
    // is gone, and the Inspector body now shows that node's title field.
    expect(combobox.value).toBe(node.key);
    expect(within(panel).queryByText(/select an element to edit/i)).toBeNull();
    expect((within(panel).getByLabelText("Title") as HTMLInputElement).value).toBe(node.concept.title);
```

with:

```ts
    // Selection round-tripped: the combobox reflects the chosen node, the hint
    // is gone, and the read-only Inspector body now shows the title as static
    // text (no editable Title input in the docked panel).
    expect(combobox.value).toBe(node.key);
    expect(within(panel).queryByText(/select an element to edit/i)).toBeNull();
    expect(within(panel).queryByLabelText("Title")).toBeNull();
    expect(within(panel).getAllByText(node.concept.title!).length).toBeGreaterThan(0);
```

Then add this test at the end of the `describe("pinnable Inspector …")` block (before its closing `});`):

```ts
  it("the docked panel's Edit button opens the edit dialog for the selected node", async () => {
    const node = store.addNode({ x: 0, y: 0 });
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    const combobox = within(panel).getByRole("combobox", { name: "Select element" }) as HTMLSelectElement;
    await fireEvent.change(combobox, { target: { value: node.key } });
    await tick();

    await fireEvent.click(within(panel).getByRole("button", { name: "Edit element" }));
    await tick();

    // The centered dialog is now open with the EDITABLE ObjectInspector body.
    const dialog = screen.getByRole("dialog");
    expect(dialog).toBeTruthy();
    expect(within(dialog).getByLabelText("Title")).toBeTruthy();
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test src/components/canvas/Canvas.test.ts`
Expected: FAIL — the docked panel still renders an editable Title input (old body), and there is no "Edit element" button yet.

- [ ] **Step 3: Implement — swap the body import**

In `packages/web/src/components/canvas/CanvasInner.svelte`, replace:

```svelte
  import Inspector from "../inspector/Inspector.svelte";
```

with:

```svelte
  import InspectorReadonly from "../inspector/InspectorReadonly.svelte";
```

- [ ] **Step 4: Implement — extend the host props**

Replace:

```svelte
  <CentralEditPanelHost
    state={centralPanel}
    nodes={$model.nodes}
    display={activeDisplay}
    profileName={activeDiagram.profile}
    onUpdateNode={store.updateNode}
    onDisplayChange={handleDisplayChange}
    onClose={() => (centralPanel = null)}
  />
```

with:

```svelte
  <CentralEditPanelHost
    state={centralPanel}
    nodes={$model.nodes}
    edges={$model.edges}
    display={activeDisplay}
    profileName={activeDiagram.profile}
    showPreview
    onUpdateNode={store.updateNode}
    onUpdateEdge={store.updateEdge}
    onDisplayChange={handleDisplayChange}
    onClose={() => (centralPanel = null)}
  />
```

- [ ] **Step 5: Implement — add `onEdit` and swap the docked body**

Replace:

```svelte
    <InspectorPanel
      options={inspectorOptions}
      selectedKey={inspectorSelectedKey}
      focusedKind={inspectorFocusedKind}
      onSelect={(key) => (selectionSet = key ? { nodes: [key], edges: [] } : EMPTY_SELECTION)}
      pinned={inspectorPinned}
      bind:width={inspectorWidth}
      onTogglePin={() => (inspectorPinned = !inspectorPinned)}
    >
      <Inspector
        selection={focused}
        nodes={$model.nodes}
        edges={$model.edges}
        onUpdateNode={store.updateNode}
        onUpdateEdge={store.updateEdge}
        onClose={() => {
          selectionSet = EMPTY_SELECTION;
        }}
        profileName={activeDiagram.profile}
        embedded
      >
        {#snippet externalRefs()}
          {#if focused?.type === "node"}
            <ExternalRefs
              nodeKey={focused.id}
              nodes={$model.nodes}
              edges={$model.edges}
              members={activeDiagram.members}
              diagrams={diagrams}
              onNavigate={(diagramKey, nodeKey) => {
                activeDiagramKey = diagramKey;
                selectionSet = { nodes: [nodeKey], edges: [] };
              }}
            />
          {/if}
        {/snippet}
      </Inspector>
    </InspectorPanel>
```

with:

```svelte
    <InspectorPanel
      options={inspectorOptions}
      selectedKey={inspectorSelectedKey}
      focusedKind={inspectorFocusedKind}
      onSelect={(key) => (selectionSet = key ? { nodes: [key], edges: [] } : EMPTY_SELECTION)}
      pinned={inspectorPinned}
      bind:width={inspectorWidth}
      onTogglePin={() => (inspectorPinned = !inspectorPinned)}
      onEdit={() => {
        if (focused?.type === "node") centralPanel = { kind: "element", nodeKey: focused.id };
        else if (focused?.type === "edge") centralPanel = { kind: "edge", edgeKey: focused.id };
      }}
    >
      <InspectorReadonly
        selection={focused}
        nodes={$model.nodes}
        edges={$model.edges}
      >
        {#snippet externalRefs()}
          {#if focused?.type === "node"}
            <ExternalRefs
              nodeKey={focused.id}
              nodes={$model.nodes}
              edges={$model.edges}
              members={activeDiagram.members}
              diagrams={diagrams}
              onNavigate={(diagramKey, nodeKey) => {
                activeDiagramKey = diagramKey;
                selectionSet = { nodes: [nodeKey], edges: [] };
              }}
            />
          {/if}
        {/snippet}
      </InspectorReadonly>
    </InspectorPanel>
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test src/components/canvas/Canvas.test.ts`
Expected: PASS (updated + new tests green).

- [ ] **Step 7: Run the full web test suite + type check**

Run: `pnpm --filter @waml/web test && pnpm --filter @waml/web check`
Expected: All tests pass; `svelte-check` reports 0 errors. (`Inspector.svelte` is now referenced only by its own test — that is expected and intentionally left in place.)

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.test.ts
git commit -m "feat(web): wire read-only inspector + edit dialog into canvas"
```

---

## Final Verification

- [ ] **Run the whole gate**

Run: `pnpm -r test && pnpm lint && pnpm build`
Expected: all packages' tests pass, ESLint clean, production build (incl. `svelte-check`) succeeds.

- [ ] **Manual smoke (optional, if a dev server is available)**

Run: `pnpm --filter @waml/web dev`, then in the browser:
1. Select a node → docked panel shows read-only fields (no inputs) + an Edit (pencil) button.
2. Click Edit → centered dialog opens tall (`~95vh`), with a live cropped preview (focal node full-opacity, neighbors dimmed) above editable fields; editing the title updates the preview label.
3. Select an edge → Edit opens the edge dialog with a two-node preview and editable relationship fields.
4. Delete the selected element while its dialog is open → dialog closes.
5. Open Diagram properties (Dock) → unchanged size (`85vh`), no preview.

---

## Notes on Resolved Open Questions (spec §Open Questions)

1. **Neighbor-dimming styling** → opacity `0.4` (`"opacity:0.4"` inline style on dimmed nodes + context edges), reusing the app's existing `opacity-40` dim convention rather than the spec's illustrative `0.35`. Focal node(s) full opacity. An edge is full-opacity only when BOTH endpoints are focal — i.e. the focal edge in edge-edit mode; every context/connecting edge (node-edit mode) dims.
2. **`fullHeight` sizing** → `max-h-[95vh]` + forced `h-[95vh]` (so the dialog reads as full height), scrim inset `p-4`. Diagram properties unchanged (`max-h-[85vh]`, `p-8`).
3. **Preview height** → `220px` fixed, with a bottom border. Tune visually later; not a functional dependency.
4. **`showPreview` gating** → explicit boolean host prop, default `false` (omit preview — the "no active diagram" case); `CanvasInner` always passes it (it always has a live diagram). This is the concrete mechanism behind the spec's "render the preview whenever a diagram is active" default.
</content>
</invoke>
