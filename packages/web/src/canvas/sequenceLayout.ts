import type { Lifeline, SeqItem, SeqOperand, SequenceDoc } from "@waml/okf";

// ── Sequence substrate layout (behavioral substrates spec) ───────────────────
// Purely deterministic: document order fixes row Y, lifeline declaration order
// fixes column X. No constraint solving — ordered lifelines/messages ARE the
// layout, per the spec's "self-rendering" design principle.

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
  | { kind: "message"; y: number; fromX: number; toX: number; item: Extract<SeqItem, { item: "message" }>; self: boolean }
  | { kind: "fragmentStart"; y: number; depth: number; label: string; x0: number; x1: number }
  | { kind: "fragmentEnd"; y: number; depth: number; x0: number; x1: number }
  | { kind: "operandDivider"; y: number; depth: number; label?: string; x0: number; x1: number };

export interface SequenceLayout {
  lifelines: LaneLayout[];
  rows: SeqRow[];
  width: number;
  height: number;
}

function laneHandle(l: Lifeline): string {
  return l.alias ?? l.title;
}

export function layoutSequence(doc: SequenceDoc): SequenceLayout {
  const lifelines: LaneLayout[] = doc.lifelines.map((l, i) => ({
    key: l.ref ?? laneHandle(l),
    handle: laneHandle(l),
    x: LANE_MARGIN + i * LANE_WIDTH,
  }));
  const xOf = (handle: string): number => lifelines.find((l) => l.handle === handle)?.x ?? LANE_MARGIN;
  // A fragment spans every lane touched by messages inside it (min..max),
  // padded so its frame clears the endpoints.
  const bounds = (items: SeqItem[]): [number, number] => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const it of items) {
      if (it.item === "message") {
        lo = Math.min(lo, xOf(it.from), xOf(it.to));
        hi = Math.max(hi, xOf(it.from), xOf(it.to));
      } else {
        for (const op of it.operands) {
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

  const walk = (items: SeqItem[], depth: number): void => {
    for (const it of items) {
      if (it.item === "message") {
        const fromX = xOf(it.from);
        const toX = xOf(it.to);
        rows.push({ kind: "message", y, fromX, toX, item: it, self: fromX === toX });
        y += ROW_HEIGHT;
      } else {
        const [lo, hi] = bounds([it]);
        const x0 = lo - 30 - depth * 12;
        const x1 = hi + 30 + depth * 12;
        rows.push({ kind: "fragmentStart", y, depth, label: it.kind, x0, x1 });
        y += FRAGMENT_HEADER_HEIGHT;
        it.operands.forEach((op: SeqOperand, i: number) => {
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
  walk(doc.messages, 0);

  const width = lifelines.length > 0 ? Math.max(...lifelines.map((l) => l.x)) + LANE_MARGIN : LANE_MARGIN * 2;
  return { lifelines, rows, width, height: y + 40 };
}
