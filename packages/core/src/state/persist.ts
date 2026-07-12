import type { Bundle } from "./model";

// The bundle (the source of truth) lives in memory for the session, so a refresh
// or an accidental tab close would otherwise wipe it. We mirror it into
// localStorage on every change and rehydrate on load as a safety net. Stage 1b
// stores the raw `[path, markdown][]` bundle as JSON — no legacy migration
// (nothing released under the old graph key).
const KEY = "mc.bundle.v1";

// A bundle is a `[path, markdown][]` array. `build_model` throws on any other
// shape, and it runs unguarded at store construction (bootstrap), so a corrupt
// or tampered localStorage value (e.g. `[[1,2]]`) would crash the app on load
// with no recovery. Validate the pair shape here — the untrusted-input boundary —
// and drop anything malformed so bootstrap falls back to an empty model.
function isBundle(value: unknown): value is Bundle {
  return (
    Array.isArray(value) &&
    value.every(
      (entry) =>
        Array.isArray(entry) &&
        entry.length === 2 &&
        typeof entry[0] === "string" &&
        typeof entry[1] === "string",
    )
  );
}

export function loadPersistedBundle(): Bundle | undefined {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return undefined;
    const b = JSON.parse(raw);
    return isBundle(b) ? b : undefined;
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
