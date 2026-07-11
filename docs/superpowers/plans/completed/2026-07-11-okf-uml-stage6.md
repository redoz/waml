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


> **Split note:** This is **Stage 6 of 6** of the OKF UML plan. Stages 1..5 already landed on `origin/main` before this run — their code exists in the repo; references to earlier tasks/stages point at code already on main. Implement ONLY the `### Task` sections in THIS file (Tasks 18-19). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context.

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
