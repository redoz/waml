import dagre from "@dagrejs/dagre";
import type { ModelNode, ModelEdge } from "@uaml/okf";
import type { ViewMode } from "@uaml/core/state/viewMode";
import { erdAwareNodeSize } from "@uaml/core/canvas/layoutSize";

// ── Dagre auto-layout ────────────────────────────────────────────────────────
// Shared with Plan 3a (Canvas): the OKF format does not persist node positions,
// so freshly loaded / templated graphs are laid out here on load, and the
// "auto-layout" tool re-runs it on demand.
export const NODE_W = 200;
export const NODE_H = 90;

export function runDagreLayout(
  nodes: ModelNode[],
  edges: ModelEdge[],
  viewMode: ViewMode,
): Map<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 60, ranksep: 150 });
  nodes.forEach((n) => {
    const s = erdAwareNodeSize(n, viewMode);
    g.setNode(n.key, { width: s.width, height: s.height });
  });
  edges.forEach((e) => g.setEdge(e.from, e.to));
  dagre.layout(g);
  const positions = new Map<string, { x: number; y: number }>();
  nodes.forEach((n) => {
    const pos = g.node(n.key);
    const s = erdAwareNodeSize(n, viewMode);
    positions.set(n.key, { x: pos.x - s.width / 2, y: pos.y - s.height / 2 });
  });
  return positions;
}
