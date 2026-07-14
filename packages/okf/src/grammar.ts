import type { Attribute, RelEnd, RelationshipKind, Visibility } from "./types";
import { ENDED_KINDS } from "./types";

// BNF from the spec (2026-07-11): multiplicity ::= bound | lower ".." bound;
// lower ::= 0 | posint; bound ::= posint | "*". Bare 0 is not a multiplicity.
const MULTIPLICITY_RE = /^(?:[1-9]\d*|\*|(?:0|[1-9]\d*)\.\.(?:[1-9]\d*|\*))$/;

export function isValidMultiplicity(s: string): boolean {
  if (!MULTIPLICITY_RE.test(s)) return false;
  const m = /^(\d+)\.\.(\d+)$/.exec(s);
  return !m || Number(m[1]) <= Number(m[2]);
}

function stripCr(line: string): string {
  return line.replace(/\r$/, "");
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop()!.replace(/\.md$/i, "");
}

// - [visibility ]name: Type-or-link {multiplicity}
const ATTR_RE = /^- (?:([+\-#~]) )?([A-Za-z_][A-Za-z0-9_]*): (.+)$/;
const LINK_RE = /^\[([^\]]+)\]\(\.\/(.+?)\.md\)$/;

export function parseAttributeLine(line: string, resolveSlug: (slug: string) => string | undefined): Attribute | null {
  const m = ATTR_RE.exec(stripCr(line).trim());
  if (!m) return null;
  let rest = m[3].trim();
  let multiplicity = "1";
  // Multiplicity is a trailing `{…}` token whose contents are a valid multiplicity.
  // A `{…}` with any other contents is malformed — not silently accepted.
  const mm = /^(.*?)\s+\{([^{}]*)\}$/.exec(rest);
  if (mm) {
    if (!isValidMultiplicity(mm[2])) return null;
    rest = mm[1].trim();
    multiplicity = mm[2];
  }
  const link = LINK_RE.exec(rest);
  let type: Attribute["type"];
  if (link) {
    const ref = resolveSlug(basename(link[2]));
    type = ref ? { name: link[1], ref } : { name: link[1] };
  } else {
    if (!rest || /[[\](){}]/.test(rest)) return null; // stray link/bracket/brace punctuation → not an attribute
    type = { name: rest };
  }
  const attr: Attribute = { name: m[2], type, multiplicity };
  if (m[1]) attr.visibility = m[1] as Visibility;
  return attr;
}

export function parseValueLine(line: string): string | null {
  const m = /^- (\S.*)$/.exec(stripCr(line).trim());
  return m ? m[1].trim() : null;
}

// - verb [Title](./slug.md)[ as ("name"|[Title](./slug.md))][: <end> to <end>]   end ::= mult[ role]
// Groups: 1 verb · 2 target title · 3 target slug · 4 name string · 5 name-link title · 6 name-link slug · 7 ends
const REL_RE = /^- (associates|aggregates|composes|specializes|implements|depends|includes|extends) \[([^\]]+)\]\(\.\/(.+?)\.md\)(?: as (?:"([^"]*)"|\[([^\]]+)\]\(\.\/(.+?)\.md\)))?(?:\s*:\s*(.+))?$/;
const END_RE = /^(\S+)(?:\s+([A-Za-z][A-Za-z0-9_]*))?$/;

export function parseRelationshipLine(
  line: string,
): { kind: RelationshipKind; targetSlug: string; name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd } | null {
  const m = REL_RE.exec(stripCr(line).trim());
  if (!m) return null;
  const kind = m[1] as RelationshipKind;
  const endsRaw = m[7];
  // Ends: forbidden unless `kind` is in ENDED_KINDS; required for aggregates/composes;
  // OPTIONAL for associates (bare = actor↔use-case communication link, enforced
  // cross-doc by the Rust validate layer — TS grammar only enforces syntax here).
  if (endsRaw) {
    if (!ENDED_KINDS.has(kind)) return null;
  } else if (ENDED_KINDS.has(kind) && kind !== "associates") {
    return null;
  }
  // Optional `as …` UML association name — allowed on EVERY verb, before the ends.
  // String form → plain label + note handle; link form → { ref: slug } (Task 9 remaps the slug to a uml.Association node key).
  const name: string | { ref: string } | undefined =
    m[4] !== undefined ? m[4]
    : m[6] !== undefined ? { ref: basename(m[6]) }
    : undefined;
  let fromEnd: RelEnd = {};
  let toEnd: RelEnd = {};
  if (endsRaw) {
    const parts = endsRaw.split(/\s+to\s+/);
    if (parts.length !== 2) return null;
    const parsed: (RelEnd | null)[] = parts.map(p => {
      const em = END_RE.exec(p.trim());
      if (!em || !isValidMultiplicity(em[1])) return null;
      const end: RelEnd = { multiplicity: em[1] };
      if (em[2]) end.role = em[2];
      return end;
    });
    if (!parsed[0] || !parsed[1]) return null;
    fromEnd = parsed[0];
    toEnd = parsed[1];
  }
  return { kind, targetSlug: basename(m[3]), ...(name !== undefined ? { name } : {}), fromEnd, toEnd };
}

// ── render side (serializer) ─────────────────────────────────────────────────

export function renderAttributeLine(a: Attribute, slugForRef: (key: string) => string | undefined): string {
  const slug = a.type.ref ? slugForRef(a.type.ref) : undefined;
  const type = slug ? `[${a.type.name}](./${slug}.md)` : a.type.name;
  const vis = a.visibility ? `${a.visibility} ` : "";
  const mult = a.multiplicity && a.multiplicity !== "1" ? ` {${a.multiplicity}}` : "";
  return `- ${vis}${a.name}: ${type}${mult}`;
}

function renderEnd(e: RelEnd): string {
  return `${e.multiplicity ?? "1"}${e.role ? ` ${e.role}` : ""}`;
}

export function renderRelationshipLine(
  kind: RelationshipKind, targetTitle: string, targetSlug: string, fromEnd: RelEnd, toEnd: RelEnd,
  name?: string | { title: string; slug: string },
): string {
  const link = `[${targetTitle}](./${targetSlug}.md)`;
  const nameStr =
    name === undefined ? ""
    : typeof name === "string" ? ` as "${name}"`
    : ` as [${name.title}](./${name.slug}.md)`;
  const hasEnds = fromEnd.multiplicity !== undefined || toEnd.multiplicity !== undefined;
  if (!ENDED_KINDS.has(kind) || !hasEnds) return `- ${kind} ${link}${nameStr}`;
  return `- ${kind} ${link}${nameStr}: ${renderEnd(fromEnd)} to ${renderEnd(toEnd)}`;
}
