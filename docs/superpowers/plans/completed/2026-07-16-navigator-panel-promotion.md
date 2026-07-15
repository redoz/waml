# Navigator Panel Promotion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote the model Navigator tree from a dismissable TopBar dropdown into a persistent, near-full-height panel with two modes — an unpinned centered modal and a pinned left-docked rail.

**Architecture:** Extract the Navigator's tree/menus into a chrome-agnostic `NavigatorBody` that fills its host. Wrap it in a new `NavigatorPanel` that renders either the centered (CentralEditPanel-style, scrim) or left-docked (InspectorPanel-mirror, no scrim, resizable, translucent-when-idle, collapsible) chrome. Move the mount and its state from `TopBar` up to `CanvasInner`; the TopBar switcher button becomes a pure open/close trigger.

**Tech Stack:** Svelte 5 (runes: `$state`, `$props`, `$derived`, `$effect`, `$bindable`), TailwindCSS utility classes, lucide-svelte icons, Vitest + @testing-library/svelte.

## Global Constraints

- Svelte 5 runes only; match the runes/idioms already in `Navigator.svelte`, `InspectorPanel.svelte`, `CentralEditPanel.svelte`.
- All Navigator mutations stay callback-driven (presentational component, unit-tested in isolation) — no store access inside `NavigatorBody`/`NavigatorPanel`.
- Panel state is **session-local** (`$state` in `CanvasInner`) — no localStorage; resets on reload, exactly like `inspectorPinned`/`inspectorWidth`.
- Preserve every existing Navigator prop/callback name and signature: `onScope`, `onSelectDiagram`, `onReorder`, `onViewInDiagram`, `onAddToNewDiagram`, `onEditProperties`, `onCreatePackage`, `onCreateNode`, `onCreateDiagram`, `onRename`, `onSort`, `onDelete`.
- The gate that must stay green: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Keep Svelte a11y lint clean (carry over the existing `svelte-ignore` comments on scrim/backdrop divs).
- Colors/shadows/radii: reuse the exact tokens already in the two chrome components (`border-[#d8dee8]`, `bg-slate-900/30`, `shadow-[0_8px_24px_rgba(15,23,42,0.14)]`, `shadow-[0_16px_48px_rgba(15,23,42,0.22)]`, `rounded-2xl`/`rounded-xl`, `opacity-40`).

---

### Task 1: Extract `NavigatorBody` (chrome-agnostic tree body)

Pure refactor. Move the entire Navigator implementation into a body component that fills its host; leave `Navigator.svelte` as a thin wrapper so `TopBar` and its test stay green. Repoint the body-behavior tests at the new component.

**Files:**
- Create: `packages/web/src/components/NavigatorBody.svelte`
- Modify: `packages/web/src/components/Navigator.svelte` (becomes a thin wrapper)
- Rename + modify: `packages/web/src/components/Navigator.test.ts` → `packages/web/src/components/NavigatorBody.test.ts` (repoint import)

**Interfaces:**
- Consumes: `@waml/core/nav/tree` (`buildNavTree`, `packageOf`, `NavRow`, `NavKind`), `@waml/core/nav/search` (`filterNav`, `matchSpan`), `@waml/okf` (`ModelGraph`) — unchanged from `Navigator.svelte`.
- Produces: `NavigatorBody` — same `$props()` shape as today's `Navigator.svelte` (all props/callbacks listed in Global Constraints). Root element is `relative flex flex-col h-full min-h-0` (no fixed width, no `max-h`, no outer card border/shadow/bg — the host supplies chrome). The tree scroll region is `flex-1 min-h-0 overflow-y-auto` (replacing `max-h-[420px]`).

- [ ] **Step 1: Create `NavigatorBody.svelte` by moving the implementation**

Copy the **entire** current contents of `Navigator.svelte` into `NavigatorBody.svelte`, then apply exactly these three edits to the copy:

1. Root wrapper — replace the opening element (current `Navigator.svelte:201-205`):
```svelte
<div
  role="menu"
  tabindex="-1"
  class="w-[300px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] py-1 text-[13px]"
>
```
with:
```svelte
<div role="menu" tabindex="-1" class="relative flex flex-col h-full min-h-0 py-1 text-[13px] text-slate-900">
```

2. Tree scroll region — replace the tree container (current `Navigator.svelte:264`):
```svelte
  <div class="max-h-[420px] overflow-y-auto py-0.5">
```
with:
```svelte
  <div class="flex-1 min-h-0 overflow-y-auto py-0.5">
```

3. Leave everything else byte-for-byte identical (search row, breadcrumb, snippets, tree rows, `actionMenu`/`ctxMenu`/`deletePrompt` overlays, and the full `<script>`). The absolute-positioned menus keep `left-1/2 -translate-x-1/2 top-[120px]`; they now anchor to the `relative` root and center within whatever host width applies.

- [ ] **Step 2: Rewrite `Navigator.svelte` as a thin wrapper**

Replace the entire file with a wrapper that preserves the old fixed-card look by delegating to `NavigatorBody`:
```svelte
<script lang="ts">
  // Thin back-compat wrapper: the old fixed-width dropdown card around the
  // extracted NavigatorBody. Kept only until CanvasInner mounts NavigatorPanel
  // directly (then this file is removed).
  import NavigatorBody from "./NavigatorBody.svelte";
  import type { ModelGraph } from "@waml/okf";
  import type { NavKind } from "@waml/core/nav/tree";

  let props: {
    graph: ModelGraph;
    scopeKey?: string;
    activeDiagramKey?: string;
    palette?: string[];
    onScope?: (key: string) => void;
    onSelectDiagram?: (key: string) => void;
    onReorder?: (pkgKey: string, order: string[]) => void;
    onViewInDiagram?: (key: string, diagramKey: string) => void;
    onAddToNewDiagram?: (key: string) => void;
    onEditProperties?: (key: string) => void;
    onCreatePackage?: (parentKey: string, name: string) => void;
    onCreateNode?: (dir: string, metaclass: string) => void;
    onCreateDiagram?: (name: string) => void;
    onRename?: (key: string, kind: NavKind, title: string) => void;
    onSort?: (pkgKey: string) => void;
    onDelete?: (key: string, kind: NavKind, mode: "single" | "cascade" | "reparent") => void;
  } = $props();
</script>

<div class="w-[300px] h-[440px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)] overflow-hidden">
  <NavigatorBody {...props} />
</div>
```

- [ ] **Step 3: Rename and repoint the body-behavior tests**

Rename `packages/web/src/components/Navigator.test.ts` to `packages/web/src/components/NavigatorBody.test.ts` and change only the import + `render` target from `Navigator` to `NavigatorBody`:
```ts
import NavigatorBody from "./NavigatorBody.svelte";
// ...every `render(Navigator, ...)` becomes `render(NavigatorBody, ...)`
```
All assertions (roles, `Search model` field, breadcrumb text, `<mark>`, context menus, delete prompt) are unchanged — they exercise the body's DOM, which is identical.

- [ ] **Step 4: Run the web test suite**

Run: `pnpm -r test --filter @waml/web` (or `pnpm --filter @waml/web test`)
Expected: PASS — `NavigatorBody.test.ts` green (all migrated cases), and `TopBar.test.ts` still green because the wrapper renders the same `Search model` field and `acme-model` text the TopBar test asserts.

- [ ] **Step 5: Lint + build**

Run: `pnpm lint && pnpm build`
Expected: PASS, no a11y regressions.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/NavigatorBody.svelte packages/web/src/components/NavigatorBody.test.ts packages/web/src/components/Navigator.svelte
git rm packages/web/src/components/Navigator.test.ts
git commit -m "refactor(web): extract NavigatorBody from Navigator dropdown"
```

---

### Task 2: Build `NavigatorPanel` (two-mode chrome)

A new host that renders `NavigatorBody` in either centered-modal or left-docked chrome, with a pin button toggling between them, unified Esc-to-close, docked resize + translucency + collapse.

**Files:**
- Create: `packages/web/src/components/NavigatorPanel.svelte`
- Create: `packages/web/src/components/NavigatorPanel.test.ts`

**Interfaces:**
- Consumes: `NavigatorBody` (Task 1) and its full prop set; `lucide-svelte` (`Pin`, `PinOff`, `ChevronUp`).
- Produces: `NavigatorPanel` with props:
  - `open: boolean` — renders nothing when false.
  - `mode: "centered" | "docked"`.
  - `width = $bindable(340)` — docked width (px).
  - `collapsed = $bindable(false)` — docked body fold.
  - `title: string` — panel header label + dialog aria-label.
  - `onClose: () => void`, `onToggleMode: () => void` (pin).
  - All `NavigatorBody` passthrough callbacks/props (Global Constraints list) + `graph`, `scopeKey`, `activeDiagramKey`, `palette`.

- [ ] **Step 1: Write the failing test file**

Create `packages/web/src/components/NavigatorPanel.test.ts`:
```ts
import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import NavigatorPanel from "./NavigatorPanel.svelte";
import type { ModelGraph } from "@waml/okf";

const node = (key: string, title: string, type = "uml.Class") => ({
  key, type, concept: { id: key, type, title, body: "" },
  stereotypes: [], attributes: [], position: { x: 0, y: 0 },
});
const graph = {
  path: "acme-model",
  nodes: [node("customer", "Customer")],
  edges: [],
  diagrams: [{ key: "overview", title: "Sales overview", profile: "uml-domain", members: [] }],
  packages: [
    { ...node("", "", "uml.Package"), members: ["sales"] },
    { ...node("sales", "sales", "uml.Package"), members: ["overview", "customer"] },
  ],
} as unknown as ModelGraph;

const props = (over = {}) => ({
  open: true, mode: "centered" as const, title: "acme-model",
  graph, scopeKey: "sales", activeDiagramKey: "overview", palette: ["uml.Class"],
  onClose: vi.fn(), onToggleMode: vi.fn(), onScope: vi.fn(), onSelectDiagram: vi.fn(),
  ...over,
});

test("renders nothing when closed", () => {
  render(NavigatorPanel, { props: props({ open: false }) });
  expect(screen.queryByLabelText("Search model")).toBeNull();
});

test("centered mode mounts the body and a dismissing scrim", () => {
  render(NavigatorPanel, { props: props() });
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  expect(screen.getByRole("dialog", { name: "acme-model" })).toBeTruthy();
});

test("pin button fires onToggleMode", async () => {
  const onToggleMode = vi.fn();
  render(NavigatorPanel, { props: props({ onToggleMode }) });
  await fireEvent.click(screen.getByRole("button", { name: /pin to left|dock/i }));
  expect(onToggleMode).toHaveBeenCalledTimes(1);
});

test("close button fires onClose", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByRole("button", { name: /^close$/i }));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Escape closes when no input is focused", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("first Escape blurs a focused input, second closes", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  const input = screen.getByLabelText("Search model") as HTMLInputElement;
  input.focus();
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).not.toHaveBeenCalled();
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("docked mode exposes a resize handle and unpin control", () => {
  render(NavigatorPanel, { props: props({ mode: "docked" }) });
  expect(screen.getByLabelText("Model navigator")).toBeTruthy();
  expect(screen.getByRole("button", { name: /unpin|center/i })).toBeTruthy();
  expect(screen.getByTestId("nav-resize")).toBeTruthy();
});

test("docked collapse toggle hides the body", async () => {
  render(NavigatorPanel, { props: props({ mode: "docked" }) });
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /collapse/i }));
  expect(screen.queryByLabelText("Search model")).toBeNull();
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web test -- NavigatorPanel`
Expected: FAIL — `Failed to resolve import "./NavigatorPanel.svelte"`.

- [ ] **Step 3: Implement `NavigatorPanel.svelte`**

Create `packages/web/src/components/NavigatorPanel.svelte`:
```svelte
<script lang="ts">
  // Two-mode host for the model navigator. Unpinned = centered modal over a
  // dismissing scrim (CentralEditPanel treatment). Pinned = left-docked rail
  // mirroring InspectorPanel (no scrim, right-edge resize, translucent-when-idle,
  // collapsible). The pin button toggles the two modes; Esc / scrim / close
  // dismiss. State is owned by the caller (CanvasInner) and passed in.
  import { Pin, PinOff, ChevronUp } from "lucide-svelte";
  import NavigatorBody from "./NavigatorBody.svelte";
  import type { ModelGraph } from "@waml/okf";
  import type { NavKind } from "@waml/core/nav/tree";

  const MIN_WIDTH = 300;

  let {
    open,
    mode,
    width = $bindable(340),
    collapsed = $bindable(false),
    title,
    onClose,
    onToggleMode,
    graph,
    scopeKey = "",
    activeDiagramKey = "",
    palette = [],
    onScope,
    onSelectDiagram,
    onReorder,
    onViewInDiagram,
    onAddToNewDiagram,
    onEditProperties,
    onCreatePackage,
    onCreateNode,
    onCreateDiagram,
    onRename,
    onSort,
    onDelete,
  }: {
    open: boolean;
    mode: "centered" | "docked";
    width?: number;
    collapsed?: boolean;
    title: string;
    onClose: () => void;
    onToggleMode: () => void;
    graph: ModelGraph;
    scopeKey?: string;
    activeDiagramKey?: string;
    palette?: string[];
    onScope?: (key: string) => void;
    onSelectDiagram?: (key: string) => void;
    onReorder?: (pkgKey: string, order: string[]) => void;
    onViewInDiagram?: (key: string, diagramKey: string) => void;
    onAddToNewDiagram?: (key: string) => void;
    onEditProperties?: (key: string) => void;
    onCreatePackage?: (parentKey: string, name: string) => void;
    onCreateNode?: (dir: string, metaclass: string) => void;
    onCreateDiagram?: (name: string) => void;
    onRename?: (key: string, kind: NavKind, title: string) => void;
    onSort?: (pkgKey: string) => void;
    onDelete?: (key: string, kind: NavKind, mode: "single" | "cascade" | "reparent") => void;
  } = $props();

  // Bundle the body passthrough once so both mode branches stay DRY.
  const body = $derived({
    graph, scopeKey, activeDiagramKey, palette,
    onScope, onSelectDiagram, onReorder, onViewInDiagram, onAddToNewDiagram,
    onEditProperties, onCreatePackage, onCreateNode, onCreateDiagram,
    onRename, onSort, onDelete,
  });

  let card = $state<HTMLElement | null>(null);

  // Docked translucency: solid while engaged (pointer over / focus inside), dim
  // when idle. Mirrors InspectorPanel.
  let engaged = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  const translucent = $derived(mode === "docked" && !engaged);
  function engage() {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = undefined; }
    engaged = true;
  }
  function disengage() {
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => { engaged = false; hideTimer = undefined; }, 250);
  }

  // Right-edge resize (mirror of InspectorPanel's left-edge drag: dragging right
  // widens, so delta = current - start).
  let resizing = false;
  let startX = 0;
  let startWidth = 0;
  function onResizeMouseDown(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    startX = e.clientX;
    startWidth = width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }
  $effect(() => {
    function onMouseMove(e: MouseEvent) {
      if (!resizing) return;
      const delta = e.clientX - startX;
      width = Math.min(window.innerWidth * 0.6, Math.max(MIN_WIDTH, startWidth + delta));
    }
    function onMouseUp() {
      if (!resizing) return;
      resizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    }
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  });

  // Unified Esc: while open, first Esc blurs a focused inner input (protecting an
  // in-progress inline create/rename), otherwise closes. Works in both modes.
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (!open || e.key !== "Escape") return;
      const active = document.activeElement as HTMLElement | null;
      const editing =
        !!active && ["INPUT", "TEXTAREA", "SELECT"].includes(active.tagName);
      if (editing) {
        active!.blur();
        e.stopPropagation();
      } else {
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Move focus into the centered card on open so it reads as a dialog.
  $effect(() => {
    if (open && mode === "centered") card?.focus();
  });
</script>

{#snippet header(docked: boolean)}
  <div class="px-4 py-[13px] border-b border-[#d8dee8] flex items-center gap-2 flex-shrink-0 bg-white">
    <h2 class="text-[14px] font-[650] flex-1 min-w-0 truncate text-slate-900">{title}</h2>
    {#if docked}
      <button
        onclick={() => (collapsed = !collapsed)}
        aria-label={collapsed ? "Expand navigator" : "Collapse navigator"}
        aria-expanded={!collapsed}
        title={collapsed ? "Expand navigator" : "Collapse navigator"}
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
      >
        <span class={`flex transition-transform duration-200 ${collapsed ? "rotate-180" : ""}`}>
          <ChevronUp size={16} />
        </span>
      </button>
    {/if}
    <button
      onclick={onToggleMode}
      aria-label={docked ? "Unpin navigator (center)" : "Pin navigator to left"}
      aria-pressed={docked}
      title={docked ? "Unpin navigator (center)" : "Pin navigator to left"}
      class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${docked ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
    >
      {#if docked}<PinOff size={16} />{:else}<Pin size={16} />{/if}
    </button>
    <button
      onclick={onClose}
      aria-label="Close"
      title="Close"
      class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7] text-[20px] leading-none"
    >
      ×
    </button>
  </div>
{/snippet}

{#if open}
  {#if mode === "centered"}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      data-testid="nav-scrim"
      onclick={onClose}
      class="fixed inset-0 z-[60] flex items-center justify-center bg-slate-900/30 p-4"
      style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        bind:this={card}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        class="relative w-full max-w-[620px] h-[95vh] max-h-[95vh] flex flex-col rounded-2xl border border-[#d8dee8] bg-white shadow-[0_16px_48px_rgba(15,23,42,0.22)] overflow-hidden"
      >
        {@render header(false)}
        <div class="flex-1 min-h-0 overflow-hidden">
          <NavigatorBody {...body} />
        </div>
      </div>
    </div>
  {:else}
    <aside
      aria-label="Model navigator"
      style={`width: ${width}px`}
      class={`absolute top-3 left-3 max-w-[calc(100%-24px)] max-h-[calc(100%-24px)] bg-white border border-[#d8dee8] rounded-xl overflow-hidden shadow-[0_8px_24px_rgba(15,23,42,0.14)] z-[16] flex flex-col transition-opacity duration-200 ${translucent ? "opacity-40" : "opacity-100"}`}
      onpointerenter={engage}
      onpointerleave={disengage}
      onfocusin={engage}
      onfocusout={disengage}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        data-testid="nav-resize"
        onmousedown={onResizeMouseDown}
        title="Drag to resize"
        class="absolute right-0 top-0 bottom-0 w-[6px] -mr-[3px] cursor-col-resize z-[17] hover:bg-[#1e88e5]/20"
      ></div>
      {@render header(true)}
      {#if !collapsed}
        <div class="flex-1 min-h-0 overflow-hidden">
          <NavigatorBody {...body} />
        </div>
      {/if}
    </aside>
  {/if}
{/if}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --filter @waml/web test -- NavigatorPanel`
Expected: PASS — all eight cases green.

- [ ] **Step 5: Lint + build**

Run: `pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/NavigatorPanel.svelte packages/web/src/components/NavigatorPanel.test.ts
git commit -m "feat(web): NavigatorPanel — two-mode centered/left-docked chrome"
```

---

### Task 3: Wire `NavigatorPanel` into `CanvasInner`; slim `TopBar` to a trigger

Move the mount + state up to `CanvasInner`, turn the TopBar switcher into an open/close trigger, delete the dead `Navigator.svelte` wrapper, and update `TopBar.test.ts`.

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (state block ~118-125; TopBar mount 611-664; canvas row ~739-914)
- Modify: `packages/web/src/components/TopBar.svelte` (props/state/switcher block 172-209)
- Modify: `packages/web/src/components/TopBar.test.ts` (switcher test 58-75)
- Delete: `packages/web/src/components/Navigator.svelte`

**Interfaces:**
- Consumes: `NavigatorPanel` (Task 2).
- Produces: `TopBar` new props `navOpen?: boolean`, `onToggleNav?: () => void`; the switcher button calls `onToggleNav` and reflects `navOpen` via `aria-expanded`. `CanvasInner` new session state `navOpen`, `navMode`, `navWidth`, `navCollapsed`.

- [ ] **Step 1: Update `TopBar.test.ts` switcher expectation (write the failing test first)**

Replace the test at `TopBar.test.ts:58-75` ("the center switcher opens the Navigator sheet…") with:
```ts
test("the center switcher toggles the navigator via onToggleNav + aria-expanded", async () => {
  const onToggleNav = vi.fn();
  render(TopBar, {
    props: {
      diagrams: [diagram("d1", "Overview")],
      activeDiagramKey: "d1",
      navOpen: false,
      onToggleNav,
    },
  });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
  await fireEvent.click(btn);
  expect(onToggleNav).toHaveBeenCalledTimes(1);
  // The navigator no longer mounts inside the TopBar.
  expect(screen.queryByLabelText("Search model")).toBeNull();
});
```
Also delete the now-unused `navGraph` fixture (`TopBar.test.ts:13-31`).

- [ ] **Step 2: Run it to verify it fails**

Run: `pnpm --filter @waml/web test -- TopBar`
Expected: FAIL — `onToggleNav` is not a prop yet (still renders the old sheet; `Search model` present).

- [ ] **Step 3: Slim down `TopBar.svelte`**

In `packages/web/src/components/TopBar.svelte`:

1. Remove the `Navigator` import (`TopBar.svelte:7`).
2. In `$props()` (15-70), **remove** these Navigator-only props (both destructure and type): `graph`, `scopeKey`, `palette`, `onScope`, `onReorder`, `onViewInDiagram`, `onAddToNewDiagram`, `onEditProperties`, `onCreatePackage`, `onCreateNode`, `onRename`, `onSort`, `onDelete`, `onSelectDiagram`, `onCreateDiagram`. **Keep** `diagrams`, `activeDiagramKey` (needed for `activeTitle`). **Add**:
```ts
    navOpen = false,
    onToggleNav,
```
and to the type block:
```ts
    navOpen?: boolean;
    onToggleNav?: () => void;
```
3. Remove the switcher state/handlers (`switcherOpen` at 81, `openSwitcher` 87-89, `selectDiagram` 91-94, `viewInDiagram` 95-98, `addToNewDiagram` 99-102, `editProperties` 103-106, `createDiagram` 107-110). Keep `activeTitle` (83-85).
4. Replace the switcher markup + mounted sheet (`TopBar.svelte:172-209`) with a pure trigger:
```svelte
  <div class="relative">
    <button
      onclick={() => onToggleNav?.()}
      aria-label={`Diagram: ${activeTitle} — switch diagram`}
      aria-haspopup="dialog"
      aria-expanded={navOpen}
      title="Open the model navigator"
      class="flex items-center gap-[6px] rounded-lg px-[10px] py-[6px] text-[13px] font-[600] cursor-pointer transition-colors text-[#1e88e5] bg-[#e6f1fb] hover:bg-[#d8e8f9]"
    >
      <span class="max-w-[240px] truncate">{activeTitle}</span>
      <ChevronDown size={14} class="text-[#1e88e5]/70" />
    </button>
  </div>
```

- [ ] **Step 4: Add navigator state + mount to `CanvasInner.svelte`**

1. Add the import near the other component imports (beside `import InspectorPanel from "../inspector/InspectorPanel.svelte";` at `CanvasInner.svelte:50`):
```svelte
  import NavigatorPanel from "../NavigatorPanel.svelte";
```
2. Add session state beside the inspector state (after `CanvasInner.svelte:121`):
```svelte
  // Navigator panel — session-local, like the inspector state above. `navMode`
  // is remembered across close/reopen; unpinning returns to "centered".
  let navOpen = $state(false);
  let navMode = $state<"centered" | "docked">("centered");
  let navWidth = $state(340);
  let navCollapsed = $state(false);
```
3. On the `<TopBar>` mount (611-664), **remove** the Navigator-only attributes that were passed to TopBar: `graph={$model}`, `scopeKey={scopeKey}`, `palette={palette}`, `onScope`, `onReorder`, `onSort`, `onCreatePackage`, `onCreateNode`, `onRename`, `onDelete`, `onViewInDiagram`, `onAddToNewDiagram`, `onEditProperties`, `onSelectDiagram`, `onCreateDiagram` — **but preserve every one of their handler bodies verbatim** by moving them onto the `<NavigatorPanel>` mount in the next sub-step. Keep on `<TopBar>`: `diagrams`, `activeDiagramKey`, and add:
```svelte
    navOpen={navOpen}
    onToggleNav={() => (navOpen = !navOpen)}
```
4. Mount `NavigatorPanel` inside the canvas row `<div class="flex flex-1 min-h-0 relative">` (the block starting `CanvasInner.svelte:739`), as a sibling of `<InspectorPanel>` (e.g. immediately before the `<InspectorPanel` at line 855), carrying the exact handler bodies removed from TopBar:
```svelte
    <NavigatorPanel
      open={navOpen}
      mode={navMode}
      bind:width={navWidth}
      bind:collapsed={navCollapsed}
      title={$model.path || "model"}
      onClose={() => (navOpen = false)}
      onToggleMode={() => (navMode = navMode === "centered" ? "docked" : "centered")}
      graph={$model}
      scopeKey={scopeKey}
      activeDiagramKey={activeDiagram.key}
      palette={palette}
      onScope={(key) => (scopeKey = key)}
      onSelectDiagram={(key) => {
        selectionSet = EMPTY_SELECTION;
        activeDiagramKey = key;
        navOpen = false;
      }}
      onReorder={(pkgKey, order) => store.reorderMembers(pkgKey, order)}
      onSort={(pkgKey) => store.sortPackage(pkgKey)}
      onCreatePackage={(parent, name) => store.createGhostPackage(parent, name)}
      onCreateNode={(dir, metaclass) => store.createNodeInPackage(dir, metaclass, metaclass.split(".").pop() || metaclass)}
      onRename={(key, kind, title) => {
        if (kind === "package") store.renamePackage(key, reslugPackage(key, title));
        else if (kind === "diagram") store.updateDiagram(key, { title });
        else {
          const n = $model.nodes.find((x) => x.key === key);
          if (n) store.updateNode(key, { concept: { ...n.concept, title } });
        }
      }}
      onDelete={(key, kind, mode) => {
        if (kind === "package") store.deletePackage(key, mode === "cascade");
        else store.removeNode(key);
      }}
      onViewInDiagram={(key, diagramKey) => {
        activeDiagramKey = diagramKey;
        selectionSet = { nodes: [key], edges: [] };
        navOpen = false;
      }}
      onAddToNewDiagram={(key) => {
        const d = store.addDiagramFromMembers("New diagram", [key]);
        activeDiagramKey = d.key;
        navOpen = false;
      }}
      onEditProperties={(key) => {
        centralPanel = { kind: "element", nodeKey: key };
        navOpen = false;
      }}
    />
```
Note: `onSelectDiagram` here replicates the diagram-switch guard the TopBar mount used (clear selection so it never crosses into a read-only Flow/Sequence view — see the existing `onSelectDiagram` handler body at `CanvasInner.svelte:620-634`; copy its real body verbatim rather than the abbreviated version above if it differs). Navigation actions close the panel (`navOpen = false`); structural edits (create/rename/reorder/sort/delete) leave it open so the user can chain them.

- [ ] **Step 5: Delete the dead wrapper**

```bash
git rm packages/web/src/components/Navigator.svelte
```
Confirm nothing else imports it:

Run: `grep -rn "components/Navigator\.svelte\|from \"./Navigator\"\|from \"../Navigator\"" packages/web/src`
Expected: no matches (only `NavigatorBody` / `NavigatorPanel` remain).

- [ ] **Step 6: Run web tests**

Run: `pnpm --filter @waml/web test`
Expected: PASS — `TopBar.test.ts` (new switcher test), `NavigatorPanel.test.ts`, `NavigatorBody.test.ts` all green.

- [ ] **Step 7: Lint + build**

Run: `pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 8: Full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/TopBar.svelte packages/web/src/components/TopBar.test.ts
git rm packages/web/src/components/Navigator.svelte
git commit -m "feat(web): promote navigator to two-mode panel; TopBar switcher just toggles it"
```

---

## Self-Review

**Spec coverage:**
- Two-mode panel (centered/docked) → Task 2 `NavigatorPanel`. ✓
- Centered = CentralEditPanel treatment (scrim, `h-[95vh]`, header+close, two-stage Esc) → Task 2 Step 3 centered branch + unified Esc effect. ✓
- Docked = InspectorPanel mirror (left anchor, right-edge resize, translucent-when-idle, collapse) → Task 2 Step 3 docked branch. ✓
- Switcher toggles open/closed → Task 3 (`onToggleNav`). ✓
- Pin toggles centered↔docked → Task 2 header pin → `onToggleMode` → Task 3 `navMode` flip. ✓
- Esc closes → Task 2 unified Esc effect. ✓
- Unpin-while-docked returns to centered → `onToggleMode` flips both directions. ✓
- Reopen remembers last mode → `navMode` is separate from `navOpen` in `CanvasInner`; closing leaves `navMode` untouched. ✓
- Session-local, no persistence → `$state` in `CanvasInner`, no localStorage. ✓
- Coexists with right Inspector → both mounted in the same canvas row, left vs right anchors. ✓
- Chrome-agnostic body extraction + menu re-anchoring → Task 1. ✓
- Move mount TopBar→CanvasInner; TopBar becomes trigger → Task 3. ✓
- Tests preserved/adapted → Task 1 Step 3 (rename), Task 2 Step 1 (new), Task 3 Step 1 (TopBar). ✓

**Placeholder scan:** No TBD/TODO; all component code and test code shown in full; the one "copy verbatim if it differs" note (Task 3 Step 4 `onSelectDiagram`) points at an exact line range to copy, not a vague instruction. ✓

**Type consistency:** `mode: "centered" | "docked"` consistent across `NavigatorPanel` prop, `navMode` state, and `onToggleMode`. Body passthrough prop/callback names match `NavigatorBody` (= original `Navigator`) exactly. `width`/`collapsed` are `$bindable` in `NavigatorPanel` and bound with `bind:` in `CanvasInner`. `navOpen`/`onToggleNav` names match between `TopBar` props and `CanvasInner` mount. ✓

## Notes / Verify (manual)

After the gate passes, drive the app (`/run` or `pnpm --filter @waml/web dev`) and confirm:
- Switcher opens the centered panel; Esc / scrim-click / × all close it; Esc with the search field focused blurs first, closes second.
- Pin moves it to a left rail with the canvas interactive; drag-reorder a member; right-edge drag resizes; idle dims, hover restores; collapse folds to the header.
- Unpin returns it to centered. Close, reopen — it returns in the last mode.
- Left dock and the right Inspector open together without overlapping the canvas content.
