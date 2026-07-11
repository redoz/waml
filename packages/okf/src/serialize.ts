import type { ModelGraph, ModelNode, RelEnd } from "./types";
import { slugify, renderFrontmatter } from "./slug";

export interface OkfBundle { files: Record<string, string>; }

// INTERIM (stage 1): emits the legacy doc shape (Schema table + Joins lines)
// from the generalized model so the stage lands green. Stage 2 replaces this
// with the Attributes/Values/Relationships format.

export function serializeBundle(graph: ModelGraph, projectTitle = "Model"): OkfBundle {
  const folder = slugify(projectTitle, "model");
  const slugByKey = new Map<string, string>();
  const taken = new Set<string>();
  for (const n of graph.nodes) {
    const s = slugify(n.title, n.key);
    let u = s; let i = 2;
    while (taken.has(u)) u = `${s}-${i++}`;
    taken.add(u);
    slugByKey.set(n.key, u);
  }
  const files: Record<string, string> = {};
  for (const n of graph.nodes) files[`${folder}/${slugByKey.get(n.key)}.md`] = renderNode(n, graph, slugByKey);
  const rows = graph.nodes.map(n =>
    `| [${n.title}](./${slugByKey.get(n.key)}.md) | ${n.type} |`).join("\n");
  files[`${folder}/index.md`] =
    `---\n${renderFrontmatter({ type: "index", title: projectTitle, description: "Index of exported documents." })}\n---\n\n# ${projectTitle}\n\n| Document | Type |\n|----------|------|\n${rows}\n`;
  return { files };
}

// "*" or an unbounded range reads as N; anything else as 1. Interim only.
const cardToken = (m?: string) => (m && (m === "*" || m.endsWith("..*")) ? "N" : "1");

function cardinalitySuffix(fromEnd: RelEnd, toEnd: RelEnd): string {
  if (!fromEnd.multiplicity && !toEnd.multiplicity) return "";
  return ` [${cardToken(fromEnd.multiplicity)}:${cardToken(toEnd.multiplicity)}]`;
}

function renderNode(n: ModelNode, g: ModelGraph, slugByKey: Map<string, string>): string {
  const fm = renderFrontmatter({ type: n.type, title: n.title, description: n.description || undefined });
  const schema = n.attributes.length
    ? "# Schema\n\n| Column | Type | Description |\n|--------|------|-------------|\n" +
      n.attributes.map(a => `| \`${a.name}\` | ${a.type.name} | ${a.description ?? ""} |`).join("\n") + "\n\n"
    : "";
  const outgoing = g.edges.filter(e => e.from === n.key || (e.bidirectional && e.to === n.key));
  const joins = outgoing.length
    ? "## Joins\n\n" + outgoing.map(e => {
        const forward = e.from === n.key;
        const otherKey = forward ? e.to : e.from;
        const other = g.nodes.find(x => x.key === otherKey)!;
        const suffix = forward ? cardinalitySuffix(e.fromEnd, e.toEnd) : cardinalitySuffix(e.toEnd, e.fromEnd);
        return `- [${other.title}](./${slugByKey.get(otherKey)}.md)${suffix}`;
      }).join("\n") + "\n"
    : "";
  return `---\n${fm}\n---\n\n# ${n.title}\n${n.description ? "\n" + n.description + "\n" : ""}\n${schema}${joins}`;
}
