import type { ModelNode, DiagramDisplay } from "@uaml/okf";

const COMPACT = { width: 200, height: 90 };
const ERD_WIDTH = 250;
const ERD_HEADER = 66; // header + type-chip row
const ERD_ROW = 24;
const ERD_EXPAND_ROW = 26; // "show N more / less" toggle row

// ERD nodes show at most this many field rows by default; the rest collapse
// behind an expand toggle. Keeps dense nodes from turning the canvas into a wall
// of fields. Layout always sizes to the COLLAPSED height so the default picture
// is tidy (an expanded node may overlap below until the user re-runs layout).
export const ERD_COLLAPSED_ROWS = 4;

// Node footprint depends on the active diagram's resolved display: when attributes
// are hidden the box is the fixed compact size; when shown it grows with the
// (capped) field/value count so dagre can reserve room for the rows.
export function erdAwareNodeSize(node: ModelNode, display: DiagramDisplay): { width: number; height: number } {
  if (!display.showAttributes) return { ...COMPACT };
  const total = node.values ? node.values.length : node.attributes.length;
  const rows = Math.max(Math.min(total, ERD_COLLAPSED_ROWS), 1);
  const expandRow = total > ERD_COLLAPSED_ROWS ? ERD_EXPAND_ROW : 0;
  return { width: ERD_WIDTH, height: ERD_HEADER + rows * ERD_ROW + expandRow };
}
