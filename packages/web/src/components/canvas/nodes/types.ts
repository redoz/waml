import type { ModelNode, DiagramDisplay } from "@waml/okf";

// The `data` payload shape SvelteFlow nodes carry — set by `toRFNode`. `_display`
// is the active diagram's resolved render settings (per-diagram, replacing the old
// global `_viewMode`).
export type OkfNodeData = ModelNode & { _display?: DiagramDisplay; _profile?: string; _collapsed?: boolean };

export const NODE_FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif";
