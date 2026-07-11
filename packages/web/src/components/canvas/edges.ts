import type { Edge } from "@xyflow/react";
import type { ModelNode, ModelEdge } from "@mc/okf";
import type { ViewMode } from "../../state/viewMode";
import type { RelLabelMode } from "../../state/relLabels";
import { erdAwareNodeSize } from "./layoutSize";

type Side = "left" | "right";

// Pick the horizontal side each end of an edge attaches to.
//
// A handle the user explicitly chose (stored on the edge from a manual drag)
// always wins, so their choice is preserved. Otherwise the side is derived from
// the nodes' relative position: each end exits *toward* the other node — the
// shortest route, no loop-around. The SAME rule runs in compact and ERD mode, so
// the side never jumps when toggling views (imported/template edges carry no
// stored handle, which is exactly the case that used to disagree between modes).
// A hub's edges naturally split across both sides because each one faces its own
// neighbour.
function edgeSides(
  src: ModelNode | undefined,
  tgt: ModelNode | undefined,
  e: ModelEdge,
  viewMode: ViewMode,
): { source: Side; target: Side } {
  let source: Side = "right";
  let target: Side = "left";
  if (src && tgt) {
    const sx = src.position.x + erdAwareNodeSize(src, viewMode).width / 2;
    const tx = tgt.position.x + erdAwareNodeSize(tgt, viewMode).width / 2;
    if (tx < sx) { source = "left"; target = "right"; }
  }
  const storedSource = e.sourceHandle === "left" || e.sourceHandle === "right" ? e.sourceHandle : null;
  const storedTarget = e.targetHandle === "left" || e.targetHandle === "right" ? e.targetHandle : null;
  return { source: storedSource ?? source, target: storedTarget ?? target };
}

function compactEdge(e: ModelEdge, sides: { source: Side; target: Side }, relLabelMode: RelLabelMode): Edge {
  return {
    id: e.id, source: e.from, target: e.to,
    sourceHandle: sides.source, targetHandle: sides.target,
    type: "rel",
    data: { kind: e.kind, fromEnd: e.fromEnd, toEnd: e.toEnd, bidirectional: e.bidirectional, modelEdgeId: e.id, relLabelMode } as unknown as Record<string, unknown>,
  };
}

// Reconnect is scoped to the SELECTED relationship only (overlapping anchors).
export function isEdgeReconnectable(modelEdgeId: string | undefined, selectedEdgeId: string | null): boolean {
  return modelEdgeId != null && modelEdgeId === selectedEdgeId;
}

export function buildRfEdges(edges: ModelEdge[], nodes: ModelNode[], viewMode: ViewMode, relLabelMode: RelLabelMode = "all"): Edge[] {
  const byKey = new Map(nodes.map(n => [n.key, n]));
  return edges.map(e => compactEdge(e, edgeSides(byKey.get(e.from), byKey.get(e.to), e, viewMode), relLabelMode));
}

// Synthesise the dashed connectors that tie annotation elements to what they
// annotate: an association-class box to the association line it names, and a
// uml.Note to each element it annotates. RF edges attach only to nodes, so an
// edge-midpoint anchor is approximated by anchoring to the association's source
// node. Endpoints that reference missing nodes are dropped (never error).
export function buildAnchorEdges(nodes: ModelNode[], edges: ModelEdge[]): Edge[] {
  const has = new Set(nodes.map(n => n.key));
  const out: Edge[] = [];
  const anchor = (id: string, source: string, target: string): void => {
    if (has.has(source) && has.has(target)) out.push({ id, source, target, type: "anchor", selectable: false });
  };
  // Association class → the association line it names (approx: the association's source node).
  for (const e of edges) {
    if (e.name && typeof e.name === "object") anchor(`ac-${e.id}`, e.name.ref, e.from);
  }
  // uml.Note → each annotated element.
  for (const n of nodes) {
    if (n.type !== "uml.Note") continue;
    (n.annotates ?? []).forEach((a, i) => {
      const target = "targetKey" in a && !("sourceKey" in a) ? a.targetKey : (a as { sourceKey: string }).sourceKey;
      anchor(`note-${n.key}-${i}`, n.key, target);
    });
  }
  return out;
}
