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
