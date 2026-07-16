import { DEFAULT_DISPLAY } from "@waml/okf";
import { createModelStore, type Bundle } from "@waml/core/state/model";
import { loadPersistedBundle, persistBundle } from "@waml/core/state/persist";
import { readSharedModel, clearSharedModelFromUrl } from "@waml/core/share/url";
import { readTemplateModel, clearTemplateFromUrl } from "@waml/core/lib/templateLink";
import { runDagreLayout } from "../canvas/layout";

// NOTE: `initWasm()` is awaited by the app entry (`main.ts`) BEFORE this module is
// imported, so `build_model`/`apply_ops` are callable synchronously here. This
// module must not be imported before that init resolves (tests init the wasm
// module explicitly before importing bootstrap).

// ── store singleton (exported so the app + bridge modules share this instance) ─
// Precedence: a `?template=<id>` deep-link and a `#m=…` share link are both
// explicit "open this model" intents, so they win over localStorage; otherwise
// rehydrate from localStorage so a refresh doesn't wipe work. Each source now
// yields a BUNDLE (`[path, markdown][]`).
const templateBundle = readTemplateModel();
clearTemplateFromUrl(); // strip the param (clean URL on refresh) even if the id was unknown

const sharedBundle = readSharedModel();
const persistedBundle = loadPersistedBundle();

const initialBundle: Bundle = templateBundle ?? sharedBundle ?? persistedBundle ?? [];

// The store error surface (a rejected `apply_ops`) is fanned out to subscribers;
// the canvas wires one to a toast (see CanvasInner). Kept here so the singleton
// owns it.
type ErrHandler = (error: string) => void;
const errorHandlers = new Set<ErrHandler>();
export function onStoreError(h: ErrHandler): () => void {
  errorHandlers.add(h);
  return () => errorHandlers.delete(h);
}

export const store = createModelStore(initialBundle, {
  onError: (e) => errorHandlers.forEach((h) => h(e)),
});

// The OKF bundle carries no node positions, so Dagre-lay the derived model out
// and feed the positions into the store's overlay (bundle text stays untouched).
{
  const g = store.get();
  const positions = runDagreLayout(g.nodes, g.edges, DEFAULT_DISPLAY);
  positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
}

if (templateBundle || sharedBundle) {
  // Persist the opened model right away — it's the store's initial value, so it
  // never fires a change that the mirror-to-localStorage effect would catch; a
  // refresh would otherwise lose it once the URL is cleaned.
  persistBundle(store.getBundle());
}
// Drop the share payload from the address bar so a refresh doesn't re-clobber the
// canvas and the URL stays clean (the template param is already cleared above).
if (sharedBundle) clearSharedModelFromUrl();

// A truly first-ever visit has no template deep-link, no persisted model and no
// shared link. Captured at module load — before any persist effect writes an
// (empty) bundle — so it stays true for the session. Gates the first-screen
// "start" chooser: shown once for new visitors, never over an opened model.
export const isFirstVisit = !templateBundle && !sharedBundle && persistedBundle === undefined;
