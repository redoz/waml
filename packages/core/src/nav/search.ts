import type { ModelGraph } from "@waml/okf";
import { buildNavTree, type NavRow } from "./tree";

export interface SearchResult {
  inScope: NavRow[];
  elsewhere: NavRow[];
  state: "matches" | "empty-scope" | "empty-all";
}

/** Byte range of `query` within `title` (case-insensitive), or null. */
export function matchSpan(title: string, query: string): [number, number] | null {
  if (!query) return null;
  const i = title.toLowerCase().indexOf(query.toLowerCase());
  return i < 0 ? null : [i, i + query.length];
}

/** Filter a nav subtree: keep rows whose title matches `query` and whose type
 *  passes `typeFilter`, plus every ancestor package of a kept row (retained
 *  full-strength for structure). Non-matching siblings are pruned. */
function filterRows(rows: NavRow[], types: Map<string, string>, query: string, typeFilter: string): NavRow[] {
  const q = query.toLowerCase();
  const selfMatch = (r: NavRow): boolean => {
    const titleOk = q === "" || r.title.toLowerCase().includes(q);
    const typeOk = typeFilter === "all" || types.get(r.key) === typeFilter;
    return titleOk && typeOk;
  };
  const keep = rows.map(selfMatch);
  // Retain any package that has a kept descendant (walk each package's DFS block).
  for (let i = rows.length - 1; i >= 0; i--) {
    if (rows[i].kind !== "package" || keep[i]) continue;
    for (let j = i + 1; j < rows.length && rows[j].depth > rows[i].depth; j++) {
      if (keep[j]) {
        keep[i] = true;
        break;
      }
    }
  }
  return rows.filter((_, i) => keep[i]);
}

function typeMap(graph: ModelGraph): Map<string, string> {
  const types = new Map<string, string>();
  for (const n of graph.nodes) types.set(n.key, n.type);
  for (const d of graph.diagrams) types.set(d.key, "Diagram");
  for (const p of graph.packages) types.set(p.key, "uml.Package");
  return types;
}

/** Filtered navigator tree with three empty states. Empty query returns all
 *  rows (still type-filtered). If nothing matches in `scopeKey` but matches
 *  exist elsewhere, returns the whole-model filtered tree as `elsewhere`. */
export function filterNav(graph: ModelGraph, scopeKey: string, query: string, typeFilter: string): SearchResult {
  const types = typeMap(graph);
  const inScope = filterRows(buildNavTree(graph, scopeKey), types, query, typeFilter);
  if (inScope.length > 0) return { inScope, elsewhere: [], state: "matches" };

  const whole = scopeKey === "" ? [] : filterRows(buildNavTree(graph, ""), types, query, typeFilter);
  if (whole.length > 0) return { inScope: [], elsewhere: whole, state: "empty-scope" };
  return { inScope: [], elsewhere: [], state: "empty-all" };
}
