# Diagram Properties Panel — Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Diagram properties panel actually change what is drawn on the canvas (today every toggle is inert), and close the one missing unit test the spec's Testing section calls for.

**Architecture:** The per-diagram `display` data model, the panel UI, all rendering wiring, and the rendering/component tests are already implemented and verified (see "Already Verified" below). The single functional gap is that `store.updateDiagram` is a hard no-op ("Stage 1b, diagram editing returns in 1c"), so the panel's `onChange` never reaches `activeDisplay` and nothing re-renders. Rather than pulling in the entire Stage-1c diagram-editing + OKF-serialization stack, we take the escape hatch the spec itself sanctions (lines 108-112: "default to a session-level default that does not force diagram creation") — hold per-diagram display overrides in an in-memory session store keyed by diagram key, mirroring the existing `hints.svelte.ts` pattern, and wire `CanvasInner` to read/write it.

**Tech Stack:** Svelte 5 runes (`$state`/`$derived`), TypeScript, Vitest + @testing-library/svelte, pnpm workspaces (`@uaml/okf`, `@uaml/core`, `@uaml/web`).

## Global Constraints

- READ-ONLY at the OKF/Rust wire: do NOT add a `display` field to the Rust `Diagram` struct or change OKF serialization. Per-diagram OKF persistence is out of scope (see "Out of Scope / Blocked").
- Session store is in-memory only — it must NOT write to `localStorage`. The spec's whole point was to remove per-browser render preferences; the interim default is session-level and resets on reload.
- Do not touch already-implemented and already-tested files except `CanvasInner.svelte` (the one wiring change in Task 2).
- Package test commands: `pnpm --filter @uaml/web test`, `pnpm --filter @uaml/okf test`. Full gate: `pnpm -r test`.
- Resolution precedence is always `DEFAULT_DISPLAY` < diagram's own `display` (base) < session override.

---

## Already Verified (no work required)

Checked field-for-field against the full spec; these are complete and tested — do NOT re-plan them:

- **Data model** — `packages/okf/src/types.ts`: `DiagramDisplay`, `DEFAULT_DISPLAY`, `resolveDisplay()`, `Diagram.display?`.
- **Panel UI** — `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` (all 5 controls; attribute-detail disabled when showAttributes off) mounted via `CentralEditPanelHost.svelte` (`kind: "diagram"`), opened by the Dock's `onOpenProperties`.
- **Rendering wiring** — `toRFNode` sets `_display`; `ClassifierBox.svelte` consumes it (rows / count / type column / stereotype row); `buildRfEdges` threads `associationLabels` + `emphasizeMultiplicity`; `erdAwareNodeSize` and `runDagreLayout` size on `showAttributes`.
- **Rendering tests** — `nodes/ClassifierBox.test.ts`, `edges.test.ts`, `canvas/layoutSize.test.ts` (node size responds to showAttributes), `toRFNode.test.ts`.
- **Component tests** — `DiagramPropertiesBody.test.ts` (renders active values, toggling emits patch, attribute-detail disabled/inert when showAttributes off).
- **Old globals removed** — no `state/viewMode.ts` or `state/relLabels.ts`.

---

## Task 1: Session-level per-diagram display store

**Files:**
- Create: `packages/web/src/state/displaySettings.svelte.ts`
- Test: `packages/web/src/state/displaySettings.svelte.test.ts`

**Interfaces:**
- Consumes: `DiagramDisplay`, `DEFAULT_DISPLAY`, `resolveDisplay` from `@uaml/okf`.
- Produces: `displaySettings` object with:
  - `resolve(key: string, base?: Partial<DiagramDisplay>): DiagramDisplay`
  - `patch(key: string, p: Partial<DiagramDisplay>): void`
  - `_reset(): void` (test-only)

- [ ] **Step 1: Write the failing test**

Create `packages/web/src/state/displaySettings.svelte.test.ts`:

```ts
import { test, expect, beforeEach } from "vitest";
import { DEFAULT_DISPLAY } from "@uaml/okf";
import { displaySettings } from "./displaySettings.svelte";

beforeEach(() => displaySettings._reset());

test("resolve returns DEFAULT_DISPLAY for an untouched diagram", () => {
  expect(displaySettings.resolve("d1")).toEqual(DEFAULT_DISPLAY);
});

test("patch overrides a single field, leaving the rest at defaults", () => {
  displaySettings.patch("d1", { showAttributes: false });
  expect(displaySettings.resolve("d1")).toEqual({ ...DEFAULT_DISPLAY, showAttributes: false });
});

test("overrides are isolated per diagram key", () => {
  displaySettings.patch("d1", { associationLabels: "hidden" });
  expect(displaySettings.resolve("d2")).toEqual(DEFAULT_DISPLAY);
});

test("successive patches for the same diagram merge", () => {
  displaySettings.patch("d1", { showAttributes: false });
  displaySettings.patch("d1", { attributeDetail: "name-only" });
  expect(displaySettings.resolve("d1")).toEqual({
    ...DEFAULT_DISPLAY, showAttributes: false, attributeDetail: "name-only",
  });
});

test("a base display is overlaid under the session override", () => {
  const base = { showStereotype: false } as const;
  expect(displaySettings.resolve("d1", base)).toEqual({ ...DEFAULT_DISPLAY, showStereotype: false });
  displaySettings.patch("d1", { showStereotype: true });
  expect(displaySettings.resolve("d1", base).showStereotype).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test displaySettings`
Expected: FAIL — cannot resolve `./displaySettings.svelte`.

- [ ] **Step 3: Write minimal implementation**

Create `packages/web/src/state/displaySettings.svelte.ts`:

```ts
import { resolveDisplay, type DiagramDisplay } from "@uaml/okf";

// Session-level per-diagram display overrides. The model store's diagram
// mutations are no-ops in Stage 1b (`store.updateDiagram` never persists) and
// the OKF wire carries no `display` block, so per-diagram render settings can't
// live in the model yet. Until Stage 1c diagram editing + OKF `display`
// serialization land, we hold them in memory for the browser session — spec
// lines 108-112: "a session-level default that does not force diagram creation".
// Keyed by diagram key so the implicit "All" diagram and real diagrams keep
// independent settings; resets on reload (NOT the per-browser localStorage
// preference the spec set out to remove).
let overrides = $state<Record<string, Partial<DiagramDisplay>>>({});

export const displaySettings = {
  // DEFAULT_DISPLAY < the diagram's own persisted display (`base`, currently
  // always absent) < the session override. Reads `overrides` so a $derived
  // caller re-runs when it changes.
  resolve(key: string, base?: Partial<DiagramDisplay>): DiagramDisplay {
    return resolveDisplay({ ...base, ...overrides[key] });
  },
  // Merge a single-field patch into a diagram's override (immutable reassign so
  // the $state dependency re-fires).
  patch(key: string, p: Partial<DiagramDisplay>): void {
    overrides = { ...overrides, [key]: { ...overrides[key], ...p } };
  },
  // Test-only: clear all session overrides.
  _reset(): void {
    overrides = {};
  },
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test displaySettings`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/state/displaySettings.svelte.ts packages/web/src/state/displaySettings.svelte.test.ts
git commit -m "feat(web): session-level per-diagram display store"
```

---

## Task 2: Route the panel through the session store in CanvasInner

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`

**Interfaces:**
- Consumes: `displaySettings.resolve` / `displaySettings.patch` from Task 1.
- Produces: nothing new — `activeDisplay` keeps the same type (`DiagramDisplay`) and still flows unchanged into `toRFNode` / `buildRfEdges` / `runDagreLayout` / `CentralEditPanelHost`.

**Context:** Today (`CanvasInner.svelte`) line ~152 derives `activeDisplay = resolveDisplay(activeDiagram.display)` and `handleDisplayChange` (line ~328) calls `store.updateDiagram(...)`, which is a no-op — so toggles never change `activeDisplay` and the canvas never re-renders. Redirect both through the session store. This task has no unit test of its own (it is glue over already-tested units); verify via typecheck/build + the manual smoke check below.

- [ ] **Step 1: Repoint `activeDisplay` at the session store**

In `packages/web/src/components/canvas/CanvasInner.svelte`, replace the derived (currently):

```ts
  const activeDisplay = $derived(resolveDisplay(activeDiagram.display));
```

with:

```ts
  const activeDisplay = $derived(displaySettings.resolve(activeDiagram.key, activeDiagram.display));
```

- [ ] **Step 2: Rewrite `handleDisplayChange` to persist to the session store**

Replace the body (currently `store.updateDiagram(activeDiagram.key, { display: { ...activeDisplay, ...p } });`) so the function reads:

```ts
  // Merge a single-field edit into the active diagram's session-level display.
  // Held in memory (displaySettings), not the model: `store.updateDiagram` is a
  // no-op until Stage 1c and the OKF wire carries no `display` block. Works
  // identically for the implicit "All" diagram and real diagrams — no diagram
  // creation is forced (spec lines 108-112).
  function handleDisplayChange(p: Partial<DiagramDisplay>) {
    displaySettings.patch(activeDiagram.key, p);
  }
```

- [ ] **Step 3: Fix imports**

Add the store import alongside the other `../../state/*` imports:

```ts
  import { displaySettings } from "../../state/displaySettings.svelte";
```

Then drop the now-unused `resolveDisplay` from the `@uaml/okf` import so the line reads:

```ts
  import { slugify, type DiagramDisplay } from "@uaml/okf";
```

- [ ] **Step 4: Typecheck + build the web package**

Run: `pnpm --filter @uaml/web build`
Expected: PASS, no unused-import or type errors (confirms `resolveDisplay` was the only dropped use and `DiagramDisplay` is still referenced by `handleDisplayChange`).

- [ ] **Step 5: Manual smoke verification**

Run: `pnpm dev` (or `pnpm --filter @uaml/web dev`), add a class with attributes, open Dock → Diagram properties, and confirm each control changes the canvas live:
- Toggle **Show attributes** off → boxes collapse to an "N attributes" count and shrink.
- **Attribute detail** → **Name only** hides the type column.
- **Associations** → **Hide labels** removes edge multiplicity/role labels.
- **Show stereotype** off → the «stereotype» row disappears.
Switch diagrams and confirm each keeps its own settings for the session.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "fix(web): drive canvas render from session display store (toggles were inert)"
```

---

## Task 3: Unit-test DiagramDisplay defaulting in @uaml/okf

**Files:**
- Test: `packages/okf/test/display.test.ts`

**Interfaces:**
- Consumes: `DEFAULT_DISPLAY`, `resolveDisplay` from `@uaml/okf` (`../src/types`).

**Context:** The spec's Testing section requires a unit test for `DiagramDisplay` defaulting (missing `display` ⇒ defaults). `resolveDisplay` is exercised indirectly by component/render tests but has no dedicated okf-level unit test. (The OKF round-trip half of that Testing bullet is blocked — see "Out of Scope / Blocked".)

- [ ] **Step 1: Write the failing test**

Create `packages/okf/test/display.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { DEFAULT_DISPLAY, resolveDisplay } from "../src/types";

describe("resolveDisplay", () => {
  it("returns the full DEFAULT_DISPLAY when display is absent", () => {
    expect(resolveDisplay(undefined)).toEqual(DEFAULT_DISPLAY);
  });

  it("returns the documented default values", () => {
    expect(DEFAULT_DISPLAY).toEqual({
      showAttributes: true,
      attributeDetail: "name-type",
      associationLabels: "all",
      emphasizeMultiplicity: false,
      showStereotype: true,
    });
  });

  it("overlays a partial display onto the defaults", () => {
    expect(resolveDisplay({ showAttributes: false, attributeDetail: "name-only" })).toEqual({
      ...DEFAULT_DISPLAY,
      showAttributes: false,
      attributeDetail: "name-only",
    });
  });

  it("does not mutate DEFAULT_DISPLAY", () => {
    resolveDisplay({ showStereotype: false });
    expect(DEFAULT_DISPLAY.showStereotype).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/okf test display`
Expected: FAIL — file not found / no matching tests before creation; after creation it runs. (If the harness reports "no tests" pre-creation, that is the expected failing state.)

- [ ] **Step 3: Implement**

No implementation needed — `resolveDisplay`/`DEFAULT_DISPLAY` already exist in `packages/okf/src/types.ts`. This task only adds the missing coverage.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/okf test display`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/test/display.test.ts
git commit -m "test(okf): cover DiagramDisplay defaulting via resolveDisplay"
```

---

## Out of Scope / Blocked

- **OKF round-trip preserving `display`** (spec Testing, second half of the first bullet; spec Goal "persist per diagram in model / OKF"). **Blocked, not implemented.** The Rust `Diagram` struct (`crates/uaml/src/model.rs`) has no `display` field — it carries `key/title/profile/groups/layout` — so `display` cannot survive an OKF export/reimport today. Real per-diagram persistence also needs `store.updateDiagram` (currently `/* no-op in 1b */` in `packages/core/src/state/model.ts`) implemented. Both belong to the deferred "Stage 1c" diagram-editing workstream and are deliberately excluded here per the Global Constraints. Task 1's session store is the spec-sanctioned interim (spec lines 108-112). When Stage 1c lands, `displaySettings.resolve(key, base)` already threads `base = activeDiagram.display`, so persisted per-diagram settings will layer in with no change to the render path.
- **localStorage seeding / migration** of old `viewMode`/`relLabelMode` (spec lines 113-114) — explicitly marked "not required" by the spec; skipped.
