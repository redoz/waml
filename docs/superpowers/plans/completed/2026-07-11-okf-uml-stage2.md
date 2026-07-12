# OKF Agnostic Profiles + UML Domain Model — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn OKF Canvas from a data-node/ERD tool into a profile-agnostic modeling canvas whose first profile is a UML class-diagram / domain model, per the approved spec `docs/superpowers/specs/2026-07-11-okf-agnostic-profiles-uml-domain-design.md`.

**Architecture:** The core `ModelGraph` in `packages/okf` is generalized (classifier nodes with `family.Metaclass` type + stereotypes + attributes; relationship edges with a verb `kind` + per-end multiplicity/role/navigability; diagrams as first-class curated views). The markdown format gains `## Attributes` / `## Values` / `## Relationships` sections and `type: Diagram` docs. The web canvas dispatches rendering through a metaclass renderer registry with a mandatory generic fallback, and a data-only profile (`uml-domain`) supplies emphasis, stereotype styles, and the palette.

**Tech Stack:** TypeScript 5.6, pnpm workspaces (`@mc/okf`, `@mc/web`), React 18 + @xyflow/react 12, Vitest 2, ESLint 9. No new dependencies.

## Global Constraints

- Repo root: `C:\dev\vendor\owox`. All paths below are relative to it.
- Six rollout stages, exactly as the spec orders them; **the full suite must be green at the end of every stage** before pushing to `main`: `pnpm lint && pnpm build && pnpm -r test` (run from repo root).
- Web tests import `@mc/okf` from its compiled `dist/` — after ANY change in `packages/okf/src`, run `pnpm --filter @mc/okf build` before running web tests, or web tests test stale code.
- CI runs lint + build + test on **ubuntu AND windows** (`.github/workflows/ci.yml`). Parser code splits on `"\n"`; every new line-matching regex must tolerate a trailing `\r` (strip with `.replace(/\r$/, "")` before matching).
- Graceful degradation is mandatory: unknown `type` family/metaclass renders as a generic labelled box; the canvas never errors on an unrecognized `type`. Unknown markdown sections are carried through round-trip, never dropped.
- Deferred (do NOT build): operations/methods (`## Operations`), non-UML families (erd/bpmn/c4), the data/ERD profile (join keys, PK/FK, `inputSource`), attribute adornments (defaults, `{readOnly}`).
- `docs/superpowers/` is gitignored — never `git add` this plan or the specs; never use `git add -f`.
- Commit per task with a conventional-commit message. End every commit message with the Co-Authored-By line from your environment instructions.
- ESLint fails on unused imports/vars — every step that removes code must also remove its now-unused imports.

## Pinned TypeScript shapes (the spec left these to this plan — they are now FIXED)

All later tasks use exactly these names. Defined in `packages/okf/src/types.ts` (Task 1):

```ts
export type Visibility = "+" | "-" | "#" | "~";                    // public/private/protected/package
export interface TypeRef { name: string; ref?: string }           // ref = node key when authored as [Money](./money.md)
export interface Attribute {
  name: string;
  type: TypeRef;
  multiplicity: string;              // UML string as authored: "1" | "0..1" | "*" | "1..*" | "2..5"; parser defaults "1"
  visibility?: Visibility;
  description?: string;
}
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "annotates"] as const;
export type RelationshipKind = (typeof RELATIONSHIP_KINDS)[number];   // "annotates" is valid ONLY on a uml.Note's ## Relationships
export const ENDED_KINDS: ReadonlySet<RelationshipKind>;           // associates | aggregates | composes
export interface RelEnd { multiplicity?: string; role?: string; navigable?: boolean }
/** A uml.Note anchor: a classifier, a NAMED association, or an association by endpoint (unnamed). */
export type NoteAnchor =
  | { targetKey: string }                                       // any node (any metaclass, incl. another note) — plain link; attributes not anchorable
  | { sourceKey: string; name: string }                         // association named on sourceKey
  | { sourceKey: string; kind: RelationshipKind; targetKey: string }; // association by source + verb + target (unnamed)
export interface ModelNode {
  key: string;
  type: string;                      // "family.Metaclass" (e.g. "uml.Class") or opaque legacy token
  title: string;
  stereotypes: string[];             // open set; [] when none
  abstract?: boolean;
  description?: string;
  attributes: Attribute[];
  values?: string[];                 // uml.Enum literals
  body?: string;                     // uml.Note markdown body (## Body)
  annotates?: NoteAnchor[];          // uml.Note anchor targets; ## Notes shorthand desugars to a self-anchored note
  position: { x: number; y: number };
  extra?: string;                    // raw markdown of unrecognized ## sections — carried, never dropped
}
export interface ModelEdge {
  id: string;
  kind: RelationshipKind;
  from: string;                      // declaring/near end (whole for aggregates/composes; child for specializes/implements; dependent for depends)
  to: string;                        // far end (part / parent / interface / dependency target)
  name?: string | { ref: string };  // UML association name: string label (also the note handle) OR ref to a uml.Association node key (association class)
  fromEnd: RelEnd;
  toEnd: RelEnd;
  bidirectional: boolean;            // derived from reciprocity (both docs declare the association)
  sourceHandle?: string | null;      // canvas-only routing hints (unchanged role)
  targetHandle?: string | null;
}
export interface DiagramHints { emphasize?: string[]; collapse?: string[] }   // collapse = node keys drawn as ref chips
export interface Diagram {
  key: string;                       // file slug
  title: string;
  profile: string;                   // e.g. "uml-domain"
  members: string[];                 // node keys, curated order
  hints?: DiagramHints;
}
export interface ModelGraph {
  nodes: ModelNode[];
  edges: ModelEdge[];
  diagrams: Diagram[];               // empty array ⇒ ONE implicit diagram containing every node
}
export function splitType(type: string): { family: string; metaclass: string } | null;
```

**Decisions the spec delegated, now pinned:**

1. **Implicit default diagram:** `diagrams: []` means the canvas renders one implicit diagram (key `"__all__"`, title `"All"`, profile `"uml-domain"`, members = every node). This is how "today's single implicit graph = one default diagram" maps — no data fabrication in shares/templates/persist.
2. **Dropped legacy fields:** `InputSource`, `NodeStatus`, `Cardinality`, `SchemaField`, `JoinKey`, `ModelNode.{inputSource,definition,schema,status,owoxId,owoxStorageId,createdAt,createdBy,error}`, `ModelEdge.{keys,cardinality,existing}`, `ModelGraph.storageId` are deleted from core (spec: data-profile-only concerns, profile deferred). Legacy payloads are *migrated*, not supported in memory.
3. **Migration mapping (one shared module `packages/okf/src/migrate.ts`, used by URL-share decode, localStorage rehydrate, and legacy markdown import):** `SchemaField {name,type,pk,alias,description}` → `Attribute {name, type:{name:type}, multiplicity:"1", description}` (pk/alias dropped); legacy edge → `kind:"associates"` with `cardinality "X:Y"` mapped per end (`"1"→"1"`, `"N"→"*"`), `keys` dropped; `bidirectional` preserved and sets both ends navigable; one-way sets only `toEnd.navigable`.
4. **Positions stay node-level and canonical.** Diagram `at x,y` render hints are read into `node.position` at parse and emitted from it at serialize; per-diagram positions are NOT stored at runtime (deferred).
5. **localStorage:** same key `mc.model.v1`; shape-detected + migrated on read (a graph without a `diagrams` array is legacy). URL shares: `decodeModel` migrates before sanitizing — old share links keep opening.
6. **Reciprocity merge, first-wins:** when both docs declare `associates` at each other, the first-parsed declaration's ends win; the reverse declaration only flips `bidirectional: true` and sets both `navigable`. Mismatched reciprocal multiplicities are not an error.
7. **RelLabelMode** shrinks from `"all"|"defined"|"undefined"|"hidden"` (join-key concepts) to `"all"|"hidden"`; persisted legacy values coerce to `"all"`.
8. **One React Flow node type** (`"okf"`) whose component dispatches on `node.type` via the registry — RF `nodeTypes` stays static.
9. **Profiles are data modules in the web package** (`packages/web/src/profiles/`), keyed by name; a diagram's `profile:` frontmatter selects one; unknown profile name falls back to `uml-domain`. (The spec's YAML block is illustrative, not a parsed artifact.)
10. **Interim stage-1 serialization:** stage 1 keeps emitting the legacy markdown shape (Schema table + Joins lines with `[N:1]`-style suffix derived from end multiplicities) so the stage lands green before stage 2 replaces the format. Known, accepted interim limitation: non-`associates` kinds degrade to `associates` on export until stage 2.

## File structure (target)

```
packages/okf/src/
  types.ts        — pinned shapes above (rewritten)
  migrate.ts      — NEW: legacy-graph → ModelGraph migration (+ endsFromCardinality)
  grammar.ts      — NEW (stage 2): line grammar: multiplicity, attribute, relationship, render helpers
  parse.ts        — new-format primary pass + legacy fallback + diagram docs (stages 1/2/5)
  serialize.ts    — new-format emission incl. diagram docs (stages 1/2/5)
  slug.ts         — unchanged
  index.ts        — re-exports incl. migrate + grammar
packages/okf/test/
  migrate.test.ts — NEW; grammar.test.ts — NEW; format.test.ts — NEW (new-format parse/serialize/roundtrip)
  parse-owox.test.ts, prose-join.test.ts, roundtrip.test.ts, serialize.test.ts — updated (legacy import coverage)
packages/web/src/
  state/model.ts, state/persist.ts, state/relLabels.ts, state/diagrams.ts (NEW, stage 5)
  share/url.ts    — sanitize/migrate for new shape
  sync/merge.ts   — NEW (replaces sync/owoxImport.ts); sync/detach.ts + sync/joinFieldType.ts DELETED
  templates/helpers.ts — legacy helpers mapped to new shape + new UML helpers (stage 6)
  components/canvas/nodes/ — NEW (stage 3): shared.tsx, GenericNode.tsx, uml.tsx, registry.ts, OkfNode.tsx (NodeNode.tsx DELETED)
  components/canvas/{Canvas.tsx, edges.ts, RelEdge.tsx, layoutSize.ts, Dock.tsx, DiagramTabs.tsx (NEW, stage 5)}
  components/inspector/{ObjectInspector.tsx, RelationshipInspector.tsx, AttributeEditor.tsx (NEW, replaces SchemaEditor.tsx), ExternalRefs.tsx (NEW, stage 5)}
  profiles/ — NEW (stage 4): index.ts, umlDomain.ts
  templates/orders-domain.ts — NEW (stage 6)
packages/web/public/okf-format.md — rewritten author guide (stage 6)
```

---


> **Split note:** This is **Stage 2 of 6** of the OKF UML plan. Stages 1..1 already landed on `origin/main` before this run — their code exists in the repo; references to earlier tasks/stages point at code already on main. Implement ONLY the `### Task` sections in THIS file (Tasks 8-10). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context.

# Stage 2 — Parser/serializer: Attributes / Values / Relationships (incl. `as "…"` / `as [link]` association names), Body / Notes, and `annotates` resolution (spec rollout step 2)

### Task 8: okf line grammar module

**Files:**
- Create: `packages/okf/src/grammar.ts`
- Modify: `packages/okf/src/index.ts` (add exports)
- Test: `packages/okf/test/grammar.test.ts`

**Interfaces:**
- Consumes: types from Task 1.
- Produces (all exported from `@mc/okf`):
  - `isValidMultiplicity(s: string): boolean`
  - `parseAttributeLine(line: string, resolveSlug: (slug: string) => string | undefined): Attribute | null`
  - `parseValueLine(line: string): string | null`
  - `parseRelationshipLine(line: string): { kind: RelationshipKind; targetSlug: string; name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd } | null` — `name` is the optional `as …` association name: a plain string (`as "places"`) or `{ ref: <associationSlug> }` (`as [Places](./places.md)`; Task 9 remaps the slug to a node key)
  - `renderAttributeLine(a: Attribute, slugForRef: (key: string) => string | undefined): string`
  - `renderRelationshipLine(kind: RelationshipKind, targetTitle: string, targetSlug: string, fromEnd: RelEnd, toEnd: RelEnd, name?: string | { title: string; slug: string }): string` — `name` string → ` as "…"`; `{ title, slug }` → ` as [Title](./slug.md)`

- [ ] **Step 1: Write the failing tests**

Create `packages/okf/test/grammar.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import {
  isValidMultiplicity, parseAttributeLine, parseValueLine, parseRelationshipLine,
  renderAttributeLine, renderRelationshipLine,
} from "../src/grammar";

describe("isValidMultiplicity", () => {
  it.each(["1", "5", "*", "0..1", "1..*", "0..*", "2..5"])("accepts %s", s =>
    expect(isValidMultiplicity(s)).toBe(true));
  it.each(["0", "", "N", "1..", "..1", "5..2", "*..1", "01"])("rejects %s", s =>
    expect(isValidMultiplicity(s)).toBe(false));
});

describe("parseAttributeLine", () => {
  const resolve = (slug: string) => (slug === "money" ? "money" : undefined);
  it("bare token with default multiplicity", () => {
    expect(parseAttributeLine("- placedAt: Timestamp", resolve))
      .toEqual({ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" });
  });
  it("linked type with multiplicity", () => {
    expect(parseAttributeLine("- total: [Money](./money.md) [1]", resolve))
      .toEqual({ name: "total", type: { name: "Money", ref: "money" }, multiplicity: "1" });
  });
  it("unresolvable link keeps the display name as a token", () => {
    expect(parseAttributeLine("- addr: [Address](./address.md) [0..1]", resolve))
      .toEqual({ name: "addr", type: { name: "Address" }, multiplicity: "0..1" });
  });
  it("leading visibility", () => {
    expect(parseAttributeLine("- + id: OrderId [1]", resolve))
      .toEqual({ name: "id", type: { name: "OrderId" }, multiplicity: "1", visibility: "+" });
  });
  it("tolerates CRLF", () => {
    expect(parseAttributeLine("- a: B\r", resolve)).toEqual({ name: "a", type: { name: "B" }, multiplicity: "1" });
  });
  it("rejects non-attribute lines", () => {
    expect(parseAttributeLine("- just prose", resolve)).toBeNull();
  });
});

describe("parseValueLine", () => {
  it("reads a literal", () => expect(parseValueLine("- DRAFT")).toBe("DRAFT"));
  it("rejects blanks", () => expect(parseValueLine("-  ")).toBeNull());
});

describe("parseRelationshipLine", () => {
  it("associates with ends and roles", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md): 1 order to 1 buyer"))
      .toEqual({ kind: "associates", targetSlug: "customer",
        fromEnd: { multiplicity: "1", role: "order" }, toEnd: { multiplicity: "1", role: "buyer" } });
  });
  it("composes with range multiplicity", () => {
    expect(parseRelationshipLine("- composes [OrderLine](./order-line.md): 1 to 1..*"))
      .toEqual({ kind: "composes", targetSlug: "order-line",
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1..*" } });
  });
  it("specializes takes no ends", () => {
    expect(parseRelationshipLine("- specializes [Party](./party.md)"))
      .toEqual({ kind: "specializes", targetSlug: "party", fromEnd: {}, toEnd: {} });
  });
  it("captures an `as \"string\"` association name before the ends", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md) as \"places\": 1 order to 1 customer"))
      .toEqual({ kind: "associates", targetSlug: "customer", name: "places",
        fromEnd: { multiplicity: "1", role: "order" }, toEnd: { multiplicity: "1", role: "customer" } });
  });
  it("captures an `as [link]` association-class name as { ref: slug }", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1"))
      .toEqual({ kind: "associates", targetSlug: "customer", name: { ref: "places" },
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1" } });
  });
  it("allows a name on a no-ends verb too", () => {
    expect(parseRelationshipLine("- depends [PricingService](./pricing-service.md) as \"prices\""))
      .toEqual({ kind: "depends", targetSlug: "pricing-service", name: "prices", fromEnd: {}, toEnd: {} });
  });
  it("ends are REQUIRED for associates", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md)")).toBeNull();
  });
  it("ends are FORBIDDEN for depends", () => {
    expect(parseRelationshipLine("- depends [PricingService](./pricing-service.md): 1 to 1")).toBeNull();
  });
  it("rejects invalid multiplicities and unknown verbs", () => {
    expect(parseRelationshipLine("- associates [C](./c.md): N to 1")).toBeNull();
    expect(parseRelationshipLine("- likes [C](./c.md)")).toBeNull();
  });
});

describe("render round-trip", () => {
  it("attribute line", () => {
    const slugFor = (key: string) => (key === "money" ? "money" : undefined);
    expect(renderAttributeLine({ name: "total", type: { name: "Money", ref: "money" }, multiplicity: "1" }, slugFor))
      .toBe("- total: [Money](./money.md)");
    expect(renderAttributeLine({ name: "addr", type: { name: "Address" }, multiplicity: "0..1", visibility: "-" }, slugFor))
      .toBe("- - addr: Address [0..1]");
  });
  it("relationship line", () => {
    expect(renderRelationshipLine("composes", "OrderLine", "order-line", { multiplicity: "1" }, { multiplicity: "1..*", role: "lines" }))
      .toBe("- composes [OrderLine](./order-line.md): 1 to 1..* lines");
    expect(renderRelationshipLine("specializes", "Party", "party", {}, {}))
      .toBe("- specializes [Party](./party.md)");
  });
  it("relationship line with a string name", () => {
    expect(renderRelationshipLine("associates", "Customer", "customer", { multiplicity: "1" }, { multiplicity: "1" }, "places"))
      .toBe("- associates [Customer](./customer.md) as \"places\": 1 to 1");
  });
  it("relationship line with an association-class link name", () => {
    expect(renderRelationshipLine("associates", "Customer", "customer", { multiplicity: "1" }, { multiplicity: "1" }, { title: "Places", slug: "places" }))
      .toBe("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1");
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- grammar`
Expected: FAIL (module missing).

- [ ] **Step 3: Create `packages/okf/src/grammar.ts`**

```ts
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

// - [visibility ]name: Type-or-link [multiplicity]
const ATTR_RE = /^- (?:([+\-#~]) )?([A-Za-z_][A-Za-z0-9_]*): (.+)$/;
const LINK_RE = /^\[([^\]]+)\]\(\.\/(.+?)\.md\)$/;

export function parseAttributeLine(line: string, resolveSlug: (slug: string) => string | undefined): Attribute | null {
  const m = ATTR_RE.exec(stripCr(line).trim());
  if (!m) return null;
  let rest = m[3].trim();
  let multiplicity = "1";
  const mm = /^(.*?)\s+\[([^\]]+)\]$/.exec(rest);
  if (mm && isValidMultiplicity(mm[2])) { rest = mm[1].trim(); multiplicity = mm[2]; }
  const link = LINK_RE.exec(rest);
  let type: Attribute["type"];
  if (link) {
    const ref = resolveSlug(basename(link[2]));
    type = ref ? { name: link[1], ref } : { name: link[1] };
  } else {
    if (!rest || /[[\]()]/.test(rest)) return null; // malformed link / brackets → not an attribute
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
const REL_RE = /^- (associates|aggregates|composes|specializes|implements|depends) \[([^\]]+)\]\(\.\/(.+?)\.md\)(?: as (?:"([^"]*)"|\[([^\]]+)\]\(\.\/(.+?)\.md\)))?(?:\s*:\s*(.+))?$/;
const END_RE = /^(\S+)(?:\s+([A-Za-z][A-Za-z0-9_]*))?$/;

export function parseRelationshipLine(
  line: string,
): { kind: RelationshipKind; targetSlug: string; name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd } | null {
  const m = REL_RE.exec(stripCr(line).trim());
  if (!m) return null;
  const kind = m[1] as RelationshipKind;
  const endsRaw = m[7];
  const needsEnds = ENDED_KINDS.has(kind);
  if (needsEnds !== Boolean(endsRaw)) return null; // ends required XOR forbidden (spec context rules)
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
  const mult = a.multiplicity && a.multiplicity !== "1" ? ` [${a.multiplicity}]` : "";
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
  if (!ENDED_KINDS.has(kind)) return `- ${kind} ${link}${nameStr}`;
  return `- ${kind} ${link}${nameStr}: ${renderEnd(fromEnd)} to ${renderEnd(toEnd)}`;
}
```

Add to `packages/okf/src/index.ts`:

```ts
export {
  isValidMultiplicity, parseAttributeLine, parseValueLine, parseRelationshipLine,
  renderAttributeLine, renderRelationshipLine,
} from "./grammar";
```

- [ ] **Step 4: Run to verify pass**

Run: `pnpm --filter @mc/okf test -- grammar`
Expected: PASS. Note the render test `"- - addr: Address [0..1]"` — a `-` visibility after the bullet dash is correct per the format (`- [visibility ]name: …`).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/grammar.ts packages/okf/src/index.ts packages/okf/test/grammar.test.ts
git commit -m "feat(okf): attribute/values/relationship line grammar (spec BNF)"
```

### Task 9: parse.ts — new-format primary pass (+ legacy fallback intact)

**Files:**
- Modify: `packages/okf/src/parse.ts` (Task 2 version)
- Test: `packages/okf/test/format.test.ts` (new, parse half)

**Interfaces:**
- Consumes: Task 8 grammar functions.
- Produces: `parseBundle` reads frontmatter `type` / `stereotype` (scalar or list) / `abstract` / `title` / `description`; sections `## Attributes`, `## Values`, `## Relationships` (incl. the optional `as "…"` / `as [link]` association name — the link form resolved slug→key onto `edge.name = { ref }`), `## Body` (for `uml.Note`); merges reciprocal `associates`; desugars a classifier's `## Notes` bullets into standalone self-anchored `uml.Note` nodes; resolves a `uml.Note`'s `annotates` lines into `node.annotates` anchors (classifier / named-association / endpoint form); carries unrecognized sections on `node.extra`. A `uml.Association` doc parses as an ordinary classifier (via `## Attributes`), NOT as an edge and never requiring ends. Docs WITHOUT any new-format section still go through the Task 2 legacy readers (Schema/Joins/prose) untouched — the existing legacy tests keep passing.

- [ ] **Step 1: Write the failing tests**

Create `packages/okf/test/format.test.ts` (parse half):

```ts
import { describe, it, expect } from "vitest";
import { parseBundle } from "../src/parse";

const order = `---
type: uml.Class
stereotype: [aggregateRoot, entity]
abstract: false
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId [1]
- placedAt: Timestamp
- status: [OrderStatus](./order-status.md)
- shippingAddress: [Address](./address.md) [0..1]

## Relationships
- associates [Customer](./customer.md) as "places": 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- depends [PricingService](./pricing-service.md)

## Glossary
Free-form section the parser does not know.
`;

const orderStatus = `---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
`;

const customer = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- associates [Order](./order.md): 1 customer to 1 order\n`;
const orderLine = `---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n`;
const pricing = `---\ntype: uml.Interface\ntitle: PricingService\n---\n# PricingService\n`;

const files = {
  "m/order.md": order, "m/order-status.md": orderStatus, "m/customer.md": customer,
  "m/order-line.md": orderLine, "m/pricing-service.md": pricing,
};

describe("parseBundle — UML format", () => {
  const g = parseBundle(files);
  const orderNode = g.nodes.find(n => n.key === "order")!;

  it("reads frontmatter type, stereotypes (list) and description", () => {
    expect(orderNode.type).toBe("uml.Class");
    expect(orderNode.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(orderNode.abstract).toBeUndefined();       // false → omitted
    expect(orderNode.description).toBe("A customer's placed order.");
  });
  it("reads scalar stereotype too", () => {
    const g2 = parseBundle({ "m/a.md": `---\ntype: uml.Class\nstereotype: entity\ntitle: A\n---\n# A\n` });
    expect(g2.nodes[0].stereotypes).toEqual(["entity"]);
  });
  it("parses attributes with refs and multiplicities", () => {
    expect(orderNode.attributes).toEqual([
      { name: "id", type: { name: "OrderId" }, multiplicity: "1" },
      { name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" },
      { name: "status", type: { name: "OrderStatus", ref: "order-status" }, multiplicity: "1" },
      { name: "shippingAddress", type: { name: "Address" }, multiplicity: "0..1" }, // no address.md → token
    ]);
  });
  it("parses enum values", () => {
    expect(g.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED"]);
  });
  it("parses relationships with kinds and ends", () => {
    const compose = g.edges.find(e => e.kind === "composes")!;
    expect(compose).toMatchObject({ from: "order", to: "order-line" });
    expect(compose.fromEnd).toEqual({ multiplicity: "1" });
    expect(compose.toEnd).toEqual({ multiplicity: "1..*", role: "lines" });
    expect(g.edges.find(e => e.kind === "depends")).toMatchObject({ from: "order", to: "pricing-service" });
  });
  it("merges reciprocal associates into one bidirectional edge (first declaration wins ends)", () => {
    const assoc = g.edges.filter(e => e.kind === "associates");
    expect(assoc).toHaveLength(1);
    expect(assoc[0]).toMatchObject({ from: "order", to: "customer", bidirectional: true, name: "places" });
    expect(assoc[0].fromEnd).toMatchObject({ multiplicity: "1", role: "order", navigable: true });
    expect(assoc[0].toEnd).toMatchObject({ multiplicity: "1", role: "customer", navigable: true });
  });
  it("one-way associates sets only the far end navigable", () => {
    const g2 = parseBundle({
      "m/a.md": `---\ntitle: A\n---\n# A\n\n## Relationships\n- associates [B](./b.md): 1 to *\n`,
      "m/b.md": `---\ntitle: B\n---\n# B\n`,
    });
    expect(g2.edges[0].fromEnd.navigable).toBeUndefined();
    expect(g2.edges[0].toEnd.navigable).toBe(true);
  });
  it("carries unknown sections on extra, never dropped", () => {
    expect(orderNode.extra).toContain("## Glossary");
    expect(orderNode.extra).toContain("Free-form section");
  });
  it("reads abstract: true", () => {
    const g2 = parseBundle({ "m/a.md": `---\ntype: uml.Class\nabstract: true\ntitle: A\n---\n# A\n` });
    expect(g2.nodes[0].abstract).toBe(true);
  });
});

describe("parseBundle — association classes (uml.Association)", () => {
  const orderAC = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md) as [Places](./places.md): 1 order to 1 customer\n`;
  const customerAC = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;
  const places = `---\ntype: uml.Association\ntitle: Places\n---\n# Places\n\n## Attributes\n- placedAt: Timestamp [1]\n- channel: [Channel](./channel.md) [1]\n`;
  const channel = `---\ntype: uml.Class\ntitle: Channel\n---\n# Channel\n`;
  const g = parseBundle({ "m/order.md": orderAC, "m/customer.md": customerAC, "m/places.md": places, "m/channel.md": channel });

  it("resolves the `as [link]` name to a { ref: nodeKey } on the edge", () => {
    const e = g.edges.find(x => x.from === "order" && x.to === "customer")!;
    expect(e.name).toEqual({ ref: "places" });
  });
  it("the association class is an ordinary classifier node with attributes — not an edge, no ends", () => {
    const ac = g.nodes.find(n => n.key === "places")!;
    expect(ac.type).toBe("uml.Association");
    expect(ac.attributes).toEqual([
      { name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" },
      { name: "channel", type: { name: "Channel", ref: "channel" }, multiplicity: "1" },
    ]);
    // The ends stay on the inline bullet; order→customer remains a direct edge.
    expect(g.edges.filter(e => e.from === "places" || e.to === "places")).toHaveLength(0);
  });
});

describe("parseBundle — notes (uml.Note)", () => {
  const noteDoc = `---\ntype: uml.Note\ntitle: Domestic-only\n---\n# Domestic-only\n\n## Body\nOnly valid for domestic customers.\n\n## Relationships\n- annotates [Order](./order.md)\n- annotates [Order](./order.md) as "places"\n`;
  const orderN = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md) as "places": 1 to 1\n\n## Notes\n- Drafts expire after 24h.\n`;
  const customerN = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;
  const g = parseBundle({ "m/domestic-only.md": noteDoc, "m/order.md": orderN, "m/customer.md": customerN });

  it("a uml.Note carries its ## Body and anchor targets (classifier + named association)", () => {
    const note = g.nodes.find(n => n.key === "domestic-only")!;
    expect(note.type).toBe("uml.Note");
    expect(note.body).toBe("Only valid for domestic customers.");
    expect(note.annotates).toEqual([
      { targetKey: "order" },
      { sourceKey: "order", name: "places" },
    ]);
    // annotates never becomes a ModelEdge.
    expect(g.edges.some(e => e.kind === "annotates")).toBe(false);
  });
  it("the ## Notes shorthand desugars to a self-anchored uml.Note node", () => {
    const shorthand = g.nodes.find(n => n.type === "uml.Note" && n.body === "Drafts expire after 24h.")!;
    expect(shorthand).toBeTruthy();
    expect(shorthand.annotates).toEqual([{ targetKey: "order" }]);
    // The host node does NOT keep ## Notes on `extra` (it desugared, not unknown-carried).
    expect(g.nodes.find(n => n.key === "order")!.extra ?? "").not.toContain("## Notes");
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- format`
Expected: FAIL (attributes empty, no `## Relationships` handling).

- [ ] **Step 3: Extend `packages/okf/src/parse.ts`**

Add a section splitter and the primary pass; the Task 2 legacy code stays as the fallback. Insert after `basename`:

```ts
// Split a body into `## `-level sections. `pre` = text before the first `##`.
function splitSections(body: string): { pre: string; sections: { title: string; raw: string; lines: string[] }[] } {
  const lines = body.split("\n");
  const sections: { title: string; raw: string; lines: string[] }[] = [];
  let pre: string[] = [];
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
```

Rework the node loop: for each doc, compute `const { sections } = splitSections(body);` and:

```ts
    const attrSection = sections.find(s => /^attributes$/i.test(s.title));
    const valSection = sections.find(s => /^values$/i.test(s.title));
    const relSection = sections.find(s => /^relationships$/i.test(s.title));
    const bodySection = sections.find(s => /^body$/i.test(s.title));
    const notesSection = sections.find(s => /^notes$/i.test(s.title));
    const isNote = data.type === "uml.Note";
    const isNewFormat = Boolean(attrSection || valSection || relSection || bodySection || notesSection);
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
      ? relSection.lines.map(l => parseAnnotatesLine(l, s => slugToKeyLater(s))).filter((a): a is NoteAnchor => a !== null)
      : [];
    nodes.push({
      key,
      type: typeof data.type === "string" && data.type ? data.type : "uml.Class",
      title,
      stereotypes,
      ...(data.abstract === true ? { abstract: true } : {}),
      ...(data.description ? { description: data.description } : {}),
      attributes: attrSection
        ? attrSection.lines.map(l => parseAttributeLine(l, s => slugToKeyLater(s))).filter((a): a is Attribute => a !== null)
        : isNewFormat ? [] : parseLegacySchema(body),
      ...(valSection ? { values: valSection.lines.map(parseValueLine).filter((v): v is string => v !== null) } : {}),
      ...(bodyText ? { body: bodyText } : {}),
      ...(annotates.length ? { annotates } : {}),
      position: (data.owox && data.owox.position) || { x: 0, y: 0 },
      ...(extra ? { extra } : {}),
    });

    // `## Notes` shorthand on a classifier: each bullet desugars to a standalone
    // self-anchored uml.Note node (one internal model — every note is a uml.Note).
    // Synthetic key/title are irrelevant to round-trip: Task 10 detects a note that
    // anchors exactly its host (and nothing else) and re-collapses it to `## Notes`.
    if (!isNote && notesSection) {
      notesSection.lines
        .map(parseValueLine)                             // reuse "- <text>" bullet reader
        .filter((t): t is string => t !== null)
        .forEach((text, i) => desugaredNotes.push({
          key: `${key}--note-${i + 1}`,
          type: "uml.Note",
          title: `Note on ${title}`,
          stereotypes: [],
          attributes: [],
          body: text,
          annotates: [{ targetKey: key }],
          position: { x: 0, y: 0 },
        }));
    }
```

(`desugaredNotes: ModelNode[]` is declared alongside `nodes`; after the doc loop, `nodes.push(...desugaredNotes)`. Because `## Notes` is in `KNOWN_SECTIONS`, a desugared section is NOT also carried on the host's `extra`.)

**Two-pass note:** attribute `ref`, `annotates` and association-name resolution all need the full `slugToKey` map, which isn't complete during the first node loop. Structure it as: pass 1 builds `slugToKey` only (path → key); pass 2 builds nodes using `const slugToKeyLater = (s: string) => slugToKey.get(s);`. (The Task 2 code already loops docs twice — fold node construction into the second loop.)

Relationship pass (replaces the legacy joins scan for new-format docs; legacy docs keep the old scan). A `uml.Note`'s `## Relationships` are `annotates` anchors (handled in the node loop above) and are skipped here:

```ts
  const rawRels: { from: string; to: string; kind: RelationshipKind; name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd }[] = [];
  for (const [path, text] of docs) {
    const { data, body } = parseFrontmatter(text);
    if (data.type === "uml.Note") continue;   // note anchors are not edges
    const fromKey = (data.owox && data.owox.key) || basename(path);
    const { sections } = splitSections(body);
    const relSection = sections.find(s => /^relationships$/i.test(s.title));
    if (relSection) {
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
    } else {
      // legacy joins + prose recovery (existing Task 2 code) push
      // { from, to, kind: "associates", ...endsFromCardinality(card, false) }
    }
  }
```

Merge pass replaces the Task 2 collapse (threading `name`; first declaration wins the name as it wins ends):

```ts
  const edges: ModelEdge[] = [];
  const seen = new Map<string, ModelEdge>();
  for (const r of rawRels) {
    if (r.kind === "associates") {
      const pairKey = ["assoc", ...[r.from, r.to].sort()].join("|");
      const ex = seen.get(pairKey);
      if (ex) {
        // Reciprocity: both docs declare → bidirectional, both ends navigable.
        // First declaration's ends + name win (pinned decision 6).
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
```

Imports to add at top: `parseAttributeLine, parseValueLine, parseRelationshipLine` from `./grammar`; `RelationshipKind, RelEnd, NoteAnchor` types.

- [ ] **Step 4: Run all okf tests**

Run: `pnpm --filter @mc/okf test`
Expected: PASS — format.test parse half AND the legacy suites (parse-owox, prose-join, roundtrip) — the fallback path must not regress.

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/parse.ts packages/okf/test/format.test.ts
git commit -m "feat(okf): parse Attributes/Values/Relationships sections with reciprocity merge"
```

### Task 10: serialize.ts — emit the UML format, lossless round-trip + STAGE 2 GATE

**Files:**
- Modify: `packages/okf/src/serialize.ts` (replace the Task 3 interim emission)
- Modify: `packages/okf/test/serialize.test.ts`, `packages/okf/test/roundtrip.test.ts` (update to new emission), `packages/okf/test/format.test.ts` (add round-trip half)

**Interfaces:**
- Consumes: `renderAttributeLine`, `renderRelationshipLine` from grammar; `ENDED_KINDS`.
- Produces: `serializeBundle(graph, projectTitle = "Model")` emitting frontmatter (`type`, `stereotype` list, `abstract`, `title`, `description`), `## Attributes`, `## Values`, `## Relationships` (reciprocal line in the far doc for bidirectional associates; the optional `as "…"` / `as [link]` association name — the `{ ref }` form resolved to the association node's title+slug), `## Body` for `uml.Note` docs, `extra` re-emitted verbatim at the end. Self-anchored notes (a `uml.Note` whose only anchor is a single classifier) are collapsed to a `## Notes` list on that classifier instead of getting their own doc. Round-trip `parseBundle(serializeBundle(g).files)` is lossless for the UML profile.

- [ ] **Step 1: Write the failing round-trip tests**

Append to `packages/okf/test/format.test.ts`:

```ts
import { serializeBundle } from "../src/serialize";
import type { ModelGraph } from "../src/types";

describe("UML format round-trip (lossless)", () => {
  const graph: ModelGraph = {
    nodes: [
      { key: "order", type: "uml.Class", title: "Order", stereotypes: ["aggregateRoot", "entity"],
        description: "A customer's placed order.",
        attributes: [
          { name: "id", type: { name: "OrderId" }, multiplicity: "1" },
          { name: "status", type: { name: "OrderStatus", ref: "order-status" }, multiplicity: "1" },
          { name: "note", type: { name: "String" }, multiplicity: "0..1", visibility: "-" },
        ],
        position: { x: 0, y: 0 }, extra: "## Glossary\nKeep me." },
      { key: "order-line", type: "uml.Class", title: "OrderLine", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "order-status", type: "uml.Enum", title: "OrderStatus", stereotypes: [], attributes: [],
        values: ["DRAFT", "PLACED"], position: { x: 0, y: 0 } },
      { key: "base", type: "uml.Class", title: "Base", stereotypes: [], abstract: true, attributes: [], position: { x: 0, y: 0 } },
    ],
    edges: [
      { id: "e1", kind: "composes", from: "order", to: "order-line",
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1..*", role: "lines" }, bidirectional: false },
      { id: "e2", kind: "associates", from: "order", to: "customer", name: "places",
        fromEnd: { multiplicity: "1", navigable: true }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: true },
      { id: "e3", kind: "specializes", from: "order", to: "base", fromEnd: {}, toEnd: {}, bidirectional: false },
    ],
    diagrams: [],
  };

  const files = serializeBundle(graph, "Shop").files;
  const back = parseBundle(files);
  const order2 = back.nodes.find(n => n.key === "order")!;

  it("emits the spec sections", () => {
    const doc = files["shop/order.md"];
    expect(doc).toContain("stereotype: [\"aggregateRoot\", \"entity\"]");
    expect(doc).toContain("## Attributes");
    expect(doc).toContain("- id: OrderId");
    expect(doc).toContain("- status: [OrderStatus](./order-status.md)");
    expect(doc).toContain("- - note: String [0..1]");
    expect(doc).toContain("## Relationships");
    expect(doc).toContain("- composes [OrderLine](./order-line.md): 1 to 1..* lines");
    expect(doc).toContain("- specializes [Base](./base.md)");
    expect(doc).toContain("## Glossary\nKeep me.");
    expect(files["shop/order-status.md"]).toContain("## Values\n- DRAFT\n- PLACED");
    expect(files["shop/base.md"]).toContain("abstract: true");
  });
  it("bidirectional associates (with a string name) appears in BOTH docs and merges back", () => {
    expect(files["shop/order.md"]).toContain("- associates [Customer](./customer.md) as \"places\": 1 to 1");
    expect(files["shop/customer.md"]).toContain("- associates [Order](./order.md) as \"places\": 1 to 1");
    const assoc = back.edges.find(e => e.kind === "associates")!;
    expect(assoc.bidirectional).toBe(true);
    expect(assoc.name).toBe("places");
  });
  it("round-trips node substance losslessly", () => {
    expect(order2.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(order2.attributes).toEqual(graph.nodes[0].attributes);
    expect(order2.extra).toContain("## Glossary");
    expect(back.nodes.find(n => n.key === "base")!.abstract).toBe(true);
    expect(back.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED"]);
  });
  it("round-trips edge kinds and ends", () => {
    const compose = back.edges.find(e => e.kind === "composes")!;
    expect(compose.fromEnd.multiplicity).toBe("1");
    expect(compose.toEnd).toMatchObject({ multiplicity: "1..*", role: "lines" });
    expect(back.edges.find(e => e.kind === "specializes")).toMatchObject({ from: "order", to: "base" });
  });
});

describe("UML format round-trip — association classes & notes (lossless)", () => {
  const graph: ModelGraph = {
    nodes: [
      { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "customer", type: "uml.Class", title: "Customer", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      { key: "places", type: "uml.Association", title: "Places", stereotypes: [],
        attributes: [{ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" }], position: { x: 0, y: 0 } },
      // A standalone note keeps its own doc because it anchors MORE THAN its host
      // (multi-target ⇒ not collapsible); the self-anchored note collapses to `## Notes`.
      { key: "domestic", type: "uml.Note", title: "Domestic-only", stereotypes: [], attributes: [],
        body: "Only valid for domestic customers.",
        annotates: [{ targetKey: "order" }, { targetKey: "customer" }], position: { x: 0, y: 0 } },
      { key: "order--note-1", type: "uml.Note", title: "Note on Order", stereotypes: [], attributes: [],
        body: "Drafts expire after 24h.", annotates: [{ targetKey: "order" }], position: { x: 0, y: 0 } },
    ],
    edges: [
      { id: "e1", kind: "associates", from: "order", to: "customer", name: { ref: "places" },
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false },
    ],
    diagrams: [],
  };
  const files = serializeBundle(graph, "Shop").files;
  const back = parseBundle(files);

  it("emits the association-class link name and re-resolves it to { ref }", () => {
    expect(files["shop/order.md"]).toContain("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1");
    expect(files["shop/places.md"]).toContain("type: \"uml.Association\"");
    expect(files["shop/places.md"]).toContain("- placedAt: Timestamp");
    expect(back.edges.find(e => e.from === "order" && e.to === "customer")!.name).toEqual({ ref: "places" });
  });
  it("emits a multi-target note's ## Body + annotates and reads it back (not collapsed)", () => {
    const note = files["shop/domestic-only.md"];   // file slug derives from the title "Domestic-only"
    expect(note).toContain("type: \"uml.Note\"");
    expect(note).toContain("## Body\nOnly valid for domestic customers.");
    expect(note).toContain("## Relationships\n- annotates [Order](./order.md)\n- annotates [Customer](./customer.md)");
    expect(back.nodes.find(n => n.type === "uml.Note" && n.title === "Domestic-only")!.annotates)
      .toEqual([{ targetKey: "order" }, { targetKey: "customer" }]);
  });
  it("collapses a self-anchored note back to `## Notes` on the host (lossless)", () => {
    // The self-anchored note is NOT emitted as its own doc; it rides on order.md.
    expect(files["shop/order--note-1.md"]).toBeUndefined();
    expect(files["shop/order.md"]).toContain("## Notes\n- Drafts expire after 24h.");
    const desugared = back.nodes.filter(n => n.type === "uml.Note" && n.body === "Drafts expire after 24h.");
    expect(desugared).toHaveLength(1);
    expect(desugared[0].annotates).toEqual([{ targetKey: "order" }]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- format`
Expected: round-trip half FAILS (serializer still emits Schema/Joins).

- [ ] **Step 3: Replace the emission in `packages/okf/src/serialize.ts`**

Keep `serializeBundle`'s slug/index scaffolding from Task 3; replace `renderNode`, add note-collapse coordination, and drop `cardToken`/`cardinalitySuffix`:

```ts
import type { ModelGraph, ModelNode, NoteAnchor } from "./types";
import { renderAttributeLine, renderRelationshipLine } from "./grammar";
import { slugify, renderFrontmatter } from "./slug";

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
```

`serializeBundle` changes to drive the collapse: before assigning slugs / index rows / node files, compute which notes collapse and onto whom:

```ts
  const notesByHost = new Map<string, string[]>();
  const collapsed = new Set<string>();
  for (const n of graph.nodes) {
    const host = selfAnchorHost(n);
    if (host && n.body) {
      (notesByHost.get(host) ?? notesByHost.set(host, []).get(host)!).push(n.body);
      collapsed.add(n.key);
    }
  }
```

Then **every** node loop in `serializeBundle` (slug assignment, `files[...] = renderNode(...)`, and the index-table rows) must `if (collapsed.has(n.key)) continue;` so a collapsed note gets no slug, no file, and no index row. The node render call becomes `renderNode(n, graph, slugByKey, notesByHost.get(n.key) ?? [])`.

- [ ] **Step 4: Update the interim tests**

`packages/okf/test/serialize.test.ts`: the "interim legacy emission" describe block's expectations change — Schema/Joins assertions become Attributes/Relationships assertions (mirror the Step 1 expectations; keep the index/bundle-layout tests as-is). `packages/okf/test/roundtrip.test.ts`: the first test's assertions hold unchanged (keys/kind survive); the `[N:1]` suffix test now asserts exact multiplicity survival:

```ts
    expect(back.edges[0].fromEnd.multiplicity).toBe("*");
    expect(back.edges[0].toEnd.multiplicity).toBe("1");
```

(now via `## Relationships` — no change to the assertion, only the emission changed). The "mutual join lines" test keeps passing via the legacy fallback (it hand-writes `## Joins`) — leave it as permanent legacy-import coverage.

- [ ] **Step 5: Full okf suite + STAGE 2 GATE**

```bash
pnpm --filter @mc/okf build && pnpm --filter @mc/okf test
pnpm lint && pnpm build && pnpm -r test
```

Expected: all green (web's `guideExample.test.ts` exercises the legacy fallback; `io.test.ts` round-trips through the new emission).

- [ ] **Step 6: Commit + push (stage 2 lands)**

```bash
git add packages/okf/src/serialize.ts packages/okf/test
git commit -m "feat(okf): emit UML-profile markdown; lossless round-trip incl. extra sections"
git push origin main
```

---


## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
