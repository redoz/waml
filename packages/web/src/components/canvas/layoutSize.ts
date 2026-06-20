import type { ModelNode } from "@mc/okf";
import type { ViewMode } from "../../state/viewMode";

const COMPACT = { width: 200, height: 90 };
const ERD_WIDTH = 250;
const ERD_HEADER = 66; // header + type-chip row
const ERD_ROW = 24;

export function erdAwareNodeSize(node: ModelNode, viewMode: ViewMode): { width: number; height: number } {
  if (viewMode !== "erd") return { ...COMPACT };
  const rows = Math.max(node.schema.length, 1);
  return { width: ERD_WIDTH, height: ERD_HEADER + rows * ERD_ROW };
}
