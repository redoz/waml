# Keyboard Shortcut Hints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a stateful "Show keyboard shortcuts" toggle to the left Dock that reveals key-hint badges on every keyboard-accessible action; the same badges also appear on hover of an action's control even when the toggle is off.

**Architecture:** A `shortcuts.ts` registry is the single source of truth for key events + display glyphs, consumed by both the actual key handlers and a presentational `KeyHint.svelte`. A shared `hints.svelte.ts` `$state` module holds the toggle (persisted to `localStorage`) and reflects it as a `data-show-shortcuts` attribute on `<html>`. Two global CSS rules make `.keyhint` visible on `.group:hover` or under `[data-show-shortcuts]` — no per-component reactive visibility prop.

**Tech Stack:** Svelte 5 (runes), TypeScript, Vitest + @testing-library/svelte, Tailwind, lucide-svelte.

## Global Constraints

- Svelte 5 runes mode (`$props`, `$state`, `$effect`, `$derived`). No Svelte-4 stores for new component state.
- TypeScript strict. No new npm dependencies — `Keyboard` icon comes from the already-installed `lucide-svelte`.
- Tests: Vitest, `render`/`screen`/`fireEvent` from `@testing-library/svelte`. Query by role/aria-name where possible (matches existing `Dock.test.ts`).
- Package commands run from `packages/web`: test = `pnpm --filter @uaml/web test`, check = `pnpm --filter @uaml/web check`.
- Hover-vs-toggle visibility is CSS — do NOT assert computed opacity in jsdom. Assert class/attribute presence only.
- All new files live under `packages/web/src/`. No license header (existing sibling files have none).

---

### Task 1: Shortcut registry

**Files:**
- Create: `packages/web/src/lib/shortcuts.ts`
- Test: `packages/web/src/lib/shortcuts.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `interface Shortcut { id: ShortcutId; event: string[]; display: string[]; label: string }`
  - `type ShortcutId = "tool.select" | "tool.add" | "tool.connect" | "selection.delete" | "hints.toggle"`
  - `SHORTCUTS: readonly Shortcut[]`
  - `shortcut(id: ShortcutId): Shortcut` (throws on unknown)
  - `keyLabel(id: ShortcutId): string[]` — display glyphs
  - `matchesShortcut(id: ShortcutId, e: KeyboardEvent): boolean` — true when `e.key` is in the entry's `event[]`

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/lib/shortcuts.test.ts
import { test, expect } from "vitest";
import { SHORTCUTS, shortcut, keyLabel, matchesShortcut } from "./shortcuts";

test("every shortcut has a unique id, non-empty event + display, and a label", () => {
  const ids = SHORTCUTS.map((s) => s.id);
  expect(new Set(ids).size).toBe(ids.length);
  for (const s of SHORTCUTS) {
    expect(s.event.length).toBeGreaterThan(0);
    expect(s.display.length).toBeGreaterThan(0);
    expect(s.label.length).toBeGreaterThan(0);
  }
});

test("keyLabel returns the display glyphs", () => {
  expect(keyLabel("tool.select")).toEqual(["V"]);
  expect(keyLabel("selection.delete")).toEqual(["⌫"]); // ⌫
  expect(keyLabel("hints.toggle")).toEqual(["?"]);
});

test("shortcut throws on an unknown id", () => {
  // @ts-expect-error unknown id
  expect(() => shortcut("nope")).toThrow(/unknown shortcut/);
});

test("matchesShortcut compares against KeyboardEvent.key", () => {
  expect(matchesShortcut("tool.select", new KeyboardEvent("keydown", { key: "v" }))).toBe(true);
  expect(matchesShortcut("tool.select", new KeyboardEvent("keydown", { key: "x" }))).toBe(false);
  // delete binds both Delete and Backspace
  expect(matchesShortcut("selection.delete", new KeyboardEvent("keydown", { key: "Delete" }))).toBe(true);
  expect(matchesShortcut("selection.delete", new KeyboardEvent("keydown", { key: "Backspace" }))).toBe(true);
  expect(matchesShortcut("hints.toggle", new KeyboardEvent("keydown", { key: "?" }))).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test shortcuts`
Expected: FAIL — cannot find module `./shortcuts`.

- [ ] **Step 3: Write minimal implementation**

```ts
// packages/web/src/lib/shortcuts.ts
export type ShortcutId =
  | "tool.select"
  | "tool.add"
  | "tool.connect"
  | "selection.delete"
  | "hints.toggle";

export interface Shortcut {
  id: ShortcutId;
  /** KeyboardEvent.key values that trigger this action. */
  event: string[];
  /** Glyphs shown in the hint badge. */
  display: string[];
  /** Human label (for tooltips / aria). */
  label: string;
}

export const SHORTCUTS: readonly Shortcut[] = [
  { id: "tool.select", event: ["v"], display: ["V"], label: "Select & move" },
  { id: "tool.add", event: ["n"], display: ["N"], label: "Add object" },
  { id: "tool.connect", event: ["c"], display: ["C"], label: "Connect" },
  { id: "selection.delete", event: ["Delete", "Backspace"], display: ["⌫"], label: "Delete selection" },
  { id: "hints.toggle", event: ["?"], display: ["?"], label: "Toggle keyboard shortcuts" },
];

const byId = new Map<ShortcutId, Shortcut>(SHORTCUTS.map((s) => [s.id, s]));

export function shortcut(id: ShortcutId): Shortcut {
  const s = byId.get(id);
  if (!s) throw new Error(`unknown shortcut ${id}`);
  return s;
}

export function keyLabel(id: ShortcutId): string[] {
  return shortcut(id).display;
}

export function matchesShortcut(id: ShortcutId, e: KeyboardEvent): boolean {
  return shortcut(id).event.includes(e.key);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test shortcuts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/lib/shortcuts.ts packages/web/src/lib/shortcuts.test.ts
git commit -m "feat(web): keyboard shortcut registry"
```

---

### Task 2: Hints toggle state module

**Files:**
- Create: `packages/web/src/state/hints.svelte.ts`
- Test: `packages/web/src/state/hints.test.ts`

**Interfaces:**
- Consumes: `localStorage` (via `globalThis`, guarded).
- Produces: singleton `hints` with:
  - `get show(): boolean`
  - `set show(v: boolean)` — writes `localStorage["uaml:show-shortcuts"]` = `"1"`/`"0"`
  - `toggle(): void`
  - Initial value read from `localStorage` (`"1"` → true), default `false`.

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/state/hints.test.ts
import { test, expect, beforeEach, vi } from "vitest";

const KEY = "uaml:show-shortcuts";

beforeEach(() => {
  localStorage.clear();
  vi.resetModules(); // re-import so the module re-reads localStorage at init
});

test("defaults to false when nothing is stored", async () => {
  const { hints } = await import("./hints.svelte");
  expect(hints.show).toBe(false);
});

test("initializes from a stored '1'", async () => {
  localStorage.setItem(KEY, "1");
  const { hints } = await import("./hints.svelte");
  expect(hints.show).toBe(true);
});

test("toggle flips the value and persists it", async () => {
  const { hints } = await import("./hints.svelte");
  hints.toggle();
  expect(hints.show).toBe(true);
  expect(localStorage.getItem(KEY)).toBe("1");
  hints.toggle();
  expect(hints.show).toBe(false);
  expect(localStorage.getItem(KEY)).toBe("0");
});

test("setting show persists it", async () => {
  const { hints } = await import("./hints.svelte");
  hints.show = true;
  expect(localStorage.getItem(KEY)).toBe("1");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test hints`
Expected: FAIL — cannot find module `./hints.svelte`.

- [ ] **Step 3: Write minimal implementation**

```ts
// packages/web/src/state/hints.svelte.ts
const KEY = "uaml:show-shortcuts";

function load(): boolean {
  try {
    return globalThis.localStorage?.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

function save(v: boolean): void {
  try {
    globalThis.localStorage?.setItem(KEY, v ? "1" : "0");
  } catch {
    // ignore (private mode / unavailable)
  }
}

let show = $state(load());

export const hints = {
  get show(): boolean {
    return show;
  },
  set show(v: boolean) {
    show = v;
    save(v);
  },
  toggle(): void {
    this.show = !show;
  },
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test hints`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add packages/web/src/state/hints.svelte.ts packages/web/src/state/hints.test.ts
git commit -m "feat(web): persisted show-shortcuts toggle state"
```

---

### Task 3: KeyHint badge component + global visibility CSS

**Files:**
- Create: `packages/web/src/components/KeyHint.svelte`
- Test: `packages/web/src/components/KeyHint.test.ts`
- Modify: `packages/web/src/app.css` (append visibility rules)

**Interfaces:**
- Consumes: nothing (presentational).
- Produces: `KeyHint` — props `{ keys: string[] }`. Renders a `<span class="keyhint ... opacity-0 transition-opacity">` wrapping one `<kbd>` per key. Visibility comes ONLY from ancestor CSS (`.group:hover` or `[data-show-shortcuts]`).

- [ ] **Step 1: Write the failing test**

```ts
// packages/web/src/components/KeyHint.test.ts
import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import KeyHint from "./KeyHint.svelte";

test("renders one kbd per key with the glyph text", () => {
  const { container } = render(KeyHint, { props: { keys: ["V"] } });
  const kbds = container.querySelectorAll("kbd");
  expect(kbds.length).toBe(1);
  expect(kbds[0].textContent).toBe("V");
});

test("wrapper carries the keyhint class (hidden by default via opacity-0)", () => {
  const { container } = render(KeyHint, { props: { keys: ["⌫"] } });
  const span = container.querySelector("span.keyhint");
  expect(span).not.toBeNull();
  expect(span!.classList.contains("opacity-0")).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test KeyHint`
Expected: FAIL — cannot find module `./KeyHint.svelte`.

- [ ] **Step 3: Write minimal implementation**

```svelte
<!-- packages/web/src/components/KeyHint.svelte -->
<script lang="ts">
  let { keys }: { keys: string[] } = $props();
</script>

<!-- Visibility is driven by ancestor CSS (see app.css):
     shown on `.group:hover` or under `[data-show-shortcuts]`. -->
<span
  class="keyhint pointer-events-none inline-flex items-center gap-0.5 opacity-0 transition-opacity"
  aria-hidden="true"
>
  {#each keys as k}
    <kbd
      class="inline-flex min-w-[16px] items-center justify-center rounded border border-slate-300 bg-slate-50 px-1 text-[10px] font-semibold leading-[15px] text-slate-600 shadow-[0_1px_0_rgba(15,23,42,0.08)]"
      >{k}</kbd
    >
  {/each}
</span>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test KeyHint`
Expected: PASS (2 tests).

- [ ] **Step 5: Append global visibility rules to app.css**

Append to the END of `packages/web/src/app.css` (after `@tailwind utilities;`). The compound selectors have specificity (0,2,0), beating the `.opacity-0` utility (0,1,0), so they win regardless of source order:

```css
/* Keyboard shortcut hints: hidden by default (opacity-0 utility on .keyhint),
   revealed on hover of the control group, or globally when the Dock toggle is on
   (data-show-shortcuts on <html>). */
.group:hover .keyhint,
[data-show-shortcuts] .keyhint {
  opacity: 1;
}
```

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/KeyHint.svelte packages/web/src/components/KeyHint.test.ts packages/web/src/app.css
git commit -m "feat(web): KeyHint badge + hover/toggle visibility CSS"
```

---

### Task 4: Dock toggle button, `?` key, attr reflect, tool badges

**Files:**
- Modify: `packages/web/src/components/canvas/Dock.svelte`
- Modify: `packages/web/src/components/canvas/Dock.test.ts`

**Interfaces:**
- Consumes: `matchesShortcut`, `keyLabel` from `../../lib/shortcuts`; `hints` from `../../state/hints.svelte`; `KeyHint` from `../KeyHint.svelte`; `Keyboard` from `lucide-svelte`.
- Produces: no new exported props. A new button with `aria-label="Show keyboard shortcuts"` and `aria-pressed={hints.show}`; tool buttons render a corner `KeyHint`; `document.documentElement` gains/loses `data-show-shortcuts`.

- [ ] **Step 1: Write the failing tests (append to Dock.test.ts)**

Add these imports at the top of `packages/web/src/components/canvas/Dock.test.ts` (alongside the existing imports):

```ts
import { beforeEach } from "vitest";
import { hints } from "../../state/hints.svelte";
```

Add this reset + these tests to `packages/web/src/components/canvas/Dock.test.ts`:

```ts
beforeEach(() => {
  localStorage.clear();
  hints.show = false;
  document.documentElement.removeAttribute("data-show-shortcuts");
});

test("the shortcuts toggle button flips hints.show, aria-pressed, and the root attribute", async () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  const btn = screen.getByRole("button", { name: "Show keyboard shortcuts" });
  expect(btn.getAttribute("aria-pressed")).toBe("false");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(false);

  await fireEvent.click(btn);
  expect(hints.show).toBe(true);
  expect(btn.getAttribute("aria-pressed")).toBe("true");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(true);
});

test("pressing ? toggles the hints; ? while typing in an input is ignored", async () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  await fireEvent.keyDown(window, { key: "?" });
  expect(hints.show).toBe(true);

  // Typing ? inside an input must NOT toggle.
  const input = document.createElement("input");
  document.body.appendChild(input);
  await fireEvent.keyDown(input, { key: "?" });
  expect(hints.show).toBe(true); // unchanged
  input.remove();
});

test("tool buttons render their key-hint glyph", () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  // V / N / C glyphs are present in the DOM (hidden via CSS, but rendered).
  const glyphs = Array.from(document.querySelectorAll("kbd")).map((k) => k.textContent);
  expect(glyphs).toEqual(expect.arrayContaining(["V", "N", "C"]));
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --filter @uaml/web test Dock`
Expected: FAIL — no button named "Show keyboard shortcuts"; no `kbd` glyphs.

- [ ] **Step 3: Wire the imports and state into Dock.svelte**

In the `<script lang="ts">` block of `packages/web/src/components/canvas/Dock.svelte`, add these imports below the existing ones (line ~7):

```ts
import { Keyboard } from "lucide-svelte";
import KeyHint from "../KeyHint.svelte";
import { hints } from "../../state/hints.svelte";
import { matchesShortcut, keyLabel } from "../../lib/shortcuts";
```

Replace the existing keyboard-shortcut `$effect` (lines ~31-42, the `handler` that checks `e.key === "v"` etc.) with a registry-sourced version that also handles `?`:

```ts
// Keyboard shortcuts, sourced from the registry so displayed glyphs and the
// handled keys can never drift.
$effect(() => {
  function handler(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
    if (matchesShortcut("tool.select", e)) onToolChange("select");
    else if (matchesShortcut("tool.add", e)) onToolChange("add");
    else if (matchesShortcut("tool.connect", e)) onToolChange("connect");
    else if (matchesShortcut("hints.toggle", e)) hints.toggle();
  }
  window.addEventListener("keydown", handler);
  return () => window.removeEventListener("keydown", handler);
});

// Reflect the toggle onto <html> so the global CSS reveals every .keyhint.
$effect(() => {
  document.documentElement.toggleAttribute("data-show-shortcuts", hints.show);
});
```

- [ ] **Step 4: Add a `keys` param to the toolButton snippet and pass glyphs**

Replace the `toolButton` snippet (lines ~114-127) with a version that overlays a `KeyHint`:

```svelte
{#snippet toolButton(icon: Snippet, tip: string, active: boolean, onClick: () => void, keys?: string[])}
  <div class="relative group">
    <button
      onclick={onClick}
      aria-label={tip}
      class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {active
        ? 'bg-[#e6f1fb] text-[#1e88e5]'
        : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
    >
      {@render icon()}
    </button>
    {#if keys}
      <span class="pointer-events-none absolute -top-1 -right-1">
        <KeyHint {keys} />
      </span>
    {/if}
    {@render dockTip(tip)}
  </div>
{/snippet}
```

Update the three tool-button call sites (lines ~187-199) to pass glyphs from the registry:

```svelte
{@render toolButton(selectIcon, "Select & move (V)", activeTool === "select", () => onToolChange("select"), keyLabel("tool.select"))}
{@render toolButton(
  addIcon,
  "Add object (N) — double-click canvas",
  activeTool === "add",
  () => onToolChange("add"),
  keyLabel("tool.add"),
)}
{@render toolButton(
  connectIcon,
  "Connect (C) — drag from node's port",
  activeTool === "connect",
  () => onToolChange("connect"),
  keyLabel("tool.connect"),
)}
```

- [ ] **Step 5: Add the toggle button to the dock markup**

Insert this block into the dock's `<div data-dock ...>` immediately BEFORE the final Clear-canvas section (before the `<div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>` that precedes the Clear button, ~line 274):

```svelte
<div class="h-px bg-[#d8dee8] mx-1 my-[3px]"></div>
<div class="relative group">
  <button
    onclick={() => hints.toggle()}
    aria-label="Show keyboard shortcuts"
    aria-pressed={hints.show}
    class="w-[38px] h-[38px] rounded-[9px] border-none flex items-center justify-center cursor-pointer transition-colors {hints.show
      ? 'bg-[#e6f1fb] text-[#1e88e5]'
      : 'bg-transparent text-slate-500 hover:bg-[#f1f3f7] hover:text-slate-900'}"
  >
    <Keyboard size={19} />
  </button>
  <span class="pointer-events-none absolute -top-1 -right-1">
    <KeyHint keys={keyLabel("hints.toggle")} />
  </span>
  {@render dockTip("Show keyboard shortcuts (?)")}
</div>
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `pnpm --filter @uaml/web test Dock`
Expected: PASS — all existing Dock tests plus the 3 new ones.

- [ ] **Step 7: Type-check**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/Dock.svelte packages/web/src/components/canvas/Dock.test.ts
git commit -m "feat(web): Dock show-shortcuts toggle, ? hotkey, tool key badges"
```

---

### Task 5: Delete hint on SelectionToolbar + re-source CanvasInner delete keys

**Files:**
- Modify: `packages/web/src/components/canvas/SelectionToolbar.svelte`
- Modify: `packages/web/src/components/canvas/SelectionToolbar.test.ts`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (delete handler)

**Interfaces:**
- Consumes: `KeyHint` from `../KeyHint.svelte`; `keyLabel` from `../../lib/shortcuts` (SelectionToolbar); `matchesShortcut` from `../../lib/shortcuts` (CanvasInner).
- Produces: the Delete button renders a `KeyHint` glyph (`⌫`); CanvasInner's `handleKeyDown` matches delete via the registry.

- [ ] **Step 1: Write the failing test (append to SelectionToolbar.test.ts)**

```ts
test("the Delete button shows the ⌫ key hint", () => {
  render(SelectionToolbar, {
    props: {
      x: 100,
      y: 100,
      nodeCount: 1,
      edgeCount: 0,
      onNewDiagram: vi.fn(),
      onDelete: vi.fn(),
    },
  });
  const del = screen.getByRole("button", { name: "Delete selection" });
  const kbd = del.querySelector("kbd");
  expect(kbd?.textContent).toBe("⌫"); // ⌫
});
```

If `SelectionToolbar.test.ts` lacks the needed imports, ensure the top of the file has:

```ts
import { test, expect, vi } from "vitest";
import { render, screen } from "@testing-library/svelte";
import SelectionToolbar from "./SelectionToolbar.svelte";
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @uaml/web test SelectionToolbar`
Expected: FAIL — no `kbd` inside the Delete button.

- [ ] **Step 3: Add the KeyHint to the Delete button**

In `packages/web/src/components/canvas/SelectionToolbar.svelte`, add the import below the existing lucide import (line ~5):

```ts
import KeyHint from "../KeyHint.svelte";
import { keyLabel } from "../../lib/shortcuts";
```

Add the `group` class to the Delete button and render the hint after the label. Replace the Delete `<button>` (lines ~111-118) with:

```svelte
<button
  onclick={onDelete}
  aria-label="Delete selection"
  title="Delete the selected objects and relationships"
  class="group flex items-center gap-[6px] rounded-lg px-2.5 py-[7px] text-[12px] font-semibold text-slate-500 hover:bg-[#fdf2f2] hover:text-[#dc2626] cursor-pointer whitespace-nowrap transition-colors"
>
  <Trash2 size={14} /> Delete selection
  <KeyHint keys={keyLabel("selection.delete")} />
</button>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @uaml/web test SelectionToolbar`
Expected: PASS.

- [ ] **Step 5: Re-source the CanvasInner delete handler from the registry**

In `packages/web/src/components/canvas/CanvasInner.svelte`, add the import near the other `./` imports (alongside the `Dock`/`SelectionToolbar` imports, ~line 34):

```ts
import { matchesShortcut } from "../../lib/shortcuts";
```

Replace the `handleKeyDown` function (lines ~305-311) with:

```ts
function handleKeyDown(e: KeyboardEvent) {
  if (matchesShortcut("selection.delete", e) && !isSelectionEmpty(selectionSet)) {
    const tag = (e.target as HTMLElement).tagName;
    if (["INPUT", "TEXTAREA", "SELECT"].includes(tag)) return;
    handleDeleteSelection();
  }
}
```

- [ ] **Step 6: Run the canvas + selection tests to confirm nothing regressed**

Run: `pnpm --filter @uaml/web test canvas`
Expected: PASS (existing selection/delete tests still green).

- [ ] **Step 7: Type-check**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/SelectionToolbar.svelte packages/web/src/components/canvas/SelectionToolbar.test.ts packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): Delete key hint on selection toolbar; registry-sourced delete key"
```

---

### Task 6: Full-suite verification

**Files:** none (verification only).

- [ ] **Step 1: Run the whole web test suite**

Run: `pnpm --filter @uaml/web test`
Expected: PASS — all suites green.

- [ ] **Step 2: Type-check + build**

Run: `pnpm --filter @uaml/web check`
Expected: no errors.

- [ ] **Step 3: Manual smoke (dev server)**

Run: `pnpm --filter @uaml/web dev`, open the app, and verify:
- Dock shows a keyboard-icon button; clicking it turns it blue and reveals `V`/`N`/`C` badges on the tool buttons (and `?` on itself).
- Hovering a tool button with the toggle OFF reveals just that button's badge.
- Selecting a node shows the selection toolbar; its Delete button shows a `⌫` badge under the toggle / on hover.
- Pressing `?` (Shift+/) toggles the hints; reloading the page preserves the toggle state.
- Pressing `V`/`N`/`C` still switches tools; `Delete`/`Backspace` still deletes the selection.
