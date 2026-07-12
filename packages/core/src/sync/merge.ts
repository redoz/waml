import type { ModelGraph } from "@uaml/okf";

type Bundle = [string, string][];

/** Bundle-native merge (OKF import / template "merge" mode): append every
 *  incoming document that doesn't collide with an existing one, keyed by slug
 *  (the filename without `.md` — the node identity). Colliding docs are skipped
 *  so the merge is idempotent and never produces duplicate slugs (build_model
 *  would otherwise see two documents for the same key). */
export function mergeBundles(current: Bundle, incoming: Bundle): Bundle {
  const slug = (p: string) => (p.split(/[\\/]/).pop() ?? p).replace(/\.md$/, "");
  const used = new Set(current.map(([p]) => slug(p)));
  const out: Bundle = current.map(([p, m]) => [p, m]);
  for (const [p, m] of incoming) {
    const s = slug(p);
    if (used.has(s)) continue;
    used.add(s);
    out.push([p, m]);
  }
  return out;
}

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
