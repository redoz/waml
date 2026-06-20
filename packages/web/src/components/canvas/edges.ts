import type { Edge } from "@xyflow/react";
import type { ModelNode, ModelEdge } from "@mc/okf";
import type { ViewMode } from "../../state/viewMode";

function compactEdge(e: ModelEdge): Edge {
  return {
    id: e.id,
    source: e.from,
    target: e.to,
    sourceHandle: e.sourceHandle ?? undefined,
    targetHandle: e.targetHandle ?? undefined,
    type: "rel",
    data: { keys: e.keys, bidirectional: e.bidirectional, modelEdgeId: e.id } as unknown as Record<string, unknown>,
  };
}

export function buildRfEdges(edges: ModelEdge[], nodes: ModelNode[], viewMode: ViewMode): Edge[] {
  if (viewMode !== "erd") return edges.map(compactEdge);

  const fieldsByKey = new Map<string, Set<string>>(
    nodes.map(n => [n.key, new Set(n.schema.map(f => f.name))]),
  );

  return edges.flatMap(e => {
    const usable = e.keys.filter(k => k.left || k.right);
    if (usable.length === 0) return [compactEdge(e)];

    const srcFields = fieldsByKey.get(e.from);
    const tgtFields = fieldsByKey.get(e.to);

    return usable.map((k, i): Edge => ({
      id: `${e.id}::${i}`,
      source: e.from,
      target: e.to,
      sourceHandle: k.left && srcFields?.has(k.left) ? `fr:${k.left}` : "right",
      targetHandle: k.right && tgtFields?.has(k.right) ? `fl:${k.right}` : "left",
      type: "rel",
      data: { keys: [k], bidirectional: e.bidirectional, modelEdgeId: e.id } as unknown as Record<string, unknown>,
    }));
  });
}
