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


> **Split note:** This is **Stage 3 of 6** of the OKF UML plan. Stages 1..2 already landed on `origin/main` before this run — their code exists in the repo; references to earlier tasks/stages point at code already on main. Implement ONLY the `### Task` sections in THIS file (Tasks 11-12). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context.

# Stage 3 — Metaclass renderer registry + generic fallback, incl. `uml.Association` class box (+ dashed mid-line connector) and `uml.Note` dog-eared box (+ dashed anchors) (spec rollout step 3)

### Task 11: node renderer registry, UML metaclass renderers (incl. `uml.Association`, `uml.Note`), generic box

**Files:**
- Create: `packages/web/src/components/canvas/nodes/shared.tsx`, `nodes/GenericNode.tsx`, `nodes/uml.tsx`, `nodes/registry.ts`, `nodes/OkfNode.tsx`
- Delete: `packages/web/src/components/canvas/NodeNode.tsx`, `NodeNode.test.tsx`
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
      <Handle type="source" position={Position.Left} id="left" style={{ ...common, left: -7 }} className="node-handle" />
      <Handle type="source" position={Position.Right} id="right" style={{ ...common, right: -7 }} className="node-handle" />
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

In `Canvas.tsx`: line 45 `import { NodeNode } from "./NodeNode";` → `import { OkfNode } from "./nodes/OkfNode";`; line 151 `const nodeTypes = { okf: OkfNode };`. Then `git rm packages/web/src/components/canvas/NodeNode.tsx packages/web/src/components/canvas/NodeNode.test.tsx`. In `layoutSize.ts`, count values too: `const total = node.values ? node.values.length : node.attributes.length;` (keep the rest).

- [ ] **Step 5: Run tests and the web suite**

Run: `pnpm --filter @mc/web test -- registry && pnpm --filter @mc/web test`
Expected: PASS (NodeNode tests are gone; nothing else referenced it except Canvas).

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


## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
