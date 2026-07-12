import type { Bundle } from "./model";

// The bundle (the source of truth) lives in memory for the session, so a refresh
// or an accidental tab close would otherwise wipe it. We mirror it into
// localStorage on every change and rehydrate on load as a safety net. Stage 1b
// stores the raw `[path, markdown][]` bundle as JSON — no legacy migration
// (nothing released under the old graph key).
const KEY = "mc.bundle.v1";

export function loadPersistedBundle(): Bundle | undefined {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return undefined;
    const b = JSON.parse(raw);
    return Array.isArray(b) ? (b as Bundle) : undefined;
  } catch {
    return undefined;
  }
}

export function persistBundle(b: Bundle): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(b));
  } catch {
    // Ignore quota / private-mode failures — persistence is best-effort.
  }
}
