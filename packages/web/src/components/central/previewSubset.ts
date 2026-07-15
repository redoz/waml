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
