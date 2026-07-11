export type ViewMode = "compact" | "erd";

const KEY = "mc.viewMode.v1";

export function loadViewMode(): ViewMode {
  try {
    return localStorage.getItem(KEY) === "erd" ? "erd" : "compact";
  } catch {
    return "compact";
  }
}

export function persistViewMode(mode: ViewMode): void {
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    // best-effort; ignore quota / private-mode failures
  }
}
