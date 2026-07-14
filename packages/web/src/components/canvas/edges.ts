import { Position, type Edge } from "@xyflow/svelte";
import type { ModelNode, ModelEdge, DiagramDisplay } from "@waml/okf";
import { erdAwareNodeSize } from "@waml/core/canvas/layoutSize";
import { oppositeSide, type Slot } from "./floating";

// The rendered edge attaches nowhere fixed: RelEdge computes a floating attach
// point on each node's border facing its neighbour (see floating.ts). Here we
// pre-assign, per end, which SIDE it exits and its SLOT within the group of
// edges sharing that (node, side) — so a hub's many edges fan out and space
// themselves along each border instead of stacking on one point. No
// sourceHandle/targetHandle is pinned; the hover connect-dots stay implicit.
type EndPlacement = { sourceSide: Position; targetSide: Position; sourceSlot: Slot; targetSlot: Slot };

function compactEdge(e: ModelEdge, place: EndPlacement | undefined, associationLabels: DiagramDisplay["associationLabels"], emphasizeMultiplicity: boolean): Edge {
  return {
    id: e.id, source: e.from, target: e.to,
    type: "rel",
    // RelEdge reads `associationLabels` (show/hide) + `emphasizeMultiplicity`.
    data: { kind: e.kind, fromEnd: e.fromEnd, toEnd: e.toEnd, bidirectional: e.bidirectional, modelEdgeId: e.id, associationLabels, emphasizeMultiplicity, ...place } as unknown as Record<string, unknown>,
  };
}

// Reconnect is scoped to the SELECTED relationship only (overlapping anchors).
export function isEdgeReconnectable(modelEdgeId: string | undefined, selectedEdgeId: string | null): boolean {
  return modelEdgeId != null && modelEdgeId === selectedEdgeId;
}

// Decide each edge's exit side (from node centres, 4-way) and its ordered slot
// within the group of edges leaving the same node on the same side. Ordering is
// by the OTHER end's position along that side's varying axis, so the fan spreads
// without crossing itself. Uses the model geometry (positions + estimated size);
// RelEdge then places the actual point on the live measured border.
function planPlacements(edges: ModelEdge[], nodes: ModelNode[], display: DiagramDisplay): Map<string, EndPlacement> {
  const byKey = new Map(nodes.map(n => [n.key, n]));
  const center = (n: ModelNode) => { const s = erdAwareNodeSize(n, display); return { x: n.position.x + s.width / 2, y: n.position.y + s.height / 2 }; };
  const isVertical = (side: Position) => side === Position.Left || side === Position.Right;

  // First pass: sides + the sort key (other end's coordinate along the side).
  type Row = { id: string; from: string; to: string; sourceSide: Position; targetSide: Position; sKey: number; tKey: number };
  const rows: Row[] = [];
  for (const e of edges) {
    const src = byKey.get(e.from); const tgt = byKey.get(e.to);
    if (!src || !tgt) continue;
    const sc = center(src); const tc = center(tgt);
    const dx = tc.x - sc.x; const dy = tc.y - sc.y;
    const sourceSide = Math.abs(dx) >= Math.abs(dy) ? (dx >= 0 ? Position.Right : Position.Left) : (dy >= 0 ? Position.Bottom : Position.Top);
    const targetSide = oppositeSide[sourceSide];
    rows.push({
      id: e.id, from: e.from, to: e.to, sourceSide, targetSide,
      sKey: isVertical(sourceSide) ? tc.y : tc.x, // order this source-side group by where the target sits
      tKey: isVertical(targetSide) ? sc.y : sc.x,
    });
  }

  // Second pass: bucket by (node, side), sort, assign slot index/count.
  const buckets = new Map<string, { id: string; key: number; end: "s" | "t" }[]>();
  const push = (nodeKey: string, side: Position, id: string, key: number, end: "s" | "t") => {
    const k = `${nodeKey}|${side}`;
    (buckets.get(k) ?? buckets.set(k, []).get(k)!).push({ id, key, end });
  };
  for (const r of rows) {
    push(r.from, r.sourceSide, r.id, r.sKey, "s");
    push(r.to, r.targetSide, r.id, r.tKey, "t");
  }
  const slotOf = new Map<string, Slot>(); // `${id}|${end}` -> slot
  for (const list of buckets.values()) {
    list.sort((a, b) => a.key - b.key || a.id.localeCompare(b.id));
    list.forEach((item, index) => slotOf.set(`${item.id}|${item.end}`, { index, count: list.length }));
  }

  const out = new Map<string, EndPlacement>();
  for (const r of rows) {
    out.set(r.id, {
      sourceSide: r.sourceSide, targetSide: r.targetSide,
      sourceSlot: slotOf.get(`${r.id}|s`) ?? { index: 0, count: 1 },
      targetSlot: slotOf.get(`${r.id}|t`) ?? { index: 0, count: 1 },
    });
  }
  return out;
}

// Builds one 'rel' edge per model edge, threading the active diagram's resolved
// display (association-label visibility + multiplicity emphasis) into edge data,
// and sizing nodes for placement via the same display.
export function buildRfEdges(edges: ModelEdge[], nodes: ModelNode[], display: DiagramDisplay): Edge[] {
  const placements = planPlacements(edges, nodes, display);
  return edges.map(e => compactEdge(e, placements.get(e.id), display.associationLabels, display.emphasizeMultiplicity));
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
