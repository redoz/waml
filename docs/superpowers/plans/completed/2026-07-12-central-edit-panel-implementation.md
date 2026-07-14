# Central Edit Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a single centered, enlarged "central edit panel" that hosts (1) single-item editing of a bare element selected from the tree navigator and (2) diagram-properties editing over the live diagram, reusing the existing inspector and diagram-display field bodies verbatim.

**Architecture:** Three new focused Svelte components under `packages/web/src/components/central/`. `CentralEditPanel.svelte` is pure presentational chrome (centered card, scrim, dismissal). `DiagramPropertiesBody.svelte` is the diagram display-controls body extracted from `Dock.svelte`'s inline popover so it can be shared. `CentralEditPanelHost.svelte` routes the correct body by selection kind, resolves the target node, and wires updates through existing store paths. `CanvasInner.svelte` owns a single `centralPanel` state field and mounts the host as an overlay; the tree-nav `onEditProperties` callback and the Dock sliders button both open it. No new persistence path — edits flow through `store.updateNode` and `store.updateDiagram` exactly as today.

**Tech Stack:** Svelte 5 (runes: `$props`, `$state`, `$derived`, `$effect`, snippets), TypeScript, Vitest + `@testing-library/svelte`, Tailwind utility classes, `@uaml/okf` types.

## Global Constraints

- **Scope guard: Feature C only.** Do NOT build the tree navigator (Feature A — already landed on main) or the solver-driven layout swap (Feature B — separate spec). This plan builds the central edit panel host chrome and its wiring, nothing else.
- **Reuse, do not rewrite field editors.** The panel body reuses `ObjectInspector.svelte` (element fields) and the extracted `DiagramPropertiesBody.svelte` (diagram display toggles) verbatim. Never re-implement field editors.
- **Right-docked `InspectorPanel` stays.** The right-dock inspector for in-diagram node/edge selection is untouched and coexists with the central panel; do not merge or remove it.
- **Diagram-properties body = display toggles only.** The five controls today are `showAttributes`, `attributeDetail`, `associationLabels`, `emphasizeMultiplicity`, `showStereotype`. No title/profile fields in this body.
- **Never add a `Co-Authored-By` / Claude trailer to commits.**
- **Commit real-root tracked files only.** The repo root is `C:/dev/uaml`; `.claude/worktrees/*` are gitignored nested copies. There are UNRELATED pre-existing modifications in `packages/core/src/state/ops-adapter.test.ts` and `packages/web/src/components/inspector/ObjectInspector.svelte` — do NOT stage or touch those; `git add` only the exact files each task lists.
- **Test command (web package):** `pnpm --filter @uaml/web test -- <relative/path/to/test>` runs a single test file. `pnpm --filter @uaml/web check` runs `svelte-check`.
- **Full green gate before every commit:** `pnpm -r test && pnpm lint && pnpm build`. `pnpm build` includes `svelte-check` on the web package (a filtered web build alone can phantom-error against a stale `@uaml/okf` dist — always run the full `pnpm build`).

---

## File Structure

**New files:**
- `packages/web/src/components/central/CentralEditPanel.svelte` — presentational chrome: centered card + scrim + dismissal (Esc two-stage / scrim click / close button). Knows nothing about what it hosts.
- `packages/web/src/components/central/CentralEditPanel.test.ts` — chrome unit tests.
- `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` — the five diagram display controls, extracted from `Dock.svelte`'s popover so both Dock and the central host can render them.
- `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts` — display-body unit tests.
- `packages/web/src/components/central/CentralEditPanelHost.svelte` — routes the body by `CentralPanelState` kind, resolves the node, wires updates; exports the `CentralPanelState` type from its module script.
- `packages/web/src/components/central/CentralEditPanelHost.test.ts` — host routing/wiring unit tests.

**Modified files:**
- `packages/web/src/components/canvas/Dock.svelte` — replace the inline 268px display popover with `<DiagramPropertiesBody>` (Task 2, behavior-preserving), then (Task 5) change the sliders button to fire an `onOpenProperties` callback instead of toggling the local popover.
- `packages/web/src/components/canvas/CanvasInner.svelte` — add `centralPanel` state, mount `<CentralEditPanelHost>`, rewire `onEditProperties` (currently `:518`), pass `onOpenProperties` to `<Dock>`.

---

## Task 1: CentralEditPanel chrome (presentational)

Pure host chrome. No knowledge of node/diagram — it renders a title, a body snippet, a close button, a scrim, and owns dismissal. Two-stage Esc: if a text field inside the panel is focused, the first Esc blurs it (protecting an in-progress edit) and only a second Esc closes.

**Files:**
- Create: `packages/web/src/components/central/CentralEditPanel.svelte`
- Test: `packages/web/src/components/central/CentralEditPanel.test.ts`

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: component `CentralEditPanel` with props
  ```ts
  {
    title: string;
    onClose: () => void;
    children: import("svelte").Snippet;  // the body content
  }
  ```
  Renders a scrim (`data-testid="central-scrim"`, click → `onClose`), a centered card with an `<h2>` title, a close button (`aria-label="Close"` → `onClose`), and `{@render children()}`. A window `keydown` handler (registered in an `$effect`, torn down on unmount) implements two-stage Esc.

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/components/central/CentralEditPanel.test.ts`:

```ts
import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import CentralEditPanel from "./CentralEditPanel.svelte";

// A minimal body snippet containing a focusable text input, so the two-stage
// Esc behaviour can be exercised.
const bodySnippet = createRawSnippet(() => ({
  render: () => `<input aria-label="field" />`,
}));

const props = (over = {}) => ({
  title: "Customer",
  onClose: vi.fn(),
  children: bodySnippet,
  ...over,
});

test("renders the title and body", () => {
  render(CentralEditPanel, { props: props() });
  expect(screen.getByRole("heading", { name: "Customer" })).toBeTruthy();
  expect(screen.getByLabelText("field")).toBeTruthy();
});

test("close button fires onClose", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("clicking the scrim fires onClose", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByTestId("central-scrim"));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Esc with no field focused closes immediately", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Esc while a field is focused blurs first, then a second Esc closes", async () => {
  const onClose = vi.fn();
  render(CentralEditPanel, { props: props({ onClose }) });
  const field = screen.getByLabelText("field") as HTMLInputElement;
  field.focus();
  expect(document.activeElement).toBe(field);

  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).not.toHaveBeenCalled();      // first Esc only blurs
  expect(document.activeElement).not.toBe(field);

  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);     // second Esc closes
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @uaml/web test -- src/components/central/CentralEditPanel.test.ts`
Expected: FAIL — cannot resolve `./CentralEditPanel.svelte`.

- [ ] **Step 3: Write the component**

Create `packages/web/src/components/central/CentralEditPanel.svelte`:

```svelte
<script lang="ts">
  // Presentational chrome for the central edit panel: a centered, enlarged card
  // over a dismissing scrim. It hosts an arbitrary body snippet and knows nothing
  // about what that body edits. Dismissal: close button, scrim click, or Esc.
  // Esc is two-stage — if a text field inside the panel is focused, the first Esc
  // blurs it (so a stray keypress can't discard an in-progress edit) and only a
  // second Esc closes the panel.
  import type { Snippet } from "svelte";

  let { title, onClose, children }: {
    title: string;
    onClose: () => void;
    children: Snippet;
  } = $props();

  let card = $state<HTMLDivElement | null>(null);

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      const active = document.activeElement as HTMLElement | null;
      const editing =
        !!active &&
        !!card?.contains(active) &&
        ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName);
      if (editing) {
        active!.blur();      // first Esc: protect the in-progress edit
        e.stopPropagation();
      } else {
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- Scrim: dims whatever is rendered behind (neutral app or the live diagram) and
     dismisses on click. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-testid="central-scrim"
  onclick={onClose}
  class="fixed inset-0 z-[60] bg-slate-900/30 flex items-center justify-center p-8"
  style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
>
  <!-- Card: stops propagation so clicks inside never reach the scrim. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={card}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    onclick={(e) => e.stopPropagation()}
    class="w-full max-w-[560px] max-h-[85vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)]"
  >
    <div class="px-5 py-[15px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0">
      <h2 class="text-[15px] font-[650] flex-1 text-slate-900 truncate">{title}</h2>
      <button
        onclick={onClose}
        aria-label="Close"
        title="Close"
        class="cursor-pointer text-slate-500 bg-transparent border-none text-[20px] leading-none hover:text-slate-900 transition-colors p-0"
      >
        ×
      </button>
    </div>
    <div class="px-5 py-5 overflow-y-auto flex-1 min-h-0">
      {@render children()}
    </div>
  </div>
</div>
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @uaml/web test -- src/components/central/CentralEditPanel.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/central/CentralEditPanel.svelte packages/web/src/components/central/CentralEditPanel.test.ts
git commit -m "feat(uaml): central edit panel chrome with scrim and two-stage Esc"
```

---

## Task 2: Extract DiagramPropertiesBody from Dock (behavior-preserving refactor)

The five diagram display controls currently live inline in `Dock.svelte`'s popover, built from local `toggleRow`/`segmented` snippets. Extract them into a shared body component so the central host can render the identical controls. Dock keeps rendering them inside its existing popover in this task — pure refactor, no behavior change yet.

**Files:**
- Create: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`
- Create: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`
- Modify: `packages/web/src/components/canvas/Dock.svelte` (replace inline controls + the now-unused `toggleRow`/`segmented` snippets with `<DiagramPropertiesBody>`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: component `DiagramPropertiesBody` with props
  ```ts
  {
    display: import("@uaml/okf").DiagramDisplay;
    onChange: (patch: Partial<import("@uaml/okf").DiagramDisplay>) => void;
  }
  ```
  Renders the five controls; each control calls `onChange` with the single changed field. The controls carry the same accessible roles/names they have today (`role="switch"` / `role="radio"` with `aria-label` equal to the visible label), so existing behavior and tests transfer unchanged.

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`:

```ts
import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import DiagramPropertiesBody from "./DiagramPropertiesBody.svelte";
import { DEFAULT_DISPLAY } from "@uaml/okf";

const props = (over = {}) => ({
  display: { ...DEFAULT_DISPLAY },
  onChange: vi.fn(),
  ...over,
});

test("renders all five display controls", () => {
  render(DiagramPropertiesBody, { props: props() });
  expect(screen.getByRole("switch", { name: "Show attributes" })).toBeTruthy();
  expect(screen.getByRole("radiogroup", { name: "Attribute detail" })).toBeTruthy();
  expect(screen.getByRole("radiogroup", { name: "Associations" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Emphasize multiplicity" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Show stereotype" })).toBeTruthy();
});

test("toggling 'Show attributes' emits the inverted flag", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("switch", { name: "Show attributes" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributes: false });
});

test("picking an attribute-detail option emits that value", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("radio", { name: "Name + type" }));
  expect(onChange).toHaveBeenCalledWith({ attributeDetail: "name-type" });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @uaml/web test -- src/components/canvas/DiagramPropertiesBody.test.ts`
Expected: FAIL — cannot resolve `./DiagramPropertiesBody.svelte`.

- [ ] **Step 3: Create the extracted component**

Create `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` (the two snippets and five controls moved verbatim from `Dock.svelte`, now driven by props):

```svelte
<script lang="ts">
  // The active diagram's display controls, extracted from Dock's popover so the
  // central edit panel can host the identical set. Display toggles only — no
  // title/profile. Each control emits a single changed field via onChange.
  import type { DiagramDisplay } from "@uaml/okf";

  let { display, onChange }: {
    display: DiagramDisplay;
    onChange: (patch: Partial<DiagramDisplay>) => void;
  } = $props();

  function patch(p: Partial<DiagramDisplay>) {
    onChange(p);
  }
</script>

<!-- A labelled on/off toggle row. -->
{#snippet toggleRow(label: string, checked: boolean, onToggle: () => void, disabled = false)}
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    disabled={disabled}
    onclick={() => { if (!disabled) onToggle(); }}
    class="flex w-full items-center justify-between gap-3 rounded-lg px-2 py-1.5 text-left transition-colors {disabled
      ? 'opacity-40 cursor-not-allowed'
      : 'hover:bg-[#f1f3f7]'}"
  >
    <span class="text-[13px] font-medium text-slate-800">{label}</span>
    <span
      class="relative inline-flex h-[18px] w-[32px] flex-shrink-0 items-center rounded-full transition-colors {checked
        ? 'bg-[#1e88e5]'
        : 'bg-slate-300'}"
    >
      <span
        class="inline-block h-[14px] w-[14px] rounded-full bg-white shadow transition-transform {checked
          ? 'translate-x-[16px]'
          : 'translate-x-[2px]'}"
      ></span>
    </span>
  </button>
{/snippet}

<!-- A two-option segmented control (radio group). -->
{#snippet segmented(label: string, options: { value: string; label: string }[], value: string, onPick: (v: string) => void, disabled = false)}
  <div class="px-2 py-1.5 {disabled ? 'opacity-40' : ''}">
    <div class="mb-1 text-[13px] font-medium text-slate-800">{label}</div>
    <div role="radiogroup" aria-label={label} class="flex gap-1 rounded-lg bg-[#f1f3f7] p-0.5">
      {#each options as opt (opt.value)}
        {@const selected = opt.value === value}
        <button
          type="button"
          role="radio"
          aria-checked={selected}
          aria-label={opt.label}
          disabled={disabled}
          onclick={() => { if (!disabled) onPick(opt.value); }}
          class="flex-1 rounded-md px-2 py-1 text-[12px] font-semibold transition-colors {disabled
            ? 'cursor-not-allowed'
            : 'cursor-pointer'} {selected ? 'bg-white text-[#1e88e5] shadow-sm' : 'text-slate-500 hover:text-slate-800'}"
        >
          {opt.label}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<div>
  {@render toggleRow("Show attributes", display.showAttributes, () =>
    patch({ showAttributes: !display.showAttributes }),
  )}
  {@render segmented(
    "Attribute detail",
    [
      { value: "name-only", label: "Name only" },
      { value: "name-type", label: "Name + type" },
    ],
    display.attributeDetail,
    (v) => patch({ attributeDetail: v as DiagramDisplay["attributeDetail"] }),
    !display.showAttributes,
  )}

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render segmented(
    "Associations",
    [
      { value: "all", label: "Show labels" },
      { value: "hidden", label: "Hide labels" },
    ],
    display.associationLabels,
    (v) => patch({ associationLabels: v as DiagramDisplay["associationLabels"] }),
  )}
  {@render toggleRow("Emphasize multiplicity", display.emphasizeMultiplicity, () =>
    patch({ emphasizeMultiplicity: !display.emphasizeMultiplicity }),
  )}

  <div class="h-px bg-[#eef1f5] mx-1 my-1"></div>

  {@render toggleRow("Show stereotype", display.showStereotype, () =>
    patch({ showStereotype: !display.showStereotype }),
  )}
</div>
```

- [ ] **Step 4: Rewire Dock to use the extracted body**

In `packages/web/src/components/canvas/Dock.svelte`:

1. Add the import at the top of the `<script lang="ts">` block (after the existing imports):

```svelte
  import DiagramPropertiesBody from "./DiagramPropertiesBody.svelte";
```

2. Delete the now-unused local `toggleRow` and `segmented` snippet definitions (the two `{#snippet toggleRow(...)}...{/snippet}` and `{#snippet segmented(...)}...{/snippet}` blocks) and the local `patch` function.

3. Inside the `role="dialog"` popover, replace the block from the first `{@render toggleRow("Show attributes", ...)}` through the final `{@render toggleRow("Show stereotype", ...)}` (keep the `Diagram properties` heading `<div>`) with:

```svelte
          <DiagramPropertiesBody {display} onChange={(p) => onDisplayChange?.(p)} />
```

- [ ] **Step 5: Run the display-body tests to verify they pass**

Run: `pnpm --filter @uaml/web test -- src/components/canvas/DiagramPropertiesBody.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 6: Verify Dock still type-checks and its own tests pass**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.
Run: `pnpm --filter @uaml/web test`
Expected: PASS (whole web suite, including any existing Dock test).

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/canvas/DiagramPropertiesBody.svelte packages/web/src/components/canvas/DiagramPropertiesBody.test.ts packages/web/src/components/canvas/Dock.svelte
git commit -m "refactor(uaml): extract DiagramPropertiesBody from Dock popover"
```

---

## Task 3: CentralEditPanelHost — body routing and update wiring

The host owns the `CentralPanelState` type and picks the correct body: an element opens `ObjectInspector`; a diagram opens `DiagramPropertiesBody`. It resolves the node from the supplied nodes list, guards a since-deleted key by calling `onClose`, computes the panel title, and wraps everything in `CentralEditPanel`. Keeping this logic out of `CanvasInner` lets it be unit-tested without SvelteFlow.

**Files:**
- Create: `packages/web/src/components/central/CentralEditPanelHost.svelte`
- Test: `packages/web/src/components/central/CentralEditPanelHost.test.ts`

**Interfaces:**
- Consumes:
  - `CentralEditPanel` (Task 1) — props `{ title, onClose, children }`.
  - `DiagramPropertiesBody` (Task 2) — props `{ display, onChange }`.
  - `ObjectInspector` (`../inspector/ObjectInspector.svelte`, existing) — props `{ node: ModelNode, onUpdate: (patch: Partial<ModelNode>) => void, profileName?: string }`.
- Produces: exported type and component.
  ```ts
  // exported from <script module lang="ts">
  export type CentralPanelState =
    | { kind: "element"; nodeKey: string }
    | { kind: "diagram" };
  ```
  Component `CentralEditPanelHost` props:
  ```ts
  {
    state: CentralPanelState | null;   // null => render nothing
    nodes: import("@uaml/okf").ModelNode[];
    display: import("@uaml/okf").DiagramDisplay;
    profileName?: string;
    onUpdateNode: (key: string, patch: Partial<import("@uaml/okf").ModelNode>) => void;
    onDisplayChange: (patch: Partial<import("@uaml/okf").DiagramDisplay>) => void;
    onClose: () => void;
  }
  ```
  Behavior: `state === null` → renders nothing. `kind: "element"` with a resolvable `nodeKey` → `CentralEditPanel` (title = node title or `"Untitled"`) wrapping `ObjectInspector`. `kind: "element"` with an unresolvable key → renders nothing and calls `onClose()` (deleted-key guard). `kind: "diagram"` → `CentralEditPanel` (title = `"Diagram properties"`) wrapping `DiagramPropertiesBody`.

- [ ] **Step 1: Write the failing tests**

Create `packages/web/src/components/central/CentralEditPanelHost.test.ts`:

```ts
import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import CentralEditPanelHost from "./CentralEditPanelHost.svelte";
import { DEFAULT_DISPLAY, type ModelNode } from "@uaml/okf";

const node = (key: string, title: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title, description: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as unknown as ModelNode;

const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer")],
  display: { ...DEFAULT_DISPLAY },
  profileName: "uml-domain",
  onUpdateNode: vi.fn(),
  onDisplayChange: vi.fn(),
  onClose: vi.fn(),
  ...over,
});

test("null state renders nothing", () => {
  render(CentralEditPanelHost, { props: props({ state: null }) });
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("element state mounts ObjectInspector titled by the node", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" } }),
  });
  expect(screen.getByRole("heading", { name: "Customer" })).toBeTruthy();
  // ObjectInspector's Title field is present inside the host.
  expect(screen.getByLabelText("Title")).toBeTruthy();
});

test("editing the title in the element body calls onUpdateNode with the node key", async () => {
  const onUpdateNode = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, onUpdateNode }),
  });
  await fireEvent.input(screen.getByLabelText("Title"), { target: { value: "Client" } });
  expect(onUpdateNode).toHaveBeenCalledWith(
    "customer",
    expect.objectContaining({ concept: expect.objectContaining({ title: "Client" }) }),
  );
});

test("element state with an unknown key closes and renders nothing", () => {
  const onClose = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "ghost" }, onClose }),
  });
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("diagram state mounts the display controls titled 'Diagram properties'", () => {
  render(CentralEditPanelHost, { props: props({ state: { kind: "diagram" } }) });
  expect(screen.getByRole("heading", { name: "Diagram properties" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Show attributes" })).toBeTruthy();
});

test("toggling a display control in the diagram body calls onDisplayChange", async () => {
  const onDisplayChange = vi.fn();
  render(CentralEditPanelHost, {
    props: props({
      state: { kind: "diagram" },
      display: { ...DEFAULT_DISPLAY, showAttributes: true },
      onDisplayChange,
    }),
  });
  await fireEvent.click(screen.getByRole("switch", { name: "Show attributes" }));
  expect(onDisplayChange).toHaveBeenCalledWith({ showAttributes: false });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm --filter @uaml/web test -- src/components/central/CentralEditPanelHost.test.ts`
Expected: FAIL — cannot resolve `./CentralEditPanelHost.svelte`.

- [ ] **Step 3: Write the host component**

Create `packages/web/src/components/central/CentralEditPanelHost.svelte`:

```svelte
<script module lang="ts">
  // What the central panel is currently editing. `null` means the panel is
  // closed. An element edits one model node's fields; a diagram edits the active
  // diagram's display settings.
  export type CentralPanelState =
    | { kind: "element"; nodeKey: string }
    | { kind: "diagram" };
</script>

<script lang="ts">
  import type { DiagramDisplay, ModelNode } from "@uaml/okf";
  import CentralEditPanel from "./CentralEditPanel.svelte";
  import ObjectInspector from "../inspector/ObjectInspector.svelte";
  import DiagramPropertiesBody from "../canvas/DiagramPropertiesBody.svelte";

  let { state, nodes, display, profileName, onUpdateNode, onDisplayChange, onClose }: {
    state: CentralPanelState | null;
    nodes: ModelNode[];
    display: DiagramDisplay;
    profileName?: string;
    onUpdateNode: (key: string, patch: Partial<ModelNode>) => void;
    onDisplayChange: (patch: Partial<DiagramDisplay>) => void;
    onClose: () => void;
  } = $props();

  // Resolve the edited node (element context only); a since-deleted key resolves
  // to undefined, mirroring today's `focused` guard.
  const node = $derived(
    state?.kind === "element" ? nodes.find((n) => n.key === state.nodeKey) : undefined,
  );

  // Element pointing at a since-deleted key: close instead of showing an empty
  // shell. Runs as an effect so it fires on the offending render.
  $effect(() => {
    if (state?.kind === "element" && !node) onClose();
  });
</script>

{#if state?.kind === "element" && node}
  <CentralEditPanel title={node.concept.title?.trim() || "Untitled"} {onClose}>
    <ObjectInspector
      {node}
      onUpdate={(patch) => onUpdateNode(node.key, patch)}
      {profileName}
    />
  </CentralEditPanel>
{:else if state?.kind === "diagram"}
  <CentralEditPanel title="Diagram properties" {onClose}>
    <DiagramPropertiesBody {display} onChange={onDisplayChange} />
  </CentralEditPanel>
{/if}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm --filter @uaml/web test -- src/components/central/CentralEditPanelHost.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/central/CentralEditPanelHost.svelte packages/web/src/components/central/CentralEditPanelHost.test.ts
git commit -m "feat(uaml): central edit panel host routes element and diagram bodies"
```

---

## Task 4: Wire the element (single-item) context into CanvasInner

Replace the synthetic-selection path. Today `onEditProperties` (at `CanvasInner.svelte:518`) sets `selectionSet = { nodes: [key], edges: [] }`, faking a single-item view through the right-dock inspector over the implicit `__all__` diagram. Route it to the central panel instead, and mount the host.

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`

**Interfaces:**
- Consumes: `CentralEditPanelHost` + `CentralPanelState` (Task 3). Existing `store.updateNode`, `handleDisplayChange` (`:323`), `activeDisplay` (`:147`), `activeDiagram` (`:134`).
- Produces: a `centralPanel` state field owned by `CanvasInner`, set by `onEditProperties` and (Task 5) by the Dock button.

- [ ] **Step 1: Add the import and state field**

In `packages/web/src/components/canvas/CanvasInner.svelte`, add to the imports (near the other component imports around `:46`–`:48`):

```svelte
  import CentralEditPanelHost, { type CentralPanelState } from "../central/CentralEditPanelHost.svelte";
```

Add a state field alongside the other UI state (e.g. near `inspectorPinned` around `:104`):

```svelte
  // The central edit panel's current target (null = closed). Element context is
  // opened by the navigator's "View / edit properties"; diagram context by the
  // Dock sliders button (Task 5).
  let centralPanel = $state<CentralPanelState | null>(null);
```

- [ ] **Step 2: Rewire `onEditProperties`**

Replace the `onEditProperties` prop on `<TopBar>` (`:518`):

```svelte
    onEditProperties={(key) => (selectionSet = { nodes: [key], edges: [] })}
```

with:

```svelte
    onEditProperties={(key) => (centralPanel = { kind: "element", nodeKey: key })}
```

- [ ] **Step 3: Mount the host**

Immediately after the closing `</TopBar>` tag (`:519`), add:

```svelte
  <CentralEditPanelHost
    state={centralPanel}
    nodes={$model.nodes}
    display={activeDisplay}
    profileName={activeDiagram.profile}
    onUpdateNode={(key, patch) => store.updateNode(key, patch)}
    onDisplayChange={handleDisplayChange}
    onClose={() => (centralPanel = null)}
  />
```

- [ ] **Step 4: Verify the web package type-checks and the whole suite passes**

Run: `pnpm --filter @uaml/web check`
Expected: no errors (in particular, `CentralPanelState` imports cleanly and `centralPanel` is well-typed).
Run: `pnpm --filter @uaml/web test`
Expected: PASS (full web suite unchanged).

- [ ] **Step 5: Manual verification**

Run: `pnpm dev` and open the app. Open the diagram switcher / navigator, pick a classifier row, choose **View / edit properties**. Confirm the central panel opens centered over a dimmed backdrop showing the `ObjectInspector` fields (Title/Description/Type/Stereotypes/Attributes). Edit the Title and confirm it persists (the node updates live behind the scrim). Confirm Esc-blurs-then-Esc-closes, scrim click closes, and the × closes. Confirm the right-dock `InspectorPanel` still works independently for a node selected inside the diagram.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(uaml): open central panel for navigator single-item edit"
```

---

## Task 5: Route the Dock sliders button to the diagram-properties context

The Dock sliders button today toggles its own inline popover (`propsOpen`). Change it to fire an `onOpenProperties` callback so `CanvasInner` opens the central panel's diagram context instead. Remove the now-dead inline popover (open-Q #2 resolution: delete it outright — the display controls now live in the central panel).

**Files:**
- Modify: `packages/web/src/components/canvas/Dock.svelte`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`

**Interfaces:**
- Consumes: `centralPanel` state (Task 4), `<Dock>` mount (`CanvasInner.svelte:577`).
- Produces: new `Dock` prop `onOpenProperties?: () => void`. `Dock` no longer needs `display` / `onDisplayChange` (the popover is gone); remove those two props and the `propsOpen` state.

- [ ] **Step 1: Update Dock's props and the sliders button**

In `packages/web/src/components/canvas/Dock.svelte`:

1. In the `$props()` destructure and its type, remove `display` and `onDisplayChange`, and add `onOpenProperties`:

```svelte
  let {
    activeTool,
    onToolChange,
    onClear,
    clearDisabled,
    onOpenProperties,
  }: {
    activeTool: Tool;
    onToolChange: (tool: Tool) => void;
    onClear: () => void;
    clearDisabled?: boolean;
    // Opens the central edit panel's diagram-properties context.
    onOpenProperties?: () => void;
  } = $props();
```

2. Remove the `let propsOpen = $state(false);` line and the `DiagramPropertiesBody` import added in Task 2 (it is no longer used here — the central host now renders it).

3. Replace the diagram-properties button + popover block (the `<div class="relative group">` that contains the `onclick={() => (propsOpen = !propsOpen)}` button and the entire `{#if propsOpen} ... {/if}` popover) with a plain trigger button:

```svelte
  <div class="relative group">
    <button
      onclick={() => onOpenProperties?.()}
      aria-label="Diagram properties"
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900"
    >
      {@render slidersIcon()}
    </button>
    {@render dockTip("Diagram properties")}
  </div>
```

- [ ] **Step 2: Update the Dock mount in CanvasInner**

In `packages/web/src/components/canvas/CanvasInner.svelte`, replace the `display` / `onDisplayChange` props on `<Dock>` (`:582`–`:583`) with the new callback:

```svelte
      <Dock
        activeTool={tool}
        onToolChange={handleToolChange}
        onClear={() => (showClear = true)}
        clearDisabled={$model.nodes.length === 0}
        onOpenProperties={() => (centralPanel = { kind: "diagram" })}
      />
```

- [ ] **Step 3: Verify the web package type-checks and the suite passes**

Run: `pnpm --filter @uaml/web check`
Expected: no errors (no dangling references to `display`/`onDisplayChange`/`propsOpen` in Dock; `handleDisplayChange` is still referenced by the `CentralEditPanelHost` mount from Task 4, so it is not dead).
Run: `pnpm --filter @uaml/web test`
Expected: PASS (full web suite).

- [ ] **Step 4: Manual verification**

Run: `pnpm dev`. Open a diagram, click the Dock sliders button. Confirm the central panel opens titled **Diagram properties** with the live diagram still visible (dimmed) behind the scrim. Toggle **Show attributes** and the segmented controls; confirm the diagram re-renders live behind the panel. Dismiss via Esc / scrim / ×; confirm the diagram is intact and unchanged in position.

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/components/canvas/Dock.svelte packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(uaml): route Dock properties button to central panel diagram context"
```

---

## Task 6: Full green gate

**Files:** none (verification only).

- [ ] **Step 1: Run the full gate**

Run: `pnpm -r test && pnpm lint && pnpm build`
Expected: all green. `pnpm build` runs `svelte-check` across the web package (do not rely on a filtered web build alone — it can phantom-error against a stale `@uaml/okf` dist).

- [ ] **Step 2: Confirm no unrelated files were staged across the series**

Run: `git status`
Expected: working tree shows only the pre-existing UNRELATED modifications (`packages/core/src/state/ops-adapter.test.ts`, `packages/web/src/components/inspector/ObjectInspector.svelte`) still unstaged and untouched by this work; every central-panel file is committed.

---

## Self-Review Notes

- **Spec coverage:**
  - Central panel chrome (centered, enlarged, scrim, Esc/scrim/close dismissal, snippet body) → Task 1.
  - Body routed by selection kind (element → `ObjectInspector`; diagram → display fields) → Task 3.
  - Reuse existing field bodies verbatim (`ObjectInspector`, extracted display controls) → Tasks 2–3.
  - Diagram-properties trigger = Dock sliders button, popover deleted, controls lifted into panel (open-Q #2) → Tasks 2 & 5.
  - Two invocation contexts + backdrops → Tasks 4 (element) & 5 (diagram, live diagram behind scrim).
  - Edits dispatch existing update paths (`store.updateNode`, `handleDisplayChange`/`store.updateDiagram`) → Tasks 3–5.
  - Right-docked `InspectorPanel` stays / coexists (open-Q #4) → untouched; verified manually in Task 4 Step 5.
  - Selection interface (open-Q #3) = `onEditProperties(key)` → Task 4 Step 2.
  - Deleted-key guard → Task 3 (unknown key closes). Two-stage Esc → Task 1.
  - Package body reuse full `ObjectInspector` (open-Q #1 default): packages don't currently fire `onEditProperties`, so this is latent — no package-specific task needed until tree-nav routes packages through the callback. Documented in the spec's open-Q #3 resolution; no code here forecloses it (the host renders `ObjectInspector` for any element node key).
  - Backdrop for single-item context (open-Q #5): a scrim over whatever is currently rendered; a truly neutral empty backdrop is latent until tree-nav deletes `__all__`. The scrim path already satisfies "centered, not edge-to-edge."
- **Placeholder scan:** every code step contains complete component/test source; no TBD/TODO/"handle edge cases".
- **Type consistency:** `CentralPanelState` defined once (Task 3 module script), imported by Task 4. `onUpdateNode(key, patch)` matches `store.updateNode` and `ObjectInspector`'s `onUpdate` shape. `DiagramPropertiesBody` props `{ display, onChange }` used identically in Dock (Task 2) and the host (Task 3). `onDisplayChange` / `handleDisplayChange` naming consistent across Tasks 3–5.
