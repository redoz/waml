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


> **Split note:** This is **Stage 5 of 6** of the OKF UML plan. Stages 1..4 already landed on `origin/main` before this run — their code exists in the repo; references to earlier tasks/stages point at code already on main. Implement ONLY the `### Task` sections in THIS file (Tasks 15-17). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context.

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


## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
