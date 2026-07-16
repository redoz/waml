import dagre from "@dagrejs/dagre";
import type { Edge, Node } from "@xyflow/svelte";
import type { FlowDoc, FlowEdge, FlowNode } from "@waml/okf";

// ── Flow substrate rendering (behavioral substrates spec) ────────────────────
// A flow document is self-rendering: dagre lays the directed graph out at
// render time (relational, never coordinates — nothing is stored).

export function flowNodeSize(n: FlowNode): { width: number; height: number } {
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

const KIND_TO_TYPE: Record<FlowNode["kind"], string> = {
  plain: "flowStep",
  object: "flowObject",
  initial: "flowControl",
  final: "flowControl",
  decision: "flowControl",
  merge: "flowControl",
  fork: "flowControl",
  join: "flowControl",
};

export function flowToRf(doc: FlowDoc): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
  for (const n of doc.nodes) {
    const s = flowNodeSize(n);
    g.setNode(n.id, { width: s.width, height: s.height });
  }
  const local = new Set(doc.nodes.map((n) => n.id));
  for (const e of doc.edges) if (local.has(e.from) && local.has(e.to)) g.setEdge(e.from, e.to);
  dagre.layout(g);

  const nodes: Node[] = doc.nodes.map((n) => {
    const s = flowNodeSize(n);
    const pos = g.node(n.id);
    return {
      id: n.id,
      type: KIND_TO_TYPE[n.kind],
      position: { x: (pos?.x ?? 0) - s.width / 2, y: (pos?.y ?? 0) - s.height / 2 },
      data: { node: n, flavor: doc.flavor } as unknown as Record<string, unknown>,
      draggable: false,
      connectable: false,
      selectable: false,
    };
  });
  const kindById = new Map(doc.nodes.map((n) => [n.id, n.kind]));
  const edges: Edge[] = doc.edges
    .filter((e) => local.has(e.from) && local.has(e.to))
    .map((e, i) => ({
      id: `t${i}`,
      source: e.from,
      target: e.to,
      type: "transition",
      // flavor picks the path shape; fromKind lets a decision source snap to a tip.
      data: { label: transitionLabel(e), carries: e.carries, flavor: doc.flavor, fromKind: kindById.get(e.from) } as unknown as Record<string, unknown>,
      selectable: false,
    }));
  return { nodes, edges };
}
