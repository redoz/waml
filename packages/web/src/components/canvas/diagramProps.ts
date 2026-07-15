import type { ModelNode } from "@waml/okf";
import { ALL_DIAGRAM_KEY } from "@waml/core/state/diagrams";

/** Unique, sorted stereotypes present on the diagram's member nodes. */
export function diagramCandidateStereotypes(nodes: ModelNode[], members: string[]): string[] {
  const memberSet = new Set(members);
  const names = new Set<string>();
  for (const n of nodes) {
    if (!memberSet.has(n.key)) continue;
    for (const s of n.stereotypes) names.add(s);
  }
  return [...names].sort();
}

/** The implicit "All" diagram has no backing document and cannot persist settings. */
export function isDiagramEditable(diagramKey: string): boolean {
  return diagramKey !== ALL_DIAGRAM_KEY;
}
