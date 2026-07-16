# Model Navigator Switcher Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "click the TopBar title → heavy model editor" flow with a light read-only diagram-switcher dropdown as the default click target, keep two explicit escape hatches into the full editor (Dock / Edit), unify the pin semantics so `pinned = solid` on both the navigator and inspector, and stop the docked navigator rail from overlapping the canvas tool Dock.

**Architecture:** Svelte 5 runes. `TopBar.svelte` owns a local `switcherOpen` dropdown (same anchoring pattern as its existing Export menu). `CanvasInner.svelte` owns all panel session state (`navOpen`, `navMode`, `navPinned`, `inspectorPinned`, `navWidth`) and wires callbacks down to `TopBar`, `NavigatorPanel`, `InspectorPanel`, and `Dock`. `NavigatorPanel.svelte` stays a two-mode host (`"centered" | "docked"`); docked gains an opacity pin. `InspectorPanel.svelte` flips its translucency boolean and icon/labels. `Dock.svelte` gains a `leftOffset` prop so it slides clear of the docked rail.

**Tech Stack:** Svelte 5 (runes: `$state`, `$derived`, `$props`, `$effect`), TypeScript, Tailwind utility classes, `lucide-svelte` icons, Vitest + `@testing-library/svelte` (co-located `*.test.ts`, jsdom).

## Global Constraints

- **TDD, always.** Every code change is preceded by a failing test in the co-located `*.test.ts`, then made green with the minimal implementation.
- **Single-file test runs** use vitest's positional path filter, executed in the web package dir:
  `pnpm --filter @waml/web exec vitest run <path>` (paths are relative to `packages/web`).
- **Green commits.** Because `TopBar`/`NavigatorPanel`/`Dock` are all mounted by the full `Canvas.svelte` integration suite (`packages/web/src/components/canvas/Canvas.test.ts`), and `svelte-check` (run by `pnpm build`) rejects unknown/missing component props, each task changes the component **and** its `CanvasInner` wiring **and** the affected tests together, so the suite and the type-check stay green at every commit. Full gate before each commit: `pnpm --filter @waml/web exec vitest run` (all web tests).
- **Pin semantics after this plan:** `translucent = !pinned && !engaged` everywhere. `pinned` icon = solid `Pin`; unpinned icon = `PinOff`. `aria-pressed={pinned}`. Action-oriented labels: pinned → "Let it dim when idle" (click unpins), unpinned → "Keep solid" (click pins).
- **No persistence** of pin/mode across sessions (session-local `$state`, unchanged). **No changes** to `NavigatorBody`'s editing behavior. **No** slim vertical-tab rail — docking always yields the full editor.
- Do not reformat untouched lines. Match the existing 2-space indentation and Tailwind class ordering.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `packages/web/src/components/inspector/InspectorPanel.svelte` | Always-present floating inspector. | Invert `translucent`; swap pin icon (pinned→`Pin`) + action labels. |
| `packages/web/src/components/inspector/InspectorPanel.test.ts` | Inspector unit tests. | Flip opacity expectations; update pin-button label regex; add icon/label-swap test. |
| `packages/web/src/components/NavigatorPanel.svelte` | Two-mode (centered/docked) navigator host. | Add `pinned` + `onTogglePin` props; docked translucency gated on `!pinned`; docked header pin toggle replaces the mode-toggle; centered keeps the stick-to-side (dock) button. |
| `packages/web/src/components/NavigatorPanel.test.ts` | Navigator unit tests. | Add `onTogglePin` to props helper; rewrite docked control test; add pin-toggle + docked-translucency tests. |
| `packages/web/src/components/TopBar.svelte` | Top bar + read-only diagram switcher. | Drop `navOpen`/`onToggleNav`; add `switcherOpen` state + read-only popover; add `onSelectDiagram`/`onDockModel`/`onEditModel` props. |
| `packages/web/src/components/TopBar.test.ts` | Top bar unit tests. | Replace the `onToggleNav` test with dropdown open/list/select/Dock/Edit/outside-click/Escape tests. |
| `packages/web/src/components/canvas/Dock.svelte` | Left-edge canvas tool dock. | Add `leftOffset` prop driving the inline `left` position with a slide transition. |
| `packages/web/src/components/canvas/Dock.test.ts` | Dock unit tests. | Add `leftOffset` default + override tests. |
| `packages/web/src/components/canvas/CanvasInner.svelte` | Canvas orchestrator: owns all panel state + wiring. | `inspectorPinned` default → `true`; add `navPinned`; wire `TopBar` callbacks; pass `pinned`/`onTogglePin` to `NavigatorPanel`; pass `leftOffset` to `Dock`. |
| `packages/web/src/components/canvas/Canvas.test.ts` | Full-mount integration suite. | Update inspector pin-button label; rewrite switcher test (dropdown, Edit escalates); add Dock-slide test. |

---

## Task 1 — Inspector pin flip (+ CanvasInner default solid)

Invert the inspector translucency boolean so `pinned = solid`, swap the icon to show state, and default `inspectorPinned` to `true` in `CanvasInner` so the out-of-the-box inspector stays solid (only the icon meaning inverts).

**Files:**
- Modify: `packages/web/src/components/inspector/InspectorPanel.svelte` (translucent derived ~line 66; pin button ~lines 174-186)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (`inspectorPinned` state ~lines 118-120)
- Test: `packages/web/src/components/inspector/InspectorPanel.test.ts`, `packages/web/src/components/canvas/Canvas.test.ts`

**Interfaces:**
- Consumes: `pinned?: boolean` (default `false`), `onTogglePin: () => void` — both already exist; unchanged signatures.
- Produces: no new props. Behavior contract: `translucent = !pinned && !engaged`. `CanvasInner` local `inspectorPinned: boolean` default flips `false → true`.

### Steps

- [ ] **Write failing test — flip the opacity expectations in `InspectorPanel.test.ts`.** Replace the existing test at lines 132-138 (`"is opaque when unpinned and translucent when pinned + idle"`) with:
```ts
  it("is opaque when pinned and translucent when unpinned + idle", () => {
    setup({ pinned: true });
    expect(screen.getByRole("complementary").classList.contains("opacity-40")).toBe(false);
    setup({ pinned: false });
    const asides = screen.getAllByRole("complementary");
    expect(asides[asides.length - 1].classList.contains("opacity-40")).toBe(true);
  });
```
Replace the hover test at lines 140-151 so it starts unpinned (now the dims-when-idle state) — change only the first line's props:
```ts
  it("becomes opaque on hover, translucent again after the pointer leaves", async () => {
    setup({ pinned: false, hideDelay: 20 });
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
```
Update the pin-fire test at lines 125-130 to the new label:
```ts
  it("fires onTogglePin when the pin control is clicked", async () => {
    const onTogglePin = vi.fn();
    setup({ onTogglePin });
    await fireEvent.click(screen.getByRole("button", { name: /keep solid|dim when idle/i }));
    expect(onTogglePin).toHaveBeenCalledTimes(1);
  });
```
Add a new icon/label-swap test after it:
```ts
  it("pinned shows the solid (Pin) state; unpinned shows PinOff — labels swap", () => {
    const { unmount } = setup({ pinned: true });
    expect(screen.getByRole("button", { name: /let it dim when idle/i }).getAttribute("aria-pressed")).toBe("true");
    unmount();
    setup({ pinned: false });
    expect(screen.getByRole("button", { name: /keep solid/i }).getAttribute("aria-pressed")).toBe("false");
  });
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/inspector/InspectorPanel.test.ts` — fails: current code has `translucent = pinned && !engaged` (so `pinned:true` renders `opacity-40`, the new test expects it absent) and the pin button's accessible name is still `"Pin inspector"`/`"Unpin inspector"`, so `getByRole(... /keep solid|dim when idle/i)` throws "Unable to find an accessible element".
- [ ] **Minimal implementation — invert the boolean.** In `InspectorPanel.svelte`, change line 66:
```ts
  const translucent = $derived(!pinned && !engaged);
```
- [ ] **Minimal implementation — swap the pin button icon + labels.** Replace the button at lines 174-186 with:
```svelte
    <button
      onclick={onTogglePin}
      aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
      aria-pressed={pinned}
      title={pinned ? "Let it dim when idle" : "Keep solid"}
      class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
    >
      {#if pinned}
        <Pin size={16} />
      {:else}
        <PinOff size={16} />
      {/if}
    </button>
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/inspector/InspectorPanel.test.ts` — all InspectorPanel tests green.
- [ ] **Write failing test — flip the `CanvasInner` default in `Canvas.test.ts`.** In the test `"exposes a pin control that makes the panel translucent while idle"` (lines 64-73), change the pin-button click line 68 to the new label (default is now pinned → "Let it dim when idle"):
```ts
    await fireEvent.click(within(panel).getByRole("button", { name: /let it dim when idle/i }));
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — fails: `inspectorPinned` still defaults to `false`, so at mount the button is labelled "Keep solid" (unpinned) and `/let it dim when idle/i` finds nothing; also with the new invert, `pinned:false` would render `opacity-40` at mount, contradicting line 67's `toBe(false)`.
- [ ] **Minimal implementation — default the inspector pinned.** In `CanvasInner.svelte`, update the comment + default at lines 118-120:
```ts
  // Inspector pin state. The panel is always mounted (never closes); an empty
  // selection rests as a compact bar + hint. Pinned = forced solid; unpinned
  // dims (translucent) while idle, fading back opaque on hover/focus. Defaults
  // pinned so the out-of-the-box inspector stays solid.
  let inspectorPinned = $state(true);
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — green.
- [ ] **Full gate:** `pnpm --filter @waml/web exec vitest run` — whole web suite green.
- [ ] **Commit:**
```
git add packages/web/src/components/inspector/InspectorPanel.svelte packages/web/src/components/inspector/InspectorPanel.test.ts packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.test.ts
git commit -m "$(cat <<'EOF'
feat(inspector): flip pin so pinned means solid; default inspector solid

Invert InspectorPanel translucency to `!pinned && !engaged`, swap the pin
icon to show state (pinned -> Pin), and default CanvasInner's inspectorPinned
to true so the out-of-the-box inspector stays solid.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ALcN4SscYGB3AXASsMzoVq
EOF
)"
```

---

## Task 2 — Navigator docked pin (+ CanvasInner navPinned wiring)

Give the docked navigator its own opacity pin (new `pinned`/`onTogglePin` props), gate docked translucency on `!pinned`, replace the docked header's centered/docked mode toggle with a pin toggle, and keep the stick-to-side (dock) button in the centered header only. Wire `navPinned` state in `CanvasInner`.

**Files:**
- Modify: `packages/web/src/components/NavigatorPanel.svelte` (props block ~lines 14-62; `translucent` ~line 78; header snippet ~lines 147-181)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (nav state ~lines 126-129; `<NavigatorPanel>` render ~lines 827-887)
- Test: `packages/web/src/components/NavigatorPanel.test.ts`

**Interfaces:**
- Consumes (NavigatorPanel new): `pinned?: boolean` (default `false`), `onTogglePin: () => void`. Existing `onToggleMode: () => void` retained (centered stick-to-side only). `mode: "centered" | "docked"` unchanged.
- Produces (CanvasInner): local `navPinned: boolean` = `$state(false)`; passes `pinned={navPinned}` and `onTogglePin={() => (navPinned = !navPinned)}`.

### Steps

- [ ] **Write failing test — add `onTogglePin` to the props helper and cover the docked pin.** In `NavigatorPanel.test.ts`, extend the `props` helper (lines 21-26) so every render supplies the new required callback:
```ts
const props = (over = {}) => ({
  open: true, mode: "centered" as const, title: "acme-model",
  graph, scopeKey: "sales", activeDiagramKey: "overview", palette: ["uml.Class"],
  onClose: vi.fn(), onToggleMode: vi.fn(), onTogglePin: vi.fn(), onScope: vi.fn(), onSelectDiagram: vi.fn(),
  ...over,
});
```
Replace the docked control test at lines 71-76 with:
```ts
test("docked mode exposes a resize handle and a pin toggle (no center toggle)", () => {
  render(NavigatorPanel, { props: props({ mode: "docked" }) });
  expect(screen.getByLabelText("Model navigator")).toBeTruthy();
  expect(screen.getByRole("button", { name: /keep solid|dim when idle/i })).toBeTruthy();
  expect(screen.queryByRole("button", { name: /center/i })).toBeNull();
  expect(screen.getByTestId("nav-resize")).toBeTruthy();
});
```
Append two new tests:
```ts
test("docked pin toggle fires onTogglePin", async () => {
  const onTogglePin = vi.fn();
  render(NavigatorPanel, { props: props({ mode: "docked", onTogglePin }) });
  await fireEvent.click(screen.getByRole("button", { name: /keep solid|dim when idle/i }));
  expect(onTogglePin).toHaveBeenCalledTimes(1);
});

test("docked translucency tracks pinned (dims when idle unless pinned)", () => {
  const { unmount } = render(NavigatorPanel, { props: props({ mode: "docked", pinned: false }) });
  expect(screen.getByLabelText("Model navigator").classList.contains("opacity-40")).toBe(true);
  unmount();
  render(NavigatorPanel, { props: props({ mode: "docked", pinned: true }) });
  expect(screen.getByLabelText("Model navigator").classList.contains("opacity-40")).toBe(false);
});
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/NavigatorPanel.test.ts` — fails: docked header still renders the `onToggleMode` button labelled "Unpin navigator (center)" (so `/center/i` matches and `queryByRole(... /center/i).toBeNull()` fails; `/keep solid|dim when idle/i` finds nothing), and `translucent` ignores `pinned` so the `pinned:true` case still shows `opacity-40`.
- [ ] **Minimal implementation — add the props.** In `NavigatorPanel.svelte`, add to the destructuring (after `onToggleMode,` at line 21):
```ts
    onToggleMode,
    pinned = false,
    onTogglePin,
```
and to the props type (after `onToggleMode: () => void;` at line 43):
```ts
    onToggleMode: () => void;
    pinned?: boolean;
    onTogglePin: () => void;
```
- [ ] **Minimal implementation — gate docked translucency on pinned.** Change line 78:
```ts
  const translucent = $derived(mode === "docked" && !pinned && !engaged);
```
- [ ] **Minimal implementation — split the header controls per mode.** Replace the whole header snippet (lines 147-181) so docked shows collapse + pin toggle and centered keeps the stick-to-side button:
```svelte
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
      <button
        onclick={onTogglePin}
        aria-label={pinned ? "Let it dim when idle" : "Keep solid"}
        aria-pressed={pinned}
        title={pinned ? "Let it dim when idle" : "Keep solid"}
        class={`w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors ${pinned ? "text-[#1e88e5] bg-[#e6f1fb]" : "text-slate-500 hover:bg-[#f1f3f7]"}`}
      >
        {#if pinned}<Pin size={16} />{:else}<PinOff size={16} />{/if}
      </button>
    {:else}
      <button
        onclick={onToggleMode}
        aria-label="Pin navigator to left"
        aria-pressed={false}
        title="Pin navigator to left"
        class="w-[30px] h-[30px] flex items-center justify-center rounded-md transition-colors text-slate-500 hover:bg-[#f1f3f7]"
      >
        <Pin size={16} />
      </button>
    {/if}
    <button
      onclick={onClose}
      aria-label="Close"
      title="Close"
      class="w-[30px] h-[30px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7] text-[20px] leading-none"
    >
      &times;
    </button>
  </div>
{/snippet}
```
> Note: the close button keeps its literal multiplication-sign glyph (the same character already in the file); the `&times;` above is only an escape for this document.

- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/NavigatorPanel.test.ts` — all green. (The existing `"pin button fires onToggleMode"` centered test still matches the stick-to-side button via `/pin navigator to left/i`.)
- [ ] **Minimal implementation — wire `navPinned` in `CanvasInner`.** Add the state after line 129 (`let navCollapsed = $state(false);`):
```ts
  let navCollapsed = $state(false);
  let navPinned = $state(false);
```
Then in the `<NavigatorPanel>` render, add the two props right after the `onToggleMode={...}` line (line 834):
```svelte
      onToggleMode={() => (navMode = navMode === "centered" ? "docked" : "centered")}
      pinned={navPinned}
      onTogglePin={() => (navPinned = !navPinned)}
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — green (no regression; navigator still defaults centered).
- [ ] **Full gate:** `pnpm --filter @waml/web exec vitest run` — whole web suite green.
- [ ] **Commit:**
```
git add packages/web/src/components/NavigatorPanel.svelte packages/web/src/components/NavigatorPanel.test.ts packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "$(cat <<'EOF'
feat(navigator): docked opacity pin; drop docked->centered toggle

Add pinned/onTogglePin to NavigatorPanel; docked translucency is now
`docked && !pinned && !engaged`. The docked header shows a pin toggle
(Pin when solid, PinOff when dimming) in place of the removed centered
toggle; the centered header keeps its stick-to-side button. CanvasInner
owns navPinned (default unpinned, matching today's docked dims-when-idle).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ALcN4SscYGB3AXASsMzoVq
EOF
)"
```

---

## Task 3 — TopBar read-only switcher dropdown (+ CanvasInner TopBar callbacks)

Turn the TopBar title button into a read-only diagram-switcher popover (same anchoring pattern as the Export menu): list every diagram with a check on the active one, a row-click that fires `onSelectDiagram(key)`, and header Dock/Edit buttons that escalate into the full editor. Drop `navOpen`/`onToggleNav`; add `onSelectDiagram`/`onDockModel`/`onEditModel`. Rewire `CanvasInner`'s `<TopBar>` render in the same commit (svelte-check rejects the stale props otherwise).

**Files:**
- Modify: `packages/web/src/components/TopBar.svelte` (import ~line 3; props block ~lines 13-41; `activeTitle` ~line 51; title button ~lines 115-127; add popover + `switcherOpen` state + Escape effect)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (`<TopBar>` render ~lines 623-634)
- Test: `packages/web/src/components/TopBar.test.ts`, `packages/web/src/components/canvas/Canvas.test.ts`

**Interfaces:**
- Consumes (TopBar): `diagrams?: Diagram[]`, `activeDiagramKey?: string` (both existing). New: `onSelectDiagram?: (key: string) => void`, `onDockModel?: () => void`, `onEditModel?: () => void`. Removed: `navOpen?: boolean`, `onToggleNav?: () => void`.
- Produces (TopBar): local `switcherOpen: boolean` = `$state(false)`; trigger `aria-expanded={switcherOpen}`.
- Produces (CanvasInner): passes `onSelectDiagram` (selection-reset + `activeDiagramKey = key`), `onDockModel` (`navOpen = true; navMode = "docked"`), `onEditModel` (`navOpen = true; navMode = "centered"`).

### Steps

- [ ] **Write failing test — extend `switcherProps` in `TopBar.test.ts`.** Replace the helper (lines 12-19) with:
```ts
const switcherProps = (over: Record<string, unknown> = {}) => ({
  diagrams: [diagram("d1", "Overview"), diagram("d2", "Details")],
  activeDiagramKey: "d1",
  onSelectDiagram: vi.fn(),
  onDockModel: vi.fn(),
  onEditModel: vi.fn(),
  ...over,
});
```
- [ ] **Write failing test — replace the `onToggleNav` test with dropdown tests.** Delete the test at lines 37-53 (`"the center switcher toggles the navigator via onToggleNav + aria-expanded"`) and add:
```ts
test("clicking the title opens the read-only switcher dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  expect(screen.getByRole("dialog", { name: /switch diagram/i })).toBeTruthy();
  expect(screen.queryByLabelText("Search model")).toBeNull();
  expect(screen.queryByRole("button", { name: /rename|new diagram|create/i })).toBeNull();
});

test("the dropdown lists every diagram with the active one checked", async () => {
  render(TopBar, { props: switcherProps() });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  expect(screen.getByRole("option", { name: /Overview/ }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("option", { name: /Details/ }).getAttribute("aria-selected")).toBe("false");
});

test("clicking a diagram row fires onSelectDiagram and closes the dropdown", async () => {
  const onSelectDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onSelectDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("option", { name: /Details/ }));
  expect(onSelectDiagram).toHaveBeenCalledWith("d2");
  expect(screen.getByRole("button", { name: /switch diagram/i }).getAttribute("aria-expanded")).toBe("false");
});

test("the Dock button fires onDockModel and closes", async () => {
  const onDockModel = vi.fn();
  render(TopBar, { props: switcherProps({ onDockModel }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /dock model editor/i }));
  expect(onDockModel).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog", { name: /switch diagram/i })).toBeNull();
});

test("the Edit button fires onEditModel and closes", async () => {
  const onEditModel = vi.fn();
  render(TopBar, { props: switcherProps({ onEditModel }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /edit model/i }));
  expect(onEditModel).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog", { name: /switch diagram/i })).toBeNull();
});

test("outside-click closes the dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  await fireEvent.click(document.querySelector(".fixed.inset-0")!);
  expect(btn.getAttribute("aria-expanded")).toBe("false");
});

test("Escape closes the dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
});
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/TopBar.test.ts` — fails: the title button still calls `onToggleNav` and renders no popover, so `getByRole("dialog", ...)`, `getByRole("option", ...)`, and `/dock model editor/i` all throw "Unable to find an accessible element".
- [ ] **Minimal implementation — extend the icon import.** In `TopBar.svelte`, replace line 3:
```svelte
  import { Download, Upload, ChevronDown, FileText, Image as ImageIcon, Share2, PanelLeft, Pencil, Check } from "lucide-svelte";
```
- [ ] **Minimal implementation — swap the props.** In the `$props()` block, remove `navOpen`/`onToggleNav` and add the three callbacks. The defaults block around lines 22-27 becomes:
```ts
    diagrams = [],
    activeDiagramKey = "",
    // The center title is now a read-only diagram switcher; CanvasInner owns the
    // heavy navigator panel and its escalation callbacks.
    onSelectDiagram,
    onDockModel,
    onEditModel,
  }: {
```
and the corresponding type entries (replacing `navOpen?: boolean;` / `onToggleNav?: () => void;` at lines 39-40):
```ts
    diagrams?: Diagram[];
    activeDiagramKey?: string;
    onSelectDiagram?: (key: string) => void;
    onDockModel?: () => void;
    onEditModel?: () => void;
  } = $props();
```
- [ ] **Minimal implementation — add local open state + Escape handler.** After the `exportMenuOpen`/`showLibraryHint` state (line 46), add:
```ts
  // Read-only diagram switcher popover — same anchoring pattern as the Export
  // menu below (full-screen click-catcher + absolutely positioned card).
  let switcherOpen = $state(false);
```
and after the `activeTitle` derived (line 53), add the Escape-to-close effect:
```ts
  $effect(() => {
    if (!switcherOpen) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") switcherOpen = false;
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
```
- [ ] **Minimal implementation — replace the title button with the switcher popover.** Replace the whole `<div class="relative"> ... </div>` block at lines 115-127 with:
```svelte
  <div class="relative">
    <button
      onclick={() => (switcherOpen = !switcherOpen)}
      aria-label={`Diagram: ${activeTitle} — switch diagram`}
      aria-haspopup="dialog"
      aria-expanded={switcherOpen}
      title="Switch diagram"
      class="flex items-center gap-[6px] rounded-lg px-[10px] py-[6px] text-[13px] font-[600] cursor-pointer transition-colors text-[#1e88e5] bg-[#e6f1fb] hover:bg-[#d8e8f9]"
    >
      <span class="max-w-[240px] truncate">{activeTitle}</span>
      <ChevronDown size={14} class="text-[#1e88e5]/70" />
    </button>
    {#if switcherOpen}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="fixed inset-0 z-40" onclick={() => (switcherOpen = false)}></div>
      <div
        role="dialog"
        aria-label="Switch diagram"
        class="absolute top-[calc(100%+6px)] left-1/2 -translate-x-1/2 z-50 w-[300px] rounded-lg border border-[#d8dee8] bg-white shadow-[0_8px_24px_rgba(15,23,42,0.18)]"
      >
        <div class="flex items-center gap-1 px-2 py-1.5 border-b border-[#eef1f5]">
          <span class="flex-1 text-[12px] font-[600] text-slate-500 px-1">Diagrams</span>
          <button
            onclick={() => {
              switcherOpen = false;
              onDockModel?.();
            }}
            aria-label="Dock model editor"
            title="Dock the model editor to the left"
            class="w-[28px] h-[28px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
          >
            <PanelLeft size={15} />
          </button>
          <button
            onclick={() => {
              switcherOpen = false;
              onEditModel?.();
            }}
            aria-label="Edit model"
            title="Open the full model editor"
            class="w-[28px] h-[28px] flex items-center justify-center rounded-md text-slate-500 hover:bg-[#f1f3f7]"
          >
            <Pencil size={15} />
          </button>
        </div>
        <div role="listbox" aria-label="Diagrams" class="py-1 max-h-[320px] overflow-y-auto">
          {#each diagrams as d (d.key)}
            <button
              role="option"
              aria-selected={d.key === activeDiagramKey}
              onclick={() => {
                switcherOpen = false;
                onSelectDiagram?.(d.key);
              }}
              class="w-full text-left text-[13px] text-slate-900 px-3 py-2 cursor-pointer flex items-center gap-[8px] hover:bg-[#f1f3f7]"
            >
              <FileText size={15} class="text-slate-400 flex-none" />
              <span class="flex-1 truncate">{d.title}</span>
              {#if d.key === activeDiagramKey}<Check size={15} class="text-[#1e88e5] flex-none" />{/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/TopBar.test.ts` — all TopBar tests green (the existing title-treatment and Business-Goal-absence tests still pass — same button classes and `switch diagram` accessible name).
- [ ] **Write failing test — rewrite the Canvas switcher test.** In `Canvas.test.ts`, replace the test at lines 31-35 (`"opens the Navigator sheet from the switcher"`) with:
```ts
  it("opens the read-only switcher dropdown; Edit escalates into the full navigator", async () => {
    render(Canvas);
    await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
    // Read-only dropdown first: the navigator's search field is NOT mounted yet.
    expect(screen.queryByLabelText("Search model")).toBeNull();
    // Edit opens the full editor.
    await fireEvent.click(screen.getByRole("button", { name: /edit model/i }));
    expect(screen.getByLabelText("Search model")).toBeTruthy();
  });
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — fails: `CanvasInner` still passes `navOpen`/`onToggleNav`, so clicking the title opens the navigator directly (mounting "Search model" immediately) and there is no "Edit model" button yet. (`svelte-check` in `pnpm build` would also reject the removed props — fixed next.)
- [ ] **Minimal implementation — rewire the `<TopBar>` render in `CanvasInner`.** Replace lines 632-633 (`navOpen={navOpen}` / `onToggleNav={...}`) with the three callbacks:
```svelte
    diagrams={diagrams}
    activeDiagramKey={activeDiagram.key}
    onSelectDiagram={(key) => {
      // Same selection-reset as the navigator's own onSelectDiagram: a selection
      // made in one diagram must never carry into another (esp. a read-only
      // Flow/Sequence view). navOpen = false here is a harmless no-op.
      activeDiagramKey = key;
      selectionSet = EMPTY_SELECTION;
      inspectorDiagramScope = false;
      navOpen = false;
    }}
    onDockModel={() => {
      navOpen = true;
      navMode = "docked";
    }}
    onEditModel={() => {
      navOpen = true;
      navMode = "centered";
    }}
  />
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — green.
- [ ] **Full gate:** `pnpm --filter @waml/web exec vitest run` — whole web suite green.
- [ ] **Commit:**
```
git add packages/web/src/components/TopBar.svelte packages/web/src/components/TopBar.test.ts packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.test.ts
git commit -m "$(cat <<'EOF'
feat(topbar): read-only diagram switcher dropdown

The TopBar title now opens a read-only popover listing every diagram with a
check on the active one; a row click fires onSelectDiagram, and header Dock /
Edit buttons escalate into the full editor (docked / centered). Drops
navOpen/onToggleNav; CanvasInner wires onSelectDiagram/onDockModel/onEditModel.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ALcN4SscYGB3AXASsMzoVq
EOF
)"
```

---

## Task 4 — Dock left-offset slide (+ CanvasInner Dock offset)

Give the tool `Dock` a `leftOffset` prop so it slides right by the docked rail's width instead of sitting under it. `CanvasInner` computes `navOpen && navMode === "docked" ? navWidth + 12 : 14`, mirroring how the right-edge flags slide by `inspectorWidth`.

**Files:**
- Modify: `packages/web/src/components/canvas/Dock.svelte` (props block ~lines 12-25; root element ~lines 132-136)
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (`<Dock>` render ~lines 723-729)
- Test: `packages/web/src/components/canvas/Dock.test.ts`, `packages/web/src/components/canvas/Canvas.test.ts`

**Interfaces:**
- Consumes (Dock new): `leftOffset?: number` (px, default `14`).
- Produces (CanvasInner): passes `leftOffset={navOpen && navMode === "docked" ? navWidth + 12 : 14}` (with `navWidth` default `340`).

### Steps

- [ ] **Write failing test — `leftOffset` drives the dock's left position, in `Dock.test.ts`.** Append:
```ts
test("defaults to a 14px left offset", () => {
  const { container } = render(Dock, { props: baseProps() });
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("14px");
});

test("leftOffset slides the dock clear of the docked rail", () => {
  const { container } = render(Dock, { props: { ...baseProps(), leftOffset: 352 } });
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("352px");
});
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Dock.test.ts` — fails: the root uses the static Tailwind class `left-[14px]` and sets no inline `style.left`, so `dock.style.left` is `""` for both cases.
- [ ] **Minimal implementation — add the prop.** In `Dock.svelte`, extend the destructuring + type (lines 12-25). Add `leftOffset = 14,` after `onOpenProperties,` and `leftOffset?: number;` after `onOpenProperties?: () => void;`:
```ts
  let {
    activeTool,
    onToolChange,
    onClear,
    clearDisabled,
    onOpenProperties,
    leftOffset = 14,
  }: {
    activeTool: Tool;
    onToolChange: (tool: Tool) => void;
    onClear: () => void;
    clearDisabled?: boolean;
    // Opens the central edit panel's diagram-properties context.
    onOpenProperties?: () => void;
    // px from the canvas left edge; CanvasInner slides it right to clear the
    // docked navigator rail. Transitions so it glides rather than jumps.
    leftOffset?: number;
  } = $props();
```
- [ ] **Minimal implementation — drive the left position inline.** Replace the root element opening tag at lines 132-136 (drop `left-[14px]`, add `transition-[left]`, set `left` via inline style):
```svelte
<div
  data-dock
  class="absolute top-[calc(50%-34px)] -translate-y-1/2 bg-white border border-[#d8dee8] rounded-xl p-[6px] flex flex-col gap-1 z-20 shadow-[0_4px_16px_rgba(15,23,42,0.06)] transition-[left] duration-200"
  style={`left: ${leftOffset}px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;`}
>
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Dock.test.ts` — green.
- [ ] **Write failing test — Dock slide on docked navigator, in `Canvas.test.ts`.** Append a top-level `test`:
```ts
test("slides the tool Dock clear of the docked navigator rail", async () => {
  const { container } = render(Canvas);
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("14px");
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /dock model editor/i }));
  await tick();
  // navWidth default (340) + 12px gap.
  expect(dock.style.left).toBe("352px");
});
```
- [ ] **Run it (expected FAIL):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — fails: `CanvasInner` does not pass `leftOffset`, so the dock stays at the `14px` default even after docking (final assertion expects `352px`).
- [ ] **Minimal implementation — pass the computed offset.** In `CanvasInner.svelte`, add the prop to the `<Dock>` render (after `onOpenProperties={...}` at line 728):
```svelte
        onOpenProperties={() => (centralPanel = { kind: "diagram" })}
        leftOffset={navOpen && navMode === "docked" ? navWidth + 12 : 14}
      />
```
- [ ] **Run it (expected PASS):** `pnpm --filter @waml/web exec vitest run src/components/canvas/Canvas.test.ts` — green.
- [ ] **Full gate:** `pnpm --filter @waml/web exec vitest run` — whole web suite green.
- [ ] **Commit:**
```
git add packages/web/src/components/canvas/Dock.svelte packages/web/src/components/canvas/Dock.test.ts packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.test.ts
git commit -m "$(cat <<'EOF'
fix(canvas): slide tool Dock clear of the docked navigator rail

Add a leftOffset prop to Dock (default 14px, transitions on change). While
the navigator is open and docked, CanvasInner slides the Dock right by
navWidth + 12, mirroring the right-edge flags' inspectorWidth slide, so the
rail no longer overlaps the tool dock.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01ALcN4SscYGB3AXASsMzoVq
EOF
)"
```

---

## Self-review

**Spec coverage**
- Unit 1 (read-only switcher dropdown, list + active check, row select, Dock/Edit, outside-click/Escape, remove `navOpen`/`onToggleNav`) → Task 3.
- Unit 2 (NavigatorPanel centered keeps stick-to-side; docked gains pin + collapse + close; `pinned`/`onTogglePin`; docked->centered toggle removed) → Task 2.
- Unit 3 (pin flip both panels; `translucent = !pinned && !engaged`; inspector icon/label swap; `inspectorPinned` default true; `navPinned` default false) → Tasks 1 & 2.
- Unit 4 (docked-rail / tool-Dock overlap via `leftOffset` slide) → Task 4.
- CanvasInner wiring (navPinned, TopBar callbacks, NavigatorPanel pin wiring, Dock offset) → distributed across Tasks 2/3/4 so every commit keeps the `Canvas.test.ts` integration suite and `svelte-check` green (documented under Global Constraints).
- Testing section (TopBar/NavigatorPanel/InspectorPanel/Canvas assertions) → covered per task.

**Placeholder scan:** No "similar to Task N", no TODO/ellipsis stand-ins — every test and implementation block is spelled out in full (repeated where needed).

**Type consistency:** New prop types are exact — `onSelectDiagram?: (key: string) => void`, `onDockModel?: () => void`, `onEditModel?: () => void` (TopBar); `pinned?: boolean` + `onTogglePin: () => void` (NavigatorPanel, mirroring InspectorPanel); `leftOffset?: number` (Dock). `CanvasInner` locals: `navPinned: boolean` (`$state(false)`), `inspectorPinned` default flips to `$state(true)`. `Diagram` rows are keyed by `d.key: string`; `onSelectDiagram(d.key)` passes a `string`.

**Ordering / green-commit check:** TopBar prop removal (Task 3) lands in the same commit as the CanvasInner `<TopBar>` rewire and the `Canvas.test.ts` update, so `svelte-check` never sees stale `navOpen`/`onToggleNav` props. NavigatorPanel's new required `onTogglePin` (Task 2) is wired in CanvasInner and added to the NavigatorPanel test props helper in the same commit. Dock's `leftOffset` (Task 4) is optional with a `14` default, so intermediate states never break.
