import type { ModelNode } from "@uaml/okf";
import type { ViewMode } from "@uaml/core/state/viewMode";

// Mirrors packages/web/src/components/canvas/nodes/shared.tsx L9-L12.
// The `data` payload shape SvelteFlow nodes carry — set by `toRFNode` (a later task).
export type OkfNodeData = ModelNode & { _viewMode?: ViewMode; _profile?: string; _collapsed?: boolean };

export const NODE_FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif";
