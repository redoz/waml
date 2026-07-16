import type { ModelGraph } from "@waml/okf";

// NOTE: the former `mergeBundles` (global-basename dedup) is retired — bundle
// merging is now the Rust `pkg.insert` op (full-path identity). `mergeGraphs`
// below is graph-level remap by fresh generated keys (not basename dedup); it has
// no production caller and is kept for its unit test only.

// Merge incoming into current (OKF import / template "merge" mode): every
// incoming node is appended under a fresh key; edges, diagrams and members are
// remapped. Returns the new keys so the caller can lay out only those.
export function mergeGraphs(current: ModelGraph, incoming: ModelGraph): { graph: ModelGraph; newKeys: Set<string> } {
  const keyRemap = new Map<string, string>();
  const newKeys = new Set<string>();
  let nc = Math.max(0, ...current.nodes.map(n => Number(/(\d+)$/.exec(n.key)?.[1] ?? 0)));
  const nodes = [...current.nodes];
  for (const inc of incoming.nodes) {
    const key = `n${++nc}`;
    keyRemap.set(inc.key, key);
    nodes.push({ ...inc, key });
    newKeys.add(key);
  }
  let ec = Math.max(0, ...current.edges.map(e => Number(/(\d+)$/.exec(e.id)?.[1] ?? 0)));
  const edges = [...current.edges];
  for (const inc of incoming.edges) {
    const from = keyRemap.get(inc.from);
    const to = keyRemap.get(inc.to);
    if (!from || !to) continue; // dangling → drop
    edges.push({ ...inc, id: `e${++ec}`, from, to });
  }
  const diagrams = [
    ...current.diagrams,
    ...incoming.diagrams.map(d => ({ ...d, members: d.members.map(m => keyRemap.get(m)).filter((k): k is string => !!k) })),
  ];
  // Package/path derivation is bundle-level (re-derived by build_model); the
  // graph merge carries `current`'s values through unchanged.
  return { graph: { nodes, edges, diagrams, path: current.path, packages: current.packages }, newKeys };
}
