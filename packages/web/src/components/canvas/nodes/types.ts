import type { ModelNode, DiagramDisplay } from "@waml/okf";

// The `data` payload shape SvelteFlow nodes carry — set by `toRFNode`. `_display`
// is the active diagram's resolved render settings (per-diagram, replacing the old
// global `_viewMode`).
export type OkfNodeData = ModelNode & { _display?: DiagramDisplay; _profile?: string; _collapsed?: boolean };

export const NODE_FONT = "'IBM Plex Mono', ui-monospace, 'SF Mono', 'Cascadia Code', Menlo, monospace";

/** Profile stereotype colors are hex (`#eab308`); Atlas needs an rgb TRIPLE so
 *  a node can self-theme via style="--accent:<r,g,b>". Accepts #RGB or #RRGGBB;
 *  anything else (or undefined) falls back to the default blue triple. */
export function hexToTriple(hex?: string): string {
  const DEFAULT = "20, 150, 220";
  if (!hex) return DEFAULT;
  let h = hex.trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{3}$/.test(h)) h = h.split("").map((c) => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return DEFAULT;
  const n = parseInt(h, 16);
  return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
}
