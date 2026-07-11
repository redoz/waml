import type { Diagram, ModelGraph } from "@mc/okf";

export const ALL_DIAGRAM_KEY = "__all__";

/** Empty diagrams array = today's single implicit graph as one default diagram. */
export function effectiveDiagrams(g: ModelGraph): Diagram[] {
  if (g.diagrams.length > 0) return g.diagrams;
  return [{ key: ALL_DIAGRAM_KEY, title: "All", profile: "uml-domain", members: g.nodes.map(n => n.key) }];
}

const KEY = "mc.activeDiagram.v1";

export function loadActiveDiagramKey(): string | null {
  try { return localStorage.getItem(KEY); } catch { return null; }
}
export function persistActiveDiagramKey(key: string): void {
  try { localStorage.setItem(KEY, key); } catch { /* best-effort */ }
}
