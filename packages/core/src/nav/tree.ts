import type { ModelGraph } from "@waml/okf";

export type NavKind = "package" | "diagram" | "note" | "classifier" | "flow" | "sequence";
export interface NavRow {
  key: string;
  title: string;
  kind: NavKind;
  depth: number;
  members?: string[];
}

function kindOf(type: string): NavKind {
  if (type === "uml.Package") return "package";
  if (type === "Diagram") return "diagram";
  if (type === "uml.Note") return "note";
  if (type === "Flow") return "flow";
  if (type === "Sequence") return "sequence";
  return "classifier";
}

/** Rows for `scopeKey`'s subtree, fully expanded. Within each package: diagrams
 *  first (in members order), then the rest (in members order); recurses into
 *  sub-packages so diagrams float at every level. Title/kind read from the
 *  single stored source (`concept.title` for nodes/packages, flat for diagrams). */
export function buildNavTree(graph: ModelGraph, scopeKey: string): NavRow[] {
  const byKey = new Map<string, { title: string; type: string; members?: string[] }>();
  for (const n of graph.nodes) byKey.set(n.key, { title: n.concept.title ?? n.key, type: n.type });
  for (const d of graph.diagrams) byKey.set(d.key, { title: d.title, type: "Diagram" });
  for (const p of graph.packages)
    byKey.set(p.key, { title: p.concept.title || graph.path, type: "uml.Package", members: p.members });
  // Flow/interaction doc keys already live in the owning package's `members`
  // (parse.rs's build_packages does not exclude behavior docs) — they just
  // need a byKey entry so emitPackage's `if (!m) continue;` doesn't skip them.
  for (const f of graph.flows ?? []) byKey.set(f.key, { title: f.title, type: "Flow" });
  for (const s of graph.interactions ?? []) byKey.set(s.key, { title: s.title, type: "Sequence" });

  const rows: NavRow[] = [];
  const emitPackage = (pkgKey: string, depth: number) => {
    const pkg = byKey.get(pkgKey);
    const members = pkg?.members ?? [];
    const diagrams = members.filter((k) => byKey.get(k)?.type === "Diagram");
    const rest = members.filter((k) => byKey.get(k)?.type !== "Diagram");
    for (const k of [...diagrams, ...rest]) {
      const m = byKey.get(k);
      if (!m) continue;
      rows.push({ key: k, title: m.title, kind: kindOf(m.type), depth, members: m.members });
      if (m.type === "uml.Package") emitPackage(k, depth + 1);
    }
  };
  emitPackage(scopeKey, 0);

  return rows;
}

/** The package key owning `key` as a member (for context-menu targeting). */
export function packageOf(graph: ModelGraph, key: string): string {
  return graph.packages.find((p) => p.members?.includes(key))?.key ?? "";
}
