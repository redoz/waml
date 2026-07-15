import type { Diagram, ModelGraph } from "@waml/okf";

export const ALL_DIAGRAM_KEY = "__all__";

// Cache the synthesized implicit-"All" array per graph object. Canvas calls
// effectiveDiagrams on every render and feeds the result into effect deps
// (activeDiagram); a fresh array/object each call re-fires the setRfNodes effect
// every render, so React Flow keeps rebuilding unmeasured nodes and never clears
// their initial `visibility:hidden` — the canvas renders empty. The store hands
// out a new graph object only on mutation, so keying on `g` stays correct.
const implicitDiagramsCache = new WeakMap<ModelGraph, Diagram[]>();

/** Empty diagrams array = today's single implicit graph as one default diagram. */
export function effectiveDiagrams(g: ModelGraph): Diagram[] {
  if (g.diagrams.length > 0) return g.diagrams;
  let cached = implicitDiagramsCache.get(g);
  if (!cached) {
    cached = [{ key: ALL_DIAGRAM_KEY, title: "All", profile: "uml-domain", members: g.nodes.map(n => n.key) }];
    implicitDiagramsCache.set(g, cached);
  }
  return cached;
}

/** The view a fresh model should open on: a curated diagram, else the first
 *  behavioral flow, else the first interaction, else the synthetic "All". */
export function defaultDiagramKey(g: ModelGraph): string {
  if (g.diagrams.length) return g.diagrams[0].key;
  if (g.flows?.length) return g.flows[0].key;
  if (g.interactions?.length) return g.interactions[0].key;
  return effectiveDiagrams(g)[0].key;
}

const KEY = "mc.activeDiagram.v1";

export function loadActiveDiagramKey(): string | null {
  try { return localStorage.getItem(KEY); } catch { return null; }
}
export function persistActiveDiagramKey(key: string): void {
  try { localStorage.setItem(KEY, key); } catch { /* best-effort */ }
}
