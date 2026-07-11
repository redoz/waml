import type { ModelGraph, ModelNode, ModelEdge, Attribute } from "./types";
import { endsFromCardinality } from "./migrate";
import { parseFrontmatter } from "./slug";

// Resolve a link target by its basename, tolerating ./rel paths, nested dirs,
// and (in the prose pass) absolute paths.
function basename(path: string): string {
  return path.split(/[\\/]/).pop()!.replace(/\.md$/i, "");
}

export function parseBundle(files: Record<string, string>): ModelGraph {
  // Every markdown doc is a node. Navigation `index.md` files are the only
  // non-nodes, distinguished by filename.
  const docs = Object.entries(files)
    .filter(([p]) => p.endsWith(".md") && !p.endsWith("index.md"));
  const nodes: ModelNode[] = [];
  const slugToKey = new Map<string, string>();
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
    const title = data.title || "Untitled";
    const fileSlug = path.split("/").pop()!.replace(/\.md$/, "");
    const key = (data.owox && data.owox.key) || fileSlug;
    slugToKey.set(fileSlug, key);
    nodes.push({
      key,
      type: typeof data.type === "string" && data.type ? data.type : "uml.Class",
      title,
      stereotypes: [],
      ...(data.description ? { description: data.description } : {}),
      attributes: parseLegacySchema(body),
      position: (data.owox && data.owox.position) || { x: 0, y: 0 },
    });
  }

  // Legacy ## Joins list items: "- [Title](./slug.md) — `k = k` [N:1]" (keys ignored).
  const raw: { from: string; to: string; cardinality?: string; bidirectional?: boolean }[] = [];
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
    const fromKey = (data.owox && data.owox.key) || basename(path);
    for (const ln of body.split("\n")) {
      const m = ln.replace(/\r$/, "").match(/^- \[.*?\]\(\.\/(.+?)\.md\)\s*(?:—|--)?\s*(.*)$/);
      if (!m) continue;
      const toKey = slugToKey.get(basename(m[1]));
      if (!toKey || toKey === fromKey) continue;
      const cm = m[2].match(/\[(1:1|1:N|N:1|N:N)\]/);
      raw.push({ from: fromKey, to: toKey, cardinality: cm ? cm[1] : undefined });
    }
  }

  // Tolerant pass for prose joins ("…can be joined with the [users](users.md)
  // table…"). Conservative: lines mentioning "join" that link a known node, and
  // never list-item lines (the strict pass owns those).
  const addProseEdge = (from: string, to: string) => {
    if (raw.some(r => (r.from === from && r.to === to) || (r.from === to && r.to === from))) return;
    raw.push({ from, to });
  };
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
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

  // Collapse mutual declarations into one bidirectional edge.
  const edges: ModelEdge[] = [];
  const seen = new Map<string, ModelEdge>();
  for (const r of raw) {
    const pairKey = [r.from, r.to].sort().join("|");
    const ex = seen.get(pairKey);
    if (ex) {
      ex.bidirectional = true;
      ex.fromEnd.navigable = true;
      ex.toEnd.navigable = true;
      continue;
    }
    const e: ModelEdge = {
      id: `e${edges.length + 1}`, kind: "associates", from: r.from, to: r.to,
      ...endsFromCardinality(r.cardinality, false), bidirectional: false,
    };
    seen.set(pairKey, e);
    edges.push(e);
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
