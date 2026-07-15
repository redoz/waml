# Diagram Properties Panel v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** `docs/superpowers/plans/2026-07-15-diagram-field-persistence.md` (Spec A —
the diagram-field **persistence** plan). This plan assumes Spec A has already landed on
`main`: `Op::DiagramSet`, `updateDiagramOps`, the real `store.updateDiagram`, the retired
`packages/web/src/state/displaySettings.svelte.ts`, and the full `DiagramDisplay` field set
(`showAttributeVisibility` / `showAttributeMultiplicity` / `maxAttributes` / `stereotypeFilter`
/ `stereotypeColors`) plus `Diagram.description`. **Check that Spec A is merged before
starting.** Every field/type/function name this plan consumes is taken from Spec A's plan,
which is authoritative over Spec B's design doc where they differ.

**Goal:** Turn the diagram properties panel into a full authoring surface for the persisted
`DiagramDisplay` field set (title/note editing, finer attribute controls, a live stereotype
allowlist, and per-stereotype colors) and make the canvas render every new field.

**Architecture:** The panel (`DiagramPropertiesBody.svelte`) grows new props
(`diagram`, `candidateStereotypes`, `editable`, `onUpdateDiagram`) that `CentralEditPanelHost`
threads from `CanvasInner`. All writes flow through the already-persisted paths:
`onChange` → `store.updateDiagram(display)` (Spec A's `handleDisplayChange`) and
`onUpdateDiagram` → `store.updateDiagram(key, {title|description})`. Rendering reads the new
fields off the already-resolved `data._display` in `ClassifierBox`, gating attribute markers,
capping rows, filtering stereotype tags, and applying a per-stereotype color accent + wash. No
new node-data plumbing is introduced.

**Tech Stack:** Svelte 5 runes, `@testing-library/svelte` + Vitest (jsdom), TypeScript pnpm
workspaces (`@waml/okf`, `@waml/core`, `@waml/web`), Tailwind utility classes.

## Global Constraints

- **Package names are `@waml/*`.** Import from `@waml/okf`, `@waml/core/...`. Component files
  live under `packages/web/src/components/canvas/` (the panel and node components) and
  `.../components/central/` (the host) — **not** a `panels/` directory.
- **`data._display` is the fully-resolved `DiagramDisplay`.** `ClassifierBox` already wraps it
  with `resolveDisplay(data._display)`, so every new field is present after resolve
  (`stereotypeColors` defaults to `{}`, `maxAttributes`/`stereotypeFilter` default to `undefined`).
- **`store.updateDiagram(key, patch)` is real (Spec A)** and a no-op for the implicit "All"
  diagram (`ALL_DIAGRAM_KEY` from `@waml/core/state/diagrams`) — the panel disables its controls
  in that case rather than relying on the silent no-op.
- **Every `onChange` patch carries exactly one changed field**, matching the existing panel
  cadence and `handleDisplayChange`'s merge-then-persist path.
- **Panel-only note.** `diagram.description` is never rendered on the canvas — it is authored in
  the panel only.
- **Web test runner:** `pnpm --filter @waml/web test <pattern>` (Vitest, `--passWithNoTests`).
  Typecheck gate: `pnpm --filter @waml/web build` (runs `svelte-check` then `vite build`).

---

## File Structure

**Rendering (`packages/web/src/components/canvas/nodes/`)**
- `AttributeRow.svelte` — add independent `showMultiplicity` prop; `showTypes` reverts to
  type-name only.
- `AttributeRow.test.ts` — **new**; unit-tests the type/multiplicity/visibility columns.
- `ClassifierBox.svelte` — AND-gate `showVisibility`, pass `showMultiplicity`, pass `max`,
  filter stereotype tags, apply per-stereotype color accent + wash.
- `ClassifierBox.test.ts` — extend for each new rendering behavior.
- `RowsCompartment.svelte` — add optional `max?` prop (static "+K more", no expand).
- `RowsCompartment.test.ts` + `RowsCompartmentHarness.svelte` — **new**; a fixture that supplies
  the `render` snippet so the compartment can be rendered from a `.ts` test.

**Panel + wiring (`packages/web/src/components/`)**
- `canvas/diagramProps.ts` — **new**; pure helpers `diagramCandidateStereotypes` +
  `isDiagramEditable` (extracted from `CanvasInner` for testability).
- `canvas/diagramProps.test.ts` — **new**.
- `canvas/DiagramPropertiesBody.svelte` — v2 props + Title/Note, new attribute controls,
  stereotype filter, stereotype colors, disabled/banner for the "All" view.
- `canvas/DiagramPropertiesBody.test.ts` — extend for the new controls.
- `central/CentralEditPanelHost.svelte` — new props threaded to the diagram branch.
- `central/CentralEditPanelHost.test.ts` — updated props helper + diagram-branch coverage.
- `canvas/CanvasInner.svelte` — compute `candidateStereotypes`/`editable`, pass `diagram` +
  `onUpdateDiagram` to the host.

---

## Task 1: `AttributeRow` gains an independent `showMultiplicity` column

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/AttributeRow.svelte`
- Create: `packages/web/src/components/canvas/nodes/AttributeRow.test.ts`

**Interfaces:**
- Produces: `AttributeRow` props `{ a: Attribute; showVisibility?: boolean; showTypes?: boolean; showMultiplicity?: boolean }`.
  `showTypes` now means **only** the type name; `showMultiplicity` (default `true`) gates the
  `{mult}` suffix (rendered only when `a.multiplicity !== "1"`). Consumed by `ClassifierBox` (Task 2/4).

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/canvas/nodes/AttributeRow.test.ts`:

```ts
import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import AttributeRow from "./AttributeRow.svelte";
import type { Attribute } from "@waml/okf";

const attr = (over: Partial<Attribute> = {}): Attribute =>
  ({ name: "id", type: { name: "STRING" }, multiplicity: "0..*", visibility: "+", ...over }) as Attribute;

test("showTypes shows the type name; showMultiplicity shows the {mult} suffix", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: true, showMultiplicity: true } });
  expect(container.textContent).toContain("STRING");
  expect(container.textContent).toContain("{0..*}");
});

test("multiplicity is independent of type name (name-only still shows {mult})", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: false, showMultiplicity: true } });
  expect(container.textContent).not.toContain("STRING");
  expect(container.textContent).toContain("{0..*}");
});

test("showMultiplicity off drops the suffix; showTypes on keeps the type", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: true, showMultiplicity: false } });
  expect(container.textContent).toContain("STRING");
  expect(container.textContent).not.toContain("{0..*}");
});

test("both off renders no trailing type/mult column", () => {
  const { container } = render(AttributeRow, { props: { a: attr(), showTypes: false, showMultiplicity: false } });
  expect(container.querySelector("span.font-mono.text-\\[10\\.5px\\]")).toBeNull();
});

test("multiplicity of exactly '1' is never printed", () => {
  const { container } = render(AttributeRow, {
    props: { a: attr({ multiplicity: "1" }), showTypes: false, showMultiplicity: true },
  });
  expect(container.textContent).not.toContain("{1}");
});

test("visibility marker gated by showVisibility", () => {
  const on = render(AttributeRow, { props: { a: attr(), showVisibility: true } });
  expect(on.container.textContent).toContain("+");
  const off = render(AttributeRow, { props: { a: attr(), showVisibility: false } });
  expect(off.container.querySelector("span.font-mono")?.textContent).not.toContain("+");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test AttributeRow`
Expected: FAIL — `showMultiplicity` is not a prop yet; the type column still bundles `{mult}` into `showTypes`.

- [ ] **Step 3: Implement the split**

Replace `packages/web/src/components/canvas/nodes/AttributeRow.svelte` with:

```svelte
<script lang="ts">
  import type { Attribute } from "@waml/okf";

  // `showTypes` (from the diagram's attributeDetail) toggles ONLY the type name.
  // `showMultiplicity` independently toggles the trailing `{mult}` suffix.
  let { a, showVisibility, showTypes = true, showMultiplicity = true }:
    { a: Attribute; showVisibility?: boolean; showTypes?: boolean; showMultiplicity?: boolean } = $props();
</script>

<div class="relative flex items-center gap-2 px-3 py-[5px] text-[11.5px] border-b border-[#f3f5f8] last:border-b-0">
  {#if showVisibility && a.visibility}<span class="text-slate-400 font-mono">{a.visibility}</span>{/if}
  <span class="flex-1 text-slate-800 truncate" title={a.name}>{a.name}</span>
  {#if showTypes || showMultiplicity}
    <span class="text-slate-400 font-mono text-[10.5px] truncate">
      {#if showTypes}{a.type.name}{/if}{#if showMultiplicity && a.multiplicity !== "1"} {`{${a.multiplicity}}`}{/if}
    </span>
  {/if}
</div>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test AttributeRow`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/AttributeRow.svelte packages/web/src/components/canvas/nodes/AttributeRow.test.ts
git commit -m "feat(web): AttributeRow splits type name from independent multiplicity column"
```

---

## Task 2: `ClassifierBox` AND-gates visibility and passes `showMultiplicity`

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`

**Interfaces:**
- Consumes: `AttributeRow` `showMultiplicity` prop (Task 1); `resolveDisplay`; `stereotypeStyle`.
- Produces: `showVisibility = profileAllows && display.showAttributeVisibility`, forwarded to
  `AttributeRow` along with `showMultiplicity={display.showAttributeMultiplicity}`.

**Note (registered-profile limitation):** the only registered profile, `uml-domain`, has
`hide: ["operations", "visibility"]`, so `profile.hide.includes("visibility")` is `true` and
the visibility floor is always off in `ClassifierBox` tests. This task therefore asserts the
**profile floor** (marker suppressed even with `showAttributeVisibility: true`); the
diagram-flag half of the AND (`showVisibility` prop gates the marker when the profile allows) is
covered at the `AttributeRow` unit in Task 1. Flagged for review.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts` (the file already
mocks `NodePorts` and builds `mkData`/`disp` — reuse them; give the attributes a `visibility`
and a non-trivial multiplicity):

```ts
const mkAttrData = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n", type: "uml.Class", stereotypes: ["entity"],
    attributes: [{ name: "id", type: { name: "STRING" }, multiplicity: "0..*", visibility: "+" }],
    position: { x: 0, y: 0 }, _display: display, _profile: "uml-domain",
  }) as OkfNodeData;

test("uml-domain hides visibility as a floor even when showAttributeVisibility is true", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showAttributeVisibility: true })) },
  });
  // marker "+" must not appear as an attribute-visibility glyph
  expect(container.querySelector(".relative.flex span.font-mono")?.textContent ?? "").not.toContain("+");
});

test("showAttributeMultiplicity drives the {mult} suffix independent of attributeDetail", () => {
  const shown = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, attributeDetail: "name-only", showAttributeMultiplicity: true })) },
  });
  expect(shown.container.textContent).toContain("{0..*}");
  const hidden = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, attributeDetail: "name-type", showAttributeMultiplicity: false })) },
  });
  expect(hidden.container.textContent).toContain("STRING");
  expect(hidden.container.textContent).not.toContain("{0..*}");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: FAIL — `showAttributeMultiplicity` is not forwarded, so the `{mult}` suffix ignores the flag.

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`, change the visibility derive:

```ts
  let showVisibility = $derived(!profile.hide.includes("visibility") && display.showAttributeVisibility);
```

and forward `showMultiplicity` to the attribute compartment's `AttributeRow` (the `!data.values`
block, currently around L57-63):

```svelte
    {#if isDetailed && !data.values}
      <RowsCompartment rows={data.attributes.length}>
        {#snippet render(i: number)}
          <AttributeRow a={data.attributes[i]} {showVisibility} {showTypes} showMultiplicity={display.showAttributeMultiplicity} />
        {/snippet}
      </RowsCompartment>
    {/if}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte packages/web/src/components/canvas/nodes/ClassifierBox.test.ts
git commit -m "feat(web): ClassifierBox AND-gates visibility and forwards attribute multiplicity"
```

---

## Task 3: `RowsCompartment` gains a static `max` cap

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/RowsCompartment.svelte`
- Create: `packages/web/src/components/canvas/nodes/RowsCompartmentHarness.svelte`
- Create: `packages/web/src/components/canvas/nodes/RowsCompartment.test.ts`

**Interfaces:**
- Produces: `RowsCompartment` props `{ rows: number; max?: number; render: Snippet<[number]> }`.
  When `max` is set: show at most `max` rows and a **static, non-interactive** "+K more" footer
  (`K = rows - max`); no expand button. When `max` is `undefined`: today's interactive
  `ERD_COLLAPSED_ROWS` expand/collapse is unchanged. Consumed by `ClassifierBox` (Task 4).

- [ ] **Step 1: Write the harness fixture**

Create `packages/web/src/components/canvas/nodes/RowsCompartmentHarness.svelte` (supplies a
concrete `render` snippet so the compartment can be exercised from a `.ts` test):

```svelte
<script lang="ts">
  import RowsCompartment from "./RowsCompartment.svelte";
  let { rows, max }: { rows: number; max?: number } = $props();
</script>

<RowsCompartment {rows} {max}>
  {#snippet render(i: number)}
    <div data-row={i}>row {i}</div>
  {/snippet}
</RowsCompartment>
```

- [ ] **Step 2: Write the failing test**

Create `packages/web/src/components/canvas/nodes/RowsCompartment.test.ts`:

```ts
import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import Harness from "./RowsCompartmentHarness.svelte";

test("max caps visible rows and shows a static '+K more' with no button", () => {
  const { container } = render(Harness, { props: { rows: 8, max: 3 } });
  expect(container.querySelectorAll("[data-row]")).toHaveLength(3);
  expect(container.textContent).toContain("+5 more");
  expect(container.querySelector("button")).toBeNull();
});

test("max larger than the row count shows all rows and no footer", () => {
  const { container } = render(Harness, { props: { rows: 2, max: 10 } });
  expect(container.querySelectorAll("[data-row]")).toHaveLength(2);
  expect(container.textContent).not.toContain("more");
});

test("without max, the interactive expand button is still present when rows overflow", () => {
  const { container } = render(Harness, { props: { rows: 20 } });
  expect(container.querySelector("button")).not.toBeNull();
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test RowsCompartment`
Expected: FAIL — `max` is not a prop; the static footer does not exist.

- [ ] **Step 4: Implement**

Replace `packages/web/src/components/canvas/nodes/RowsCompartment.svelte` with:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import { ChevronDown, ChevronRight } from "lucide-svelte";
  import { ERD_COLLAPSED_ROWS } from "@waml/core/canvas/layoutSize";

  // `max` (a diagram authoring cap) overrides the interactive collapse with a
  // static "+K more" footer. Absent ⇒ today's per-box expand/collapse toggle.
  let { rows, max, render }: { rows: number; max?: number; render: Snippet<[number]> } = $props();

  let expanded = $state(false);
</script>

{#if rows > 0}
  {#if max !== undefined}
    {@const visible = Math.min(rows, max)}
    {@const hiddenK = rows - visible}
    <div class="border-t border-[#eef1f5]">
      {#each Array.from({ length: visible }, (_, i) => i) as i (i)}
        {@render render(i)}
      {/each}
      {#if hiddenK > 0}
        <div class="w-full flex items-center justify-center px-3 py-[5px] text-[11px] font-medium text-slate-400 border-t border-[#f3f5f8]">
          +{hiddenK} more
        </div>
      {/if}
    </div>
  {:else}
    {@const visible = expanded ? rows : Math.min(rows, ERD_COLLAPSED_ROWS)}
    {@const hidden = rows - ERD_COLLAPSED_ROWS}
    <div class="border-t border-[#eef1f5]">
      {#each Array.from({ length: visible }, (_, i) => i) as i (i)}
        {@render render(i)}
      {/each}
      {#if hidden > 0}
        <button
          onclick={(e) => { e.stopPropagation(); expanded = !expanded; }}
          class="w-full flex items-center justify-center gap-1 px-3 py-[5px] text-[11px] font-medium text-[#1e88e5] hover:bg-[#f1f5fb] border-t border-[#f3f5f8]"
        >
          {#if expanded}
            <ChevronDown size={12} /> Show less
          {:else}
            <ChevronRight size={12} /> +{hidden} more
          {/if}
        </button>
      {/if}
    </div>
  {/if}
{/if}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test RowsCompartment`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/nodes/RowsCompartment.svelte packages/web/src/components/canvas/nodes/RowsCompartmentHarness.svelte packages/web/src/components/canvas/nodes/RowsCompartment.test.ts
git commit -m "feat(web): RowsCompartment supports a static max cap"
```

---

## Task 4: `ClassifierBox` applies the `maxAttributes` cap

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`

**Interfaces:**
- Consumes: `RowsCompartment` `max` prop (Task 3).
- Produces: `max={display.maxAttributes}` passed **only** to the attribute compartment, never the
  enum-values compartment.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts` (build a node with
several attributes):

```ts
const mkManyAttrs = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n", type: "uml.Class", stereotypes: ["entity"],
    attributes: [
      { name: "a1", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a2", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a3", type: { name: "STRING" }, multiplicity: "1" },
      { name: "a4", type: { name: "STRING" }, multiplicity: "1" },
    ],
    position: { x: 0, y: 0 }, _display: display, _profile: "uml-domain",
  }) as OkfNodeData;

test("maxAttributes caps attribute rows with a static '+K more' and no expand button", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkManyAttrs(disp({ showAttributes: true, maxAttributes: 2 })) },
  });
  expect(container.textContent).toContain("a1");
  expect(container.textContent).toContain("a2");
  expect(container.textContent).not.toContain("a3");
  expect(container.textContent).toContain("+2 more");
  expect(container.querySelector("button")).toBeNull();
});

test("maxAttributes undefined leaves the interactive compartment behavior intact", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkManyAttrs(disp({ showAttributes: true, maxAttributes: undefined })) },
  });
  expect(container.textContent).toContain("a4");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: FAIL — `max` is not passed to the attribute compartment; rows are not capped.

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`, add `max={display.maxAttributes}`
to the attribute `RowsCompartment` (the `!data.values` block) only:

```svelte
    {#if isDetailed && !data.values}
      <RowsCompartment rows={data.attributes.length} max={display.maxAttributes}>
        {#snippet render(i: number)}
          <AttributeRow a={data.attributes[i]} {showVisibility} {showTypes} showMultiplicity={display.showAttributeMultiplicity} />
        {/snippet}
      </RowsCompartment>
    {/if}
```

Leave the enum-values `RowsCompartment` (the `data.values` block) unchanged (no `max`).

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte packages/web/src/components/canvas/nodes/ClassifierBox.test.ts
git commit -m "feat(web): ClassifierBox caps attribute rows via maxAttributes"
```

---

## Task 5: `ClassifierBox` filters stereotype tags

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`

**Interfaces:**
- Produces: `stereotypeTags` = `data.stereotypes` when `stereotypeFilter === undefined`, else
  `data.stereotypes.filter((s) => stereotypeFilter.includes(s))`, passed to `StereotypeRow`.
  `StereotypeRow` is unchanged (it shows the row when `keyword || stereotypes.length > 0`).

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts` (build a node with two
stereotypes and render with `keyword`):

```ts
const mkTags = (display: DiagramDisplay): OkfNodeData =>
  ({
    concept: { id: "n", type: "uml.Class", title: "Order", body: "" },
    key: "n", type: "uml.Class", stereotypes: ["entity", "valueObject"],
    attributes: [], position: { x: 0, y: 0 }, _display: display, _profile: "uml-domain",
  }) as OkfNodeData;

test("undefined filter shows every stereotype tag", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: undefined })) } });
  expect(container.textContent).toContain("«entity»");
  expect(container.textContent).toContain("«valueObject»");
});

test("an allowlist shows only listed tags", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: ["entity"] })) } });
  expect(container.textContent).toContain("«entity»");
  expect(container.textContent).not.toContain("«valueObject»");
});

test("empty allowlist shows no tags but keeps the keyword row", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: true, stereotypeFilter: [] })), keyword: "Class" } });
  expect(container.textContent).toContain("«Class»");
  expect(container.textContent).not.toContain("«entity»");
});

test("showStereotype false renders neither keyword nor tags", () => {
  const { container } = render(ClassifierBox, { props: { data: mkTags(disp({ showStereotype: false })), keyword: "Class" } });
  expect(container.textContent).not.toContain("«Class»");
  expect(container.textContent).not.toContain("«entity»");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: FAIL — the allowlist case still shows `«valueObject»` (no filter applied).

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`, add the derived tag list (near
the other `$derived` values):

```ts
  let stereotypeTags = $derived(
    display.stereotypeFilter === undefined
      ? data.stereotypes
      : data.stereotypes.filter((s) => display.stereotypeFilter!.includes(s)),
  );
```

and pass it to `StereotypeRow` (the `{#if showStereotype}` block, currently around L42-44):

```svelte
    {#if showStereotype}
      <StereotypeRow stereotypes={stereotypeTags} {keyword} />
    {/if}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte packages/web/src/components/canvas/nodes/ClassifierBox.test.ts
git commit -m "feat(web): ClassifierBox filters stereotype tags via stereotypeFilter"
```

---

## Task 6: `ClassifierBox` per-stereotype color accent + background wash

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`

**Interfaces:**
- Produces: `overrideHeader` = last stereotype (over `data.stereotypes`) with an entry in
  `display.stereotypeColors` (later-wins, mirroring `stereotypeStyle`); `headerColor =
  overrideHeader ?? st.header` drives the 4px top accent and the thick-border fallback; when
  `overrideHeader` is set the box gets `background-color: color-mix(in srgb, {overrideHeader} 12%, white)`.

**Note (jsdom + `color-mix`):** assert the accent (`border-top-color`) and the wash by reading the
raw `style` attribute string on the box root (`container.firstElementChild`), not via CSSOM —
jsdom does not parse `color-mix()` and would drop it from `element.style`. Flagged for review.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`:

```ts
const style = (container: HTMLElement) => (container.firstElementChild as HTMLElement).getAttribute("style") ?? "";

test("a stereotype color overrides the header accent and adds a wash", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: { entity: "#ff0000" } })) },
  });
  const s = style(container);
  expect(s).toContain("border-top-color:#ff0000");
  expect(s).toContain("color-mix(in srgb, #ff0000 12%, white)");
});

test("override color follows later-wins precedence across stereotypes", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: { entity: "#111111", valueObject: "#222222" } })) },
  });
  // data.stereotypes = ["entity", "valueObject"] ⇒ valueObject wins
  expect(style(container)).toContain("border-top-color:#222222");
});

test("no override keeps the plain white background (no wash)", () => {
  const { container } = render(ClassifierBox, {
    props: { data: mkTags(disp({ showStereotype: true, stereotypeColors: {} })) },
  });
  expect(style(container)).not.toContain("color-mix");
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: FAIL — no `overrideHeader`/wash logic; `boxStyle` still keys off `st.header` only.

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`, add the override derives (near
the other `$derived` values):

```ts
  let overrideHeader = $derived(
    data.stereotypes.reduce<string | undefined>((acc, s) => display.stereotypeColors[s] ?? acc, undefined),
  );
  let headerColor = $derived(overrideHeader ?? st.header);
```

and rewrite `boxStyle` to key off `headerColor` and add the override-only wash:

```ts
  let boxStyle = $derived.by(() => {
    const decls: string[] = [`font-family:${NODE_FONT}`];
    if (headerColor) decls.push(`border-top-color:${headerColor}`, `border-top-width:4px`);
    if (st.border === "thick") decls.push(`border-color:${headerColor ?? "#334155"}`, `border-width:2.5px`);
    if (overrideHeader) decls.push(`background-color:color-mix(in srgb, ${overrideHeader} 12%, white)`);
    if (st.shape === "hexagon") {
      decls.push(`clip-path:polygon(8% 0, 92% 0, 100% 50%, 92% 100%, 8% 100%, 0 50%)`, `border-radius:0`);
    }
    return decls.join(";");
  });
```

The base `bg-white` utility class stays as the no-override fallback.

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test ClassifierBox`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte packages/web/src/components/canvas/nodes/ClassifierBox.test.ts
git commit -m "feat(web): ClassifierBox applies per-stereotype color accent and wash"
```

---

## Task 7: Pure helpers for candidate stereotypes + editability

**Files:**
- Create: `packages/web/src/components/canvas/diagramProps.ts`
- Create: `packages/web/src/components/canvas/diagramProps.test.ts`

**Interfaces:**
- Produces: `diagramCandidateStereotypes(nodes: ModelNode[], members: string[]): string[]` (unique,
  sorted, only members' stereotypes) and `isDiagramEditable(diagramKey: string): boolean`
  (`false` only for `ALL_DIAGRAM_KEY`). Consumed by `CanvasInner` (Task 10).

**Judgment call (noted for review):** Spec B inlines both derivations directly in `CanvasInner`.
This plan extracts them into a pure module so they can be unit-tested (the spec's testing section
explicitly asks for `candidateStereotypes`/`editable` coverage, and `CanvasInner` is an
orchestrator with no component test). Behavior matches the spec.

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/components/canvas/diagramProps.test.ts`:

```ts
import { test, expect } from "vitest";
import { diagramCandidateStereotypes, isDiagramEditable } from "./diagramProps";
import { ALL_DIAGRAM_KEY } from "@waml/core/state/diagrams";
import type { ModelNode } from "@waml/okf";

const node = (key: string, stereotypes: string[]): ModelNode =>
  ({ key, type: "uml.Class", concept: { id: key, type: "uml.Class", title: key, description: "" }, stereotypes, attributes: [], position: { x: 0, y: 0 } }) as unknown as ModelNode;

test("candidate stereotypes are unique, sorted, and scoped to members", () => {
  const nodes = [node("a", ["entity", "root"]), node("b", ["entity"]), node("c", ["service"])];
  expect(diagramCandidateStereotypes(nodes, ["a", "b"])).toEqual(["entity", "root"]);
});

test("no members ⇒ empty candidate list", () => {
  expect(diagramCandidateStereotypes([node("a", ["entity"])], [])).toEqual([]);
});

test("editable is false only for the implicit All diagram", () => {
  expect(isDiagramEditable(ALL_DIAGRAM_KEY)).toBe(false);
  expect(isDiagramEditable("orders")).toBe(true);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test diagramProps`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

Create `packages/web/src/components/canvas/diagramProps.ts`:

```ts
import type { ModelNode } from "@waml/okf";
import { ALL_DIAGRAM_KEY } from "@waml/core/state/diagrams";

/** Unique, sorted stereotype names present on the given diagram's member nodes. */
export function diagramCandidateStereotypes(nodes: ModelNode[], members: string[]): string[] {
  const memberSet = new Set(members);
  const names = new Set<string>();
  for (const n of nodes) {
    if (!memberSet.has(n.key)) continue;
    for (const s of n.stereotypes) names.add(s);
  }
  return [...names].sort();
}

/** The implicit "All" diagram has no backing document and cannot persist settings. */
export function isDiagramEditable(diagramKey: string): boolean {
  return diagramKey !== ALL_DIAGRAM_KEY;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test diagramProps`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/diagramProps.ts packages/web/src/components/canvas/diagramProps.test.ts
git commit -m "feat(web): pure helpers for diagram candidate stereotypes + editability"
```

---

## Task 8: Thread the new panel inputs end-to-end (host + CanvasInner + panel signature)

**Files:**
- Modify: `packages/web/src/components/central/CentralEditPanelHost.svelte`
- Modify: `packages/web/src/components/central/CentralEditPanelHost.test.ts`
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` (props only — no new UI yet)
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts` (props helper only)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`

**Interfaces:**
- Consumes: `diagramCandidateStereotypes`, `isDiagramEditable` (Task 7); `store.updateDiagram` (Spec A).
- Produces: `CentralEditPanelHost` props gain `diagram: Diagram`, `candidateStereotypes: string[]`,
  `editable: boolean`, `onUpdateDiagram: (patch: Partial<Diagram>) => void`, passed to
  `DiagramPropertiesBody` in the diagram branch. `DiagramPropertiesBody` accepts the v2 signature
  (body markup unchanged this task).

**Note:** This task keeps the build green by threading the props before any body markup uses them.
The panel body still renders only the existing five controls; Tasks 9-10 add the new UI.

- [ ] **Step 1: Expand the panel signature (no new UI)**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, replace the imports/props
block with the v2 signature (leave the rest of the file — the `toggleRow`/`segmented` snippets and
the existing five controls — unchanged for now):

```svelte
<script lang="ts">
  import type { DiagramDisplay, Diagram } from "@waml/okf";

  let { display, diagram, candidateStereotypes, editable, onChange, onUpdateDiagram }: {
    display: DiagramDisplay;
    diagram: Diagram;
    candidateStereotypes: string[];
    editable: boolean;
    onChange: (patch: Partial<DiagramDisplay>) => void;
    onUpdateDiagram: (patch: Partial<Diagram>) => void;
  } = $props();

  function patch(p: Partial<DiagramDisplay>) {
    onChange(p);
  }
</script>
```

- [ ] **Step 2: Thread props through the host**

In `packages/web/src/components/central/CentralEditPanelHost.svelte`, add the imports/props and pass
them to the diagram branch:

```svelte
  import type { DiagramDisplay, ModelNode, Diagram } from "@waml/okf";
```

```ts
  let { state, nodes, display, diagram, candidateStereotypes, editable, profileName, onUpdateNode, onDisplayChange, onUpdateDiagram, onClose }: {
    state: CentralPanelState | null;
    nodes: ModelNode[];
    display: DiagramDisplay;
    diagram: Diagram;
    candidateStereotypes: string[];
    editable: boolean;
    profileName?: string;
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onDisplayChange: (patch: Partial<DiagramDisplay>) => void;
    onUpdateDiagram: (patch: Partial<Diagram>) => void;
    onClose: () => void;
  } = $props();
```

```svelte
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" {onClose}>
    <DiagramPropertiesBody
      {display} {diagram} {candidateStereotypes} {editable}
      onChange={onDisplayChange}
      {onUpdateDiagram}
    />
  </CentralEditPanel>
{/if}
```

- [ ] **Step 3: Thread props from `CanvasInner`**

In `packages/web/src/components/canvas/CanvasInner.svelte`:

Add the helper import (near the other canvas imports):

```ts
  import { diagramCandidateStereotypes, isDiagramEditable } from "./diagramProps";
```

Add the derived values (near `activeDisplay` / `memberSet`, ~L151-154):

```ts
  const candidateStereotypes = $derived(diagramCandidateStereotypes($model.nodes, activeDiagram.members));
  const diagramEditable = $derived(isDiagramEditable(activeDiagram.key));
```

Extend the `<CentralEditPanelHost>` element (~L529-537) to pass the new props:

```svelte
  <CentralEditPanelHost
    state={centralPanel}
    nodes={$model.nodes}
    display={activeDisplay}
    diagram={activeDiagram}
    candidateStereotypes={candidateStereotypes}
    editable={diagramEditable}
    profileName={activeDiagram.profile}
    onUpdateNode={store.updateNode}
    onDisplayChange={handleDisplayChange}
    onUpdateDiagram={(patch) => store.updateDiagram(activeDiagram.key, patch)}
    onClose={() => (centralPanel = null)}
  />
```

- [ ] **Step 4: Update the test prop helpers**

In `packages/web/src/components/central/CentralEditPanelHost.test.ts`, extend the `props()` helper
so the host has the new required props (import `type Diagram`):

```ts
import { DEFAULT_DISPLAY, type ModelNode, type Diagram } from "@waml/okf";

const diagram: Diagram = { key: "orders", title: "Orders", profile: "uml-domain", members: [] };

const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer")],
  display: { ...DEFAULT_DISPLAY },
  diagram,
  candidateStereotypes: [] as string[],
  editable: true,
  profileName: "uml-domain",
  onUpdateNode: vi.fn(),
  onDisplayChange: vi.fn(),
  onUpdateDiagram: vi.fn(),
  onClose: vi.fn(),
  ...over,
});
```

In `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`, extend the `props()` helper
the same way (import `type Diagram`) so existing tests keep compiling:

```ts
import { DEFAULT_DISPLAY, type Diagram } from "@waml/okf";

const diagram: Diagram = { key: "orders", title: "Orders", profile: "uml-domain", members: [] };

const props = (over = {}) => ({
  display: { ...DEFAULT_DISPLAY },
  diagram,
  candidateStereotypes: [] as string[],
  editable: true,
  onChange: vi.fn(),
  onUpdateDiagram: vi.fn(),
  ...over,
});
```

Also, if the pre-existing `"a non-default display value drives every control's rendered state"`
test constructs a bare `display` literal, change it to spread `DEFAULT_DISPLAY`
(`{ ...DEFAULT_DISPLAY, ...overrides }`) so it satisfies the post-Spec-A required `DiagramDisplay`
fields.

- [ ] **Step 5: Run the affected suites + typecheck**

Run: `pnpm --filter @waml/web test CentralEditPanelHost DiagramPropertiesBody diagramProps`
Expected: PASS (existing panel/host tests still green with the widened props).

Run: `pnpm --filter @waml/web build`
Expected: PASS — `svelte-check` finds no missing-prop or unused-import errors (host + CanvasInner
supply every required prop).

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/central/CentralEditPanelHost.svelte packages/web/src/components/central/CentralEditPanelHost.test.ts packages/web/src/components/canvas/DiagramPropertiesBody.svelte packages/web/src/components/canvas/DiagramPropertiesBody.test.ts packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): thread diagram/candidateStereotypes/editable/onUpdateDiagram to the panel"
```

---

## Task 9: Panel v2 — Title/Note + new attribute controls + "All"-view disabling

**Files:**
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`

**Interfaces:**
- Consumes: the v2 props from Task 8 (`diagram`, `editable`, `onUpdateDiagram`).
- Produces: a Title text input + Note textarea (commit-on-blur/Enter), a Show-visibility toggle,
  a Show-multiplicity toggle, a Max-attributes control (number + "Unlimited"), and — when
  `editable` is `false` — a banner plus every control disabled.

- [ ] **Step 1: Write the failing tests**

Append to `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`:

```ts
test("editing the title commits on blur via onUpdateDiagram", async () => {
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ onUpdateDiagram }) });
  const input = screen.getByLabelText("Diagram title") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "Order lifecycle" } });
  await fireEvent.blur(input);
  expect(onUpdateDiagram).toHaveBeenCalledWith({ title: "Order lifecycle" });
});

test("editing the note commits on blur via onUpdateDiagram", async () => {
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ onUpdateDiagram }) });
  const note = screen.getByLabelText("Diagram note") as HTMLTextAreaElement;
  await fireEvent.input(note, { target: { value: "Notes for reviewers" } });
  await fireEvent.blur(note);
  expect(onUpdateDiagram).toHaveBeenCalledWith({ description: "Notes for reviewers" });
});

test("Show visibility toggle emits showAttributeVisibility", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeVisibility: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Show visibility" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeVisibility: false });
});

test("Show multiplicity toggle emits showAttributeMultiplicity", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeMultiplicity: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Show multiplicity" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeMultiplicity: false });
});

test("Max attributes: typing a number emits it; Unlimited emits undefined", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }) });
  await fireEvent.input(screen.getByLabelText("Max attributes"), { target: { value: "6" } });
  expect(onChange).toHaveBeenCalledWith({ maxAttributes: 6 });
  await fireEvent.click(screen.getByRole("button", { name: "Unlimited attributes" }));
  expect(onChange).toHaveBeenCalledWith({ maxAttributes: undefined });
});

test("editable false shows the banner and disables every control", async () => {
  const onChange = vi.fn();
  const onUpdateDiagram = vi.fn();
  render(DiagramPropertiesBody, { props: props({ editable: false, onChange, onUpdateDiagram }) });
  expect(screen.getByRole("note")).toBeTruthy();
  const showAttrs = screen.getByRole("switch", { name: "Show attributes" }) as HTMLButtonElement;
  expect(showAttrs.disabled).toBe(true);
  await fireEvent.click(showAttrs);
  expect(onChange).not.toHaveBeenCalled();
  const title = screen.getByLabelText("Diagram title") as HTMLInputElement;
  expect(title.disabled).toBe(true);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web test DiagramPropertiesBody`
Expected: FAIL — the Title/Note fields, Show-visibility/Show-multiplicity toggles, Max-attributes
control, and the banner do not exist yet.

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`:

Add a `disabledAll` derive and the commit helpers to the `<script>` block:

```ts
  let disabledAll = $derived(!editable);

  function commitTitle(v: string) {
    const t = v.trim();
    if (t && t !== diagram.title) onUpdateDiagram({ title: t });
  }
  function commitNote(v: string) {
    if (v !== (diagram.description ?? "")) onUpdateDiagram({ description: v });
  }
```

Render the banner + Title/Note section at the top of the markup (before the existing controls),
inside the outer `<div>`:

```svelte
  {#if !editable}
    <div role="note" class="mx-1 mb-2 rounded-lg bg-[#fff7ed] px-3 py-2 text-[12px] leading-snug text-[#9a3412]">
      Display and note settings save to a diagram. The <strong>All</strong> view can't store them — create a diagram to customize.
    </div>
  {/if}

  <div class="px-2 py-1.5">
    <label class="block">
      <span class="mb-1 block text-[13px] font-medium text-slate-800">Title</span>
      <input
        type="text" aria-label="Diagram title" value={diagram.title} disabled={disabledAll}
        onblur={(e) => commitTitle((e.currentTarget as HTMLInputElement).value)}
        onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); (e.currentTarget as HTMLInputElement).blur(); } }}
        class="w-full rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40" />
    </label>
    <label class="mt-2 block">
      <span class="mb-1 block text-[13px] font-medium text-slate-800">Note</span>
      <textarea
        aria-label="Diagram note" rows="3" disabled={disabledAll}
        placeholder="Notes about this diagram (not shown on the canvas)."
        value={diagram.description ?? ""}
        onblur={(e) => commitNote((e.currentTarget as HTMLTextAreaElement).value)}
        class="w-full resize-y rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"></textarea>
    </label>
  </div>

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>
```

Thread `disabledAll` into the existing controls (each `disabled` argument OR-combined with
`disabledAll`), and add the two new toggles + the Max-attributes control after the
Attribute-detail control:

```svelte
  {@render toggleRow("Show attributes", display.showAttributes, () =>
    patch({ showAttributes: !display.showAttributes }), disabledAll,
  )}
  {@render segmented(
    "Attribute detail",
    [ { value: "name-only", label: "Name only" }, { value: "name-type", label: "Name + type" } ],
    display.attributeDetail,
    (v) => patch({ attributeDetail: v as DiagramDisplay["attributeDetail"] }),
    !display.showAttributes || disabledAll,
  )}
  {@render toggleRow("Show visibility", display.showAttributeVisibility, () =>
    patch({ showAttributeVisibility: !display.showAttributeVisibility }), !display.showAttributes || disabledAll,
  )}
  {@render toggleRow("Show multiplicity", display.showAttributeMultiplicity, () =>
    patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), !display.showAttributes || disabledAll,
  )}
  <div class="px-2 py-1.5 {(!display.showAttributes || disabledAll) ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">Max attributes</div>
    <div class="flex items-center gap-2">
      <input
        type="number" min="1" aria-label="Max attributes" placeholder="∞"
        value={display.maxAttributes ?? ""} disabled={!display.showAttributes || disabledAll}
        oninput={(e) => {
          const n = Number((e.currentTarget as HTMLInputElement).value);
          if (Number.isFinite(n) && n >= 1) patch({ maxAttributes: Math.floor(n) });
        }}
        class="w-16 rounded-md border border-slate-300 px-2 py-1 text-[12px]" />
      <button
        type="button" aria-label="Unlimited attributes" disabled={!display.showAttributes || disabledAll}
        onclick={() => { if (display.showAttributes && !disabledAll) patch({ maxAttributes: undefined }); }}
        class="rounded-md px-2 py-1 text-[12px] font-semibold {display.maxAttributes === undefined ? 'bg-white text-[#1e88e5] shadow-sm' : 'text-slate-500'}">
        Unlimited
      </button>
    </div>
  </div>
```

Thread `disabledAll` into the remaining existing controls as well (`Associations` segmented,
`Emphasize multiplicity` toggle, `Show stereotype` toggle) by OR-combining their `disabled`
argument with `disabledAll`:

```svelte
  {@render segmented(
    "Associations",
    [ { value: "all", label: "Show labels" }, { value: "hidden", label: "Hide labels" } ],
    display.associationLabels,
    (v) => patch({ associationLabels: v as DiagramDisplay["associationLabels"] }),
    disabledAll,
  )}
  {@render toggleRow("Emphasize multiplicity", display.emphasizeMultiplicity, () =>
    patch({ emphasizeMultiplicity: !display.emphasizeMultiplicity }), disabledAll,
  )}
  {@render toggleRow("Show stereotype", display.showStereotype, () =>
    patch({ showStereotype: !display.showStereotype }), disabledAll,
  )}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web test DiagramPropertiesBody`
Expected: PASS (existing + 6 new tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/DiagramPropertiesBody.svelte packages/web/src/components/canvas/DiagramPropertiesBody.test.ts
git commit -m "feat(web): panel v2 adds title/note + visibility/multiplicity/max-attributes controls"
```

---

## Task 10: Panel v2 — stereotype allowlist filter + per-stereotype colors

**Files:**
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`

**Interfaces:**
- Consumes: `candidateStereotypes`, `display.stereotypeFilter`, `display.stereotypeColors`.
- Produces: a "Show all" control + one checkbox per candidate (empty allowlist ⇒ show none;
  "Show all" ⇒ `undefined`), a per-candidate color picker + clear, an empty-list hint, and both
  sections disabled when `showStereotype` is off or `editable` is false.

- [ ] **Step 1: Write the failing tests**

Append to `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`:

```ts
const withStereos = (over = {}) =>
  props({ display: { ...DEFAULT_DISPLAY, showStereotype: true }, candidateStereotypes: ["entity", "valueObject"], ...over });

test("empty candidate list shows the muted hint", () => {
  render(DiagramPropertiesBody, { props: props({ candidateStereotypes: [] }) });
  expect(screen.getAllByText(/No stereotypes on this diagram's members yet/i).length).toBeGreaterThan(0);
});

test("Show all emits stereotypeFilter undefined", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: withStereos({ display: { ...DEFAULT_DISPLAY, showStereotype: true, stereotypeFilter: ["entity"] }, onChange }) });
  await fireEvent.click(screen.getByRole("checkbox", { name: "Show all stereotypes" }));
  expect(onChange).toHaveBeenCalledWith({ stereotypeFilter: undefined });
});

test("toggling a name off from an allowlist can empty it to [] (show none)", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: withStereos({ display: { ...DEFAULT_DISPLAY, showStereotype: true, stereotypeFilter: ["entity"] }, onChange }) });
  await fireEvent.click(screen.getByRole("checkbox", { name: "entity" }));
  expect(onChange).toHaveBeenCalledWith({ stereotypeFilter: [] });
});

test("toggling a name from show-all builds an allowlist", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: withStereos({ onChange }) }); // stereotypeFilter undefined ⇒ show all
  await fireEvent.click(screen.getByRole("checkbox", { name: "valueObject" }));
  expect(onChange).toHaveBeenCalledWith({ stereotypeFilter: ["entity"] });
});

test("picking a color merges into stereotypeColors; clearing removes the key", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: withStereos({ display: { ...DEFAULT_DISPLAY, showStereotype: true, stereotypeColors: { entity: "#ffedd5" } }, onChange }) });
  await fireEvent.input(screen.getByLabelText("Color for valueObject"), { target: { value: "#dbeafe" } });
  expect(onChange).toHaveBeenCalledWith({ stereotypeColors: { entity: "#ffedd5", valueObject: "#dbeafe" } });
  await fireEvent.click(screen.getByRole("button", { name: "Clear color for entity" }));
  expect(onChange).toHaveBeenCalledWith({ stereotypeColors: {} });
});

test("stereotype controls disabled when showStereotype is off", () => {
  render(DiagramPropertiesBody, { props: withStereos({ display: { ...DEFAULT_DISPLAY, showStereotype: false }, candidateStereotypes: ["entity"] }) });
  expect((screen.getByRole("checkbox", { name: "entity" }) as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("Color for entity") as HTMLInputElement).disabled).toBe(true);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @waml/web test DiagramPropertiesBody`
Expected: FAIL — the filter and color sections do not exist yet.

- [ ] **Step 3: Implement**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, add the filter/color helpers
to the `<script>` block:

```ts
  let stereoDisabled = $derived(!display.showStereotype || disabledAll);

  function toggleFilter(name: string) {
    if (stereoDisabled) return;
    const current = display.stereotypeFilter === undefined ? [...candidateStereotypes] : [...display.stereotypeFilter];
    const idx = current.indexOf(name);
    if (idx >= 0) current.splice(idx, 1);
    else current.push(name);
    patch({ stereotypeFilter: current });
  }
  function setColor(name: string, hex: string) {
    if (stereoDisabled) return;
    patch({ stereotypeColors: { ...display.stereotypeColors, [name]: hex } });
  }
  function clearColor(name: string) {
    if (stereoDisabled) return;
    const next = { ...display.stereotypeColors };
    delete next[name];
    patch({ stereotypeColors: next });
  }
```

Render the two sections after the existing `Show stereotype` toggle:

```svelte
  <div class="px-2 py-1.5 {stereoDisabled ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">Stereotype filter</div>
    {#if candidateStereotypes.length === 0}
      <div class="text-[12px] text-slate-400">No stereotypes on this diagram's members yet.</div>
    {:else}
      <label class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
        <input
          type="checkbox" aria-label="Show all stereotypes"
          checked={display.stereotypeFilter === undefined} disabled={stereoDisabled}
          onchange={() => { if (!stereoDisabled) patch({ stereotypeFilter: undefined }); }} />
        <span>Show all</span>
      </label>
      {#each candidateStereotypes as name (name)}
        {@const checked = display.stereotypeFilter === undefined ? true : display.stereotypeFilter.includes(name)}
        <label class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
          <input type="checkbox" aria-label={name} checked={checked} disabled={stereoDisabled} onchange={() => toggleFilter(name)} />
          <span>{name}</span>
        </label>
      {/each}
    {/if}
  </div>

  <div class="px-2 py-1.5 {stereoDisabled ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">Stereotype colors</div>
    {#if candidateStereotypes.length === 0}
      <div class="text-[12px] text-slate-400">No stereotypes on this diagram's members yet.</div>
    {:else}
      {#each candidateStereotypes as name (name)}
        <div class="flex items-center gap-2 py-0.5 text-[12px] text-slate-700">
          <input
            type="color" aria-label={`Color for ${name}`} disabled={stereoDisabled}
            value={display.stereotypeColors[name] ?? "#dddddd"}
            oninput={(e) => setColor(name, (e.currentTarget as HTMLInputElement).value)}
            class="h-6 w-8 rounded border border-slate-300" />
          <span class="flex-1">{name}</span>
          {#if display.stereotypeColors[name]}
            <button
              type="button" aria-label={`Clear color for ${name}`} disabled={stereoDisabled}
              onclick={() => clearColor(name)}
              class="text-slate-400 hover:text-slate-700">Clear</button>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @waml/web test DiagramPropertiesBody`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/DiagramPropertiesBody.svelte packages/web/src/components/canvas/DiagramPropertiesBody.test.ts
git commit -m "feat(web): panel v2 adds stereotype allowlist filter and per-stereotype colors"
```

---

## Task 11: Full workspace gate + smoke verification

**Files:** none (verification only).

- [ ] **Step 1: Run the complete gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green — Rust workspace unchanged, the full Vitest suite (including the new
`AttributeRow`/`RowsCompartment`/`diagramProps` files and the extended
`ClassifierBox`/`DiagramPropertiesBody`/`CentralEditPanelHost` suites), eslint, and the full
`svelte-check` + build.

- [ ] **Step 2: Manual smoke verification**

Run: `pnpm dev`. On a model with a **real** `Diagram` doc: open Dock → Diagram properties.
- Edit the title and the note; reload — both survive (persisted via `store.updateDiagram`).
- Toggle Show visibility / Show multiplicity, set Max attributes, then Unlimited — the canvas
  boxes update live and the choices survive reload.
- Pick a stereotype color — the box gets a top accent + light wash; add an allowlist and confirm
  only listed tags render.
- Switch to the implicit **All** view — the banner appears and every control is disabled (no throw).

- [ ] **Step 3: Commit (if any smoke fixes were needed)**

Only if Step 2 surfaced a fix. Otherwise no commit — Task 11 is a gate.

---

## Self-Review

**1. Spec coverage** (Spec B design → task):
- Panel props grow to `{ display, diagram, candidateStereotypes, editable, onChange, onUpdateDiagram }` → Tasks 8-10. ✓
- Title + Note section (commit on blur/Enter; note is panel-only) → Task 9. ✓
- Show visibility toggle / Show multiplicity toggle / Max attributes (number + Unlimited) → Task 9. ✓
- Stereotype allowlist picker ("Show all" vs `[]` vs names; empty-list hint) → Task 10. ✓
- Per-stereotype colors (pick merges, clear removes) → Task 10. ✓
- Implicit "All" view: banner + all controls disabled → Task 9 (banner + disabling) / helper Task 7. ✓
- `CentralEditPanelHost` diagram branch grows the four props → Task 8. ✓
- `CanvasInner`: `candidateStereotypes` derived, `editable` derived, `diagram`/`onUpdateDiagram` passed → Tasks 7-8. ✓
- Attribute visibility AND-gate (`profileAllows && showAttributeVisibility`) → Task 2. ✓
- Attribute multiplicity split (`showTypes` = type only, independent `showMultiplicity`) → Tasks 1-2. ✓
- Max attributes cap (`RowsCompartment.max`, static "+K more", attribute compartment only) → Tasks 3-4. ✓
- Stereotype filter rendering (filtered `stereotypeTags` → `StereotypeRow`) → Task 5. ✓
- Stereotype colors rendering (later-wins `overrideHeader`, `headerColor` accent + 12% wash) → Task 6. ✓
- Edge cases (no stereotypes, colored-but-filtered, `maxAttributes` > count, `maxAttributes: 0`
  not reachable) — covered by Tasks 4-6 tests and the panel's floor-at-1 numeric input. ✓
- Testing sections (Panel / Rendering / Canvas wiring) → each has matching test steps. ✓

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to Task N" — every code step
shows real code and every test step shows real assertions.

**3. Type consistency:** `DiagramDisplay` fields (`showAttributeVisibility`,
`showAttributeMultiplicity`, `maxAttributes`, `stereotypeFilter`, `stereotypeColors`) match Spec
A's Task 7 exactly. `Diagram` (`key/title/profile/members/hints?/description?/display?`) matches
Spec A's Task 7. `AttributeRow` props `{ showVisibility?, showTypes?, showMultiplicity? }` are
identical between Task 1 (definition) and Tasks 2/4 (consumption). `RowsCompartment` `max?` prop is
identical between Task 3 and Task 4. `diagramCandidateStereotypes`/`isDiagramEditable` signatures
match between Task 7 and Task 8. Panel/host prop names (`candidateStereotypes`, `editable`,
`onUpdateDiagram`) are identical across Tasks 8-10.

**Assumptions flagged for review (also in the final report):**
- (A) `data._display` carries the resolved `DiagramDisplay` (ClassifierBox re-runs `resolveDisplay`
  regardless, so rendering is safe even if Spec A feeds a raw partial).
- (B) `candidateStereotypes`/`editable` extracted into `diagramProps.ts` (Spec B inlines them in
  `CanvasInner`) purely for unit-testability; behavior is identical.
- (C) The ClassifierBox visibility AND-gate's "profile-allows AND diagram-wants ⇒ shown" path is
  only reachable with a visibility-allowing profile, and none is registered (`uml-domain` hides
  visibility); Task 2 asserts the profile floor, Task 1 covers the diagram-flag half at the
  `AttributeRow` unit.
- (D) jsdom does not parse `color-mix()`; Task 6 asserts the wash via the raw `style` attribute
  string, not CSSOM.
- (E) Concrete panel affordances (number input + "Unlimited" button for `maxAttributes`; checkbox
  list + "Show all" for the filter; `<input type="color">` + "Clear") are design choices within
  Spec B's stated semantics.
