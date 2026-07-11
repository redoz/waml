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


> **Split note:** This is **Stage 4 of 6** of the OKF UML plan. Stages 1..3 already landed on `origin/main` before this run — their code exists in the repo; references to earlier tasks/stages point at code already on main. Implement ONLY the `### Task` sections in THIS file (Tasks 13-14). The `## Pinned TypeScript shapes` and `## Global Constraints` above are shared context.

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


## Execution notes

- Execute with superpowers:subagent-driven-development (fresh subagent per task, review between tasks) or superpowers:executing-plans. Create an isolated worktree first via superpowers:using-git-worktrees.
- Tasks 4–6 leave the web package transiently red *between* tasks; that is expected — the landable unit is the stage (gate at Tasks 7/10/12/14/17/19). Do not push mid-stage.
- After each okf task, always `pnpm --filter @mc/okf build` before running web tests.
