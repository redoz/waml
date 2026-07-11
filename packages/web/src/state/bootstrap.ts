import type { ModelGraph } from "@uaml/okf";
import { createModelStore } from "@uaml/core/state/model";
import { loadPersistedGraph, persistGraph } from "@uaml/core/state/persist";
import { loadViewMode } from "@uaml/core/state/viewMode";
import { readSharedModel, readSharedName, clearSharedModelFromUrl } from "@uaml/core/share/url";
import { readTemplateModel, clearTemplateFromUrl } from "@uaml/core/lib/templateLink";
import { runDagreLayout } from "../canvas/layout";

// ── store singleton (exported so the app + bridge modules share this instance) ─
// Precedence: a `?template=<id>` deep-link and a `#m=…` share link are both
// explicit "open this model" intents, so they win over localStorage; otherwise
// rehydrate from localStorage so a refresh doesn't wipe work.
//
// `?template=<id>` opens a named built-in template (the CTA target for the blog
// gallery, launch emails and posts). Templates ship at (0,0), so we Dagre-lay it
// out here.
const templateGraph = readTemplateModel();
clearTemplateFromUrl(); // strip the param (clean URL on refresh) even if the id was unknown
let templateInitial: ModelGraph | undefined;
if (templateGraph) {
  const positions = runDagreLayout(templateGraph.nodes, templateGraph.edges, loadViewMode());
  templateInitial = {
    ...templateGraph,
    nodes: templateGraph.nodes.map((n) => ({ ...n, position: positions.get(n.key) ?? n.position })),
  };
}

const sharedGraph = readSharedModel();
export const sharedModelName = readSharedName(); // name carried alongside a shared link, if any
const persistedGraph = loadPersistedGraph();
export const store = createModelStore(templateInitial ?? sharedGraph ?? persistedGraph ?? undefined);
if (templateInitial || sharedGraph) {
  // Persist the opened model right away — it's the store's initial value, so it
  // never fires a change that the mirror-to-localStorage effect would catch; a
  // refresh would otherwise lose it once the URL is cleaned.
  persistGraph(store.get());
}
// Drop the share payload from the address bar so a refresh doesn't re-clobber the
// canvas and the URL stays clean (the template param is already cleared above).
if (sharedGraph) clearSharedModelFromUrl();

// A truly first-ever visit has no template deep-link, no persisted model and no
// shared link. Captured at module load — before any persist effect writes an
// (empty) graph — so it stays true for the session. Gates the first-screen
// "start" chooser (Plan 3b): shown once for new visitors, never over an opened model.
export const isFirstVisit = !templateInitial && !sharedGraph && persistedGraph === undefined;
