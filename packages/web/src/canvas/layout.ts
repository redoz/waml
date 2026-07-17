import dagre from "@dagrejs/dagre";
import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
import { erdAwareNodeSize } from "@waml/core/canvas/layoutSize";
import { solve, type SolvedGroup, type FlagSet, type Diagnostic } from "@waml/wasm";

// ── Dagre auto-layout ────────────────────────────────────────────────────────
// Shared with Plan 3a (Canvas): the OKF format does not persist node positions,
// so freshly loaded / templated graphs are laid out here on load, and the
// "auto-layout" tool re-runs it on demand.
export const NODE_W = 200;
export const NODE_H = 90;

export function runDagreLayout(
  nodes: ModelNode[],
  edges: ModelEdge[],
  display: DiagramDisplay,
): Map<string, { x: number; y: number }> {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 60, ranksep: 150 });
  nodes.forEach((n) => {
    const s = erdAwareNodeSize(n, display);
    g.setNode(n.key, { width: s.width, height: s.height });
  });
  edges.forEach((e) => g.setEdge(e.from, e.to));
  dagre.layout(g);
  const positions = new Map<string, { x: number; y: number }>();
  nodes.forEach((n) => {
    const pos = g.node(n.key);
    const s = erdAwareNodeSize(n, display);
    positions.set(n.key, { x: pos.x - s.width / 2, y: pos.y - s.height / 2 });
  });
  return positions;
}

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
