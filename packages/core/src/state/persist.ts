import type { ModelGraph } from "@uaml/okf";
import { migrateGraph } from "@uaml/okf";

// The whole model lives in memory for the session, so a refresh or an
// accidental tab close would otherwise wipe it. We mirror it into localStorage
// on every change and rehydrate on load as a safety net. Legacy (mart-era)
// payloads are migrated on read so old saves keep opening.
const KEY = "mc.model.v1";

export function loadPersistedGraph(): ModelGraph | undefined {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return undefined;
    return migrateGraph(JSON.parse(raw)) ?? undefined;
  } catch {
    return undefined;
  }
}

export function persistGraph(g: ModelGraph): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(g));
  } catch {
    // Ignore quota / private-mode failures — persistence is best-effort.
  }
}
