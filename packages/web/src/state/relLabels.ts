// What relationship-edge labels show on the canvas (multiplicities/roles). A
// per-browser view preference — persisted in localStorage, mirroring viewMode.
export type RelLabelMode = "all" | "hidden";

const KEY = "mc.relLabels.v1";

export function loadRelLabelMode(): RelLabelMode {
  try {
    // Legacy modes ("defined"/"undefined") were join-key concepts; coerce to "all".
    return localStorage.getItem(KEY) === "hidden" ? "hidden" : "all";
  } catch {
    return "all";
  }
}

export function persistRelLabelMode(mode: RelLabelMode): void {
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    // best-effort; ignore quota / private-mode failures
  }
}
