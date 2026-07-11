# OKF Agnostic Profiles + UML Domain Model — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn OKF Canvas from a data-mart/ERD tool into a profile-agnostic modeling canvas whose first profile is a UML class-diagram / domain model, per the approved spec `docs/superpowers/specs/2026-07-11-okf-agnostic-profiles-uml-domain-design.md`.

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
  components/canvas/nodes/ — NEW (stage 3): shared.tsx, GenericNode.tsx, uml.tsx, registry.ts, OkfNode.tsx (MartNode.tsx DELETED)
  components/canvas/{Canvas.tsx, edges.ts, RelEdge.tsx, layoutSize.ts, Dock.tsx, DiagramTabs.tsx (NEW, stage 5)}
  components/inspector/{ObjectInspector.tsx, RelationshipInspector.tsx, AttributeEditor.tsx (NEW, replaces SchemaEditor.tsx), ExternalRefs.tsx (NEW, stage 5)}
  profiles/ — NEW (stage 4): index.ts, umlDomain.ts
  templates/orders-domain.ts — NEW (stage 6)
packages/web/public/okf-format.md — rewritten author guide (stage 6)
```

---

# Stage 1 — Generalize the data model (spec rollout step 1)

Everything compiles and passes on the new `ModelGraph`; markdown format unchanged (legacy emission). Tasks 1–3 (okf) and 4–7 (web) form one stage; run the full gate at the end of Task 7 before pushing.

### Task 1: okf core types + migration module

**Files:**
- Rewrite: `packages/okf/src/types.ts` (currently 44 lines, mart-shaped)
- Create: `packages/okf/src/migrate.ts`
- Modify: `packages/okf/src/index.ts` (4 lines)
- Test: `packages/okf/test/migrate.test.ts`

**Interfaces:**
- Consumes: nothing (root task).
- Produces: every pinned type above; `splitType(type: string)`; `migrateGraph(raw: unknown): ModelGraph | null`; `endsFromCardinality(cardinality: string | undefined, bidirectional: boolean): { fromEnd: RelEnd; toEnd: RelEnd }`. All exported from `@mc/okf`.

- [ ] **Step 1: Write the failing tests**

Create `packages/okf/test/migrate.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { migrateGraph, endsFromCardinality, splitType } from "../src/index";

const legacy = {
  storageId: "stor_1",
  nodes: [{
    key: "orders", title: "Orders", inputSource: "SQL", description: "d", status: "created", owoxId: "x",
    position: { x: 5, y: 6 },
    schema: [
      { name: "id", type: "STRING", pk: true, alias: "oid", description: "Unique id" },
      { name: "total", type: "NUMERIC", pk: false },
    ],
  }],
  edges: [{ id: "e1", from: "orders", to: "customers", keys: [{ left: "customer_id", right: "id" }],
            bidirectional: false, cardinality: "N:1", sourceHandle: "right" }],
};

describe("migrateGraph", () => {
  it("maps a legacy mart graph onto the UML model", () => {
    const g = migrateGraph(legacy)!;
    expect(g.diagrams).toEqual([]);
    const n = g.nodes[0];
    expect(n).toMatchObject({ key: "orders", type: "uml.Class", title: "Orders", stereotypes: [], position: { x: 5, y: 6 } });
    expect(n.attributes).toEqual([
      { name: "id", type: { name: "STRING" }, multiplicity: "1", description: "Unique id" },
      { name: "total", type: { name: "NUMERIC" }, multiplicity: "1" },
    ]);
    expect((n as Record<string, unknown>).schema).toBeUndefined();
    const e = g.edges[0];
    expect(e).toMatchObject({ id: "e1", kind: "associates", from: "orders", to: "customers", bidirectional: false, sourceHandle: "right" });
    expect(e.fromEnd).toEqual({ multiplicity: "*" });
    expect(e.toEnd).toEqual({ multiplicity: "1", navigable: true });
  });
  it("passes a current-shape graph through and defaults missing diagrams", () => {
    const g = migrateGraph({ nodes: [], edges: [] })!;
    expect(g).toEqual({ nodes: [], edges: [], diagrams: [] });
  });
  it("returns null for garbage", () => {
    expect(migrateGraph(null)).toBeNull();
    expect(migrateGraph({ nodes: "x" })).toBeNull();
  });
  it("bidirectional legacy edges get both ends navigable", () => {
    const g = migrateGraph({ ...legacy, edges: [{ id: "e1", from: "a", to: "b", keys: [], bidirectional: true }] })!;
    expect(g.edges[0].fromEnd.navigable).toBe(true);
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});

describe("endsFromCardinality", () => {
  it("maps 1:N", () => {
    expect(endsFromCardinality("1:N", false)).toEqual({ fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*", navigable: true } });
  });
  it("no cardinality → only navigability", () => {
    expect(endsFromCardinality(undefined, false)).toEqual({ fromEnd: {}, toEnd: { navigable: true } });
  });
});

describe("splitType", () => {
  it("splits family.Metaclass", () => expect(splitType("uml.Class")).toEqual({ family: "uml", metaclass: "Class" }));
  it("rejects opaque tokens", () => {
    expect(splitType("Data Mart")).toBeNull();
    expect(splitType("uml.")).toBeNull();
    expect(splitType("noDot")).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- migrate`
Expected: FAIL — `migrateGraph` is not exported.

- [ ] **Step 3: Rewrite `packages/okf/src/types.ts`**

Replace the whole file with:

```ts
// ── Profile-agnostic modeling core ───────────────────────────────────────────
// Nodes are classifiers dispatched on `type` = "family.Metaclass"; everything
// domain-specific rides as data (stereotypes). Unknown types render generically.

export type Visibility = "+" | "-" | "#" | "~";

/** An attribute's type: a display token, optionally resolved to another classifier. */
export interface TypeRef { name: string; ref?: string }

export interface Attribute {
  name: string;
  type: TypeRef;
  /** UML multiplicity string as authored ("1", "0..1", "*", "1..*", "2..5"). Parser defaults to "1". */
  multiplicity: string;
  visibility?: Visibility;
  description?: string;
}

// "annotates" is a uml.Note-only verb; it never produces a ModelEdge (anchors live on the note node).
export const RELATIONSHIP_KINDS = ["associates", "aggregates", "composes", "specializes", "implements", "depends", "annotates"] as const;
export type RelationshipKind = (typeof RELATIONSHIP_KINDS)[number];

/** Verbs that take `: <near> to <far>` ends. The rest forbid them. */
export const ENDED_KINDS: ReadonlySet<RelationshipKind> = new Set(["associates", "aggregates", "composes"]);

export interface RelEnd { multiplicity?: string; role?: string; navigable?: boolean }

/** A uml.Note anchor: a classifier, a NAMED association, or an association addressed by its endpoint (unnamed). */
export type NoteAnchor =
  | { targetKey: string }
  | { sourceKey: string; name: string }
  | { sourceKey: string; kind: RelationshipKind; targetKey: string };

export interface ModelNode {
  key: string;
  /** Structured dispatch key "family.Metaclass" (e.g. "uml.Class") or an opaque legacy token. */
  type: string;
  title: string;
  stereotypes: string[];
  abstract?: boolean;
  description?: string;
  attributes: Attribute[];
  /** uml.Enum literals. */
  values?: string[];
  /** uml.Note markdown body (from ## Body). */
  body?: string;
  /** uml.Note anchor targets; the ## Notes shorthand desugars into a self-anchored note. */
  annotates?: NoteAnchor[];
  position: { x: number; y: number };
  /** Raw markdown of unrecognized ## sections — carried through round-trip, never dropped. */
  extra?: string;
}

export interface ModelEdge {
  id: string;
  kind: RelationshipKind;
  /** Declaring/near end: whole for aggregates/composes, child for specializes/implements, dependent for depends. */
  from: string;
  /** Far end: part / parent / interface / dependency target. */
  to: string;
  /** Optional UML association name: a string reading-label (also the note anchor handle) OR
   *  a ref to a uml.Association node key (association class). Rendered near the line midpoint. */
  name?: string | { ref: string };
  fromEnd: RelEnd;
  toEnd: RelEnd;
  /** Derived from reciprocity: both docs declared the association. */
  bidirectional: boolean;
  // Canvas-only hints for which ports the edge attaches to (not encoded in OKF).
  sourceHandle?: string | null;
  targetHandle?: string | null;
}

export interface DiagramHints {
  emphasize?: string[];
  /** Node keys drawn as collapsed ref chips instead of full boxes. */
  collapse?: string[];
}

/** A curated, profiled view over nodes — not a classifier. */
export interface Diagram {
  key: string;
  title: string;
  profile: string;
  members: string[];
  hints?: DiagramHints;
}

export interface ModelGraph {
  nodes: ModelNode[];
  edges: ModelEdge[];
  /** Empty array ⇒ the canvas shows one implicit diagram containing every node. */
  diagrams: Diagram[];
}

/** Split "family.Metaclass". Null for opaque/legacy tokens. */
export function splitType(type: string): { family: string; metaclass: string } | null {
  const m = /^([a-z][a-z0-9]*)\.([A-Za-z][A-Za-z0-9]*)$/.exec(type);
  return m ? { family: m[1], metaclass: m[2] } : null;
}
```

- [ ] **Step 4: Create `packages/okf/src/migrate.ts`**

```ts
import type { Attribute, ModelEdge, ModelGraph, ModelNode, RelEnd } from "./types";

// Pre-UML (data-mart era) shapes as found in old localStorage payloads and
// shared URLs. They exist only here; the rest of the codebase never sees them.
interface LegacyField { name: string; type: string; pk?: boolean; alias?: string; description?: string }
interface LegacyNode {
  key: string; title?: string; description?: string;
  schema?: LegacyField[]; position?: { x: number; y: number };
  [k: string]: unknown; // inputSource/status/owoxId/definition/… — dropped
}
interface LegacyEdge {
  id: string; from: string; to: string;
  bidirectional?: boolean; cardinality?: string;
  sourceHandle?: string | null; targetHandle?: string | null;
  [k: string]: unknown; // keys/existing — dropped
}

const MULT = (t: string) => (t === "N" ? "*" : "1");

/** Legacy "X:Y" cardinality → per-end multiplicities + navigability. */
export function endsFromCardinality(
  cardinality: string | undefined,
  bidirectional: boolean,
): { fromEnd: RelEnd; toEnd: RelEnd } {
  const fromEnd: RelEnd = {};
  const toEnd: RelEnd = { navigable: true };
  if (bidirectional) fromEnd.navigable = true;
  if (cardinality) {
    const [l, r] = cardinality.split(":");
    fromEnd.multiplicity = MULT(l);
    toEnd.multiplicity = MULT(r);
  }
  return { fromEnd, toEnd };
}

function migrateNode(n: LegacyNode): ModelNode {
  const attributes: Attribute[] = (n.schema ?? []).map(f => ({
    name: f.name,
    type: { name: f.type },
    multiplicity: "1",
    ...(f.description ? { description: f.description } : {}),
  }));
  return {
    key: n.key,
    type: "uml.Class",
    title: n.title ?? "Untitled",
    stereotypes: [],
    ...(n.description ? { description: n.description } : {}),
    attributes,
    position: n.position ?? { x: 0, y: 0 },
  };
}

function migrateEdge(e: LegacyEdge): ModelEdge {
  const bidirectional = e.bidirectional === true;
  return {
    id: e.id,
    kind: "associates",
    from: e.from,
    to: e.to,
    ...endsFromCardinality(typeof e.cardinality === "string" ? e.cardinality : undefined, bidirectional),
    bidirectional,
    ...(e.sourceHandle !== undefined ? { sourceHandle: e.sourceHandle } : {}),
    ...(e.targetHandle !== undefined ? { targetHandle: e.targetHandle } : {}),
  };
}

/** A payload predates the UML model when it has no `diagrams` array. */
export function isLegacyGraph(g: unknown): boolean {
  const x = g as { nodes?: unknown; diagrams?: unknown };
  return !!x && Array.isArray(x.nodes) && !Array.isArray(x.diagrams);
}

/** Normalize any persisted/shared payload (legacy or current) to a ModelGraph. Null = unusable. */
export function migrateGraph(raw: unknown): ModelGraph | null {
  const g = raw as { nodes?: unknown[]; edges?: unknown[]; diagrams?: unknown[] };
  if (!g || !Array.isArray(g.nodes) || !Array.isArray(g.edges)) return null;
  if (!isLegacyGraph(g)) {
    return {
      nodes: g.nodes as ModelNode[],
      edges: g.edges as ModelEdge[],
      diagrams: (g.diagrams ?? []) as ModelGraph["diagrams"],
    };
  }
  return {
    nodes: (g.nodes as LegacyNode[]).map(migrateNode),
    edges: (g.edges as LegacyEdge[]).map(migrateEdge),
    diagrams: [],
  };
}
```

- [ ] **Step 5: Export from `packages/okf/src/index.ts`**

```ts
export * from "./types";
export { slugify, parseFrontmatter, renderFrontmatter } from "./slug";
export { serializeBundle, type OkfBundle } from "./serialize";
export { parseBundle } from "./parse";
export { migrateGraph, isLegacyGraph, endsFromCardinality } from "./migrate";
```

- [ ] **Step 6: Run the new tests**

Run: `pnpm --filter @mc/okf test -- migrate`
Expected: PASS (the migrate tests only — `parse.ts`/`serialize.ts` don't compile yet under vitest's per-file transform, but the migrate test file doesn't import them; if vitest still trips on project-wide type errors, proceed — Task 2/3 fix them, and the gate is at stage end).

- [ ] **Step 7: Commit**

```bash
git add packages/okf/src/types.ts packages/okf/src/migrate.ts packages/okf/src/index.ts packages/okf/test/migrate.test.ts
git commit -m "feat(okf): profile-agnostic core model (classifiers, relationship kinds, diagrams) + legacy migration"
```

### Task 2: okf parse.ts — legacy markdown → new model

**Files:**
- Modify: `packages/okf/src/parse.ts` (218 lines — major surgery)
- Modify: `packages/okf/test/parse-owox.test.ts`, `packages/okf/test/prose-join.test.ts`

**Interfaces:**
- Consumes: `endsFromCardinality` from `./migrate`; pinned types.
- Produces: `parseBundle(files: Record<string,string>): ModelGraph` returning new-shape nodes/edges, `diagrams: []`. Legacy `# Schema` tables/bullets → `attributes`; `## Joins`/prose joins → `kind: "associates"` edges; `[X:Y]` suffix → per-end multiplicities. Join *keys*, FK-note recovery, Overview/owoxId/status/inputSource reading are deleted.

- [ ] **Step 1: Update the legacy-import tests to the new shape**

Replace `packages/okf/test/parse-owox.test.ts` body assertions (keep the `customers` / `ordersSuperset` / `customersCard` / `txCard` fixture strings exactly as they are, lines 4–47 and 74–98; delete the `ordersFaithful` fixture at line 50):

```ts
describe("parseBundle (legacy OWOX format)", () => {
  it("maps a legacy mart doc onto a generic classifier with attributes", () => {
    const g = parseBundle({ "b/customers.md": customers });
    const n = g.nodes[0];
    expect(n.type).toBe("OWOX Data Mart");           // opaque token carried, renders generically
    expect(n.stereotypes).toEqual([]);
    expect(n.attributes[0]).toEqual({ name: "id", type: { name: "INTEGER" }, multiplicity: "1", description: "Customer id" });
    expect(g.diagrams).toEqual([]);
  });

  it("reads legacy joins as associates edges (keys dropped)", () => {
    const g = parseBundle({ "b/customers.md": customers, "b/orders.md": ordersSuperset });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0]).toMatchObject({ from: "orders", to: "customers", kind: "associates", bidirectional: false });
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});

describe("legacy cardinality suffix", () => {
  it("maps [N:1] onto per-end multiplicities", () => {
    const g = parseBundle({ "b/blocks.md": customersCard, "b/transactions.md": txCard });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].fromEnd.multiplicity).toBe("*");
    expect(g.edges[0].toEnd.multiplicity).toBe("1");
  });
  it("leaves multiplicities undefined when absent", () => {
    const txNo = txCard.replace(" [N:1]", "");
    const g = parseBundle({ "b/blocks.md": customersCard, "b/transactions.md": txNo });
    expect(g.edges[0].fromEnd.multiplicity).toBeUndefined();
  });
});
```

In `packages/okf/test/prose-join.test.ts`: replace the three key assertions —
line 25 `expect(g.edges[0].keys).toEqual(...)` → `expect(g.edges[0].kind).toBe("associates")`;
lines 50–51 (`e!.keys.some(...)`) → `expect(e!.kind).toBe("associates")`;
line 72 `expect(e!.keys).toEqual([])` → `expect(e!.kind).toBe("associates")`. Everything else (fixtures, index.md non-node test) stays.

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- parse-owox prose-join`
Expected: FAIL (compile errors: `keys`/`cardinality` no longer on `ModelEdge`).

- [ ] **Step 3: Rewrite `packages/okf/src/parse.ts`**

Replace the whole file with:

```ts
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
```

(Deleted: `FLIP_CARDINALITY`, `parseOverview`, `parseDefinition`, join-key extraction, FK-note key recovery, `pkByKey`, `storageId` reading, `on \`key\`` prose binding.)

- [ ] **Step 4: Run the parse tests**

Run: `pnpm --filter @mc/okf test -- parse-owox prose-join`
Expected: PASS. (`roundtrip`/`serialize` tests still fail — Task 3.)

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/parse.ts packages/okf/test/parse-owox.test.ts packages/okf/test/prose-join.test.ts
git commit -m "feat(okf): parse legacy markdown onto generalized classifier/relationship model"
```

### Task 3: okf serialize.ts — interim emission from the new model

**Files:**
- Rewrite: `packages/okf/src/serialize.ts` (91 lines)
- Rewrite: `packages/okf/test/serialize.test.ts`, `packages/okf/test/roundtrip.test.ts`

**Interfaces:**
- Consumes: pinned types.
- Produces: `serializeBundle(graph: ModelGraph, projectTitle = "Model"): OkfBundle` — legacy-shaped markdown (Schema table from `attributes`, `## Joins` lines with `[X:Y]` suffix derived from end multiplicities) that `parseBundle` (Task 2) reads back. Replaced wholesale in Stage 2 (accepted interim, decision 10).

- [ ] **Step 1: Rewrite the tests**

Replace `packages/okf/test/serialize.test.ts` with:

```ts
import { describe, it, expect } from "vitest";
import { serializeBundle } from "../src/serialize";
import type { ModelGraph } from "../src/types";

const graph: ModelGraph = {
  nodes: [
    { key: "orders", title: "Orders", type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 },
      attributes: [
        { name: "order_id", type: { name: "STRING" }, multiplicity: "1", description: "Unique order id" },
        { name: "customer_id", type: { name: "INTEGER" }, multiplicity: "1" },
      ] },
    { key: "customers", title: "Customers", type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 },
      attributes: [{ name: "id", type: { name: "INTEGER" }, multiplicity: "1" }] },
  ],
  edges: [{ id: "e1", kind: "associates", from: "orders", to: "customers",
            fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false }],
  diagrams: [],
};

describe("serializeBundle (interim legacy emission)", () => {
  const { files } = serializeBundle(graph, "Demo");
  const index = files["demo/index.md"];
  const orders = files["demo/orders.md"];

  it("writes a folder bundle with index + per-doc files", () => {
    expect(Object.keys(files).sort()).toEqual(["demo/customers.md", "demo/index.md", "demo/orders.md"]);
  });
  it("index lists documents with their type", () => {
    expect(index).toContain("| Document | Type |");
    expect(index).toContain("[Orders](./orders.md) | uml.Class |");
    expect(index).not.toContain("owox");
  });
  it("doc frontmatter carries the node type verbatim", () => {
    expect(orders).toContain(`type: "uml.Class"`);
    expect(orders).not.toContain("owox:");
    expect(orders).not.toContain("tags:");
    expect(orders).not.toContain("## Overview");
  });
  it("schema table renders from attributes", () => {
    expect(orders).toContain("# Schema\n\n| Column | Type | Description |");
    expect(orders).toContain("| `order_id` | STRING | Unique order id |");
  });
  it("joins render with a cardinality suffix from end multiplicities", () => {
    expect(orders).toContain("## Joins");
    expect(orders).toContain("- [Customers](./customers.md) [N:1]");
  });
});
```

Replace `packages/okf/test/roundtrip.test.ts` with:

```ts
import { describe, it, expect } from "vitest";
import { serializeBundle, parseBundle } from "../src/index";
import type { ModelGraph } from "../src/types";

const node = (key: string, title: string, attrs: ModelGraph["nodes"][0]["attributes"] = []): ModelGraph["nodes"][0] =>
  ({ key, title, type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 }, attributes: attrs });

describe("okf round-trip (interim legacy format)", () => {
  it("serializes to files and parses back to an equivalent graph", () => {
    const graph: ModelGraph = {
      nodes: [
        node("fb", "Facebook Ads", [{ name: "campaign_id", type: { name: "STRING" }, multiplicity: "1" }]),
        node("camp", "Campaigns", [{ name: "id", type: { name: "STRING" }, multiplicity: "1", description: "Unique id" }]),
      ],
      edges: [{ id: "e1", kind: "associates", from: "fb", to: "camp",
                fromEnd: {}, toEnd: { navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const bundle = serializeBundle(graph, "Demo");
    expect(Object.keys(bundle.files)).toContain("demo/index.md");
    const back = parseBundle(bundle.files);
    expect(back.nodes.map(n => n.key).sort()).toEqual(["campaigns", "facebook-ads"]);
    expect(back.nodes.find(n => n.key === "campaigns")!.attributes[0])
      .toEqual({ name: "id", type: { name: "STRING" }, multiplicity: "1", description: "Unique id" });
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0]).toMatchObject({ from: "facebook-ads", to: "campaigns", kind: "associates" });
  });

  it("survives 1/* end multiplicities via the [N:1] suffix", () => {
    const graph: ModelGraph = {
      nodes: [node("tx", "Transactions"), node("blocks", "Blocks")],
      edges: [{ id: "e1", kind: "associates", from: "tx", to: "blocks",
                fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const back = parseBundle(serializeBundle(graph, "Demo").files);
    expect(back.edges[0].fromEnd.multiplicity).toBe("*");
    expect(back.edges[0].toEnd.multiplicity).toBe("1");
  });

  it("keeps both nodes when two titles slugify to the same value", () => {
    const graph: ModelGraph = {
      nodes: [node("posts", "Posts Answers"), node("answers", "Posts & Answers")],
      edges: [{ id: "e1", kind: "associates", from: "posts", to: "answers",
                fromEnd: {}, toEnd: { navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const { files } = serializeBundle(graph, "Demo");
    expect(Object.keys(files).filter(f => !f.endsWith("index.md"))).toHaveLength(2);
    const back = parseBundle(files);
    expect(back.nodes).toHaveLength(2);
    expect(new Set(back.nodes.map(n => n.key)).size).toBe(2);
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0].from).not.toBe(back.edges[0].to);
  });

  it("collapses mutual join lines into one bidirectional edge", () => {
    const front = (t: string) => `---\ntitle: "${t}"\n---\n# ${t}\n`;
    const g = parseBundle({
      "p/a.md": front("A") + "\n## Joins\n- [B](./b.md)\n",
      "p/b.md": front("B") + "\n## Joins\n- [A](./a.md)\n",
    });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].bidirectional).toBe(true);
    expect(g.edges[0].fromEnd.navigable).toBe(true);
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- serialize roundtrip`
Expected: FAIL (serialize.ts still consumes `inputSource`/`schema`/`keys`).

- [ ] **Step 3: Rewrite `packages/okf/src/serialize.ts`**

```ts
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
```

- [ ] **Step 4: Run all okf tests + build**

Run: `pnpm --filter @mc/okf build && pnpm --filter @mc/okf test`
Expected: PASS — all 6 okf test files (migrate, parse-owox, prose-join, roundtrip, serialize, slug).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/serialize.ts packages/okf/test/serialize.test.ts packages/okf/test/roundtrip.test.ts
git commit -m "feat(okf): serialize the generalized model (interim legacy emission)"
```

### Task 4: web state, persistence, sharing, merge

**Files:**
- Modify: `packages/web/src/state/model.ts` (41 lines), `packages/web/src/state/persist.ts` (28 lines), `packages/web/src/share/url.ts` (99 lines), `packages/web/src/state/relLabels.ts` (50 lines)
- Create: `packages/web/src/sync/merge.ts`
- Delete: `packages/web/src/sync/owoxImport.ts`, `sync/owoxImport.test.ts`, `sync/detach.ts`, `sync/detach.test.ts`, `sync/joinFieldType.ts`, `sync/joinFieldType.test.ts`
- Test: `packages/web/src/sync/merge.test.ts` (new), `packages/web/src/share/url.test.ts`, `packages/web/src/state/relLabels.test.ts` (updated)

**Interfaces:**
- Consumes: `migrateGraph`, pinned types from `@mc/okf` (Tasks 1–3; run `pnpm --filter @mc/okf build` first).
- Produces: `store.addNode(position)` → node `{ key, type: "uml.Class", title: "New object", stereotypes: [], attributes: [], position }`; `store.addEdge(from, to, sourceHandle?, targetHandle?)` → edge `{ id, kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, ... }`; `mergeGraphs(current, incoming): { graph, newKeys }` from `sync/merge`; `RelLabelMode = "all" | "hidden"`.

- [ ] **Step 1: Write failing tests**

Create `packages/web/src/sync/merge.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { mergeGraphs } from "./merge";
import type { ModelGraph } from "@mc/okf";

const node = (key: string, title: string): ModelGraph["nodes"][0] =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge = (id: string, from: string, to: string): ModelGraph["edges"][0] =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false });

describe("mergeGraphs", () => {
  it("appends incoming nodes with fresh keys and remaps edges + diagram members", () => {
    const current: ModelGraph = { nodes: [node("n1", "A")], edges: [], diagrams: [] };
    const incoming: ModelGraph = {
      nodes: [node("n1", "B"), node("n2", "C")],
      edges: [edge("e1", "n1", "n2")],
      diagrams: [{ key: "d", title: "D", profile: "uml-domain", members: ["n1", "n2"] }],
    };
    const { graph, newKeys } = mergeGraphs(current, incoming);
    expect(graph.nodes).toHaveLength(3);
    expect(new Set(graph.nodes.map(n => n.key)).size).toBe(3);
    expect(newKeys.size).toBe(2);
    const merged = graph.edges[0];
    expect(newKeys.has(merged.from)).toBe(true);
    expect(newKeys.has(merged.to)).toBe(true);
    expect(graph.diagrams[0].members.every(k => newKeys.has(k))).toBe(true);
  });
});
```

Update `packages/web/src/state/relLabels.test.ts`: delete every test of `isKeySet`/`visibleKeys`/`showCardinality` and the `"defined"`/`"undefined"` modes; replace the file with:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { loadRelLabelMode, persistRelLabelMode } from "./relLabels";

describe("relLabels", () => {
  beforeEach(() => localStorage.clear());
  it("defaults to all", () => expect(loadRelLabelMode()).toBe("all"));
  it("round-trips hidden", () => { persistRelLabelMode("hidden"); expect(loadRelLabelMode()).toBe("hidden"); });
  it("coerces persisted legacy modes to all", () => {
    localStorage.setItem("mc.relLabels.v1", "defined");
    expect(loadRelLabelMode()).toBe("all");
  });
});
```

Update `packages/web/src/share/url.test.ts`: rewrite fixtures to the new node/edge shape (same builders as merge.test.ts) and add one migration test:

```ts
import { gzipSync, strToU8 } from "fflate";
it("decodes a legacy (mart-era) share payload via migration", () => {
  const legacyJson = JSON.stringify({
    storageId: null,
    nodes: [{ key: "n1", title: "Orders", inputSource: "SQL", schema: [{ name: "id", type: "STRING", pk: true }],
              position: { x: 1, y: 2 }, status: "pending", owoxId: null }],
    edges: [{ id: "e1", from: "n1", to: "n2", keys: [], bidirectional: false, cardinality: "N:1" }],
  });
  const bytes = gzipSync(strToU8(legacyJson));
  let bin = ""; for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  const payload = btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  const g = decodeModel(payload)!;
  expect(g.nodes[0].type).toBe("uml.Class");
  expect(g.nodes[0].attributes[0].name).toBe("id");
  expect(g.edges[0].kind).toBe("associates");
  expect(g.diagrams).toEqual([]);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf build && pnpm --filter @mc/web test -- merge relLabels url`
Expected: FAIL (`./merge` missing; relLabels exports gone; url fixtures don't compile).

- [ ] **Step 3: Rewrite `packages/web/src/state/model.ts` internals**

Only the initial value and `addNode`/`addEdge`/`removeNode` change; subscribe/set/updateNode/updateEdge/removeEdge stay byte-identical.

```ts
  let g: ModelGraph = { nodes: [], edges: [], diagrams: [], ...initial } as ModelGraph;
```

```ts
    addNode(position: { x: number; y: number }): ModelNode {
      const n: ModelNode = { key: uid("n"), type: "uml.Class", title: "New object", stereotypes: [], attributes: [], position };
      g = { ...g, nodes: [...g.nodes, n] }; emit(); return n;
    },
```

```ts
    removeNode(key: string) {
      g = { ...g,
        nodes: g.nodes.filter(n => n.key !== key),
        edges: g.edges.filter(e => e.from !== key && e.to !== key),
        diagrams: g.diagrams.map(d => d.members.includes(key) ? { ...d, members: d.members.filter(m => m !== key) } : d),
      }; emit();
    },
```

```ts
    addEdge(from: string, to: string, sourceHandle?: string | null, targetHandle?: string | null): ModelEdge | null {
      if (from === to) return null;
      const pair = [from, to].sort().join("|");
      const existing = g.edges.find(e => [e.from, e.to].sort().join("|") === pair);
      if (existing) {
        g = { ...g, edges: g.edges.map(e => e === existing
          ? { ...e, bidirectional: true, fromEnd: { ...e.fromEnd, navigable: true }, toEnd: { ...e.toEnd, navigable: true } }
          : e) };
        emit(); return existing;
      }
      const e: ModelEdge = { id: uid("e"), kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, sourceHandle, targetHandle };
      g = { ...g, edges: [...g.edges, e] }; emit(); return e;
    },
```

- [ ] **Step 4: Migrate on rehydrate — `packages/web/src/state/persist.ts`**

Same key `mc.model.v1`; replace `loadPersistedGraph`:

```ts
import type { ModelGraph } from "@mc/okf";
import { migrateGraph } from "@mc/okf";

const KEY = "mc.model.v1";

export function loadPersistedGraph(): ModelGraph | undefined {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return undefined;
    return migrateGraph(JSON.parse(raw)) ?? undefined;
  } catch {
    return undefined;
  }
}
```

(`persistGraph` unchanged.)

- [ ] **Step 5: Rewrite share sanitize/decode — `packages/web/src/share/url.ts`**

Replace `sanitize` (lines 15–37) and `decodeModel` (lines 60–69); the b64url helpers, `encodeModel`, `buildShareUrl`, `readSharedModel`, `readSharedName`, `clearSharedModelFromUrl` are unchanged:

```ts
import { migrateGraph } from "@mc/okf";
import type { ModelGraph, ModelNode, ModelEdge } from "@mc/okf";

// A shared model is a clean draft: canvas-only handle hints are dropped, and the
// field list is explicit so hand-edited payloads can't smuggle extra data.
function sanitize(g: ModelGraph): ModelGraph {
  return {
    nodes: g.nodes.map((n): ModelNode => ({
      key: n.key, type: n.type, title: n.title,
      stereotypes: n.stereotypes ?? [],
      ...(n.abstract ? { abstract: true } : {}),
      ...(n.description ? { description: n.description } : {}),
      attributes: n.attributes ?? [],
      ...(n.values ? { values: n.values } : {}),
      position: n.position,
    })),
    edges: g.edges.map((e): ModelEdge => ({
      id: e.id, kind: e.kind, from: e.from, to: e.to,
      fromEnd: e.fromEnd ?? {}, toEnd: e.toEnd ?? {}, bidirectional: e.bidirectional,
    })),
    diagrams: g.diagrams ?? [],
  };
}

/** Reverse of encodeModel. Returns null on any malformed/corrupt payload.
 *  Legacy (mart-era) payloads are migrated — old share links keep opening. */
export function decodeModel(payload: string): ModelGraph | null {
  try {
    const json = strFromU8(gunzipSync(b64urlToBytes(payload)));
    const g = migrateGraph(JSON.parse(json));
    return g ? sanitize(g) : null;
  } catch {
    return null;
  }
}
```

- [ ] **Step 6: Shrink `packages/web/src/state/relLabels.ts`**

Replace the whole file:

```ts
// What relationship-edge labels show on the canvas (multiplicities/roles). A
// per-browser view preference — persisted in localStorage, mirroring viewMode.
export type RelLabelMode = "all" | "hidden";

const KEY = "mc.relLabels.v1";

export function loadRelLabelMode(): RelLabelMode {
  try {
    // Legacy modes ("defined"/"undefined") were join-key concepts; coerce to "all".
    return localStorage.getItem(KEY) === "hidden" ? "hidden" : "all";
  } catch {
    return "all";
  }
}

export function persistRelLabelMode(mode: RelLabelMode): void {
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    // best-effort; ignore quota / private-mode failures
  }
}
```

- [ ] **Step 7: Create `packages/web/src/sync/merge.ts`; delete the OWOX sync modules**

```ts
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
```

Then: `git rm packages/web/src/sync/owoxImport.ts packages/web/src/sync/owoxImport.test.ts packages/web/src/sync/detach.ts packages/web/src/sync/detach.test.ts packages/web/src/sync/joinFieldType.ts packages/web/src/sync/joinFieldType.test.ts`.
`Canvas.tsx` still imports `mergeGraphs` from `"../../sync/owoxImport"` — fixed in Task 6; the web build stays red until then (expected mid-stage; the stage gate is at Task 7).

- [ ] **Step 8: Run the task's tests**

Run: `pnpm --filter @mc/web test -- merge relLabels url`
Expected: PASS for these three files (other web suites still red until Tasks 5–7).

- [ ] **Step 9: Commit**

```bash
git add -A packages/web/src/state packages/web/src/share packages/web/src/sync
git commit -m "feat(web): store/persist/share/merge on the generalized model; drop OWOX sync remnants"
```

### Task 5: templates on the new model

**Files:**
- Rewrite: `packages/web/src/templates/helpers.ts` (33 lines)
- Mechanical sweep: all 22 `packages/web/src/templates/*.ts` graph literals
- Test: create `packages/web/src/templates/templates.test.ts`; existing `packages/web/src/lib/templateLink.test.ts` must pass

**Interfaces:**
- Consumes: pinned types, `endsFromCardinality` from `@mc/okf`.
- Produces: `f(name, type, pk?, description?): Attribute` (pk ignored); `mart(key, title, inputSource, attributes, description?): ModelNode` (inputSource ignored); `rel(id, from, to, left, right, cardinality?, bidirectional?): ModelEdge` (left/right ignored; cardinality → end multiplicities). Signatures UNCHANGED so the 22 template files compile untouched except the graph-literal sweep.

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/templates/templates.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { TEMPLATES } from "./index";

describe("built-in templates", () => {
  it("every template graph is new-shape", () => {
    for (const t of TEMPLATES) {
      expect(Array.isArray(t.graph.diagrams)).toBe(true);
      for (const n of t.graph.nodes) {
        expect(n.type).toBe("uml.Class");
        expect(Array.isArray(n.attributes)).toBe(true);
        expect((n as Record<string, unknown>).schema).toBeUndefined();
      }
      for (const e of t.graph.edges) {
        expect(e.kind).toBe("associates");
        expect((e as Record<string, unknown>).keys).toBeUndefined();
      }
    }
  });
  it("default N:1 cardinality became */1 end multiplicities", () => {
    const withEdges = TEMPLATES.find(t => t.graph.edges.length > 0)!;
    const e = withEdges.graph.edges[0];
    expect(e.fromEnd.multiplicity).toBe("*");
    expect(e.toEnd.multiplicity).toBe("1");
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- templates.test`
Expected: FAIL (helpers still build the mart shape; template literals still carry `storageId`).

- [ ] **Step 3: Rewrite `packages/web/src/templates/helpers.ts`**

```ts
import type { ModelGraph, ModelNode, ModelEdge, Attribute } from "@mc/okf";
import { endsFromCardinality } from "@mc/okf";

// ── tiny authoring helpers ─────────────────────────────────────────────────
// Signatures kept from the mart era so the 22 template files stay untouched:
// `pk`, `inputSource` and join fields are accepted and dropped (data-profile
// concerns, deferred); cardinality maps onto per-end multiplicities.
export const f = (name: string, type: string, _pk = false, description?: string): Attribute =>
  ({ name, type: { name: type }, multiplicity: "1", ...(description ? { description } : {}) });

export const mart = (
  key: string,
  title: string,
  _inputSource: string,
  attributes: Attribute[],
  description?: string,
): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], ...(description ? { description } : {}), attributes, position: { x: 0, y: 0 } });

export const rel = (
  id: string,
  from: string,
  to: string,
  _left: string,
  _right: string,
  cardinality: "1:1" | "1:N" | "N:1" | "N:N" = "N:1",
  bidirectional = false,
): ModelEdge => ({ id, kind: "associates", from, to, ...endsFromCardinality(cardinality, bidirectional), bidirectional });

export interface Template {
  id: string;                    // immutable — ?template=<id> deep links are public CTAs
  nicheId: string | null;
  category: "industry" | "dataset";
  name: string;
  description: string;
  graph: ModelGraph;
}
```

(The `_`-prefixed params satisfy typescript-eslint's default `argsIgnorePattern` — confirm with `pnpm lint`.)

- [ ] **Step 4: Sweep the 22 template graph literals**

Every template declares `const graph: ModelGraph = { storageId: null, nodes: [...], edges: [...] }`. Swap the dead property (bash):

```bash
cd packages/web/src/templates
sed -i 's/storageId: null,/diagrams: [],/' *.ts
grep -rn "storageId" . && echo "LEFTOVERS — fix manually" || echo OK
```

- [ ] **Step 5: Run tests**

Run: `pnpm --filter @mc/web test -- templates.test templateLink`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/templates
git commit -m "feat(web): templates emit the generalized model (helpers remap mart-era args)"
```

### Task 6: canvas on the new model (interim rendering)

**Files:**
- Modify: `packages/web/src/components/canvas/Canvas.tsx` (625 lines), `edges.ts` (95 lines), `MartNode.tsx` (161 lines), `RelEdge.tsx` (126 lines), `layoutSize.ts` (23 lines), `Dock.tsx` (lines 7–19), `packages/web/src/components/LibraryDialog.tsx`, `packages/web/src/components/ImportDialog.tsx` (line 50)
- Test: update `edges.test.ts`, `RelEdge.test.tsx`, `MartNode.test.tsx`, `layoutSize.test.ts`, `Dock.test.tsx`, `okf/io.test.ts`, `okf/guideExample.test.ts`

**Interfaces:**
- Consumes: Task 4 store/merge, Task 5 templates.
- Produces: `buildRfEdges(edges, nodes, viewMode, relLabelMode)` (same signature), edge `data` now `{ kind, fromEnd, toEnd, bidirectional, modelEdgeId, relLabelMode }`; `isEdgeReconnectable(modelEdgeId, selectedEdgeId)` (viewMode param dropped); `MartNodeData = ModelNode & { _viewMode?: ViewMode }`; RF node type renamed `"okf"`. Stage 3 replaces the visuals; this task only makes them truthful to the new model.

- [ ] **Step 1: `Canvas.tsx` edits (exact spots)**

  - Line 39: `import { mergeGraphs } from "../../sync/owoxImport";` → `import { mergeGraphs } from "../../sync/merge";`
  - Delete `keyFieldsByNode` (lines 109–117). `toRFNode` (97–104) loses `keyFields`:

    ```ts
    function toRFNode(n: ModelNode, viewMode: ViewMode): Node {
      return { id: n.key, type: "okf", position: n.position, data: { ...n, _viewMode: viewMode } as unknown as Record<string, unknown> };
    }
    ```

    and the sync effect (199–202) becomes:

    ```ts
    useEffect(() => {
      setRfNodes(graph.nodes.map(n => toRFNode(n, viewMode)));
    }, [graph.nodes, viewMode, setRfNodes]);
    ```

  - Line 151: `const nodeTypes = { okf: MartNode };` (renamed now so stage 3 doesn't touch Canvas again).
  - Line 214: `isEdgeReconnectable(modelEdgeId, selId)` (drop `viewMode`); remove `viewMode` from that effect's dep list only if ESLint agrees (it's still used for zIndex? it isn't — remove it).
  - `onReconnect` (235–246): drop the ERD guard (edges are now 1:1 with model edges in every mode):

    ```ts
    const onReconnect = useCallback((oldEdge: Edge, conn: Connection) => {
      if (!conn.source || !conn.target || conn.source === conn.target) return;
      store.updateEdge(oldEdge.id, { from: conn.source, to: conn.target, sourceHandle: conn.sourceHandle, targetHandle: conn.targetHandle });
    }, []);
    ```

    and pass `edgesReconnectable={false}` unchanged.
  - `clearCanvas` (line 342): `store.set({ nodes: [], edges: [], diagrams: [] });`
  - `handleImportConfirm` (395–405): drop the storageId dance; trust imported positions when present:

    ```ts
    const handleImportConfirm = useCallback((g: ModelGraph, mode: "replace" | "merge") => {
      if (mode === "merge") applyMergeWithLayout(g);
      else {
        const hasPositions = g.nodes.some(n => n.position.x !== 0 || n.position.y !== 0);
        store.set(hasPositions ? g : withLayout(g));
      }
      setShowImport(false);
    }, [withLayout, applyMergeWithLayout]);
    ```

  - `applyTemplate` (407–411): `else store.set(withLayout(g));`

- [ ] **Step 2: Simplify `packages/web/src/components/canvas/edges.ts`**

`edgeSides` stays identical. Replace the rest:

```ts
function compactEdge(e: ModelEdge, sides: { source: Side; target: Side }, relLabelMode: RelLabelMode): Edge {
  return {
    id: e.id, source: e.from, target: e.to,
    sourceHandle: sides.source, targetHandle: sides.target,
    type: "rel",
    data: { kind: e.kind, fromEnd: e.fromEnd, toEnd: e.toEnd, bidirectional: e.bidirectional, modelEdgeId: e.id, relLabelMode } as unknown as Record<string, unknown>,
  };
}

// Reconnect is scoped to the SELECTED relationship only (overlapping anchors).
export function isEdgeReconnectable(modelEdgeId: string | undefined, selectedEdgeId: string | null): boolean {
  return modelEdgeId != null && modelEdgeId === selectedEdgeId;
}

export function buildRfEdges(edges: ModelEdge[], nodes: ModelNode[], viewMode: ViewMode, relLabelMode: RelLabelMode = "all"): Edge[] {
  const byKey = new Map(nodes.map(n => [n.key, n]));
  return edges.map(e => compactEdge(e, edgeSides(byKey.get(e.from), byKey.get(e.to), e, viewMode), relLabelMode));
}
```

- [ ] **Step 3: Interim `MartNode.tsx` truthfulness pass**

Keep the file/component name (stage 3 replaces it wholesale). Delete `SOURCE_COLOR`, `STATUS_TIP`, `StatusDot`, `FieldAnchors`; drop `_keyFields`:

```ts
export type MartNodeData = ModelNode & { _viewMode?: ViewMode };
```

```tsx
function FieldRow({ a }: { a: Attribute }) {
  return (
    <div className="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
      <span className="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
      <span className="text-slate-400 font-mono text-[10.5px] truncate">{a.type.name}{a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}</span>
    </div>
  );
}
```

`ErdBody`: `const ordered = node.attributes;` (key-first ordering dies with keys); keep the `ERD_COLLAPSED_ROWS` expand toggle. `MartNodeInner`: `const color = "#94a3b8";`, remove `<StatusDot …/>`, the meta chip shows `node.type`, `fieldCount` reads `node.attributes.length`. Imports shrink (drop `KeyRound`, `SchemaField`).

- [ ] **Step 4: Interim `RelEdge.tsx` label pass**

```ts
export type RelEdgeData = Pick<ModelEdge, "kind" | "fromEnd" | "toEnd" | "bidirectional"> & { relLabelMode?: RelLabelMode };
```

```ts
const endText = (e?: { multiplicity?: string; role?: string }) =>
  [e?.multiplicity, e?.role].filter(Boolean).join(" ");
const label = mode === "hidden" ? "" :
  [endText(edgeData?.fromEnd), endText(edgeData?.toEnd)].filter(Boolean).join(" → ");
```

Keep the existing arrow markers and mid-path label div; delete `visibleKeys`/`showCardinality` imports, the `keys`/`cardinality` reads, and the cardinality badge span.

- [ ] **Step 5: `layoutSize.ts`, `Dock.tsx`, dialogs**

  - `layoutSize.ts` line 18: `const total = node.attributes.length;`
  - `Dock.tsx` lines 7–19:

    ```ts
    const REL_LABEL_GLYPH: Record<RelLabelMode, string> = { all: "≡", hidden: "⊘" };
    const REL_LABEL_OPTIONS: { mode: RelLabelMode; label: string; helper: string }[] = [
      { mode: "all", label: "Show labels", helper: "Multiplicities and roles on every relationship" },
      { mode: "hidden", label: "Hide all labels", helper: "Just the connector lines" },
    ];
    ```

  - `LibraryDialog.tsx`: line 77 `fields={n.schema}` → `fields={n.attributes}`; delete line 87 (`const cond = e.keys…`) and the `<code>` span + separator rendering it (lines 92–93); `MartRow` prop type (line 106) becomes `fields: { name: string; type: { name: string } }[]`, and inside `MartRow` print `f.type.name` and drop any `f.pk` icon.
  - `ImportDialog.tsx` line 50: `return graph;` (the status/owoxId scrub is dead). Update the comment above it.

- [ ] **Step 6: Update canvas tests**

Use the Task 4 `node`/`edge` builders in each file.

  - `edges.test.ts`: assert one RF edge per model edge in BOTH `"compact"` and `"erd"` modes, with `data.kind === "associates"` and `data.modelEdgeId`; keep the side-selection tests (they only use positions); delete per-key fan-out tests (ids like `"e1::0"`) and `isEdgeReconnectable` ERD-mode tests (now 2-arg).
  - `RelEdge.test.tsx`: fixture data `{ kind: "associates", fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*" }, bidirectional: false, relLabelMode: "all" }`; assert the label text contains `1` and `*`; assert `relLabelMode: "hidden"` renders no label div; delete "? = ?" and cardinality-badge tests.
  - `MartNode.test.tsx`: node data via builder + `_viewMode: "erd"`; assert title renders, attribute name + type token render, and no status dot / PK icon markup.
  - `layoutSize.test.ts`: fixtures swap `schema:` for `attributes:` (via builder); expectations unchanged.
  - `Dock.test.tsx`: label-mode menu shows exactly "Show labels" and "Hide all labels".
  - `okf/io.test.ts`: fixtures to new shape; round-trip through `graphToBundleFiles` → `filesToGraph` asserts node keys + edge kind survive.
  - `okf/guideExample.test.ts`: keep the fixture verbatim (it IS the legacy-import guard); replace the joins/pk assertions:

    ```ts
    it("parses all 3 relationships with correct direction", () => {
      const j = graph.edges.map(e => `${e.from}->${e.to}:${e.kind}`).sort();
      expect(j).toEqual([
        "order-items->orders:associates",
        "order-items->products:associates",
        "orders->customers:associates",
      ]);
    });
    it("parses the [N:1] suffix onto end multiplicities", () => {
      const e = graph.edges.find(x => x.from === "order-items" && x.to === "orders")!;
      expect(e.fromEnd.multiplicity).toBe("*");
      expect(e.toEnd.multiplicity).toBe("1");
    });
    ```

- [ ] **Step 7: Run canvas tests**

Run: `pnpm --filter @mc/web test -- edges RelEdge MartNode layoutSize Dock io.test guideExample`
Expected: PASS. (Inspector tests still red — Task 7.)

- [ ] **Step 8: Commit**

```bash
git add -A packages/web/src/components packages/web/src/okf
git commit -m "feat(web): canvas renders the generalized model (interim visuals)"
```

### Task 7: inspector on the new model + STAGE 1 GATE

**Files:**
- Rewrite: `packages/web/src/components/inspector/ObjectInspector.tsx` (176 lines), `RelationshipInspector.tsx` (143 lines)
- Create: `packages/web/src/components/inspector/AttributeEditor.tsx`
- Delete: `packages/web/src/components/inspector/SchemaEditor.tsx`
- Modify: `packages/web/src/components/inspector/Inspector.tsx` (lines 6, 127–149: drop `joinFieldType` import and `onEnsureField`)
- Test: rewrite `packages/web/src/components/inspector/RelationshipInspector.test.tsx`

**Interfaces:**
- Consumes: store patch setters `onUpdateNode(key, patch: Partial<ModelNode>)`, `onUpdateEdge(id, patch: Partial<ModelEdge>)` (signatures unchanged).
- Produces: `<AttributeEditor attributes={Attribute[]} onChange={(a: Attribute[]) => void} />`; `<RelationshipInspector edge fromNode toNode onUpdate />` (no `onEnsureField`); ObjectInspector edits title/description/type/stereotypes/abstract/attributes/values. Task 14 later swaps the hardcoded `METACLASSES` list for the profile palette.

- [ ] **Step 1: Write the failing test**

Replace `packages/web/src/components/inspector/RelationshipInspector.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RelationshipInspector } from "./RelationshipInspector";
import type { ModelEdge, ModelNode } from "@mc/okf";

const node = (key: string, title: string): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge: ModelEdge = { id: "e1", kind: "associates", from: "a", to: "b",
  fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*", navigable: true }, bidirectional: false };

describe("RelationshipInspector", () => {
  it("changes the kind through the verb select", () => {
    const onUpdate = vi.fn();
    render(<RelationshipInspector edge={edge} fromNode={node("a", "Order")} toNode={node("b", "OrderLine")} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "composes" } });
    expect(onUpdate).toHaveBeenCalledWith({ kind: "composes" });
  });
  it("edits the near-end multiplicity", () => {
    const onUpdate = vi.fn();
    render(<RelationshipInspector edge={edge} fromNode={node("a", "Order")} toNode={node("b", "OrderLine")} onUpdate={onUpdate} />);
    fireEvent.change(screen.getByLabelText("Order multiplicity"), { target: { value: "0..1" } });
    expect(onUpdate).toHaveBeenCalledWith({ fromEnd: { multiplicity: "0..1" } });
  });
  it("hides end editors for specializes", () => {
    render(<RelationshipInspector edge={{ ...edge, kind: "specializes", fromEnd: {}, toEnd: {} }} fromNode={node("a", "A")} toNode={node("b", "B")} onUpdate={() => {}} />);
    expect(screen.queryByLabelText("A multiplicity")).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- RelationshipInspector`
Expected: FAIL (component still renders join keys and requires `onEnsureField`).

- [ ] **Step 3: Rewrite `RelationshipInspector.tsx`**

```tsx
import type { ModelEdge, ModelNode, RelationshipKind, RelEnd } from "@mc/okf";
import { RELATIONSHIP_KINDS, ENDED_KINDS } from "@mc/okf";
import { InfoTip } from "./InfoTip";

interface RelationshipInspectorProps {
  edge: ModelEdge;
  fromNode: ModelNode | undefined;
  toNode: ModelNode | undefined;
  onUpdate: (patch: Partial<ModelEdge>) => void;
}

const KIND_HELP: Record<RelationshipKind, string> = {
  associates: "Plain association — solid line, arrowhead on navigable end(s).",
  aggregates: "Shared aggregation — hollow diamond on this (whole) end.",
  composes: "Composition — filled diamond on this (whole) end; parts live and die with the whole.",
  specializes: "Generalization — hollow triangle at the parent (near→far reads child→parent).",
  implements: "Realization — dashed line, hollow triangle at the interface.",
  depends: "Dependency — dashed open arrow at the target.",
  annotates: "Note anchor — uml.Note only; never selectable here.",
};

// `annotates` is a uml.Note-only verb (anchors live on the note node, not on edges) — hide it from the edge verb select.
const EDGE_KINDS = RELATIONSHIP_KINDS.filter(k => k !== "annotates");

const inputCls = "w-full text-[13px] px-[10px] py-[8px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

function EndEditor({ title, end, onChange }: { title: string; end: RelEnd; onChange: (end: RelEnd) => void }) {
  return (
    <div className="flex gap-[6px]">
      <label className="flex-1 text-[11px] text-slate-500">
        {title} multiplicity
        <input aria-label={`${title} multiplicity`} type="text" value={end.multiplicity ?? ""} placeholder="1, 0..1, *"
          onChange={e => onChange({ ...end, multiplicity: e.target.value || undefined })} className={inputCls} />
      </label>
      <label className="flex-1 text-[11px] text-slate-500">
        {title} role
        <input aria-label={`${title} role`} type="text" value={end.role ?? ""} placeholder="role"
          onChange={e => onChange({ ...end, role: e.target.value || undefined })} className={inputCls} />
      </label>
    </div>
  );
}

export function RelationshipInspector({ edge, fromNode, toNode, onUpdate }: RelationshipInspectorProps) {
  const fromTitle = fromNode?.title ?? "Source";
  const toTitle = toNode?.title ?? "Target";
  const hasEnds = ENDED_KINDS.has(edge.kind);
  return (
    <div className="flex flex-col gap-[15px]">
      <div className="text-[13px] text-slate-500">
        <strong className="text-slate-900">{fromTitle}</strong>{" → "}<strong className="text-slate-900">{toTitle}</strong>
      </div>
      <div>
        <label htmlFor="rel-kind" className="flex items-center gap-[5px] text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
          Kind <InfoTip text={KIND_HELP[edge.kind]} />
        </label>
        <select id="rel-kind" aria-label="Kind" value={edge.kind}
          onChange={e => onUpdate({ kind: e.target.value as RelationshipKind })} className={inputCls}>
          {EDGE_KINDS.map(k => <option key={k} value={k}>{k}</option>)}
        </select>
      </div>
      {hasEnds && (
        <div className="flex flex-col gap-[10px]">
          <EndEditor title={fromTitle} end={edge.fromEnd} onChange={fromEnd => onUpdate({ fromEnd })} />
          <EndEditor title={toTitle} end={edge.toEnd} onChange={toEnd => onUpdate({ toEnd })} />
        </div>
      )}
      {edge.kind === "associates" && (
        <label className="flex items-start gap-[9px] cursor-pointer">
          <input type="checkbox" checked={edge.bidirectional}
            onChange={e => onUpdate({
              bidirectional: e.target.checked,
              fromEnd: { ...edge.fromEnd, navigable: e.target.checked ? true : undefined },
              toEnd: { ...edge.toEnd, navigable: true },
            })}
            className="w-4 h-4 mt-[1px] accent-[#1e88e5] cursor-pointer" />
          <span className="text-[12.5px]">
            <strong className="text-[13px]">Bidirectional</strong>
            <span className="text-slate-500 mt-[2px] leading-[1.4] block">Both ends navigable — arrowheads on both ends.</span>
          </span>
        </label>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create `AttributeEditor.tsx`; delete `SchemaEditor.tsx`**

Model it on SchemaEditor's grid; keep the drag-to-reorder logic verbatim (SchemaEditor.tsx lines 16–39, 65–81). Columns `Handle · Name · Type · Mult · Vis · Description · ×`:

```tsx
import { useState } from "react";
import { GripVertical } from "lucide-react";
import type { Attribute, Visibility } from "@mc/okf";
import { InfoTip } from "./InfoTip";

const VISIBILITIES: (Visibility | "")[] = ["", "+", "-", "#", "~"];

interface AttributeEditorProps {
  attributes: Attribute[];
  onChange: (attributes: Attribute[]) => void;
}

export function AttributeEditor({ attributes, onChange }: AttributeEditorProps) {
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [overIdx, setOverIdx] = useState<number | null>(null);

  const update = (i: number, patch: Partial<Attribute>) =>
    onChange(attributes.map((a, idx) => idx === i ? { ...a, ...patch } : a));
  const remove = (i: number) => onChange(attributes.filter((_, idx) => idx !== i));
  const add = () => onChange([...attributes, { name: "", type: { name: "String" }, multiplicity: "1" }]);
  const move = (from: number, to: number) => {
    if (from === to || from < 0 || to < 0) return;
    const next = attributes.slice();
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    onChange(next);
  };

  const cols = "16px minmax(100px,1fr) minmax(90px,1fr) 62px 52px minmax(120px,1.3fr) 24px";
  const inputCls = "w-full text-[12.5px] px-[7px] py-[5px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

  return (
    <div className="border border-[#d8dee8] rounded-[10px] overflow-hidden">
      <div className="overflow-x-auto">
        <div className="min-w-[540px]">
          <div className="grid bg-[#f8fafc] px-[10px] py-[7px] text-[10.5px] font-semibold text-slate-500 uppercase tracking-[0.3px] border-b border-[#d8dee8] gap-[6px]" style={{ gridTemplateColumns: cols }}>
            <span />
            <span>Name</span>
            <span className="flex items-center gap-[3px]">Type <InfoTip text="A bare token (String, OrderId) or another classifier's title. Links to other docs survive import; editing the text keeps a plain token." /></span>
            <span className="flex items-center gap-[3px]">Mult <InfoTip text="UML multiplicity: 1, 0..1, *, 1..*, 2..5. Blank means 1." /></span>
            <span className="flex items-center gap-[3px]">Vis <InfoTip text="Visibility: + public, - private, # protected, ~ package. Optional; the uml-domain profile hides it on canvas." /></span>
            <span>Description</span>
            <span />
          </div>
          {attributes.map((a, i) => (
            <div key={i}
              onDragOver={e => { if (dragIdx === null) return; e.preventDefault(); if (overIdx !== i) setOverIdx(i); }}
              onDrop={e => { e.preventDefault(); if (dragIdx !== null) move(dragIdx, i); setDragIdx(null); setOverIdx(null); }}
              className={`grid px-[10px] py-[6px] border-b border-[#eef1f5] last:border-b-0 items-center gap-[6px] ${dragIdx === i ? "opacity-40" : ""} ${overIdx === i && dragIdx !== null && dragIdx !== i ? "bg-[#e6f1fb]" : ""}`}
              style={{ gridTemplateColumns: cols }}>
              <span draggable
                onDragStart={e => { setDragIdx(i); e.dataTransfer.effectAllowed = "move"; }}
                onDragEnd={() => { setDragIdx(null); setOverIdx(null); }}
                title="Drag to reorder"
                className="flex items-center justify-center text-slate-300 hover:text-slate-500 cursor-grab active:cursor-grabbing">
                <GripVertical size={13} />
              </span>
              <input type="text" value={a.name} placeholder="name" onChange={e => update(i, { name: e.target.value })} className={inputCls} />
              <input type="text" value={a.type.name} placeholder="String"
                onChange={e => update(i, { type: { name: e.target.value } })} className={inputCls} />
              <input type="text" value={a.multiplicity} placeholder="1"
                onChange={e => update(i, { multiplicity: e.target.value || "1" })} className={inputCls} />
              <select value={a.visibility ?? ""} aria-label="Visibility"
                onChange={e => update(i, { visibility: (e.target.value || undefined) as Visibility | undefined })}
                className="w-full text-[11.5px] px-[4px] py-[5px] border border-[#d8dee8] rounded-lg text-slate-900">
                {VISIBILITIES.map(v => <option key={v} value={v}>{v || "—"}</option>)}
              </select>
              <input type="text" value={a.description ?? ""} placeholder="description"
                onChange={e => update(i, { description: e.target.value || undefined })} className={inputCls} />
              <button onClick={() => remove(i)} title="Remove attribute"
                className="border-none bg-transparent text-slate-300 cursor-pointer text-[15px] p-0 hover:text-[#ef4444] flex items-center justify-center">×</button>
            </div>
          ))}
        </div>
      </div>
      <button onClick={add}
        className="w-full border-none bg-white px-2 py-[8px] text-[12.5px] font-semibold text-[#1e88e5] cursor-pointer hover:bg-[#f8fafc] transition-colors border-t border-[#eef1f5]">
        + Add attribute
      </button>
    </div>
  );
}
```

Pinned decision reminder: editing the Type text replaces the whole `type` object with `{ name }` — a linked `ref` survives only if the text is untouched.

Then `git rm packages/web/src/components/inspector/SchemaEditor.tsx`.

- [ ] **Step 5: Rewrite `ObjectInspector.tsx`**

```tsx
import type { ModelNode, Attribute } from "@mc/okf";
import { AttributeEditor } from "./AttributeEditor";
import { InfoTip } from "./InfoTip";

// Task 14 (stage 4) replaces this hardcoded list with the active profile's palette.
// `uml.Association` and `uml.Note` are intentionally NOT offered here: association classes
// are authored via an `as [link]` name on a relationship, and notes via the `## Notes`
// shorthand / a standalone note doc — not by adding a bare node. Both still render if imported.
const METACLASSES = ["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType", "uml.Package"];

interface ObjectInspectorProps {
  node: ModelNode;
  onUpdate: (patch: Partial<ModelNode>) => void;
}

const inputCls = "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";
const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";

export function ObjectInspector({ node, onUpdate }: ObjectInspectorProps) {
  const isEnum = node.type === "uml.Enum";
  return (
    <div className="flex flex-col gap-[15px]">
      <div>
        <label className={labelCls}>Title</label>
        <input type="text" value={node.title} onChange={e => onUpdate({ title: e.target.value })} className={inputCls} />
      </div>
      <div>
        <label className={labelCls}>Description</label>
        <textarea value={node.description ?? ""} rows={3}
          onChange={e => onUpdate({ description: e.target.value || undefined })}
          className={`${inputCls} resize-y min-h-[60px]`} />
      </div>
      <div className="flex gap-[10px]">
        <div className="flex-1">
          <label className={`${labelCls} flex items-center gap-[5px]`}>
            Type <InfoTip text="family.Metaclass dispatch key (e.g. uml.Class). Unknown values render as a generic box — never an error." />
          </label>
          <input type="text" list="okf-metaclasses" value={node.type}
            onChange={e => onUpdate({ type: e.target.value })} className={inputCls} />
          <datalist id="okf-metaclasses">{METACLASSES.map(t => <option key={t} value={t} />)}</datalist>
        </div>
        <label className="flex items-end gap-[7px] pb-[9px] cursor-pointer text-[12.5px] text-slate-700">
          <input type="checkbox" checked={node.abstract ?? false}
            onChange={e => onUpdate({ abstract: e.target.checked || undefined })}
            className="w-4 h-4 accent-[#1e88e5] cursor-pointer" />
          abstract
        </label>
      </div>
      <div>
        <label className={`${labelCls} flex items-center gap-[5px]`}>
          Stereotypes <InfoTip text="Comma-separated, open set: entity, valueObject, aggregateRoot, service, domainEvent — invent any. Rendered as «guillemets»." />
        </label>
        <input type="text" value={node.stereotypes.join(", ")}
          onChange={e => onUpdate({ stereotypes: e.target.value.split(",").map(s => s.trim()).filter(Boolean) })}
          placeholder="aggregateRoot, entity" className={inputCls} />
      </div>
      {isEnum ? (
        <div>
          <label className={labelCls}>Values (one per line)</label>
          <textarea value={(node.values ?? []).join("\n")} rows={5}
            onChange={e => onUpdate({ values: e.target.value.split("\n").map(v => v.trim()).filter(Boolean) })}
            className={`${inputCls} font-mono resize-y`} />
        </div>
      ) : (
        <div>
          <label className={labelCls}>Attributes</label>
          <AttributeEditor attributes={node.attributes} onChange={(attributes: Attribute[]) => onUpdate({ attributes })} />
        </div>
      )}
    </div>
  );
}
```

In `Inspector.tsx`: remove the `joinFieldType` import (line 6); the `RelationshipInspector` call site (lines 132–146) loses `onEnsureField` entirely; `EmptyState` copy line 46 "Changes here are pushed to the matching Data Mart." → "Changes apply to your local model."

- [ ] **Step 6: Run inspector tests, then the FULL STAGE GATE**

Run: `pnpm --filter @mc/web test -- RelationshipInspector`
Expected: PASS.

Stage gate (repo root):

```bash
pnpm lint && pnpm build && pnpm -r test
git grep -n "inputSource\|owoxId\|storageId\|SchemaField\|JoinKey\|cardinality" -- packages/web/src packages/okf/src
```

Expected: gate green; the grep returns hits only in `packages/okf/src/migrate.ts` (legacy mapping) and comments. Fix any straggler before proceeding.

- [ ] **Step 7: Commit + push (stage 1 lands)**

```bash
git add -A packages/web/src/components/inspector
git commit -m "feat(web): inspector edits classifiers and relationships (UML model)"
git push origin main
```

---

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

# Stage 3 — Metaclass renderer registry + generic fallback, incl. `uml.Association` class box (+ dashed mid-line connector) and `uml.Note` dog-eared box (+ dashed anchors) (spec rollout step 3)

### Task 11: node renderer registry, UML metaclass renderers (incl. `uml.Association`, `uml.Note`), generic box

**Files:**
- Create: `packages/web/src/components/canvas/nodes/shared.tsx`, `nodes/GenericNode.tsx`, `nodes/uml.tsx`, `nodes/registry.ts`, `nodes/OkfNode.tsx`
- Delete: `packages/web/src/components/canvas/MartNode.tsx`, `MartNode.test.tsx`
- Modify: `packages/web/src/components/canvas/Canvas.tsx` (line 45 import + line 151 `nodeTypes`), `packages/web/src/components/canvas/layoutSize.ts` (account for values rows)
- Test: `packages/web/src/components/canvas/nodes/registry.test.tsx` (new)

**Interfaces:**
- Consumes: `splitType` from `@mc/okf`; `OkfNodeData = ModelNode & { _viewMode?: ViewMode }` (Task 6 shape, renamed).
- Produces: `resolveNodeRenderer(type: string): ComponentType<OkfNodeProps>`; `OkfNode` (memoized RF component registered as `nodeTypes = { okf: OkfNode }`); `export interface OkfNodeProps { data: OkfNodeData }`. Task 13 extends `OkfNodeData` with `_profile`.

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/canvas/nodes/registry.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { resolveNodeRenderer } from "./registry";
import { GenericNode } from "./GenericNode";
import type { ModelNode } from "@mc/okf";

const node = (over: Partial<ModelNode>): ModelNode =>
  ({ key: "n1", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 }, ...over });

const draw = (n: ModelNode) => {
  const C = resolveNodeRenderer(n.type);
  render(<ReactFlowProvider><C data={{ ...n, _viewMode: "erd" }} /></ReactFlowProvider>);
};

describe("metaclass renderer registry", () => {
  it("unknown family falls back to the generic box (never errors)", () => {
    expect(resolveNodeRenderer("bpmn.Task")).toBe(GenericNode);
    expect(resolveNodeRenderer("Data Mart")).toBe(GenericNode);
    expect(resolveNodeRenderer("uml.Nope")).toBe(GenericNode);
  });
  it("uml.Association resolves to a dedicated renderer (class box with attributes)", () => {
    expect(resolveNodeRenderer("uml.Association")).not.toBe(GenericNode);
    draw(node({ type: "uml.Association", title: "Places",
      attributes: [{ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" }] }));
    expect(screen.getByText("Places")).toBeTruthy();
    expect(screen.getByText("placedAt")).toBeTruthy();
  });
  it("uml.Note renders its body in a dog-eared box with no attribute compartment", () => {
    expect(resolveNodeRenderer("uml.Note")).not.toBe(GenericNode);
    draw(node({ type: "uml.Note", title: "Domestic-only", body: "Only for domestic customers.",
      attributes: [{ name: "shouldNotRender", type: { name: "X" }, multiplicity: "1" }] }));
    expect(screen.getByText("Only for domestic customers.")).toBeTruthy();
    expect(screen.queryByText("shouldNotRender")).toBeNull();
  });
  it("uml.Class renders stereotypes in guillemets and italic abstract name", () => {
    draw(node({ stereotypes: ["aggregateRoot"], abstract: true,
      attributes: [{ name: "id", type: { name: "OrderId" }, multiplicity: "1" }] }));
    expect(screen.getByText("«aggregateRoot»")).toBeTruthy();
    const title = screen.getByText("Order");
    expect(title.className).toContain("italic");
    expect(screen.getByText("id")).toBeTruthy();
  });
  it("uml.Interface shows the «interface» keyword", () => {
    draw(node({ type: "uml.Interface", title: "PricingService" }));
    expect(screen.getByText("«interface»")).toBeTruthy();
  });
  it("uml.Enum lists its literals under «enumeration»", () => {
    draw(node({ type: "uml.Enum", title: "OrderStatus", values: ["DRAFT", "PLACED"] }));
    expect(screen.getByText("«enumeration»")).toBeTruthy();
    expect(screen.getByText("DRAFT")).toBeTruthy();
  });
  it("generic box still shows title and attributes", () => {
    draw(node({ type: "whatever", attributes: [{ name: "x", type: { name: "Y" }, multiplicity: "1" }] }));
    expect(screen.getByText("Order")).toBeTruthy();
    expect(screen.getByText("x")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- registry`
Expected: FAIL (modules missing).

- [ ] **Step 3: Create `nodes/shared.tsx`**

```tsx
import { useState } from "react";
import { Handle, Position } from "@xyflow/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { Attribute, ModelNode } from "@mc/okf";
import type { ViewMode } from "../../../state/viewMode";
import { ERD_COLLAPSED_ROWS } from "../layoutSize";

export type OkfNodeData = ModelNode & { _viewMode?: ViewMode };
export interface OkfNodeProps { data: OkfNodeData }

export const NODE_FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif";

// Node-level connectable ports (the only way to draw a new relationship).
export function NodePorts() {
  const common = {
    width: 13, height: 13, borderRadius: "50%",
    background: "#fff", border: "2px solid #1e88e5",
    top: 24, opacity: 0, transition: "opacity 0.12s",
  } as const;
  return (
    <>
      <Handle type="source" position={Position.Left} id="left" style={{ ...common, left: -7 }} className="mart-handle" />
      <Handle type="source" position={Position.Right} id="right" style={{ ...common, right: -7 }} className="mart-handle" />
    </>
  );
}

export function StereotypeRow({ stereotypes, keyword }: { stereotypes: string[]; keyword?: string }) {
  if (!keyword && stereotypes.length === 0) return null;
  return (
    <div className="px-3 pt-[7px] text-center text-[10.5px] leading-tight text-slate-500">
      {keyword && <span className="block">{`«${keyword}»`}</span>}
      {stereotypes.map(s => <span key={s} className="mr-1">{`«${s}»`}</span>)}
    </div>
  );
}

export function AttributeRow({ a, showVisibility }: { a: Attribute; showVisibility?: boolean }) {
  return (
    <div className="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
      {showVisibility && a.visibility && <span className="text-slate-400 font-mono">{a.visibility}</span>}
      <span className="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
      <span className="text-slate-400 font-mono text-[10.5px] truncate">
        {a.type.name}{a.multiplicity !== "1" ? ` [${a.multiplicity}]` : ""}
      </span>
    </div>
  );
}

// Attribute compartment with the collapsed/expand toggle (ERD_COLLAPSED_ROWS).
export function RowsCompartment({ rows, render }: { rows: number; render: (i: number) => React.ReactNode }) {
  const [expanded, setExpanded] = useState(false);
  if (rows === 0) return null;
  const visible = expanded ? rows : Math.min(rows, ERD_COLLAPSED_ROWS);
  const hidden = rows - ERD_COLLAPSED_ROWS;
  return (
    <div className="border-t border-[#eef1f5]">
      {Array.from({ length: visible }, (_, i) => render(i))}
      {hidden > 0 && (
        <button onClick={e => { e.stopPropagation(); setExpanded(v => !v); }}
          className="w-full flex items-center justify-center gap-1 px-3 py-[5px] text-[11px] font-medium text-[#1e88e5] hover:bg-[#f1f5fb] border-t border-[#f3f5f8]">
          {expanded ? <><ChevronDown size={12} /> Show less</> : <><ChevronRight size={12} /> +{hidden} more</>}
        </button>
      )}
    </div>
  );
}

export function ClassifierBox({ data, keyword, header }: { data: OkfNodeData; keyword?: string; header?: React.ReactNode }) {
  const isDetailed = (data._viewMode ?? "compact") === "erd";
  return (
    <div className="relative bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-[0_2px_8px_rgba(15,23,42,0.05)] cursor-grab hover:border-[#c2cad8] select-none w-[230px]"
      style={{ fontFamily: NODE_FONT }}>
      {header}
      <StereotypeRow stereotypes={data.stereotypes} keyword={keyword} />
      <div className={`px-3 pb-[9px] pt-[3px] text-center text-[13.5px] font-semibold text-slate-900 ${data.abstract ? "italic" : ""}`}>
        {data.title}
      </div>
      {isDetailed && data.values && data.values.length > 0 && (
        <RowsCompartment rows={data.values.length}
          render={i => (
            <div key={data.values![i]} className="px-3 py-[5px] text-[11.5px] text-slate-800 border-b border-[#f3f5f8] last:border-b-0">
              {data.values![i]}
            </div>
          )} />
      )}
      {isDetailed && !data.values && (
        <RowsCompartment rows={data.attributes.length}
          render={i => <AttributeRow key={data.attributes[i].name + i} a={data.attributes[i]} />} />
      )}
      {!isDetailed && (
        <div className="px-3 pb-[10px] text-center text-[11px] text-slate-500">
          {data.values ? `${data.values.length} values` : `${data.attributes.length} attribute${data.attributes.length === 1 ? "" : "s"}`}
        </div>
      )}
      <NodePorts />
    </div>
  );
}
```

- [ ] **Step 4: Create `nodes/GenericNode.tsx`, `nodes/uml.tsx`, `nodes/registry.ts`, `nodes/OkfNode.tsx`**

`GenericNode.tsx` — the mandatory fallback (name + attributes, opaque type shown as a chip):

```tsx
import { ClassifierBox, type OkfNodeProps } from "./shared";

export function GenericNode({ data }: OkfNodeProps) {
  return (
    <ClassifierBox data={data}
      header={<div className="px-3 pt-[8px]">
        <span className="text-[10px] font-[650] uppercase tracking-[0.3px] px-[7px] py-[2px] rounded-full text-white bg-[#94a3b8]">
          {data.type}
        </span>
      </div>} />
  );
}
```

`nodes/uml.tsx` — the closed metaclass set:

```tsx
import { ClassifierBox, NodePorts, type OkfNodeProps } from "./shared";

export function UmlClassNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} />;
}
export function UmlInterfaceNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="interface" />;
}
export function UmlEnumNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="enumeration" />;
}
export function UmlDataTypeNode({ data }: OkfNodeProps) {
  return <ClassifierBox data={data} keyword="dataType" />;
}
export function UmlPackageNode({ data }: OkfNodeProps) {
  // Tabbed-folder: a small tab above the box.
  return (
    <div className="relative">
      <div className="absolute -top-[10px] left-[10px] h-[12px] w-[64px] rounded-t-md border-[1.5px] border-b-0 border-[#d8dee8] bg-white" />
      <ClassifierBox data={data} />
    </div>
  );
}
export function UmlAssociationNode({ data }: OkfNodeProps) {
  // Association class: an ordinary class box (name / attributes), drawn with a dashed
  // outline. The dashed connector from the box to the association line's midpoint is
  // drawn by the edge renderer (Task 12) for the edge whose `name = { ref }` points here.
  return <div className="[&>div]:border-dashed"><ClassifierBox data={data} keyword="association" /></div>;
}
export function UmlNoteNode({ data }: OkfNodeProps) {
  // UML Comment: a dog-eared note box carrying the markdown body; NO attribute /
  // operation compartments. Its dashed anchor(s) to the annotated element(s) are
  // drawn by the edge/anchor layer (Task 12).
  return (
    <div className="relative w-[210px] bg-[#fffdf3] border-[1.5px] border-[#e3d9a8] shadow-[0_2px_8px_rgba(15,23,42,0.05)] select-none"
      style={{ clipPath: "polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 0 100%)" }}>
      <div className="absolute top-0 right-0 h-[14px] w-[14px] border-l border-b border-[#e3d9a8] bg-[#f3ebc0]" />
      <div className="px-3 py-[9px] text-[11.5px] leading-snug text-slate-700 whitespace-pre-wrap">
        {data.body ?? data.title}
      </div>
      <NodePorts />
    </div>
  );
}
```

`nodes/registry.ts`:

```ts
import type { ComponentType } from "react";
import { splitType } from "@mc/okf";
import { GenericNode } from "./GenericNode";
import { UmlClassNode, UmlInterfaceNode, UmlEnumNode, UmlDataTypeNode, UmlPackageNode, UmlAssociationNode, UmlNoteNode } from "./uml";
import type { OkfNodeProps } from "./shared";

// Closed metaclass set per family — everything else degrades to GenericNode.
const FAMILIES: Record<string, Record<string, ComponentType<OkfNodeProps>>> = {
  uml: {
    Class: UmlClassNode,
    Interface: UmlInterfaceNode,
    Enum: UmlEnumNode,
    DataType: UmlDataTypeNode,
    Package: UmlPackageNode,
    Association: UmlAssociationNode,   // association class — class box + dashed mid-line connector (edge side)
    Note: UmlNoteNode,                 // dog-eared comment box — no compartments
  },
};

export function resolveNodeRenderer(type: string): ComponentType<OkfNodeProps> {
  const t = splitType(type);
  return (t && FAMILIES[t.family]?.[t.metaclass]) ?? GenericNode;
}
```

`nodes/OkfNode.tsx` (the single RF node type):

```tsx
import { memo } from "react";
import type { NodeProps } from "@xyflow/react";
import { resolveNodeRenderer } from "./registry";
import type { OkfNodeData } from "./shared";

function OkfNodeInner({ data }: NodeProps) {
  const node = data as unknown as OkfNodeData;
  const Renderer = resolveNodeRenderer(node.type);
  return <Renderer data={node} />;
}

export const OkfNode = memo(OkfNodeInner);
```

In `Canvas.tsx`: line 45 `import { MartNode } from "./MartNode";` → `import { OkfNode } from "./nodes/OkfNode";`; line 151 `const nodeTypes = { okf: OkfNode };`. Then `git rm packages/web/src/components/canvas/MartNode.tsx packages/web/src/components/canvas/MartNode.test.tsx`. In `layoutSize.ts`, count values too: `const total = node.values ? node.values.length : node.attributes.length;` (keep the rest).

- [ ] **Step 5: Run tests and the web suite**

Run: `pnpm --filter @mc/web test -- registry && pnpm --filter @mc/web test`
Expected: PASS (MartNode tests are gone; nothing else referenced it except Canvas).

- [ ] **Step 6: Commit**

```bash
git add -A packages/web/src/components/canvas
git commit -m "feat(web): metaclass renderer registry with generic fallback box"
```

### Task 12: UML edge rendering (verb → line style + end adornments); `uml.Association` dashed connector + `uml.Note` dashed anchors + STAGE 3 GATE

**Files:**
- Rewrite: `packages/web/src/components/canvas/RelEdge.tsx`
- Create: `packages/web/src/components/canvas/AnchorEdge.tsx` (dashed connector for association classes + note anchors)
- Modify: `packages/web/src/components/canvas/edges.ts` (add `buildAnchorEdges`), `packages/web/src/components/canvas/Canvas.tsx` (register the `anchor` edge type + append anchor edges)
- Test: rewrite `packages/web/src/components/canvas/RelEdge.test.tsx`; extend `edges.test.ts` (anchor synthesis)

**Interfaces:**
- Consumes: edge `data` `{ kind, fromEnd, toEnd, bidirectional, modelEdgeId, relLabelMode }` (Task 6); `graph.nodes`/`graph.edges` for anchor synthesis.
- Produces: `RelEdge` drawing per spec: associates = solid + arrowhead on navigable end(s); aggregates = hollow diamond at source; composes = filled diamond at source; specializes = hollow triangle at target; implements = dashed + hollow triangle; depends = dashed + open arrow; multiplicity/role labels near each end. Plus `buildAnchorEdges(nodes, edges): Edge[]` — a dashed, marker-less `AnchorEdge` from each `uml.Association` node to the association it names (`edge.name = { ref }`), and from each `uml.Note` node to each of its `annotates` targets (classifier node, or the source node of an annotated association — an RF edge can only attach to a node, so an edge-midpoint anchor is approximated by anchoring to the association's source node).

- [ ] **Step 1: Write the failing tests**

Replace `RelEdge.test.tsx` (render through ReactFlow's edge test harness pattern already used in the Task 6 version — a plain `render(<svg><RelEdge …/></svg>)` with the minimal `EdgeProps`):

```tsx
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { RelEdge } from "./RelEdge";
import type { EdgeProps } from "@xyflow/react";
import { Position } from "@xyflow/react";

const base: EdgeProps = {
  id: "e1", source: "a", target: "b",
  sourceX: 0, sourceY: 0, targetX: 100, targetY: 0,
  sourcePosition: Position.Right, targetPosition: Position.Left,
  selected: false,
} as unknown as EdgeProps;

const draw = (data: Record<string, unknown>) =>
  render(<svg>{/* EdgeLabelRenderer needs no provider when labels off */}<RelEdge {...base} data={data} /></svg>);

describe("RelEdge UML adornments", () => {
  it("composes draws a filled diamond marker at the source", () => {
    const { container } = draw({ kind: "composes", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    const marker = container.querySelector("marker#diamond-filled-e1");
    expect(marker).toBeTruthy();
    const path = container.querySelector(".react-flow__edge-path") ?? container.querySelector("path[marker-start]");
    expect(container.innerHTML).toContain("marker-start");
  });
  it("aggregates draws a hollow diamond", () => {
    const { container } = draw({ kind: "aggregates", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.querySelector("marker#diamond-hollow-e1")).toBeTruthy();
  });
  it("specializes draws a hollow triangle at the target on a solid line", () => {
    const { container } = draw({ kind: "specializes", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.querySelector("marker#triangle-e1")).toBeTruthy();
    expect(container.innerHTML).not.toContain("stroke-dasharray");
  });
  it("implements and depends are dashed", () => {
    const { container } = draw({ kind: "implements", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.innerHTML).toContain("stroke-dasharray");
  });
  it("associates puts an arrowhead only on navigable ends", () => {
    const one = draw({ kind: "associates", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, relLabelMode: "hidden" });
    expect(one.container.innerHTML).toContain("marker-end");
    expect(one.container.innerHTML).not.toContain("marker-start");
    const both = draw({ kind: "associates", fromEnd: { navigable: true }, toEnd: { navigable: true }, bidirectional: true, relLabelMode: "hidden" });
    expect(both.container.innerHTML).toContain("marker-start");
  });
});
```

(If `EdgeLabelRenderer` throws outside a ReactFlow provider even when unused, wrap `draw` in `<ReactFlowProvider>` — the Task 6 tests establish which harness works; reuse it.)

Add to `edges.test.ts` (anchor synthesis) using the Task 4 `node`/`edge` builders:

```ts
import { buildAnchorEdges } from "./edges";

it("synthesises a dashed anchor from a uml.Association node to the association it names", () => {
  const nodes = [node("order", "Order"), node("customer", "Customer"),
    { ...node("places", "Places"), type: "uml.Association" }];
  const edges = [{ ...edge("e1", "order", "customer"), name: { ref: "places" } }];
  const anchors = buildAnchorEdges(nodes, edges);
  expect(anchors).toEqual([{ id: "ac-e1", source: "places", target: "order", type: "anchor", selectable: false }]);
});

it("synthesises a dashed anchor from a uml.Note to each annotated target", () => {
  const nodes = [node("order", "Order"),
    { ...node("n", "Domestic-only"), type: "uml.Note", annotates: [{ targetKey: "order" }, { sourceKey: "order", name: "places" }] }];
  const anchors = buildAnchorEdges(nodes, []);
  expect(anchors.map(a => `${a.source}->${a.target}`)).toEqual(["n->order", "n->order"]);
  expect(anchors.every(a => a.type === "anchor")).toBe(true);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- RelEdge edges`
Expected: FAIL (no kind-specific markers yet; `buildAnchorEdges` missing).

- [ ] **Step 3: Rewrite `RelEdge.tsx`**

```tsx
import { memo } from "react";
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from "@xyflow/react";
import type { ModelEdge, RelEnd, RelationshipKind } from "@mc/okf";
import type { RelLabelMode } from "../../state/relLabels";

export type RelEdgeData = Pick<ModelEdge, "kind" | "fromEnd" | "toEnd" | "bidirectional"> & {
  relLabelMode?: RelLabelMode;
  modelEdgeId?: string;
};

const DASHED: ReadonlySet<RelationshipKind> = new Set(["implements", "depends"]);

function RelEdgeInner(props: EdgeProps) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data, selected } = props;
  const d = data as unknown as RelEdgeData | undefined;
  const kind: RelationshipKind = d?.kind ?? "associates";
  const fromEnd: RelEnd = d?.fromEnd ?? {};
  const toEnd: RelEnd = d?.toEnd ?? {};
  const mode: RelLabelMode = d?.relLabelMode ?? "all";

  const [edgePath] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  const stroke = selected ? "#1e88e5" : "#64748b";
  const strokeWidth = selected ? 2.5 : 1.8;

  // Verb → end adornments (spec table).
  let markerStart: string | undefined;
  let markerEnd: string | undefined;
  const defs: React.ReactNode[] = [];
  const diamond = (fill: string, mid: string) => (
    <marker key={mid} id={`${mid}-${id}`} markerWidth="14" markerHeight="10" refX="1" refY="5" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,5 L7,1 L13,5 L7,9 z" fill={fill} stroke={stroke} strokeWidth="1" />
    </marker>
  );
  const triangle = (
    <marker key="triangle" id={`triangle-${id}`} markerWidth="14" markerHeight="12" refX="12" refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d="M1,1 L12,6 L1,11 z" fill="#fff" stroke={stroke} strokeWidth="1.2" />
    </marker>
  );
  const arrow = (key: string, flip: boolean) => (
    <marker key={key} id={`${key}-${id}`} markerWidth="12" markerHeight="12" refX={flip ? 1 : 10} refY="6" orient="auto" markerUnits="userSpaceOnUse">
      <path d={flip ? "M10,1 L1,6 L10,11" : "M1,1 L10,6 L1,11"} fill="none" stroke={stroke} strokeWidth="1.5" />
    </marker>
  );

  if (kind === "composes") { defs.push(diamond(stroke, "diamond-filled")); markerStart = `url(#diamond-filled-${id})`; }
  else if (kind === "aggregates") { defs.push(diamond("#fff", "diamond-hollow")); markerStart = `url(#diamond-hollow-${id})`; }
  else if (kind === "specializes" || kind === "implements") { defs.push(triangle); markerEnd = `url(#triangle-${id})`; }
  else if (kind === "depends") { defs.push(arrow("dep-arrow", false)); markerEnd = `url(#dep-arrow-${id})`; }
  else { // associates: arrowhead on navigable end(s)
    if (toEnd.navigable) { defs.push(arrow("nav-end", false)); markerEnd = `url(#nav-end-${id})`; }
    if (fromEnd.navigable) { defs.push(arrow("nav-start", true)); markerStart = `url(#nav-start-${id})`; }
  }

  const endText = (e: RelEnd) => [e.multiplicity, e.role].filter(Boolean).join(" ");
  const showLabels = mode !== "hidden";
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const labels: { x: number; y: number; text: string }[] = [];
  if (showLabels) {
    const ft = endText(fromEnd); const tt = endText(toEnd);
    if (ft) labels.push({ x: lerp(sourceX, targetX, 0.18), y: lerp(sourceY, targetY, 0.18) - 10, text: ft });
    if (tt) labels.push({ x: lerp(sourceX, targetX, 0.82), y: lerp(sourceY, targetY, 0.82) - 10, text: tt });
  }

  return (
    <>
      <defs>{defs}</defs>
      <BaseEdge id={id} path={edgePath} markerStart={markerStart} markerEnd={markerEnd}
        style={{ stroke, strokeWidth, ...(DASHED.has(kind) ? { strokeDasharray: "6 4" } : {}) }} />
      {labels.length > 0 && (
        <EdgeLabelRenderer>
          {labels.map((l, i) => (
            <div key={i} className="nodrag nopan"
              style={{ position: "absolute", transform: `translate(-50%, -50%) translate(${l.x}px,${l.y}px)`,
                background: "rgba(255,255,255,0.9)", borderRadius: 4, padding: "0 4px",
                fontSize: 10.5, fontWeight: 600, color: "#334155", pointerEvents: "all", whiteSpace: "nowrap" }}>
              {l.text}
            </div>
          ))}
        </EdgeLabelRenderer>
      )}
    </>
  );
}

export const RelEdge = memo(RelEdgeInner);
```

Create `packages/web/src/components/canvas/AnchorEdge.tsx` — a dashed, marker-less line for association-class connectors and note anchors (no adornments, no labels):

```tsx
import { memo } from "react";
import { BaseEdge, getStraightPath, type EdgeProps } from "@xyflow/react";

function AnchorEdgeInner({ id, sourceX, sourceY, targetX, targetY }: EdgeProps) {
  const [path] = getStraightPath({ sourceX, sourceY, targetX, targetY });
  return <BaseEdge id={id} path={path} style={{ stroke: "#94a3b8", strokeWidth: 1.2, strokeDasharray: "4 3" }} />;
}
export const AnchorEdge = memo(AnchorEdgeInner);
```

Add `buildAnchorEdges` to `edges.ts` (synthesises the dashed connectors as extra RF edges — anchors attach to nodes, so an edge-midpoint anchor is approximated by the association's source node):

```ts
export function buildAnchorEdges(nodes: ModelNode[], edges: ModelEdge[]): Edge[] {
  const has = new Set(nodes.map(n => n.key));
  const out: Edge[] = [];
  const anchor = (id: string, source: string, target: string): void => {
    if (has.has(source) && has.has(target)) out.push({ id, source, target, type: "anchor", selectable: false });
  };
  // Association class → the association line it names (approx: the association's source node).
  for (const e of edges) {
    if (e.name && typeof e.name === "object") anchor(`ac-${e.id}`, e.name.ref, e.from);
  }
  // uml.Note → each annotated element.
  for (const n of nodes) {
    if (n.type !== "uml.Note") continue;
    (n.annotates ?? []).forEach((a, i) => {
      const target = "targetKey" in a && !("sourceKey" in a) ? a.targetKey : (a as { sourceKey: string }).sourceKey;
      anchor(`note-${n.key}-${i}`, n.key, target);
    });
  }
  return out;
}
```

In `Canvas.tsx`: register `const edgeTypes = { rel: RelEdge, anchor: AnchorEdge };` and append `buildAnchorEdges(graph.nodes, graph.edges)` to the RF edges built by `buildRfEdges` in the edge-sync effect.

- [ ] **Step 4: Run tests + STAGE 3 GATE**

```bash
pnpm --filter @mc/web test -- RelEdge edges
pnpm lint && pnpm build && pnpm -r test
```

Expected: green.

- [ ] **Step 5: Commit + push (stage 3 lands)**

```bash
git add -A packages/web/src/components/canvas
git commit -m "feat(web): UML verb edge rendering (diamonds, triangles, navigability arrows) + association-class/note dashed anchors"
git push origin main
```

---

# Stage 4 — Profile mechanism: uml-domain (spec rollout step 4)

### Task 13: profiles module + stereotype styling

**Files:**
- Create: `packages/web/src/profiles/index.ts`, `packages/web/src/profiles/umlDomain.ts`
- Modify: `packages/web/src/components/canvas/nodes/shared.tsx` (apply styles), `nodes/OkfNode.tsx` + `Canvas.tsx` `toRFNode` (thread `_profile`)
- Test: `packages/web/src/profiles/profiles.test.ts` (new), extend `nodes/registry.test.tsx` assertions

**Interfaces:**
- Consumes: nothing new.
- Produces:
  ```ts
  export interface StereotypeStyle { header?: string; border?: "thick"; shape?: "hexagon" }
  export interface Profile {
    name: string;
    emphasize: readonly string[];   // open lens hint (spec: freeform), e.g. multiplicity, aggregation/composition diamonds, generalization, realization
    hide: readonly ("operations" | "visibility")[];
    stereotypes: Record<string, StereotypeStyle>;
    palette: { metaclasses: readonly string[]; stereotypes: readonly string[] };
  }
  export function getProfile(name?: string): Profile;   // unknown/undefined → UML_DOMAIN
  ```
  `OkfNodeData` gains `_profile?: string`. Task 14 consumes `palette` and `emphasize`.

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/profiles/profiles.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { getProfile } from "./index";

describe("profiles", () => {
  it("uml-domain is the default and the unknown-name fallback", () => {
    expect(getProfile().name).toBe("uml-domain");
    expect(getProfile("no-such-profile").name).toBe("uml-domain");
    expect(getProfile("uml-domain").hide).toContain("visibility");
    expect(getProfile("uml-domain").emphasize).toContain("multiplicity");
  });
  it("uml-domain styles the DDD stereotypes and offers the spec palette", () => {
    const p = getProfile("uml-domain");
    expect(p.stereotypes.aggregateRoot).toEqual({ header: "#eab308", border: "thick" });
    expect(p.stereotypes.valueObject).toEqual({ header: "#64748b" });
    expect(p.stereotypes.domainEvent).toEqual({ shape: "hexagon" });
    expect(p.palette.metaclasses).toEqual(["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"]);
    expect(p.palette.stereotypes).toEqual(["entity", "valueObject", "aggregateRoot", "service", "domainEvent"]);
  });
});
```

Add to `nodes/registry.test.tsx`:

```tsx
  it("stereotype styles from the profile decorate the box", () => {
    const C = resolveNodeRenderer("uml.Class");
    const { container } = render(
      <ReactFlowProvider>
        <C data={{ ...node({ stereotypes: ["aggregateRoot"] }), _viewMode: "erd", _profile: "uml-domain" }} />
      </ReactFlowProvider>,
    );
    const box = container.querySelector("[data-stereotyped]") as HTMLElement;
    expect(box.style.borderColor).toBe("rgb(234, 179, 8)");
  });
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- profiles registry`
Expected: FAIL.

- [ ] **Step 3: Create the profile modules**

`packages/web/src/profiles/index.ts`:

```ts
import { UML_DOMAIN } from "./umlDomain";

// A profile is pure data: render lens (emphasize/hide), stereotype → style map,
// and the palette the "add node" UI offers. Adding «saga» tomorrow = one line in
// a profile module, no renderer change.
export interface StereotypeStyle { header?: string; border?: "thick"; shape?: "hexagon" }
export interface Profile {
  name: string;
  emphasize: readonly string[];   // open lens hint (spec: freeform), e.g. multiplicity, aggregation/composition diamonds, generalization, realization
  hide: readonly ("operations" | "visibility")[];
  stereotypes: Record<string, StereotypeStyle>;
  palette: { metaclasses: readonly string[]; stereotypes: readonly string[] };
}

const PROFILES: Record<string, Profile> = { [UML_DOMAIN.name]: UML_DOMAIN };

/** Unknown or missing profile name falls back to uml-domain — never errors. */
export function getProfile(name?: string): Profile {
  return (name && PROFILES[name]) || UML_DOMAIN;
}

/** Merge every named stereotype's style; later stereotypes win per property. */
export function stereotypeStyle(profile: Profile, stereotypes: string[]): StereotypeStyle {
  return stereotypes.reduce<StereotypeStyle>((acc, s) => ({ ...acc, ...profile.stereotypes[s] }), {});
}
```

`packages/web/src/profiles/umlDomain.ts` (the spec's illustrative YAML, as data):

```ts
import type { Profile } from "./index";

export const UML_DOMAIN: Profile = {
  name: "uml-domain",
  emphasize: ["multiplicity", "aggregation", "composition", "generalization", "realization"],
  hide: ["operations", "visibility"],
  stereotypes: {
    aggregateRoot: { header: "#eab308", border: "thick" }, // gold
    valueObject: { header: "#64748b" },                    // slate
    domainEvent: { shape: "hexagon" },
  },
  palette: {
    // `uml.Association` and `uml.Note` are intentionally omitted: association classes
    // are authored via an `as [link]` name on a relationship, and notes via the `## Notes`
    // shorthand / a standalone note doc — never by adding a bare node from the palette.
    // Both still render (Task 11) when present in an imported/authored model.
    metaclasses: ["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"],
    stereotypes: ["entity", "valueObject", "aggregateRoot", "service", "domainEvent"],
  },
};
```

(Circular-import note: `index.ts` imports `umlDomain.ts` which imports the `Profile` *type* back — type-only, safe. If ESLint complains, move the interfaces to `profiles/types.ts` and re-export.)

- [ ] **Step 4: Apply styles in `nodes/shared.tsx`**

`OkfNodeData` gains `_profile?: string`. `ClassifierBox` resolves the style:

```ts
import { getProfile, stereotypeStyle } from "../../../profiles";
```

```tsx
export function ClassifierBox({ data, keyword, header }: { data: OkfNodeData; keyword?: string; header?: React.ReactNode }) {
  const profile = getProfile(data._profile);
  const st = stereotypeStyle(profile, data.stereotypes);
  const isDetailed = (data._viewMode ?? "compact") === "erd";
  const showVisibility = !profile.hide.includes("visibility");
  const boxStyle: React.CSSProperties = {
    fontFamily: NODE_FONT,
    ...(st.header ? { borderTopColor: st.header, borderTopWidth: 4 } : {}),
    ...(st.border === "thick" ? { borderColor: st.header ?? "#334155", borderWidth: 2.5 } : {}),
    ...(st.shape === "hexagon" ? { clipPath: "polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)", borderRadius: 0 } : {}),
  };
  return (
    <div data-stereotyped={Object.keys(st).length > 0 || undefined}
      className="relative bg-white border-[1.5px] border-[#d8dee8] rounded-xl shadow-[0_2px_8px_rgba(15,23,42,0.05)] cursor-grab hover:border-[#c2cad8] select-none w-[230px]"
      style={boxStyle}>
      {/* …unchanged body, but pass showVisibility down… */}
```

and `AttributeRow` receives `showVisibility={showVisibility}` in the attributes `RowsCompartment` render callback.

Thread the profile: `Canvas.tsx` `toRFNode(n, viewMode)` gains a third arg `profileName: string` → `data: { ...n, _viewMode: viewMode, _profile: profileName }`. Until Task 16 introduces diagram state, call it with the constant `"uml-domain"`:

```ts
useEffect(() => {
  setRfNodes(graph.nodes.map(n => toRFNode(n, viewMode, "uml-domain")));
}, [graph.nodes, viewMode, setRfNodes]);
```

- [ ] **Step 5: Run tests**

Run: `pnpm --filter @mc/web test -- profiles registry`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/profiles packages/web/src/components/canvas
git commit -m "feat(web): uml-domain profile — stereotype styles as data, applied by renderers"
```

### Task 14: palette + emphasis wiring + STAGE 4 GATE

**Files:**
- Modify: `packages/web/src/components/inspector/ObjectInspector.tsx` (palette-driven selects), `packages/web/src/components/canvas/edges.ts` + `RelEdge.tsx` (emphasis), `Canvas.tsx` (pass profile to `buildRfEdges`), `Inspector.tsx` (pass profile name through)
- Test: extend `RelationshipInspector.test.tsx` sibling coverage via a new `ObjectInspector.test.tsx`

**Interfaces:**
- Consumes: `getProfile`, `Profile` (Task 13).
- Produces: `ObjectInspector` props gain `profileName?: string`; datalists fed by `getProfile(profileName).palette`; `buildRfEdges(edges, nodes, viewMode, relLabelMode, emphasizeMultiplicity: boolean)` → edge data gains `emphasizeMultiplicity`; RelEdge hides end labels when `emphasizeMultiplicity` is false.

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/inspector/ObjectInspector.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ObjectInspector } from "./ObjectInspector";
import type { ModelNode } from "@mc/okf";

const node: ModelNode = { key: "n1", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 0, y: 0 } };

describe("ObjectInspector palette", () => {
  it("offers the profile's metaclasses in the type datalist", () => {
    const { container } = render(<ObjectInspector node={node} onUpdate={() => {}} profileName="uml-domain" />);
    const options = [...container.querySelectorAll("datalist#okf-metaclasses option")].map(o => o.getAttribute("value"));
    expect(options).toEqual(["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"]);
  });
  it("offers the profile's stereotypes in a datalist", () => {
    const { container } = render(<ObjectInspector node={node} onUpdate={() => {}} profileName="uml-domain" />);
    const options = [...container.querySelectorAll("datalist#okf-stereotypes option")].map(o => o.getAttribute("value"));
    expect(options).toContain("aggregateRoot");
  });
  it("switching type to uml.Enum shows the values editor", () => {
    const onUpdate = vi.fn();
    render(<ObjectInspector node={{ ...node, type: "uml.Enum", values: ["A"] }} onUpdate={onUpdate} profileName="uml-domain" />);
    expect(screen.getByText("Values (one per line)")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- ObjectInspector`
Expected: FAIL (no `profileName` prop; hardcoded METACLASSES includes `uml.Package`).

- [ ] **Step 3: Palette-drive `ObjectInspector.tsx`**

Replace the `METACLASSES` constant with the profile palette:

```tsx
import { getProfile } from "../../profiles";

interface ObjectInspectorProps {
  node: ModelNode;
  onUpdate: (patch: Partial<ModelNode>) => void;
  profileName?: string;
}

export function ObjectInspector({ node, onUpdate, profileName }: ObjectInspectorProps) {
  const palette = getProfile(profileName).palette;
  const isEnum = node.type === "uml.Enum";
  ...
          <datalist id="okf-metaclasses">{palette.metaclasses.map(t => <option key={t} value={t} />)}</datalist>
```

and under the Stereotypes input add:

```tsx
        <datalist id="okf-stereotypes">{palette.stereotypes.map(s => <option key={s} value={s} />)}</datalist>
```

(attach `list="okf-stereotypes"` to the stereotypes input — the comma-separated field still parses freely; the datalist is a hint, agnosticism preserved). `Inspector.tsx` gains a passthrough prop `profileName?: string` handed to `ObjectInspector`; `Canvas.tsx` passes `profileName="uml-domain"` (Task 16 replaces with the active diagram's profile).

- [ ] **Step 4: Emphasis wiring**

`edges.ts`: `buildRfEdges(edges, nodes, viewMode, relLabelMode = "all", emphasizeMultiplicity = true)`; `compactEdge` gains the flag and puts `emphasizeMultiplicity` into `data`. `RelEdge.tsx`: `const showLabels = mode !== "hidden" && (d?.emphasizeMultiplicity ?? true);` (add `emphasizeMultiplicity?: boolean` to `RelEdgeData`). `Canvas.tsx` line 203:

```ts
useEffect(() => {
  const emphasizeMultiplicity = getProfile("uml-domain").emphasize.includes("multiplicity");
  setRfEdges(buildRfEdges(graph.edges, graph.nodes, viewMode, relLabelMode, emphasizeMultiplicity));
}, [graph.edges, graph.nodes, viewMode, relLabelMode, setRfEdges]);
```

Add an `edges.test.ts` case: `buildRfEdges(..., false)` → every edge's `data.emphasizeMultiplicity === false`; and a `RelEdge.test.tsx` case: `emphasizeMultiplicity: false` renders no label div even in `"all"` mode.

- [ ] **Step 5: Run tests + STAGE 4 GATE**

```bash
pnpm --filter @mc/web test -- ObjectInspector edges RelEdge
pnpm lint && pnpm build && pnpm -r test
```

Expected: green.

- [ ] **Step 6: Commit + push (stage 4 lands)**

```bash
git add -A packages/web/src
git commit -m "feat(web): profile palette drives inspector; emphasis gates multiplicity labels"
git push origin main
```

---

# Stage 5 — Diagrams as first-class views (spec rollout step 5)

### Task 15: okf — parse/serialize `type: Diagram` docs

**Files:**
- Modify: `packages/okf/src/parse.ts`, `packages/okf/src/serialize.ts`
- Test: `packages/okf/test/diagram.test.ts` (new)

**Interfaces:**
- Consumes: `Diagram`, `DiagramHints` types; slug machinery.
- Produces: `parseBundle` splits docs into diagram docs (`frontmatter type === "Diagram"`) and classifier docs; diagram docs are NOT nodes. `## Members` links → member keys; `## Render hints` lines: `- emphasize: a, b`, `- collapse [T](./slug.md)`, `- [T](./slug.md) at 12,34` (position applied onto `node.position`, pinned decision 4). `serializeBundle` emits one `<folder>/<diagram.key>.md` per diagram, members carrying `at x,y` from node positions.

- [ ] **Step 1: Write the failing tests**

Create `packages/okf/test/diagram.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { parseBundle, serializeBundle } from "../src/index";
import type { ModelGraph } from "../src/types";

const diagram = `---
type: Diagram
title: Orders Domain Model
profile: uml-domain
---
# Orders Domain Model

## Members
- [Order](./order.md) at 40,80
- [Customer](./customer.md)

## Render hints
- emphasize: multiplicity, composition
- collapse [Customer](./customer.md)
`;
const order = `---\ntype: uml.Class\ntitle: Order\n---\n# Order\n`;
const customer = `---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n`;

describe("diagram docs", () => {
  const g = parseBundle({ "m/orders-domain-model.md": diagram, "m/order.md": order, "m/customer.md": customer });

  it("a Diagram doc is not a node", () => {
    expect(g.nodes.map(n => n.key).sort()).toEqual(["customer", "order"]);
  });
  it("members resolve to node keys in order; profile read from frontmatter", () => {
    expect(g.diagrams).toHaveLength(1);
    expect(g.diagrams[0]).toMatchObject({
      key: "orders-domain-model", title: "Orders Domain Model", profile: "uml-domain",
      members: ["order", "customer"],
    });
  });
  it("render hints: emphasize + collapse", () => {
    expect(g.diagrams[0].hints).toEqual({ emphasize: ["multiplicity", "composition"], collapse: ["customer"] });
  });
  it("member `at x,y` lands on node.position", () => {
    expect(g.nodes.find(n => n.key === "order")!.position).toEqual({ x: 40, y: 80 });
  });
  it("round-trips diagrams", () => {
    const graph: ModelGraph = {
      nodes: [
        { key: "order", type: "uml.Class", title: "Order", stereotypes: [], attributes: [], position: { x: 12, y: 34 } },
        { key: "money", type: "uml.DataType", title: "Money", stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
      ],
      edges: [],
      diagrams: [{ key: "core", title: "Core", profile: "uml-domain", members: ["order", "money"], hints: { collapse: ["money"] } }],
    };
    const files = serializeBundle(graph, "Shop").files;
    expect(files["shop/core.md"]).toContain("type: \"Diagram\"");
    expect(files["shop/core.md"]).toContain("- [Order](./order.md) at 12,34");
    expect(files["shop/core.md"]).toContain("- collapse [Money](./money.md)");
    const back = parseBundle(files);
    expect(back.diagrams).toHaveLength(1);
    expect(back.diagrams[0].members).toEqual(["order", "money"]);
    expect(back.diagrams[0].hints).toEqual({ collapse: ["money"] });
    expect(back.nodes.find(n => n.key === "order")!.position).toEqual({ x: 12, y: 34 });
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/okf test -- diagram`
Expected: FAIL (diagram doc parsed as a node).

- [ ] **Step 3: Implement in `parse.ts`**

At the top of `parseBundle`, partition docs:

```ts
  const all = Object.entries(files).filter(([p]) => p.endsWith(".md") && !p.endsWith("index.md"));
  const diagramDocs = all.filter(([, t]) => parseFrontmatter(t).data.type === "Diagram");
  const docs = all.filter(([, t]) => parseFrontmatter(t).data.type !== "Diagram");
```

After nodes and edges are built, parse diagrams:

```ts
  const diagrams: Diagram[] = [];
  const MEMBER_RE = /^- \[[^\]]*\]\(\.\/(.+?)\.md\)(?:\s+at\s+(-?\d+)\s*,\s*(-?\d+))?\s*$/;
  for (const [path, text] of diagramDocs) {
    const { data, body } = parseFrontmatter(text);
    const key = path.split("/").pop()!.replace(/\.md$/, "");
    const { sections } = splitSections(body);
    const membersSec = sections.find(s => /^members$/i.test(s.title));
    const hintsSec = sections.find(s => /^render hints$/i.test(s.title));
    const members: string[] = [];
    for (const raw of membersSec?.lines ?? []) {
      const m = MEMBER_RE.exec(raw.replace(/\r$/, "").trim());
      if (!m) continue;
      const k = slugToKey.get(basename(m[1]));
      if (!k) continue;
      members.push(k);
      if (m[2] !== undefined) {
        const n = nodes.find(x => x.key === k);
        if (n) n.position = { x: Number(m[2]), y: Number(m[3]) };
      }
    }
    const hints: DiagramHints = {};
    for (const raw of hintsSec?.lines ?? []) {
      const ln = raw.replace(/\r$/, "").trim();
      const em = /^- emphasize:\s*(.+)$/i.exec(ln);
      if (em) { hints.emphasize = em[1].split(",").map(s => s.trim()).filter(Boolean); continue; }
      const co = /^- collapse \[[^\]]*\]\(\.\/(.+?)\.md\)\s*$/i.exec(ln);
      if (co) {
        const k = slugToKey.get(basename(co[1]));
        if (k) hints.collapse = [...(hints.collapse ?? []), k];
        continue;
      }
      const at = MEMBER_RE.exec(ln); // hints may also carry "- [T](./s.md) at x,y" (spec example)
      if (at && at[2] !== undefined) {
        const k = slugToKey.get(basename(at[1]));
        const n = k ? nodes.find(x => x.key === k) : undefined;
        if (n) n.position = { x: Number(at[2]), y: Number(at[3]) };
      }
    }
    diagrams.push({
      key,
      title: data.title || "Untitled diagram",
      profile: typeof data.profile === "string" && data.profile ? data.profile : "uml-domain",
      members,
      ...(hints.emphasize || hints.collapse ? { hints } : {}),
    });
  }
  return { nodes, edges, diagrams };
```

(Import `Diagram, DiagramHints` types.)

- [ ] **Step 4: Implement in `serialize.ts`**

In `serializeBundle`, after node files, before index:

```ts
  for (const d of graph.diagrams) {
    const memberLines = d.members.map(k => {
      const n = graph.nodes.find(x => x.key === k);
      if (!n) return null;
      const at = n.position.x !== 0 || n.position.y !== 0 ? ` at ${Math.round(n.position.x)},${Math.round(n.position.y)}` : "";
      return `- [${n.title}](./${slugByKey.get(k)}.md)${at}`;
    }).filter((l): l is string => l !== null);
    const hintLines: string[] = [];
    if (d.hints?.emphasize?.length) hintLines.push(`- emphasize: ${d.hints.emphasize.join(", ")}`);
    for (const k of d.hints?.collapse ?? []) {
      const n = graph.nodes.find(x => x.key === k);
      if (n) hintLines.push(`- collapse [${n.title}](./${slugByKey.get(k)}.md)`);
    }
    const hints = hintLines.length ? `\n## Render hints\n${hintLines.join("\n")}\n` : "";
    const fm = renderFrontmatter({ type: "Diagram", title: d.title, profile: d.profile });
    files[`${folder}/${d.key}.md`] = `---\n${fm}\n---\n\n# ${d.title}\n\n## Members\n${memberLines.join("\n")}\n${hints}`;
  }
```

Guard diagram file-name collisions with node slugs: when building the `taken` set, pre-seed it with `graph.diagrams.map(d => d.key)` so a node titled like a diagram key gets suffixed.

- [ ] **Step 5: Run + commit**

Run: `pnpm --filter @mc/okf build && pnpm --filter @mc/okf test`
Expected: PASS (all files).

```bash
git add packages/okf/src packages/okf/test/diagram.test.ts
git commit -m "feat(okf): Diagram docs — members, profiles, render hints, positions"
```

### Task 16: web — multi-diagram state, switcher, member filtering

**Files:**
- Create: `packages/web/src/state/diagrams.ts`, `packages/web/src/components/canvas/DiagramTabs.tsx`
- Modify: `packages/web/src/components/canvas/Canvas.tsx`, `packages/web/src/state/model.ts` (diagram CRUD + auto-membership)
- Test: `packages/web/src/state/diagrams.test.ts` (new)

**Interfaces:**
- Consumes: `Diagram`, `effectiveDiagrams` below; `getProfile` (Task 13).
- Produces:
  ```ts
  // state/diagrams.ts
  export const ALL_DIAGRAM_KEY = "__all__";
  export function effectiveDiagrams(g: ModelGraph): Diagram[];   // [] ⇒ [implicit All]
  export function loadActiveDiagramKey(): string | null;         // localStorage "mc.activeDiagram.v1"
  export function persistActiveDiagramKey(key: string): void;
  ```
  Store additions: `addDiagram(title: string): Diagram` (key `d<n>`, profile `"uml-domain"`, members = current node keys), `updateDiagram(key, patch: Partial<Diagram>)`, `removeDiagram(key)`; `addNode` gains optional `diagramKey?: string` — when given and not `ALL_DIAGRAM_KEY`, the new node's key is appended to that diagram's members (new nodes must be visible where they're created).

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/state/diagrams.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { effectiveDiagrams, ALL_DIAGRAM_KEY } from "./diagrams";
import { createModelStore } from "./model";
import type { ModelGraph } from "@mc/okf";

const node = (key: string): ModelGraph["nodes"][0] =>
  ({ key, title: key, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

describe("effectiveDiagrams", () => {
  it("empty diagrams ⇒ one implicit All diagram with every node", () => {
    const g: ModelGraph = { nodes: [node("a"), node("b")], edges: [], diagrams: [] };
    const d = effectiveDiagrams(g);
    expect(d).toHaveLength(1);
    expect(d[0]).toMatchObject({ key: ALL_DIAGRAM_KEY, profile: "uml-domain", members: ["a", "b"] });
  });
  it("explicit diagrams pass through untouched", () => {
    const g: ModelGraph = { nodes: [node("a")], edges: [], diagrams: [{ key: "d1", title: "D", profile: "p", members: ["a"] }] };
    expect(effectiveDiagrams(g)).toEqual(g.diagrams);
  });
});

describe("store diagram CRUD", () => {
  it("addDiagram seeds members with current nodes; addNode joins the active diagram", () => {
    const store = createModelStore({ nodes: [node("n1")], edges: [], diagrams: [] });
    const d = store.addDiagram("Core");
    expect(d.members).toEqual(["n1"]);
    const n = store.addNode({ x: 0, y: 0 }, d.key);
    expect(store.get().diagrams[0].members).toContain(n.key);
  });
  it("removeDiagram deletes only the view", () => {
    const store = createModelStore({ nodes: [node("n1")], edges: [], diagrams: [] });
    const d = store.addDiagram("Core");
    store.removeDiagram(d.key);
    expect(store.get().diagrams).toEqual([]);
    expect(store.get().nodes).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- diagrams`
Expected: FAIL.

- [ ] **Step 3: Create `state/diagrams.ts` and extend the store**

```ts
import type { Diagram, ModelGraph } from "@mc/okf";

export const ALL_DIAGRAM_KEY = "__all__";

/** Empty diagrams array = today's single implicit graph as one default diagram. */
export function effectiveDiagrams(g: ModelGraph): Diagram[] {
  if (g.diagrams.length > 0) return g.diagrams;
  return [{ key: ALL_DIAGRAM_KEY, title: "All", profile: "uml-domain", members: g.nodes.map(n => n.key) }];
}

const KEY = "mc.activeDiagram.v1";

export function loadActiveDiagramKey(): string | null {
  try { return localStorage.getItem(KEY); } catch { return null; }
}
export function persistActiveDiagramKey(key: string): void {
  try { localStorage.setItem(KEY, key); } catch { /* best-effort */ }
}
```

In `state/model.ts` add (inside the returned object; `uid` already exists — diagram keys use a `d` prefix, sharing the counter is fine):

```ts
    addDiagram(title: string): Diagram {
      const d: Diagram = { key: uid("d"), title, profile: "uml-domain", members: g.nodes.map(n => n.key) };
      g = { ...g, diagrams: [...g.diagrams, d] }; emit(); return d;
    },
    updateDiagram(key: string, patch: Partial<Diagram>) {
      g = { ...g, diagrams: g.diagrams.map(d => d.key === key ? { ...d, ...patch } : d) }; emit();
    },
    removeDiagram(key: string) {
      g = { ...g, diagrams: g.diagrams.filter(d => d.key !== key) }; emit();
    },
```

and change `addNode`:

```ts
    addNode(position: { x: number; y: number }, diagramKey?: string): ModelNode {
      const n: ModelNode = { key: uid("n"), type: "uml.Class", title: "New object", stereotypes: [], attributes: [], position };
      g = { ...g,
        nodes: [...g.nodes, n],
        diagrams: diagramKey ? g.diagrams.map(d => d.key === diagramKey ? { ...d, members: [...d.members, n.key] } : d) : g.diagrams,
      };
      emit(); return n;
    },
```

(Import `Diagram` from `@mc/okf`.)

- [ ] **Step 4: `DiagramTabs.tsx` + Canvas wiring**

Create `packages/web/src/components/canvas/DiagramTabs.tsx`:

```tsx
import type { Diagram } from "@mc/okf";

interface DiagramTabsProps {
  diagrams: Diagram[];
  activeKey: string;
  onSelect: (key: string) => void;
  onCreate: () => void;
}

export function DiagramTabs({ diagrams, activeKey, onSelect, onCreate }: DiagramTabsProps) {
  return (
    <div data-dock className="absolute top-[12px] left-1/2 -translate-x-1/2 z-[6] flex items-center gap-1 rounded-xl bg-white/95 px-1.5 py-1 shadow-[0_1px_4px_rgba(15,23,42,0.12)]">
      {diagrams.map(d => (
        <button key={d.key} onClick={() => onSelect(d.key)}
          className={`px-3 py-[5px] rounded-lg text-[12px] font-[600] whitespace-nowrap ${d.key === activeKey ? "bg-[#e6f1fb] text-[#1e88e5]" : "text-slate-600 hover:bg-[#f1f3f7]"}`}>
          {d.title}
        </button>
      ))}
      <button onClick={onCreate} title="New diagram from the current nodes"
        className="px-2 py-[5px] rounded-lg text-[13px] text-slate-500 hover:bg-[#f1f3f7]">+</button>
    </div>
  );
}
```

In `Canvas.tsx` (CanvasInner):

```ts
import { effectiveDiagrams, ALL_DIAGRAM_KEY, loadActiveDiagramKey, persistActiveDiagramKey } from "../../state/diagrams";
import { DiagramTabs } from "./DiagramTabs";
import { getProfile } from "../../profiles";
```

```ts
  const diagrams = effectiveDiagrams(graph);
  const [activeDiagramKey, setActiveDiagramKey] = useState<string>(loadActiveDiagramKey() ?? diagrams[0].key);
  const activeDiagram = diagrams.find(d => d.key === activeDiagramKey) ?? diagrams[0];
  useEffect(() => { persistActiveDiagramKey(activeDiagram.key); }, [activeDiagram.key]);
  const profile = getProfile(activeDiagram.profile);
  const memberSet = new Set(activeDiagram.members);
```

The two RF sync effects filter to members and use the diagram's profile + hints:

```ts
  useEffect(() => {
    setRfNodes(graph.nodes.filter(n => memberSet.has(n.key))
      .map(n => toRFNode(n, viewMode, activeDiagram.profile, activeDiagram.hints?.collapse?.includes(n.key) ?? false)));
  }, [graph.nodes, graph.diagrams, viewMode, activeDiagram, setRfNodes]);   // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const visible = graph.edges.filter(e => memberSet.has(e.from) && memberSet.has(e.to));
    const emphasizeMultiplicity = profile.emphasize.includes("multiplicity") &&
      !(activeDiagram.hints?.emphasize && !activeDiagram.hints.emphasize.includes("multiplicity"));
    setRfEdges(buildRfEdges(visible, graph.nodes, viewMode, relLabelMode, emphasizeMultiplicity));
  }, [graph.edges, graph.nodes, graph.diagrams, viewMode, relLabelMode, activeDiagram, setRfEdges]); // eslint-disable-line react-hooks/exhaustive-deps
```

`toRFNode` signature: `toRFNode(n, viewMode, profileName, collapsed)` → data gains `_collapsed: collapsed`; `nodes/OkfNode.tsx` short-circuits:

```tsx
  if (node._collapsed) {
    return (
      <div className="relative rounded-full border border-[#d8dee8] bg-white px-3 py-[6px] text-[12px] font-[600] text-slate-600 shadow-sm">
        {node.title}
        <NodePorts />
      </div>
    );
  }
```

(add `_collapsed?: boolean` to `OkfNodeData`; export `NodePorts` use from `shared.tsx`). Both add-node paths pass the active diagram: `store.addNode(pos, activeDiagram.key === ALL_DIAGRAM_KEY ? undefined : activeDiagram.key)` (pane click line ~260 and double-click line ~327). Render `<DiagramTabs diagrams={diagrams} activeKey={activeDiagram.key} onSelect={setActiveDiagramKey} onCreate={() => { const name = window.prompt("Diagram name?", "New diagram"); if (name) { const d = store.addDiagram(name); setActiveDiagramKey(d.key); } }} />` inside the canvas wrapper div, right above `<Dock …/>`. Pass `profileName={activeDiagram.profile}` to `Inspector`, which forwards it to `ObjectInspector` (Task 14 prop).

- [ ] **Step 5: Run tests + full web suite**

Run: `pnpm --filter @mc/web test`
Expected: PASS (diagrams.test green; existing canvas tests unaffected because `effectiveDiagrams` keeps single-graph behavior identical).

- [ ] **Step 6: Commit**

```bash
git add -A packages/web/src
git commit -m "feat(web): multi-diagram canvas — tabs, member filtering, collapse chips, per-diagram profile"
```

### Task 17: inspector external references + STAGE 5 GATE

**Files:**
- Create: `packages/web/src/components/inspector/ExternalRefs.tsx`
- Modify: `packages/web/src/components/inspector/Inspector.tsx` (render under ObjectInspector), `packages/web/src/components/canvas/Canvas.tsx` (pass diagram context + navigation callback)
- Test: `packages/web/src/components/inspector/ExternalRefs.test.tsx`

**Interfaces:**
- Consumes: `effectiveDiagrams` (Task 16), edges/nodes from props.
- Produces:
  ```tsx
  interface ExternalRefsProps {
    nodeKey: string;
    nodes: ModelNode[];
    edges: ModelEdge[];
    members: string[];               // active diagram members
    diagrams: Diagram[];             // effective diagrams (for navigation targets)
    onNavigate: (diagramKey: string, nodeKey: string) => void;
  }
  export function ExternalRefs(props: ExternalRefsProps): JSX.Element | null;
  ```

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/components/inspector/ExternalRefs.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ExternalRefs } from "./ExternalRefs";
import type { ModelEdge, ModelNode, Diagram } from "@mc/okf";

const node = (key: string, title: string): ModelNode =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const nodes = [node("order", "Order"), node("money", "Money"), node("checkout", "Checkout")];
const edges: ModelEdge[] = [
  { id: "e1", kind: "associates", from: "order", to: "money", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false },
  { id: "e2", kind: "depends", from: "checkout", to: "order", fromEnd: {}, toEnd: {}, bidirectional: false },
  { id: "e3", kind: "associates", from: "order", to: "order2x", fromEnd: {}, toEnd: {}, bidirectional: false }, // dangling → ignored
];
const diagrams: Diagram[] = [
  { key: "domain", title: "Domain", profile: "uml-domain", members: ["order"] },
  { key: "shared", title: "Shared", profile: "uml-domain", members: ["money", "checkout"] },
];

describe("ExternalRefs", () => {
  it("lists incoming and outgoing off-diagram relationships as chips", () => {
    render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges} members={["order"]} diagrams={diagrams} onNavigate={() => {}} />);
    expect(screen.getByText(/associates → Money/)).toBeTruthy();
    expect(screen.getByText(/Checkout → depends/)).toBeTruthy();
  });
  it("clicking a chip navigates to a diagram containing the other node", () => {
    const onNavigate = vi.fn();
    render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges} members={["order"]} diagrams={diagrams} onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText(/associates → Money/));
    expect(onNavigate).toHaveBeenCalledWith("shared", "money");
  });
  it("renders nothing when every relationship is on-diagram", () => {
    const { container } = render(<ExternalRefs nodeKey="order" nodes={nodes} edges={edges}
      members={["order", "money", "checkout"]} diagrams={diagrams} onNavigate={() => {}} />);
    expect(container.firstChild).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- ExternalRefs`
Expected: FAIL (module missing).

- [ ] **Step 3: Create `ExternalRefs.tsx`**

```tsx
import type { Diagram, ModelEdge, ModelNode } from "@mc/okf";

interface ExternalRefsProps {
  nodeKey: string;
  nodes: ModelNode[];
  edges: ModelEdge[];
  members: string[];
  diagrams: Diagram[];
  onNavigate: (diagramKey: string, nodeKey: string) => void;
}

// The spec's "isolate a domain, still see other sources" behavior: relationships
// whose other end is off-diagram surface here as navigable chips.
export function ExternalRefs({ nodeKey, nodes, edges, members, diagrams, onNavigate }: ExternalRefsProps) {
  const memberSet = new Set(members);
  const byKey = new Map(nodes.map(n => [n.key, n]));
  const refs: { key: string; label: string; other: string }[] = [];
  for (const e of edges) {
    if (e.from === nodeKey && !memberSet.has(e.to) && byKey.has(e.to)) {
      refs.push({ key: e.id, label: `${e.kind} → ${byKey.get(e.to)!.title}`, other: e.to });
    } else if (e.to === nodeKey && !memberSet.has(e.from) && byKey.has(e.from)) {
      refs.push({ key: e.id, label: `${byKey.get(e.from)!.title} → ${e.kind}`, other: e.from });
    }
  }
  if (refs.length === 0) return null;
  const diagramFor = (k: string) => diagrams.find(d => d.members.includes(k))?.key;
  return (
    <div>
      <label className="block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">
        External references
      </label>
      <div className="flex flex-wrap gap-[6px]">
        {refs.map(r => {
          const target = diagramFor(r.other);
          return (
            <button key={r.key} disabled={!target}
              onClick={() => target && onNavigate(target, r.other)}
              title={target ? "Open the diagram containing this node" : "Not on any diagram"}
              className="rounded-full border border-[#d8dee8] bg-white px-[10px] py-[4px] text-[11.5px] text-slate-600 hover:border-[#1e88e5] hover:text-[#1e88e5] disabled:opacity-50">
              {r.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire into Inspector + Canvas**

`Inspector.tsx` props gain `externalRefs?: React.ReactNode`; render it after the `ObjectInspector` in the node branch (`<>{objectInspector}{externalRefs}</>`). In `Canvas.tsx`, inside the `panel.active === "inspect"` render:

```tsx
externalRefs={selection?.type === "node" ? (
  <ExternalRefs nodeKey={selection.id} nodes={graph.nodes} edges={graph.edges}
    members={activeDiagram.members} diagrams={diagrams}
    onNavigate={(diagramKey, nodeKey) => { setActiveDiagramKey(diagramKey); setSelection({ type: "node", id: nodeKey }); }} />
) : undefined}
```

(On the implicit All diagram nothing is external — `members` = every node — so the panel naturally disappears; correct per spec.)

- [ ] **Step 5: Run + STAGE 5 GATE**

```bash
pnpm --filter @mc/web test -- ExternalRefs
pnpm lint && pnpm build && pnpm -r test
```

Expected: green.

- [ ] **Step 6: Commit + push (stage 5 lands)**

```bash
git add -A packages/web/src
git commit -m "feat(web): external-reference chips navigate across diagrams"
git push origin main
```

---

# Stage 6 — UML-domain template + author guide (spec rollout step 6)

### Task 18: "Orders Domain" UML template

**Files:**
- Create: `packages/web/src/templates/orders-domain.ts`
- Modify: `packages/web/src/templates/helpers.ts` (new UML helpers), `packages/web/src/templates/index.ts` (register)
- Test: extend `packages/web/src/templates/templates.test.ts`

**Interfaces:**
- Consumes: pinned types.
- Produces: helpers `attr(name, type, opts?)`, `cls(key, title, opts?)`, `enumOf(key, title, values, description?)`, `edge(id, kind, from, to, fromEnd?, toEnd?)`; template export `ordersDomain: Template` with `id: "uml_orders_domain"`, `category: "dataset"` (it demos the format, not an industry niche), one diagram `orders-domain`.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/templates/templates.test.ts`:

```ts
import { ordersDomain } from "./orders-domain";

describe("orders-domain UML template", () => {
  it("is registered under a stable deep-link id", () => {
    expect(TEMPLATES.some(t => t.id === "uml_orders_domain")).toBe(true);
  });
  it("uses stereotypes, an enum, composition and a diagram", () => {
    const g = ordersDomain.graph;
    const order = g.nodes.find(n => n.key === "order")!;
    expect(order.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(g.nodes.find(n => n.key === "order-status")!.values).toContain("PLACED");
    const compose = g.edges.find(e => e.kind === "composes")!;
    expect(compose).toMatchObject({ from: "order", to: "order-line" });
    expect(g.diagrams).toHaveLength(1);
    expect(g.diagrams[0].profile).toBe("uml-domain");
    expect(g.diagrams[0].members).toContain("order");
  });
  it("attribute refs point at real member nodes", () => {
    const g = ordersDomain.graph;
    const keys = new Set(g.nodes.map(n => n.key));
    for (const n of g.nodes) for (const a of n.attributes)
      if (a.type.ref) expect(keys.has(a.type.ref)).toBe(true);
  });
});
```

Also update the first `templates.test.ts` shape test: it asserts every node is `uml.Class` — loosen to `expect(n.type).toMatch(/^uml\./)` (the new template has Enum/Interface/DataType nodes).

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @mc/web test -- templates.test`
Expected: FAIL.

- [ ] **Step 3: Add UML helpers to `templates/helpers.ts`**

```ts
// ── UML-profile authoring helpers (stage 6+) ───────────────────────────────
export const attr = (
  name: string,
  type: string | { name: string; ref: string },
  opts: { mult?: string; vis?: Attribute["visibility"]; desc?: string } = {},
): Attribute => ({
  name,
  type: typeof type === "string" ? { name: type } : type,
  multiplicity: opts.mult ?? "1",
  ...(opts.vis ? { visibility: opts.vis } : {}),
  ...(opts.desc ? { description: opts.desc } : {}),
});

export const cls = (
  key: string,
  title: string,
  opts: { type?: string; stereotypes?: string[]; abstract?: boolean; description?: string; attributes?: Attribute[] } = {},
): ModelNode => ({
  key, title,
  type: opts.type ?? "uml.Class",
  stereotypes: opts.stereotypes ?? [],
  ...(opts.abstract ? { abstract: true } : {}),
  ...(opts.description ? { description: opts.description } : {}),
  attributes: opts.attributes ?? [],
  position: { x: 0, y: 0 },
});

export const enumOf = (key: string, title: string, values: string[], description?: string): ModelNode =>
  ({ key, title, type: "uml.Enum", stereotypes: [], ...(description ? { description } : {}), attributes: [], values, position: { x: 0, y: 0 } });

export const edge = (
  id: string,
  kind: ModelEdge["kind"],
  from: string,
  to: string,
  fromEnd: ModelEdge["fromEnd"] = {},
  toEnd: ModelEdge["toEnd"] = {},
): ModelEdge => ({
  id, kind, from, to, fromEnd,
  toEnd: kind === "associates" ? { navigable: true, ...toEnd } : toEnd,
  bidirectional: false,
});
```

- [ ] **Step 4: Create `templates/orders-domain.ts`** (the spec's worked example as a template)

```ts
import type { ModelGraph } from "@mc/okf";
import { attr, cls, enumOf, edge, type Template } from "./helpers";

// The spec's Order domain worked example — doubles as the living demo of the
// uml-domain profile (stereotype styles, composition, enum, value objects).
const graph: ModelGraph = {
  nodes: [
    cls("order", "Order", {
      stereotypes: ["aggregateRoot", "entity"],
      description: "A customer's placed order.",
      attributes: [
        attr("id", "OrderId"),
        attr("placedAt", "Timestamp"),
        attr("status", { name: "OrderStatus", ref: "order-status" }),
        attr("shippingAddress", { name: "Address", ref: "address" }, { mult: "0..1" }),
        attr("total", { name: "Money", ref: "money" }),
      ],
    }),
    cls("order-line", "OrderLine", {
      stereotypes: ["entity"],
      attributes: [
        attr("quantity", "Int"),
        attr("unitPrice", { name: "Money", ref: "money" }),
      ],
    }),
    cls("customer", "Customer", {
      stereotypes: ["aggregateRoot", "entity"],
      attributes: [attr("id", "CustomerId"), attr("name", "String"), attr("email", "Email")],
    }),
    enumOf("order-status", "OrderStatus", ["DRAFT", "PLACED", "SHIPPED", "CANCELLED"]),
    cls("money", "Money", {
      type: "uml.DataType", stereotypes: ["valueObject"],
      attributes: [attr("amount", "Decimal"), attr("currency", "CurrencyCode")],
    }),
    cls("address", "Address", {
      stereotypes: ["valueObject"],
      attributes: [attr("street", "String"), attr("city", "String"), attr("country", "CountryCode")],
    }),
    cls("pricing-service", "PricingService", { type: "uml.Interface", stereotypes: ["service"] }),
  ],
  edges: [
    edge("e1", "associates", "order", "customer", { multiplicity: "1", role: "order" }, { multiplicity: "1", role: "customer" }),
    edge("e2", "composes", "order", "order-line", { multiplicity: "1" }, { multiplicity: "1..*", role: "lines" }),
    edge("e3", "depends", "order", "pricing-service"),
  ],
  diagrams: [{
    key: "orders-domain",
    title: "Orders Domain Model",
    profile: "uml-domain",
    members: ["order", "order-line", "customer", "order-status", "money", "address", "pricing-service"],
  }],
};

export const ordersDomain: Template = {
  id: "uml_orders_domain",
  nicheId: null,
  category: "dataset",
  name: "Orders Domain (UML)",
  description: "DDD-flavored UML domain model: aggregate root, entities, value objects, an enum and a service interface.",
  graph,
};
```

Register it in `packages/web/src/templates/index.ts` alongside the existing exports (add the import and prepend `ordersDomain` to the `TEMPLATES` array so it's the visible showcase).

- [ ] **Step 5: Run tests**

Run: `pnpm --filter @mc/web test -- templates.test templateLink`
Expected: PASS (templateLink already deep-clones any registered template — no change needed).

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/templates
git commit -m "feat(web): Orders Domain UML template (uml-domain showcase)"
```

### Task 19: author guide rewrite + FINAL GATE

**Files:**
- Rewrite: `packages/web/public/okf-format.md` (8.1K, currently the mart/Joins guide)
- Modify: `packages/web/src/okf/guideExample.test.ts` (add new-format worked example; keep the legacy block as legacy-import coverage), `packages/web/public/llms.txt` (pointer copy only)

**Interfaces:**
- Consumes: everything; this is the documentation gate.
- Produces: an author guide whose worked example is executable by the test suite (the drift guard the old guide had).

- [ ] **Step 1: Add the failing guide test**

In `packages/web/src/okf/guideExample.test.ts`, keep the existing `GUIDE_EXAMPLE` describe (rename its describe to `"legacy mart format still imports"`) and add:

```ts
const UML_GUIDE_EXAMPLE = `
<!-- shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId [1]
- status: [OrderStatus](./order-status.md) [1]
- total: [Money](./money.md) [1]

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines

<!-- shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int [1]
- unitPrice: [Money](./money.md) [1]

<!-- shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId [1]
- name: String [1]

<!-- shop/order-status.md -->
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED

<!-- shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal [1]
- currency: CurrencyCode [1]

<!-- shop/orders-domain.md -->
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
`;

describe("okf authoring guide — UML worked example imports", () => {
  const graph = filesToGraph({ "pasted.md": UML_GUIDE_EXAMPLE });

  it("parses 5 classifiers and 1 diagram", () => {
    expect(graph.nodes.map(n => n.key).sort()).toEqual(["customer", "money", "order", "order-line", "order-status"]);
    expect(graph.diagrams).toHaveLength(1);
    expect(graph.diagrams[0].members).toHaveLength(5);
  });
  it("stereotypes, refs, enum values and kinds all land", () => {
    const order = graph.nodes.find(n => n.key === "order")!;
    expect(order.stereotypes).toEqual(["aggregateRoot", "entity"]);
    expect(order.attributes.find(a => a.name === "total")!.type).toEqual({ name: "Money", ref: "money" });
    expect(graph.nodes.find(n => n.key === "order-status")!.values).toEqual(["DRAFT", "PLACED", "SHIPPED", "CANCELLED"]);
    expect(graph.edges.map(e => e.kind).sort()).toEqual(["associates", "composes"]);
    const compose = graph.edges.find(e => e.kind === "composes")!;
    expect(compose.toEnd).toMatchObject({ multiplicity: "1..*", role: "lines" });
  });
});
```

- [ ] **Step 2: Run to verify it passes already**

Run: `pnpm --filter @mc/web test -- guideExample`
Expected: PASS immediately (stages 2/5 built this) — if it fails, that's a real parser bug to fix before documenting the format.

- [ ] **Step 3: Rewrite `packages/web/public/okf-format.md`**

Structure (write it in full; the content mirrors the spec's "Node document format" / "Diagram document format" sections, which are normative):

1. **Title/intro:** "OKF Canvas — OKF authoring guide for AI agents". OKF = markdown docs; one file per classifier, `<!-- path/slug.md -->` markers when pasting; slug = title lowercased, spaces → hyphens.
2. **The 3 rules that make relationships work** (rewritten): (a) one file per classifier behind markers; (b) a relationship line is `- verb [Target Title](./target-slug.md)` and the slug must equal the target's file name; (c) `associates|aggregates|composes` REQUIRE `: <near> to <far>` ends, `specializes|implements|depends` FORBID them.
3. **Classifier doc:** frontmatter (`type: family.Metaclass` — `uml.Class`, `uml.Interface`, `uml.Enum`, `uml.DataType`, `uml.Package`; `stereotype:` scalar or list; `abstract:`; `title:`; `description:`), `## Attributes` grammar (`- [visibility ]name: Type [multiplicity]`, Type = bare token or `[Title](./slug.md)` link, multiplicities `1 0..1 * 1..* 2..5`, absent = `[1]`), `## Values` for enums.
4. **`## Relationships`:** the spec's relationship **taxonomy** (association = solid: `associates`/`aggregates`/`composes`, nested composition⊂aggregation⊂association; dependency = dashed: `depends`/`implements`-realization; generalization = solid+hollow▷: `specializes` — line derives from category, adornment from verb), the verb table, and the reciprocity rule (bidirectional = both docs declare the reverse line). Note the reading direction: for `specializes`, **near→far = child→parent** (the doc declaring `specializes [Parent]` is the child/subtype); no reading-direction arrow.
5. **Association names & association classes:** any relationship may carry an optional `as …` name after the link, before the `:` ends — `as "places"` (a plain reading-label, and the handle a note anchors by) or `as [Places](./places.md)` (a link to a `uml.Association` doc — an **association class** that carries its own `## Attributes`; the ends stay on the inline bullet so class→class navigation stays direct).
6. **Notes / comments (`uml.Note`):** a dog-eared comment with a `## Body` and an `annotates` relationship (classifier by plain link; association by source link + `as "name"`, or endpoint form `annotates [Src](./src.md) associates [Tgt](./tgt.md)` when unnamed). The `## Notes` list on a classifier is shorthand for a note that annotates just that classifier and round-trips back to `## Notes`.
7. **Diagram doc:** `type: Diagram`, `profile: uml-domain`, `## Members` (with optional `at x,y`), `## Render hints` (`- emphasize: …`, `- collapse [T](./slug.md)`).
8. **Graceful degradation note:** unknown `type` renders as a generic box; unknown sections are preserved.
9. **The worked example:** paste the exact `UML_GUIDE_EXAMPLE` content from Step 1 (they must stay character-identical — that's the drift guard; note this in a comment at the top of the test).
10. **Legacy note:** the old mart format (Schema tables + `## Joins`) still imports.

Update `packages/web/public/llms.txt`: adjust the one-line description of the format doc to "UML-flavored classifier + diagram markdown format" (keep the file's structure otherwise).

- [ ] **Step 4: FINAL GATE — full matrix-equivalent run**

```bash
pnpm lint && pnpm build && pnpm -r test
```

Expected: green. Then a manual smoke pass (dev server `pnpm dev`): load `?template=uml_orders_domain`, check stereotype styling + composition diamond, switch to the diagram tab, select Order, confirm no external refs on All, export OKF, re-import the zip, confirm round-trip.

- [ ] **Step 5: Commit + push (stage 6 lands)**

```bash
git add packages/web/public/okf-format.md packages/web/public/llms.txt packages/web/src/okf/guideExample.test.ts
git commit -m "docs: UML-profile OKF authoring guide with executable worked example"
git push origin main
```

---

## Plan self-review (performed while writing)

1. **Spec coverage:** three doc roles → Task 15 (`index.md` unchanged, Diagram split, classifier default) ✓; `family.Metaclass` + graceful degradation → Tasks 1/11 ✓; stereotypes → Tasks 1/9/11/13 ✓; profiles (lens/styles/palette) → Tasks 13/14 ✓; node doc format incl. visibility/multiplicity/values → Tasks 8/9/10 ✓; relationship BNF incl. the `<name>` production (`as "…"` / `as [link]`) + context rules + reciprocity → Tasks 8/9/10 ✓ (grammar capture + slug→key resolution + lossless emission); the `specializes` verb (renamed from `extends`) → Tasks 1/7/8/9/10/12/19 ✓; **association classes** (`uml.Association`): edge `name = { ref }` + classifier parse (no ends) + renderer + dashed mid-line connector → Tasks 1/8/9/10/11/12 ✓; **notes** (`uml.Note`): `## Body` + `annotates` anchors + `## Notes` desugar/round-trip + dog-eared renderer + dashed anchors → Tasks 1/9/10/11/12 ✓; diagram doc + members + hints + external refs → Tasks 15/16/17 ✓; data-model impact + migration of shares/templates → Tasks 1/4/5 ✓; rollout order and per-stage green → stage gates ✓; Deferred list → nothing here builds operations, other families, or the data profile ✓.
2. **Known interim compromises (explicit, not placeholders):** Task 3's legacy emission (replaced in Task 10); Task 6's interim MartNode (replaced in Task 11); hardcoded `"uml-domain"` in Canvas (replaced in Task 16).
3. **Type consistency spot-checks:** `fromEnd`/`toEnd` naming used everywhere; `attributes: Attribute[]` (never `schema`); `kind` (never `verb`); `getProfile` in Tasks 13/14/16; `effectiveDiagrams`/`ALL_DIAGRAM_KEY` in Tasks 16/17; `toRFNode(n, viewMode, profileName, collapsed)` final arity established in Task 16 (Tasks 6/13 versions are earlier evolution steps, each compiling at its own stage).

## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
