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


> **Split note:** This is **Stage 1 of 6** of the OKF UML plan — the FIRST stage; no prior stages have run. Implement ONLY the `### Task` sections in THIS file (Tasks 1-7). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context used by all stages.

# Stage 1 — Generalize the data model (spec rollout step 1)

Everything compiles and passes on the new `ModelGraph`; markdown format unchanged (legacy emission). Tasks 1–3 (okf) and 4–7 (web) form one stage; run the full gate at the end of Task 7 before pushing.

### Task 1: okf core types + migration module

**Files:**
- Rewrite: `packages/okf/src/types.ts` (currently 44 lines, node-shaped)
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
  it("maps a legacy node graph onto the UML model", () => {
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

// Pre-UML (data-node era) shapes as found in old localStorage payloads and
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
  it("maps a legacy node doc onto a generic classifier with attributes", () => {
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
it("decodes a legacy (node-era) share payload via migration", () => {
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
 *  Legacy (node-era) payloads are migrated — old share links keep opening. */
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
- Produces: `f(name, type, pk?, description?): Attribute` (pk ignored); `node(key, title, inputSource, attributes, description?): ModelNode` (inputSource ignored); `rel(id, from, to, left, right, cardinality?, bidirectional?): ModelEdge` (left/right ignored; cardinality → end multiplicities). Signatures UNCHANGED so the 22 template files compile untouched except the graph-literal sweep.

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
Expected: FAIL (helpers still build the node shape; template literals still carry `storageId`).

- [ ] **Step 3: Rewrite `packages/web/src/templates/helpers.ts`**

```ts
import type { ModelGraph, ModelNode, ModelEdge, Attribute } from "@mc/okf";
import { endsFromCardinality } from "@mc/okf";

// ── tiny authoring helpers ─────────────────────────────────────────────────
// Signatures kept from the node era so the 22 template files stay untouched:
// `pk`, `inputSource` and join fields are accepted and dropped (data-profile
// concerns, deferred); cardinality maps onto per-end multiplicities.
export const f = (name: string, type: string, _pk = false, description?: string): Attribute =>
  ({ name, type: { name: type }, multiplicity: "1", ...(description ? { description } : {}) });

export const node = (
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
git commit -m "feat(web): templates emit the generalized model (helpers remap node-era args)"
```

### Task 6: canvas on the new model (interim rendering)

**Files:**
- Modify: `packages/web/src/components/canvas/Canvas.tsx` (625 lines), `edges.ts` (95 lines), `NodeNode.tsx` (161 lines), `RelEdge.tsx` (126 lines), `layoutSize.ts` (23 lines), `Dock.tsx` (lines 7–19), `packages/web/src/components/LibraryDialog.tsx`, `packages/web/src/components/ImportDialog.tsx` (line 50)
- Test: update `edges.test.ts`, `RelEdge.test.tsx`, `NodeNode.test.tsx`, `layoutSize.test.ts`, `Dock.test.tsx`, `okf/io.test.ts`, `okf/guideExample.test.ts`

**Interfaces:**
- Consumes: Task 4 store/merge, Task 5 templates.
- Produces: `buildRfEdges(edges, nodes, viewMode, relLabelMode)` (same signature), edge `data` now `{ kind, fromEnd, toEnd, bidirectional, modelEdgeId, relLabelMode }`; `isEdgeReconnectable(modelEdgeId, selectedEdgeId)` (viewMode param dropped); `NodeNodeData = ModelNode & { _viewMode?: ViewMode }`; RF node type renamed `"okf"`. Stage 3 replaces the visuals; this task only makes them truthful to the new model.

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

  - Line 151: `const nodeTypes = { okf: NodeNode };` (renamed now so stage 3 doesn't touch Canvas again).
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

- [ ] **Step 3: Interim `NodeNode.tsx` truthfulness pass**

Keep the file/component name (stage 3 replaces it wholesale). Delete `SOURCE_COLOR`, `STATUS_TIP`, `StatusDot`, `FieldAnchors`; drop `_keyFields`:

```ts
export type NodeNodeData = ModelNode & { _viewMode?: ViewMode };
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

`ErdBody`: `const ordered = node.attributes;` (key-first ordering dies with keys); keep the `ERD_COLLAPSED_ROWS` expand toggle. `NodeNodeInner`: `const color = "#94a3b8";`, remove `<StatusDot …/>`, the meta chip shows `node.type`, `fieldCount` reads `node.attributes.length`. Imports shrink (drop `KeyRound`, `SchemaField`).

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

  - `LibraryDialog.tsx`: line 77 `fields={n.schema}` → `fields={n.attributes}`; delete line 87 (`const cond = e.keys…`) and the `<code>` span + separator rendering it (lines 92–93); `NodeRow` prop type (line 106) becomes `fields: { name: string; type: { name: string } }[]`, and inside `NodeRow` print `f.type.name` and drop any `f.pk` icon.
  - `ImportDialog.tsx` line 50: `return graph;` (the status/owoxId scrub is dead). Update the comment above it.

- [ ] **Step 6: Update canvas tests**

Use the Task 4 `node`/`edge` builders in each file.

  - `edges.test.ts`: assert one RF edge per model edge in BOTH `"compact"` and `"erd"` modes, with `data.kind === "associates"` and `data.modelEdgeId`; keep the side-selection tests (they only use positions); delete per-key fan-out tests (ids like `"e1::0"`) and `isEdgeReconnectable` ERD-mode tests (now 2-arg).
  - `RelEdge.test.tsx`: fixture data `{ kind: "associates", fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*" }, bidirectional: false, relLabelMode: "all" }`; assert the label text contains `1` and `*`; assert `relLabelMode: "hidden"` renders no label div; delete "? = ?" and cardinality-badge tests.
  - `NodeNode.test.tsx`: node data via builder + `_viewMode: "erd"`; assert title renders, attribute name + type token render, and no status dot / PK icon markup.
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

Run: `pnpm --filter @mc/web test -- edges RelEdge NodeNode layoutSize Dock io.test guideExample`
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

In `Inspector.tsx`: remove the `joinFieldType` import (line 6); the `RelationshipInspector` call site (lines 132–146) loses `onEnsureField` entirely; `EmptyState` copy line 46 "Changes here are pushed to the matching Data Node." → "Changes apply to your local model."

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


## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
