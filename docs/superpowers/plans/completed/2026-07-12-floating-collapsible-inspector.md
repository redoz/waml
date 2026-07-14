# Floating Inspector — Picker + Never-Close Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the inspector into a floating, collapsible, never-closing card styled like the diagram properties dialog, with an element **picker** in its header; an empty selection rests as a compact bar + hint.

**Architecture:** Two units. (1) `InspectorPanel.svelte` becomes a detached rounded card: local `collapsed` state, a header with kind-icon + element `<select>` picker + collapse chevron + pin, no close, no `open`. It renders its `children` body only when something is focused, otherwise a slim hint. (2) `CanvasInner.svelte` (sole host) always mounts the panel, removes the Inspect edge-flag + open/close state, and derives the picker options + selection wiring from the focused selection.

**Tech Stack:** Svelte 5 runes (`$state`, `$derived`, `$props`, `$bindable`), Tailwind utility classes, lucide-svelte icons, Vitest + @testing-library/svelte.

## Global Constraints

- Svelte 5 runes syntax only — match the existing files.
- Styling via Tailwind utility class strings on elements (no CSS modules / stylesheets).
- Design tokens, verbatim: border `#d8dee8`, accent `#1e88e5`, accent-bg `#e6f1fb`, hover-bg `#f1f3f7`, dialog shadow `0_8px_24px_rgba(15,23,42,0.14)`, radius `rounded-xl` (12px).
- The panel's `<aside>` region `aria-label` is the constant string `"Inspector"`.
- The panel never unmounts on empty selection — it always renders.
- Run web unit tests with: `pnpm -C packages/web exec vitest run <path>` from the repo root.

---

### Task 1: InspectorPanel — floating card, element picker, collapse, hint, no close/open

**Files:**
- Modify: `packages/web/src/components/inspector/InspectorPanel.svelte`
- Test: `packages/web/src/components/inspector/InspectorPanel.test.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces — `InspectorPanel` props after this task:
  - `options: { key: string; label: string }[]` — picker entries (diagram member nodes)
  - `selectedKey: string | null` — focused node key (null when an edge / nothing is focused)
  - `focusedKind: "node" | "edge" | undefined` — drives the kind-icon + body/collapse availability
  - `onSelect: (key: string | null) => void` — focus a node from the picker (null clears)
  - `pinned?: boolean` (default `false`)
  - `onTogglePin: () => void`
  - `hideDelay?: number` (default `250`)
  - `width?: number` (`$bindable`, default `380`)
  - `children?: Snippet`
  - **Removed:** `open`, `onClose`, `title`.
  - Behavior: `<aside aria-label="Inspector">` always renders. `hasSelection = focusedKind !== undefined`. Body renders `children` only when `hasSelection && !collapsed`; when `!hasSelection` a slim hint renders instead (children never rendered). Collapse chevron + kind-icon appear only when `hasSelection` / `focusedKind` set. Resize handle hidden when collapsed or unselected.

- [ ] **Step 1: Rewrite the test file**

Replace the entire contents of `packages/web/src/components/inspector/InspectorPanel.test.ts` with:

```ts
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import InspectorPanel from "./InspectorPanel.svelte";

const OPTIONS = [
  { key: "customer", label: "Customer" },
  { key: "order", label: "Order" },
];

function setup(props: Record<string, unknown> = {}) {
  return render(InspectorPanel, {
    props: {
      options: OPTIONS,
      selectedKey: "customer",
      focusedKind: "node",
      onSelect: vi.fn(),
      pinned: false,
      onTogglePin: vi.fn(),
      ...props,
    },
  });
}

describe("InspectorPanel", () => {
  it("always renders the region (never closes)", () => {
    setup({ selectedKey: null, focusedKind: undefined });
    expect(screen.getByRole("complementary", { name: "Inspector" })).toBeTruthy();
  });

  it("renders the picker options and reflects the selected key", () => {
    setup({ selectedKey: "order" });
    const select = screen.getByRole("combobox", { name: "Select element" }) as HTMLSelectElement;
    expect(select.value).toBe("order");
    expect(screen.getByRole("option", { name: "Customer" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Order" })).toBeTruthy();
  });

  it("fires onSelect with the chosen key", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    const select = screen.getByRole("combobox", { name: "Select element" });
    await fireEvent.change(select, { target: { value: "order" } });
    expect(onSelect).toHaveBeenCalledWith("order");
  });

  it("fires onSelect(null) when the placeholder is chosen", async () => {
    const onSelect = vi.fn();
    setup({ onSelect });
    const select = screen.getByRole("combobox", { name: "Select element" });
    await fireEvent.change(select, { target: { value: "" } });
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("with nothing focused: shows a hint, no collapse control, no kind icon", () => {
    const { container } = setup({ selectedKey: null, focusedKind: undefined });
    expect(screen.getByText(/select an element to edit/i)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /collapse inspector/i })).toBeNull();
    expect(container.querySelector(".inspector-kind")).toBeNull();
  });

  it("with a node focused: shows the kind icon and a collapse control, no hint", () => {
    const { container } = setup({ focusedKind: "node" });
    expect(container.querySelector(".inspector-kind svg")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Collapse inspector" })).toBeTruthy();
    expect(screen.queryByText(/select an element to edit/i)).toBeNull();
  });

  it("collapse toggle flips aria-expanded and its label", async () => {
    setup({ focusedKind: "node" });
    const collapse = screen.getByRole("button", { name: "Collapse inspector" });
    expect(collapse.getAttribute("aria-expanded")).toBe("true");
    await fireEvent.click(collapse);
    const expand = screen.getByRole("button", { name: "Expand inspector" });
    expect(expand.getAttribute("aria-expanded")).toBe("false");
  });

  it("fires onTogglePin when the pin control is clicked", async () => {
    const onTogglePin = vi.fn();
    setup({ onTogglePin });
    await fireEvent.click(screen.getByRole("button", { name: /pin inspector/i }));
    expect(onTogglePin).toHaveBeenCalledTimes(1);
  });

  it("is opaque when unpinned and translucent when pinned + idle", () => {
    setup({ pinned: false });
    expect(screen.getByRole("complementary").classList.contains("opacity-40")).toBe(false);
    setup({ pinned: true });
    const asides = screen.getAllByRole("complementary");
    expect(asides[asides.length - 1].classList.contains("opacity-40")).toBe(true);
  });

  it("becomes opaque on hover, translucent again after the pointer leaves", async () => {
    setup({ pinned: true, hideDelay: 20 });
    const aside = screen.getByRole("complementary");
    expect(aside.classList.contains("opacity-40")).toBe(true);
    await fireEvent.pointerEnter(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);
    await fireEvent.pointerLeave(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);
    await new Promise((r) => setTimeout(r, 40));
    await tick();
    expect(aside.classList.contains("opacity-40")).toBe(true);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C packages/web exec vitest run src/components/inspector/InspectorPanel.test.ts`
Expected: FAIL — no `combobox`/picker, no hint text, `focusedKind`/`options` props unhandled.

- [ ] **Step 3: Rewrite the script block**

In `packages/web/src/components/inspector/InspectorPanel.svelte`, change the icon import (line 8) to:

```ts
import { Pin, PinOff, ChevronUp, Box, Spline } from "lucide-svelte";
```

Replace the `$props` destructure + type (lines 12–31) with:

```ts
let {
  options,
  selectedKey,
  focusedKind,
  onSelect,
  pinned = false,
  onTogglePin,
  hideDelay = 250,
  width = $bindable(380),
  children,
}: {
  options: { key: string; label: string }[];
  selectedKey: string | null;
  focusedKind: "node" | "edge" | undefined;
  onSelect: (key: string | null) => void;
  pinned?: boolean;
  onTogglePin: () => void;
  /** Delay (ms) before re-dimming after the pointer leaves — avoids flicker. */
  hideDelay?: number;
  width?: number;
  children?: Snippet;
} = $props();
```

Add the collapse + selection-presence state next to `engaged` (after line 35, `let engaged = $state(false);`):

```ts
// Collapsed hides the body, leaving just the header bar. Local + not persisted.
let collapsed = $state(false);
// Whether any element is focused — drives body-vs-hint + collapse/icon affordances.
const hasSelection = $derived(focusedKind !== undefined);
```

- [ ] **Step 4: Rewrite the markup**

Replace the entire `{#if open} … {/if}` block (lines 93–138) with (note: no `{#if open}` wrapper — the panel always renders):

```svelte
<aside
  aria-label="Inspector"
  style={`width: ${width}px`}
  class={`absolute top-3 right-3 max-w-[calc(100%-24px)] max-h-[calc(100%-24px)] bg-white border border-[#d8dee8] rounded-xl overflow-hidden
    shadow-[0_8px_24px_rgba(15,23,42,0.14)] z-[16] flex flex-col transition-opacity duration-200 ${translucent ? "opacity-40" : "opacity-100"}`}
  onpointerenter={engage}
  onpointerleave={disengage}
  onfocusin={engage}
  onfocusout={disengage}
>
  <!-- Left-edge drag handle to resize (only when a body is shown) -->
  {#if hasSelection && !collapsed}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onmousedown={onResizeMouseDown}
      title="Drag to resize"
      class="absolute left-0 top-0 bottom-0 w-[6px] -ml-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
    ></div>
  {/if}

  <div class={`flex items-center gap-2 p-4 ${hasSelection && !collapsed ? "border-b border-[#d8dee8]" : ""}`}>
    {#if focusedKind}
      <span class="inspector-kind flex-none w-[26px] h-[26px] flex items-center justify-center rounded-md text-[#1e88e5] bg-[#e6f1fb]">
        {#if focusedKind === "node"}
          <Box size={15} />
        {:else}
          <Spline size={15} />
        {/if}
      </span>
    {/if}
    <select
      aria-label="Select element"
      value={selectedKey ?? ""}
      onchange={(e) => onSelect(e.currentTarget.value || null)}
      class="flex-1 min-w-0 text-[14px] font-semibold text-slate-900 bg-transparent border-0 focus:outline-none focus:ring-2 focus:ring-[#e6f1fb] rounded-md py-1 cursor-pointer"
    >
      <option value="">Select an element…</option>
      {#each options as opt (opt.key)}
        <option value={opt.key}>{opt.label}</option>
      {/each}
    </select>
    {#if hasSelection}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand inspector" : "Collapse inspector"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand inspector" : "Collapse inspector"}
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <span class={`flex transition-transform duration-200 ${collapsed ? "rotate-180" : ""}`}>
          <ChevronUp size={16} />
        </span>
      </button>
    {/if}
    <button
      onclick={onTogglePin}
      aria-label={pinned ? "Unpin inspector" : "Pin inspector"}
      aria-pressed={pinned}
      title={pinned ? "Unpin inspector" : "Pin inspector"}
      class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
    >
      {#if pinned}
        <PinOff size={16} />
      {:else}
        <Pin size={16} />
      {/if}
    </button>
  </div>

  {#if hasSelection}
    {#if !collapsed}
      <div class="p-4 overflow-y-auto flex-1 min-h-0">{@render children?.()}</div>
    {/if}
  {:else}
    <div class="px-4 pb-4 text-[13px] text-slate-500">Select an element to edit.</div>
  {/if}
</aside>
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `pnpm -C packages/web exec vitest run src/components/inspector/InspectorPanel.test.ts`
Expected: PASS — all InspectorPanel tests green.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/inspector/InspectorPanel.svelte packages/web/src/components/inspector/InspectorPanel.test.ts
git commit -m "feat(web): inspector as floating never-closing card with element picker"
```

---

### Task 2: Host wiring — always-mount, remove Inspect flag, derive picker + selection

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Test: `packages/web/src/components/canvas/Canvas.test.ts`

**Interfaces:**
- Consumes: `InspectorPanel` props from Task 1 — `options`, `selectedKey`, `focusedKind`, `onSelect`, `pinned`, `onTogglePin`, `width`, `children`.
- Consumes existing host state: `focused` (`{ type: "node" | "edge"; id: string } | null`), `$model.nodes` (`{ key: string; title: string }[]`), `memberSet` (`Set<string>` of active-diagram member keys), `selectionSet`, `EMPTY_SELECTION`, `inspectorPinned`, `inspectorWidth`.
- Produces: nothing consumed downstream.

- [ ] **Step 1: Rewrite the affected Canvas.test.ts sections**

The Inspect flag is gone and the panel is always present. In `packages/web/src/components/canvas/Canvas.test.ts`:

Delete the test `"renders an Inspect flag as a toggle button (the right icon rail is gone)"` (lines 46–55).

Replace the entire `describe("Inspect flag + pinnable Inspector", …)` block (lines 64–97) with:

```ts
describe("pinnable Inspector (always present, never closes)", () => {
  it("is always mounted, even with nothing selected", () => {
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    expect(panel).toBeTruthy();
    // Nothing selected → shows the hint and the element picker.
    expect(within(panel).getByText(/select an element to edit/i)).toBeTruthy();
    expect(within(panel).getByRole("combobox", { name: "Select element" })).toBeTruthy();
  });

  it("exposes a pin control that makes the panel translucent while idle", async () => {
    render(Canvas);
    const panel = screen.getByRole("complementary", { name: "Inspector" });
    expect(panel.classList.contains("opacity-40")).toBe(false);
    await fireEvent.click(within(panel).getByRole("button", { name: /pin inspector/i }));
    await tick();
    expect(panel.classList.contains("opacity-40")).toBe(true);
    await fireEvent.pointerEnter(panel);
    expect(panel.classList.contains("opacity-40")).toBe(false);
  });
});
```

- [ ] **Step 2: Run the canvas test to verify it fails**

Run: `pnpm -C packages/web exec vitest run src/components/canvas/Canvas.test.ts`
Expected: FAIL — the panel is still gated behind `open`/the Inspect flag, so `getByRole("complementary", { name: "Inspector" })` finds nothing at initial render.

- [ ] **Step 3: Remove the Inspector open/close state**

In `packages/web/src/components/canvas/CanvasInner.svelte`, delete the `inspectorOpen` state declaration (line 104, `let inspectorOpen = $state(false);`) and update the comment above it (lines 100–103) to drop the "Opened ONLY by the Inspect edge-flag" wording:

```ts
  // Inspector pin state. The panel is always mounted (never closes); an empty
  // selection rests as a compact bar + hint. When pinned it dims (translucent)
  // while idle, fading back opaque on hover/focus.
  let inspectorPinned = $state(false);
```

- [ ] **Step 4: Add the picker + selection deriveds**

Add these immediately after the `focused` derived (line 125, `const focused = $derived(focusedSelection(selectionSet));`):

```ts
  // Element picker entries: the active diagram's member nodes (objects + notes).
  const inspectorOptions = $derived(
    $model.nodes
      .filter((n) => memberSet.has(n.key))
      .map((n) => ({ key: n.key, label: n.title.trim() || "Untitled" })),
  );
  const inspectorSelectedKey = $derived(focused?.type === "node" ? focused.id : null);
  const inspectorFocusedKind = $derived(focused?.type);
```

- [ ] **Step 5: Remove the Inspect edge-flag and fix the Feedback offset**

Replace the Inspect + Feedback `EdgeFlag` block (lines 606–625) with just the Feedback flag, always offset by the panel width:

```svelte
    <!-- Right-edge Feedback flag; slides left by the panel width to stay clear
         of the always-present Inspector. -->
    <EdgeFlag
      label="Feedback"
      offset={62}
      rightOffset={inspectorWidth}
      href="https://github.com/redoz/uaml/issues/new"
    >
      {#snippet icon()}<MessageSquare size={16} />{/snippet}
    </EdgeFlag>
```

If `PanelRight` (imported for the removed Inspect flag icon) is now unused elsewhere in the file, remove it from its lucide-svelte import.

- [ ] **Step 6: Rewrite the InspectorPanel usage**

Replace the `<InspectorPanel …>` opening tag (lines 628–639) — keep the `<Inspector>` children unchanged — with:

```svelte
    <!-- Always-present floating Inspector (translucent when pinned + idle). -->
    <InspectorPanel
      options={inspectorOptions}
      selectedKey={inspectorSelectedKey}
      focusedKind={inspectorFocusedKind}
      onSelect={(key) => (selectionSet = key ? { nodes: [key], edges: [] } : EMPTY_SELECTION)}
      pinned={inspectorPinned}
      bind:width={inspectorWidth}
      onTogglePin={() => (inspectorPinned = !inspectorPinned)}
    >
```

- [ ] **Step 7: Run the canvas test to verify it passes**

Run: `pnpm -C packages/web exec vitest run src/components/canvas/Canvas.test.ts`
Expected: PASS — panel is always present with the hint + picker, and the pin control still drives translucency.

- [ ] **Step 8: Full green gate**

Run: `pnpm -C packages/web exec vitest run && pnpm -C packages/web lint && pnpm -C packages/web build`
Expected: PASS — all web tests, lint, and build green. No stray reference to `inspectorOpen`, the Inspect flag, `onClose`, or the removed `open`/`title` props.

- [ ] **Step 9: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.test.ts
git commit -m "feat(web): always-mount inspector, remove Inspect flag, wire element picker"
```

---

## Self-Review

**Spec coverage:**
- Floating card styling → Task 1 Step 4 `<aside>`. ✓
- Never close / always mounted → Task 1 (no `open`), Task 2 Steps 3,6. ✓
- Remove Inspect edge-flag; Feedback offset always by width → Task 2 Step 5. ✓
- Header picker (options = member nodes, placeholder, onSelect focuses) → Task 1 Step 4 `<select>` + Task 2 Step 4 deriveds + Step 6 `onSelect`. ✓
- Two-way reflect (canvas → picker via `selectedKey`) → Task 2 `inspectorSelectedKey`. ✓
- Kind-icon per focusedKind → Task 1 Step 4. ✓
- Collapse chevron only when focused; collapses body → Task 1 Step 4. ✓
- Empty → compact hint, no body/collapse → Task 1 Step 4 `{:else}` hint. ✓
- Node focused → ObjectInspector; edge focused → RelationshipInspector (unchanged child) → children rendered when `hasSelection`. ✓
- Pin + translucency unchanged → Task 1 keeps pin + `translucent`/`engage`/`disengage`. ✓
- No close → `onClose`/`X` removed. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code. ✓

**Type consistency:** `options: {key,label}[]`, `selectedKey: string|null`, `focusedKind: "node"|"edge"|undefined`, `onSelect: (key:string|null)=>void` defined in Task 1 match the host props in Task 2 (`inspectorOptions`, `inspectorSelectedKey = focused?.type==="node" ? focused.id : null`, `inspectorFocusedKind = focused?.type`, `onSelect` handler). Region name `"Inspector"` matches Task 2's `getByRole("complementary", { name: "Inspector" })`. `<select>` exposes role `combobox` named `"Select element"`, matched in both test files. ✓

**Deferred (out of scope, per design):** diagram-self picker entry + diagram-inspector body; relationships in the picker; note-vs-object visual distinction; collapse-state persistence.

**Behavior-change note:** with the Inspect flag + `onClose` gone, the panel can no longer be dismissed — it always occupies its width on the right (the "never close" tradeoff). The right Feedback flag always sits left of it.
