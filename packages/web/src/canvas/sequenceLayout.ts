import type { SeqChild, SeqEdge, SeqNode, SequenceDoc } from "@waml/okf";

// ── Sequence substrate layout (behavioral substrates spec) ───────────────────
// Purely deterministic: document order fixes row Y, lifeline declaration order
// fixes column X. No constraint solving — the flat interaction model (lifeline
// nodes, message edges, and the ordered `items` stream) IS the layout.

const LANE_WIDTH = 170;
const LANE_MARGIN = 90;
const ROW_HEIGHT = 46;
const FRAGMENT_HEADER_HEIGHT = 26;

export interface LaneLayout {
  key: string;
  handle: string;
  x: number;
}

export type SeqRow =
  | { kind: "message"; y: number; fromX: number; toX: number; edge: SeqEdge; self: boolean }
  | { kind: "fragmentStart"; y: number; depth: number; label: string; x0: number; x1: number }
  | { kind: "fragmentEnd"; y: number; depth: number; x0: number; x1: number }
  | { kind: "operandDivider"; y: number; depth: number; label?: string; x0: number; x1: number };

export interface SequenceLayout {
  lifelines: LaneLayout[];
  rows: SeqRow[];
  width: number;
  height: number;
}

export function layoutSequence(doc: SequenceDoc): SequenceLayout {
  const edgeById = new Map<string, SeqEdge>(doc.edges.map((e): [string, SeqEdge] => [e.id, e]));
  const nodeById = new Map<string, SeqNode>(doc.nodes.map((n): [string, SeqNode] => [n.id, n]));

  const lifelines: LaneLayout[] = doc.nodes
    .filter((n): n is Extract<SeqNode, { node: "lifeline" }> => n.node === "lifeline")
    .map((l, i) => ({ key: l.ref ?? l.id, handle: l.id, x: LANE_MARGIN + i * LANE_WIDTH }));
  const xOf = (id: string): number => lifelines.find((l) => l.handle === id)?.x ?? LANE_MARGIN;

  // A fragment spans every lane touched by messages inside it (min..max),
  // padded so its frame clears the endpoints.
  const bounds = (items: SeqChild[]): [number, number] => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const c of items) {
      if (c.item === "message") {
        const e = edgeById.get(c.edge);
        if (!e) continue;
        lo = Math.min(lo, xOf(e.from), xOf(e.to));
        hi = Math.max(hi, xOf(e.from), xOf(e.to));
      } else {
        const frag = nodeById.get(c.node);
        if (!frag || frag.node !== "fragment") continue;
        for (const oid of frag.operands) {
          const op = nodeById.get(oid);
          if (!op || op.node !== "operand") continue;
          const [a, b] = bounds(op.items);
          lo = Math.min(lo, a);
          hi = Math.max(hi, b);
        }
      }
    }
    return lo === Infinity ? [LANE_MARGIN, LANE_MARGIN] : [lo, hi];
  };

  const rows: SeqRow[] = [];
  let y = 60;

  const walk = (items: SeqChild[], depth: number): void => {
    for (const c of items) {
      if (c.item === "message") {
        const e = edgeById.get(c.edge);
        if (!e) continue;
        const fromX = xOf(e.from);
        const toX = xOf(e.to);
        rows.push({ kind: "message", y, fromX, toX, edge: e, self: fromX === toX });
        y += ROW_HEIGHT;
      } else {
        const frag = nodeById.get(c.node);
        if (!frag || frag.node !== "fragment") continue;
        const [lo, hi] = bounds([c]);
        const x0 = lo - 30 - depth * 12;
        const x1 = hi + 30 + depth * 12;
        rows.push({ kind: "fragmentStart", y, depth, label: frag.kind, x0, x1 });
        y += FRAGMENT_HEADER_HEIGHT;
        frag.operands.forEach((oid, i) => {
          const op = nodeById.get(oid);
          if (!op || op.node !== "operand") return;
          if (i > 0) {
            rows.push({ kind: "operandDivider", y, depth, label: op.guard, x0, x1 });
            y += 20;
          }
          walk(op.items, depth + 1);
        });
        rows.push({ kind: "fragmentEnd", y, depth, x0, x1 });
        y += 14;
      }
    }
  };
  walk(doc.items, 0);

  const width = lifelines.length > 0 ? Math.max(...lifelines.map((l) => l.x)) + LANE_MARGIN : LANE_MARGIN * 2;
  return { lifelines, rows, width, height: y + 40 };
}
