import type { ModelGraph, ModelNode, NoteAnchor } from "./types";
import { renderAttributeLine, renderRelationshipLine } from "./grammar";
import { slugify, renderFrontmatter } from "./slug";

export interface OkfBundle { files: Record<string, string>; }

// A uml.Note collapses onto its host's `## Notes` when its ONLY anchor is a single
// classifier (spec: "anchor exactly their own node with no other targets").
export function selfAnchorHost(n: ModelNode): string | undefined {
  if (n.type !== "uml.Note" || !n.annotates || n.annotates.length !== 1) return undefined;
  const a = n.annotates[0];
  return "targetKey" in a && !("sourceKey" in a) ? a.targetKey : undefined;
}

function renderAnnotates(a: NoteAnchor, g: ModelGraph, slugByKey: Map<string, string>): string | null {
  const link = (key: string) => {
    const n = g.nodes.find(x => x.key === key);
    return n ? `[${n.title}](./${slugByKey.get(key)}.md)` : null;
  };
  if ("targetKey" in a && !("sourceKey" in a)) { const l = link(a.targetKey); return l ? `- annotates ${l}` : null; }
  const src = link(a.sourceKey); if (!src) return null;
  if ("name" in a) return `- annotates ${src} as "${a.name}"`;               // named association
  const tgt = link(a.targetKey); return tgt ? `- annotates ${src} ${a.kind} ${tgt}` : null; // endpoint form
}

function renderNode(n: ModelNode, g: ModelGraph, slugByKey: Map<string, string>, hostNotes: string[]): string {
  const fm = renderFrontmatter({
    type: n.type,
    ...(n.stereotypes.length ? { stereotype: n.stereotypes } : {}),
    ...(n.abstract ? { abstract: true } : {}),
    title: n.title,
    description: n.description || undefined,
  });
  const slugFor = (key: string) => slugByKey.get(key);

  const body = n.type === "uml.Note" && n.body ? "## Body\n" + n.body + "\n\n" : "";
  const attributes = n.attributes.length
    ? "## Attributes\n" + n.attributes.map(a => renderAttributeLine(a, slugFor)).join("\n") + "\n\n"
    : "";
  const values = n.values && n.values.length
    ? "## Values\n" + n.values.map(v => `- ${v}`).join("\n") + "\n\n"
    : "";

  // Resolve an edge's association name into the renderRelationshipLine argument:
  // a string stays a string; a { ref } becomes the association node's { title, slug }.
  const nameArg = (name: string | { ref: string } | undefined): string | { title: string; slug: string } | undefined => {
    if (name === undefined) return undefined;
    if (typeof name === "string") return name;
    const an = g.nodes.find(x => x.key === name.ref);
    return an ? { title: an.title, slug: slugByKey.get(name.ref)! } : undefined;
  };

  // For a uml.Note: its anchors. For a classifier: every edge it originates, plus
  // the reciprocal line of a bidirectional association it is the far end of.
  const lines: string[] = [];
  if (n.type === "uml.Note") {
    for (const a of n.annotates ?? []) { const l = renderAnnotates(a, g, slugByKey); if (l) lines.push(l); }
  } else {
    for (const e of g.edges) {
      if (e.from === n.key) {
        const other = g.nodes.find(x => x.key === e.to)!;
        lines.push(renderRelationshipLine(e.kind, other.title, slugByKey.get(e.to)!, e.fromEnd, e.toEnd, nameArg(e.name)));
      } else if (e.to === n.key && e.kind === "associates" && e.bidirectional) {
        const other = g.nodes.find(x => x.key === e.from)!;
        lines.push(renderRelationshipLine(e.kind, other.title, slugByKey.get(e.from)!, e.toEnd, e.fromEnd, nameArg(e.name)));
      }
    }
  }
  const relationships = lines.length ? "## Relationships\n" + lines.join("\n") + "\n\n" : "";
  const notes = hostNotes.length ? "## Notes\n" + hostNotes.map(t => `- ${t}`).join("\n") + "\n\n" : "";
  const extra = n.extra ? n.extra.trimEnd() + "\n" : "";

  return `---\n${fm}\n---\n\n# ${n.title}\n\n${body}${attributes}${values}${relationships}${notes}${extra}`;
}

export function serializeBundle(graph: ModelGraph, projectTitle = "Model"): OkfBundle {
  const folder = slugify(projectTitle, "model");

  // A self-anchored note (its only anchor is one classifier) rides on that host's
  // `## Notes` list instead of getting its own doc.
  const notesByHost = new Map<string, string[]>();
  const collapsed = new Set<string>();
  for (const n of graph.nodes) {
    const host = selfAnchorHost(n);
    if (host && n.body) {
      (notesByHost.get(host) ?? notesByHost.set(host, []).get(host)!).push(n.body);
      collapsed.add(n.key);
    }
  }

  const slugByKey = new Map<string, string>();
  const taken = new Set<string>();
  for (const n of graph.nodes) {
    if (collapsed.has(n.key)) continue;
    const s = slugify(n.title, n.key);
    let u = s; let i = 2;
    while (taken.has(u)) u = `${s}-${i++}`;
    taken.add(u);
    slugByKey.set(n.key, u);
  }

  const files: Record<string, string> = {};
  for (const n of graph.nodes) {
    if (collapsed.has(n.key)) continue;
    files[`${folder}/${slugByKey.get(n.key)}.md`] = renderNode(n, graph, slugByKey, notesByHost.get(n.key) ?? []);
  }

  const rows = graph.nodes
    .filter(n => !collapsed.has(n.key))
    .map(n => `| [${n.title}](./${slugByKey.get(n.key)}.md) | ${n.type} |`)
    .join("\n");
  files[`${folder}/index.md`] =
    `---\n${renderFrontmatter({ type: "index", title: projectTitle, description: "Index of exported documents." })}\n---\n\n# ${projectTitle}\n\n| Document | Type |\n|----------|------|\n${rows}\n`;
  return { files };
}
