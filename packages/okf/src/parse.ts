import type { ModelGraph, ModelNode, ModelEdge, Attribute, RelEnd, RelationshipKind, NoteAnchor } from "./types";
import { endsFromCardinality } from "./migrate";
import { parseFrontmatter } from "./slug";
import { parseAttributeLine, parseValueLine, parseRelationshipLine } from "./grammar";

// Resolve a link target by its basename, tolerating ./rel paths, nested dirs,
// and (in the prose pass) absolute paths.
function basename(path: string): string {
  return path.split(/[\\/]/).pop()!.replace(/\.md$/i, "");
}

// Split a body into `## `-level sections. `pre` = text before the first `##`.
function splitSections(body: string): { pre: string; sections: { title: string; raw: string; lines: string[] }[] } {
  const lines = body.split("\n");
  const sections: { title: string; raw: string; lines: string[] }[] = [];
  const pre: string[] = [];
  let cur: { title: string; buf: string[] } | null = null;
  for (const raw of lines) {
    const ln = raw.replace(/\r$/, "");
    const h = /^##\s+(.+?)\s*$/.exec(ln);
    if (h) {
      if (cur) sections.push({ title: cur.title, raw: cur.buf.join("\n"), lines: cur.buf });
      cur = { title: h[1], buf: [raw] };
    } else if (cur) cur.buf.push(raw);
    else pre.push(raw);
  }
  if (cur) sections.push({ title: cur.title, raw: cur.buf.join("\n"), lines: cur.buf });
  return { pre: pre.join("\n"), sections };
}

const KNOWN_SECTIONS = /^(attributes|values|relationships|body|notes)$/i;
const LEGACY_SECTIONS = /^(overview|schema|definition|joins)$/i;

// A uml.Note's `annotates` bullet. Three forms (spec):
//   - annotates [Order](./order.md)                              → classifier
//   - annotates [Order](./order.md) as "places"                  → association named on the source doc
//   - annotates [Order](./order.md) associates [Customer](./customer.md)  → association by endpoint (unnamed)
const ANNOTATES_RE =
  /^- annotates \[[^\]]+\]\(\.\/(.+?)\.md\)(?:\s+as\s+"([^"]*)"|\s+(associates|aggregates|composes|specializes|implements|depends)\s+\[[^\]]+\]\(\.\/(.+?)\.md\))?\s*$/;

function parseAnnotatesLine(line: string, resolve: (slug: string) => string | undefined): NoteAnchor | null {
  const m = ANNOTATES_RE.exec(line.replace(/\r$/, "").trim());
  if (!m) return null;
  const sourceKey = resolve(basename(m[1]));
  if (!sourceKey) return null;
  if (m[2] !== undefined) return { sourceKey, name: m[2] };                 // named association
  if (m[3]) {                                                              // endpoint form (unnamed association)
    const targetKey = resolve(basename(m[4]));
    return targetKey ? { sourceKey, kind: m[3] as RelationshipKind, targetKey } : null;
  }
  return { targetKey: sourceKey };                                        // plain link — any node (no metaclass restriction)
}

export function parseBundle(files: Record<string, string>): ModelGraph {
  // Every markdown doc is a node. Navigation `index.md` files are the only
  // non-nodes, distinguished by filename.
  const docs = Object.entries(files)
    .filter(([p]) => p.endsWith(".md") && !p.endsWith("index.md"));

  // Pass 1: build the fileSlug → node-key map so links resolve regardless of
  // declaration order (attribute refs, association names, and note anchors all need it).
  const slugToKey = new Map<string, string>();
  for (const [path, text] of docs) {
    const { data } = parseFrontmatter(text);
    const fileSlug = path.split("/").pop()!.replace(/\.md$/, "");
    const key = (data.owox && data.owox.key) || fileSlug;
    slugToKey.set(fileSlug, key);
  }
  const slugToKeyLater = (s: string) => slugToKey.get(s);

  // Pass 2: build nodes — new-format primary pass with legacy Schema fallback.
  const nodes: ModelNode[] = [];
  const desugaredNotes: ModelNode[] = [];
  const newFormatPaths = new Set<string>();
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
    const title = data.title || "Untitled";
    const fileSlug = path.split("/").pop()!.replace(/\.md$/, "");
    const key = (data.owox && data.owox.key) || fileSlug;
    const { sections } = splitSections(body);
    const attrSection = sections.find(s => /^attributes$/i.test(s.title));
    const valSection = sections.find(s => /^values$/i.test(s.title));
    const relSection = sections.find(s => /^relationships$/i.test(s.title));
    const bodySection = sections.find(s => /^body$/i.test(s.title));
    const notesSection = sections.find(s => /^notes$/i.test(s.title));
    const isNote = data.type === "uml.Note";
    const isNewFormat = Boolean(attrSection || valSection || relSection || bodySection || notesSection);
    if (isNewFormat) newFormatPaths.add(path);
    const stereotypes = Array.isArray(data.stereotype) ? data.stereotype.map(String)
      : typeof data.stereotype === "string" && data.stereotype ? [data.stereotype] : [];
    const extra = sections
      .filter(s => !KNOWN_SECTIONS.test(s.title) && !LEGACY_SECTIONS.test(s.title))
      .map(s => s.raw.trimEnd()).join("\n\n");
    // A uml.Note's ## Body is its markdown text; its ## Relationships are `annotates` anchors (never edges).
    const bodyText = bodySection
      ? bodySection.lines.slice(1).map(l => l.replace(/\r$/, "")).join("\n").trim()   // drop the "## Body" heading line
      : undefined;
    const annotates: NoteAnchor[] = isNote && relSection
      ? relSection.lines.map(l => parseAnnotatesLine(l, slugToKeyLater)).filter((a): a is NoteAnchor => a !== null)
      : [];
    nodes.push({
      key,
      type: typeof data.type === "string" && data.type ? data.type : "uml.Class",
      title,
      stereotypes,
      ...(data.abstract === true ? { abstract: true } : {}),
      ...(data.description ? { description: data.description } : {}),
      attributes: attrSection
        ? attrSection.lines.map(l => parseAttributeLine(l, slugToKeyLater)).filter((a): a is Attribute => a !== null)
        : isNewFormat ? [] : parseLegacySchema(body),
      ...(valSection ? { values: valSection.lines.map(parseValueLine).filter((v): v is string => v !== null) } : {}),
      ...(bodyText ? { body: bodyText } : {}),
      ...(annotates.length ? { annotates } : {}),
      position: (data.owox && data.owox.position) || { x: 0, y: 0 },
      ...(extra ? { extra } : {}),
    });

    // `## Notes` shorthand on a classifier: each bullet desugars to a standalone
    // self-anchored uml.Note node (one internal model — every note is a uml.Note).
    // Task 10 re-collapses a note that anchors exactly its host back to `## Notes`.
    if (!isNote && notesSection) {
      notesSection.lines
        .map(parseValueLine)
        .filter((t): t is string => t !== null)
        .forEach((body, i) => desugaredNotes.push({
          key: `${key}--note-${i + 1}`,
          type: "uml.Note",
          title: `Note on ${title}`,
          stereotypes: [],
          attributes: [],
          body,
          annotates: [{ targetKey: key }],
          position: { x: 0, y: 0 },
        }));
    }
  }
  nodes.push(...desugaredNotes);

  // Relationship pass. New-format docs read `## Relationships`; legacy docs keep
  // the Joins + prose recovery. A uml.Note's relationships are anchors, not edges.
  const rawRels: { from: string; to: string; kind: RelationshipKind; name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd }[] = [];
  const legacyRaw: { from: string; to: string; cardinality?: string }[] = [];
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
    if (data.type === "uml.Note") continue;   // note anchors are not edges
    const fromKey = (data.owox && data.owox.key) || basename(path);
    if (newFormatPaths.has(path)) {
      const { sections } = splitSections(body);
      const relSection = sections.find(s => /^relationships$/i.test(s.title));
      if (!relSection) continue;
      for (const ln of relSection.lines) {
        const r = parseRelationshipLine(ln);
        if (!r) continue;
        const toKey = slugToKey.get(r.targetSlug);
        if (!toKey || toKey === fromKey) continue;
        // Resolve an `as [link]` name (grammar returned { ref: slug }) to a node key; keep a string name as-is.
        const name = r.name === undefined ? undefined
          : typeof r.name === "string" ? r.name
          : (() => { const k = slugToKey.get(r.name.ref); return k ? { ref: k } : undefined; })();
        rawRels.push({ from: fromKey, to: toKey, kind: r.kind, name, fromEnd: r.fromEnd, toEnd: r.toEnd });
      }
      continue;
    }
    // Legacy ## Joins list items: "- [Title](./slug.md) — `k = k` [N:1]" (keys ignored).
    for (const ln of body.split("\n")) {
      const m = ln.replace(/\r$/, "").match(/^- \[.*?\]\(\.\/(.+?)\.md\)\s*(?:—|--)?\s*(.*)$/);
      if (!m) continue;
      const toKey = slugToKey.get(basename(m[1]));
      if (!toKey || toKey === fromKey) continue;
      const cm = m[2].match(/\[(1:1|1:N|N:1|N:N)\]/);
      legacyRaw.push({ from: fromKey, to: toKey, cardinality: cm ? cm[1] : undefined });
    }
  }

  // Tolerant pass for prose joins ("…can be joined with the [users](users.md)
  // table…"). Conservative: lines mentioning "join" that link a known node, and
  // never list-item lines (the strict pass owns those). Legacy docs only.
  const addProseEdge = (from: string, to: string) => {
    if (legacyRaw.some(r => (r.from === from && r.to === to) || (r.from === to && r.to === from))) return;
    legacyRaw.push({ from, to });
  };
  for (const [path, text] of docs) {
    if (newFormatPaths.has(path)) continue;
    const { data, body } = parseFrontmatter(text);
    if (data.type === "uml.Note") continue;
    const fromKey = (data.owox && data.owox.key) || basename(path);
    for (const ln of body.split("\n")) {
      if (!/join/i.test(ln)) continue;
      if (/^[-*]\s+\[/.test(ln.trim())) continue;
      for (const tk of ln.matchAll(/\[[^\]]+\]\(([^)]+\.md)\)/g)) {
        const toKey = slugToKey.get(basename(tk[1]));
        if (toKey && toKey !== fromKey) addProseEdge(fromKey, toKey);
      }
    }
  }
  for (const r of legacyRaw) {
    rawRels.push({ from: r.from, to: r.to, kind: "associates", ...endsFromCardinality(r.cardinality, false) });
  }

  // Merge pass. Reciprocal `associates` collapse to one bidirectional edge (first
  // declaration wins ends + name, pinned decision 6); other kinds dedupe by triple.
  const edges: ModelEdge[] = [];
  const seen = new Map<string, ModelEdge>();
  for (const r of rawRels) {
    if (r.kind === "associates") {
      const pairKey = ["assoc", ...[r.from, r.to].sort()].join("|");
      const ex = seen.get(pairKey);
      if (ex) {
        ex.bidirectional = true;
        ex.fromEnd.navigable = true;
        ex.toEnd.navigable = true;
        if (ex.name === undefined && r.name !== undefined) ex.name = r.name;
        continue;
      }
      const e: ModelEdge = { id: `e${edges.length + 1}`, kind: "associates", from: r.from, to: r.to,
        ...(r.name !== undefined ? { name: r.name } : {}),
        fromEnd: r.fromEnd, toEnd: { ...r.toEnd, navigable: true }, bidirectional: false };
      seen.set(pairKey, e); edges.push(e);
    } else {
      const dupKey = [r.kind, r.from, r.to].join("|");
      if (seen.has(dupKey)) continue;
      const e: ModelEdge = { id: `e${edges.length + 1}`, kind: r.kind, from: r.from, to: r.to,
        ...(r.name !== undefined ? { name: r.name } : {}),
        fromEnd: r.fromEnd, toEnd: r.toEnd, bidirectional: false };
      seen.set(dupKey, e); edges.push(e);
    }
  }
  return { nodes, edges, diagrams: [] };
}

// ── Legacy `# Schema` readers (tables + Google-era bullet lists) ─────────────

function parseLegacySchema(body: string): Attribute[] {
  const out: Attribute[] = [];
  const lines = body.split("\n");
  let inSchema = false;
  let legacy = false;
  for (const raw of lines) {
    const ln = raw.replace(/\r$/, "");
    if (/^##?\s+Schema/i.test(ln)) { inSchema = true; continue; }
    if (!inSchema) continue;
    if (/^##?\s+/.test(ln)) break;
    if (!/^\s*\|/.test(ln)) continue;
    const cells = ln.split("|").slice(1, -1).map(c => c.trim());
    if (cells.length < 2) continue;
    const name = cells[0].replace(/`/g, "").trim();
    if (!name || name === "Column") {
      legacy = cells.some(c => /^pk$/i.test(c) || /^alias$/i.test(c)); // header row
      continue;
    }
    if (/^:?-+:?$/.test(name)) continue; // separator
    const type = (cells[1] || "STRING").replace(/`/g, "").trim() || "STRING";
    let desc = legacy ? (cells[4] || "").trim() : (cells[2] || "").trim();
    desc = desc.replace(/^PK\.\s*/, "").trim();           // pk flag is data-profile-only: strip the token
    desc = desc.replace(/\s*FK to \[[^\]]*\]\([^)]*\)/g, "").trim(); // FK notes likewise
    out.push({ name, type: { name: type }, multiplicity: "1", ...(desc ? { description: desc } : {}) });
  }
  if (out.length === 0) return parseSchemaBullets(body);
  return out;
}

const TYPE_WORDS =
  "STRING|BYTES|INTEGER|INT64|FLOAT|FLOAT64|NUMERIC|BIGNUMERIC|BOOLEAN|BOOL|" +
  "TIMESTAMP|DATE|DATETIME|TIME|RECORD|STRUCT|GEOGRAPHY|JSON|INTERVAL";
const TYPE_RE = new RegExp(`\\b(${TYPE_WORDS})\\b`, "i");

// Fallback for Google OKF v0.1 bundles (bullet-list schemas). Top-level bullets
// only; runs only when the table parser found nothing.
function parseSchemaBullets(body: string): Attribute[] {
  const out: Attribute[] = [];
  let inSchema = false;
  let schemaLevel = 0;
  for (const raw of body.split("\n")) {
    const ln = raw.replace(/\r$/, "");
    const h = ln.match(/^(#{1,6})\s+(.*)$/);
    if (h) {
      const level = h[1].length;
      if (/^schema\b/i.test(h[2].trim())) { inSchema = true; schemaLevel = level; continue; }
      if (inSchema && level <= schemaLevel) break;
      continue;
    }
    if (!inSchema) continue;
    const m = ln.match(/^[-*]\s+`([^`]+)`(.*)$/);
    if (!m) continue;
    const name = m[1].trim();
    if (!/^[\w.]+$/.test(name)) continue;
    out.push(parseFieldRest(name, m[2]));
  }
  return out;
}

function parseFieldRest(name: string, rest: string): Attribute {
  let type = "STRING";
  let description = "";
  const paren = rest.match(/^\s*\(([^)]+)\)\s*[-:]?\s*(.*)$/);
  if (paren) {
    type = (paren[1].match(TYPE_RE)?.[1] ?? paren[1].trim()).toUpperCase();
    description = paren[2].trim();
  } else {
    const tail = rest.replace(/^\s*[-:]\s*/, "");
    type = (tail.match(TYPE_RE)?.[1] ?? "STRING").toUpperCase();
    const colon = tail.indexOf(":");
    description = colon >= 0 ? tail.slice(colon + 1).trim() : "";
  }
  return { name, type: { name: type }, multiplicity: "1", ...(description ? { description } : {}) };
}
