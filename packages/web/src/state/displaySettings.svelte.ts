import { resolveDisplay, type DiagramDisplay } from "@uaml/okf";

// Session-level per-diagram display overrides. The model store's diagram
// mutations are no-ops in Stage 1b (`store.updateDiagram` never persists) and
// the OKF wire carries no `display` block, so per-diagram render settings can't
// live in the model yet. Until Stage 1c diagram editing + OKF `display`
// serialization land, we hold them in memory for the browser session — spec
// lines 108-112: "a session-level default that does not force diagram creation".
// Keyed by diagram key so the implicit "All" diagram and real diagrams keep
// independent settings; resets on reload (NOT the per-browser localStorage
// preference the spec set out to remove).
let overrides = $state<Record<string, Partial<DiagramDisplay>>>({});

export const displaySettings = {
  // DEFAULT_DISPLAY < the diagram's own persisted display (`base`, currently
  // always absent) < the session override. Reads `overrides` so a $derived
  // caller re-runs when it changes.
  resolve(key: string, base?: Partial<DiagramDisplay>): DiagramDisplay {
    return resolveDisplay({ ...base, ...overrides[key] });
  },
  // Merge a single-field patch into a diagram's override (immutable reassign so
  // the $state dependency re-fires).
  patch(key: string, p: Partial<DiagramDisplay>): void {
    overrides = { ...overrides, [key]: { ...overrides[key], ...p } };
  },
  // Test-only: clear all session overrides.
  _reset(): void {
    overrides = {};
  },
};
