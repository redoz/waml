// Owned conceptually by Canvas (the selection state machine lives in
// CanvasInner). The single-focus `Selection` type is what the Inspector consumes;
// the multi-select `SelectionSet` drives the floating toolbar + delete. Both live
// here so Canvas and the Inspector's stub import the same types without a
// cross-import cycle.
export type Selection = { type: "node"; id: string } | { type: "edge"; id: string } | null;

// The full multi-selection: model node keys + model edge ids.
export type SelectionSet = { nodes: string[]; edges: string[] };

export const EMPTY_SELECTION: SelectionSet = { nodes: [], edges: [] };

export function isSelectionEmpty(s: SelectionSet): boolean {
  return s.nodes.length === 0 && s.edges.length === 0;
}

export function selectionCount(s: SelectionSet): number {
  return s.nodes.length + s.edges.length;
}

// A single "focused" element for the Inspector — only when EXACTLY one element
// (of either kind) is selected. Any multi-selection focuses nothing, so the
// Inspector never shows a misleading single-element editor over a set.
export function focusedSelection(s: SelectionSet): Selection {
  if (s.nodes.length === 1 && s.edges.length === 0) return { type: "node", id: s.nodes[0] };
  if (s.edges.length === 1 && s.nodes.length === 0) return { type: "edge", id: s.edges[0] };
  return null;
}

// Shift/Ctrl-click accumulation. `additive=false` (plain click) replaces the set
// with just this element; `additive=true` toggles the element in/out of the set.
export function accumulate(s: SelectionSet, item: { type: "node" | "edge"; id: string }, additive: boolean): SelectionSet {
  if (!additive) {
    return item.type === "node" ? { nodes: [item.id], edges: [] } : { nodes: [], edges: [item.id] };
  }
  if (item.type === "node") {
    const nodes = s.nodes.includes(item.id) ? s.nodes.filter((x) => x !== item.id) : [...s.nodes, item.id];
    return { nodes, edges: s.edges };
  }
  const edges = s.edges.includes(item.id) ? s.edges.filter((x) => x !== item.id) : [...s.edges, item.id];
  return { nodes: s.nodes, edges };
}

// Convert SvelteFlow's `onselectionchange` payload (the marquee / click result)
// into a model-keyed set. ERD mode renders several RF edges per model edge
// ("e1::0"); collapse to the model edge id (prefer `data.modelEdgeId`, else strip
// the "::" suffix) and de-dupe both lists.
export function selectionFromFlow(
  nodes: { id: string }[],
  edges: { id: string; data?: { modelEdgeId?: string } | undefined }[],
): SelectionSet {
  const nodeIds = [...new Set(nodes.map((n) => n.id))];
  const edgeIds = [...new Set(edges.map((e) => e.data?.modelEdgeId ?? e.id.split("::")[0]))];
  return { nodes: nodeIds, edges: edgeIds };
}

// Node ids used to compute the toolbar's bounding box: the selected nodes plus
// the endpoint nodes of any selected edges — so an edges-only selection still has
// a box to anchor to.
export function anchorNodeIds(s: SelectionSet, edges: { id: string; from: string; to: string }[]): string[] {
  const ids = new Set(s.nodes);
  for (const eid of s.edges) {
    const e = edges.find((x) => x.id === eid);
    if (e) {
      ids.add(e.from);
      ids.add(e.to);
    }
  }
  return [...ids];
}

// Delete every selected node and edge. `removeNode` also drops the node's
// incident edges, so removing nodes first can make some edge removals no-ops —
// harmless, since `removeEdge` just filters. Structural store type keeps this
// module free of a store import.
export function deleteSelection(
  store: { removeNode(id: string): void; removeEdge(id: string): void },
  s: SelectionSet,
): void {
  for (const id of s.nodes) store.removeNode(id);
  for (const id of s.edges) store.removeEdge(id);
}
