import type { ModelGraph } from "@mc/okf";

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
  return { graph: { nodes, edges, diagrams }, newKeys };
}
