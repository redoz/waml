# DiagramPropertiesBody Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the diagram-properties dialog (`DiagramPropertiesBody.svelte`) match its peer inspectors — grouped sections, shared field styling, deduped disable logic — with zero behavior change.

**Architecture:** Two steps. First extract the duplicated `inputCls` / `labelCls` class strings into one shared module and point the peer inspectors at it. Then rewrite `DiagramPropertiesBody` to consume that module, wrap its flat control list in four titled sections, and collapse the repeated attribute-disable expression into one derived value.

**Tech Stack:** Svelte 5 (runes: `$props`, `$derived`, snippets), TypeScript, Tailwind (utility + arbitrary-value classes), Vitest + `@testing-library/svelte`, pnpm workspace.

## Global Constraints

- **No behavior change.** Every field, the `onChange` / `onUpdateDiagram` contract, the not-editable warning banner, and all disabled behavior stay exactly as they are.
- **Preserve every accessibility handle.** All existing `aria-label`, `role="switch"`, `role="radio"`, `role="radiogroup"`, `role="checkbox"`, `role="note"`, and label text stay byte-identical — the 18 existing tests query by these.
- **No new colors.** Reuse only hex literals already present in the app (`#1e88e5`, `#f1f3f7`, `#d8dee8`, `#e6f1fb`, `#fff7ed`, `#9a3412`, ...). No design-token layer.
- **Leave `AttributeEditor`'s `inputCls` alone** — its `text-[12.5px] px-[7px] py-[5px]` variant is a deliberately denser grid field, not a duplicate.
- **Baseline:** full web suite green — `pnpm --filter @waml/web test` → 49 files / 279 tests, 0 failures. A fresh worktree must first build the workspace packages (`pnpm build:wasm` then `pnpm -r --filter=!@waml/web build`) or vitest cannot resolve `@waml/okf` / `@waml/wasm`.

---

## File Structure

- **Create:** `packages/web/src/components/inspector/field-styles.ts` — the single source for the shared `inputCls` and `labelCls` class strings.
- **Modify:** `packages/web/src/components/inspector/ObjectInspector.svelte` — import the two constants instead of redefining them.
- **Modify:** `packages/web/src/components/inspector/RelationshipInspector.svelte` — import `inputCls` instead of redefining it.
- **Modify:** `packages/web/src/components/inspector/ObjectInspectorReadonly.svelte` — import `labelCls` instead of redefining it.
- **Modify:** `packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte` — import `labelCls` instead of redefining it.
- **Modify:** `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` — consume the shared styles, add four titled sections, dedupe the attribute-disable expression.
- **Modify:** `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts` — add one test asserting the four section headings render.

---

## Task 1: Shared field-style module + peer migration

**Files:**
- Create: `packages/web/src/components/inspector/field-styles.ts`
- Modify: `packages/web/src/components/inspector/ObjectInspector.svelte:18-19`
- Modify: `packages/web/src/components/inspector/RelationshipInspector.svelte:28`
- Modify: `packages/web/src/components/inspector/ObjectInspectorReadonly.svelte:14`
- Modify: `packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte:16`
- Test: existing suite (`packages/web/src/components/inspector/*.test.ts`, `packages/web/src/**/*.test.ts`) is the guard — no new test; this is a render-identical extraction.

**Interfaces:**
- Produces: module `../inspector/field-styles` (relative `./field-styles` from the inspector folder) exporting two `string` constants:
  - `inputCls = "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]"`
  - `labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]"`
- Note: `RelationshipInspector`'s current `inputCls` uses `py-[8px]`; the canonical value uses `py-2`. `py-2` computes to the same `0.5rem` (8px), so the rendered field is visually identical — only the class token changes.

- [ ] **Step 1: Confirm the baseline is green**

Run: `pnpm --filter @waml/web test`
Expected: `Test Files 49 passed (49)`, `Tests 279 passed (279)`. (If workspace packages are unbuilt, first run `pnpm build:wasm` then `pnpm -r --filter=!@waml/web build`.)

- [ ] **Step 2: Create the shared module**

Create `packages/web/src/components/inspector/field-styles.ts`:

```ts
// Shared Tailwind class strings for inspector form fields, so the object,
// relationship, and diagram inspectors render identical inputs and section
// labels from one source instead of drifting copies. AttributeEditor keeps its
// own denser variant on purpose.
export const inputCls =
  "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";

export const labelCls =
  "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
```

- [ ] **Step 3: Migrate `ObjectInspector.svelte`**

Add the import alongside the other imports (after `import { getProfile } from "@waml/core/profiles";`):

```svelte
  import { inputCls, labelCls } from "./field-styles";
```

Delete the two local definitions (lines 18-19):

```svelte
  const inputCls = "w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";
  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
```

- [ ] **Step 4: Migrate `RelationshipInspector.svelte`**

Add the import after `import InfoTip from "./InfoTip.svelte";`:

```svelte
  import { inputCls } from "./field-styles";
```

Delete the local definition (line 28):

```svelte
  const inputCls = "w-full text-[13px] px-[10px] py-[8px] border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb]";
```

- [ ] **Step 5: Migrate `ObjectInspectorReadonly.svelte`**

Add the import after `import { nodeAssociations } from "./associations";`:

```svelte
  import { labelCls } from "./field-styles";
```

Delete the local definition (line 14), leaving `valueCls` and `emptyCls` in place:

```svelte
  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
```

- [ ] **Step 6: Migrate `RelationshipInspectorReadonly.svelte`**

Add the import after `import { ENDED_KINDS } from "@waml/okf";`:

```svelte
  import { labelCls } from "./field-styles";
```

Delete the local definition (line 16), leaving `valueCls` in place:

```svelte
  const labelCls = "block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]";
```

- [ ] **Step 7: Run the full web suite — rendering is unchanged**

Run: `pnpm --filter @waml/web test`
Expected: `Test Files 49 passed (49)`, `Tests 279 passed (279)`.

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/inspector/field-styles.ts \
        packages/web/src/components/inspector/ObjectInspector.svelte \
        packages/web/src/components/inspector/RelationshipInspector.svelte \
        packages/web/src/components/inspector/ObjectInspectorReadonly.svelte \
        packages/web/src/components/inspector/RelationshipInspectorReadonly.svelte
git commit -m "refactor(web): share inspector field styles from one module"
```

---

## Task 2: Restructure DiagramPropertiesBody into titled sections

**Files:**
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` (full rewrite of the component)
- Test: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts` (add one heading test)

**Interfaces:**
- Consumes: `inputCls`, `labelCls` from `../inspector/field-styles` (Task 1).
- Produces: nothing new for other tasks — same props, same `onChange` / `onUpdateDiagram` contract.

- [ ] **Step 1: Write the failing test**

Append to `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`:

```ts
test("renders the four section headings", () => {
  render(DiagramPropertiesBody, { props: props() });
  for (const name of ["Identity", "Attributes", "Relationships", "Stereotypes"]) {
    expect(screen.getByRole("heading", { name })).toBeTruthy();
  }
});
```

- [ ] **Step 2: Run the test — it fails**

Run: `pnpm --filter @waml/web exec vitest run DiagramPropertiesBody`
Expected: FAIL — the new test errors with `Unable to find an accessible element with the role "heading"` (headings don't exist yet); the other 18 tests still pass.

- [ ] **Step 3: Rewrite the component**

Replace the entire contents of `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` with:

```svelte
<script lang="ts">
  // The active diagram's display controls, extracted from Dock's popover so the
  // central edit panel host can render the identical set. Display toggles only — no
  // title/profile. Each control emits a single changed field via onChange.
  import type { DiagramDisplay, Diagram } from "@waml/okf";
  import { inputCls, labelCls } from "../inspector/field-styles";

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

  let disabledAll = $derived(!editable);
  let attrDisabled = $derived(!display.showAttributes || disabledAll);
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

  function commitTitle(v: string) {
    const t = v.trim();
    if (t && t !== diagram.title) onUpdateDiagram({ title: t });
  }
  function commitNote(v: string) {
    if (v !== (diagram.description ?? "")) onUpdateDiagram({ description: v });
  }

  // In-section field label (Title, Note, Max attributes, toggle rows) — one weight
  // below the uppercase section headers so the hierarchy reads at a glance.
  const fieldLabelCls = "mb-1 block text-[13px] font-medium text-slate-800";
</script>

<!-- A labelled on/off toggle row inside the properties flyout. -->
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

<!-- A two-option segmented control (radio group) inside the properties flyout. -->
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

<div class="flex flex-col gap-4 py-1">
  {#if !editable}
    <div
      role="note"
      class="mx-1 rounded-lg bg-[#fff7ed] px-3 py-2 text-[12px] leading-snug text-[#9a3412]"
    >
      Display and note settings save to a diagram. The <strong>All</strong> view can't store them — create
      a diagram to customize.
    </div>
  {/if}

  <section class="flex flex-col">
    <h3 class="px-2 {labelCls}">Identity</h3>
    <div class="px-2">
      <label class="block">
        <span class={fieldLabelCls}>Title</span>
        <input
          type="text"
          aria-label="Diagram title"
          value={diagram.title}
          disabled={disabledAll}
          onblur={(e) => commitTitle((e.currentTarget as HTMLInputElement).value)}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              (e.currentTarget as HTMLInputElement).blur();
            }
          }}
          class={`${inputCls} disabled:opacity-40`}
        />
      </label>
      <label class="mt-2 block">
        <span class={fieldLabelCls}>Note</span>
        <textarea
          aria-label="Diagram note"
          rows="3"
          disabled={disabledAll}
          placeholder="Notes about this diagram (not shown on canvas)."
          value={diagram.description ?? ""}
          onblur={(e) => commitNote((e.currentTarget as HTMLTextAreaElement).value)}
          class={`${inputCls} resize-y disabled:opacity-40`}
        ></textarea>
      </label>
    </div>
  </section>

  <section class="flex flex-col">
    <h3 class="px-2 {labelCls}">Attributes</h3>
    <div>
      {@render toggleRow("Show attributes", display.showAttributes, () =>
        patch({ showAttributes: !display.showAttributes }), disabledAll,
      )}
      {@render segmented(
        "Attribute detail",
        [
          { value: "name-only", label: "Name only" },
          { value: "name-type", label: "Name + type" },
        ],
        display.attributeDetail,
        (v) => patch({ attributeDetail: v as DiagramDisplay["attributeDetail"] }),
        attrDisabled,
      )}
      {@render toggleRow("Show visibility", display.showAttributeVisibility, () =>
        patch({ showAttributeVisibility: !display.showAttributeVisibility }), attrDisabled,
      )}
      {@render toggleRow("Show multiplicity", display.showAttributeMultiplicity, () =>
        patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), attrDisabled,
      )}
      <div class="px-2 py-1.5 {attrDisabled ? 'opacity-40' : ''}">
        <div class="mb-1 text-[13px] font-medium text-slate-800">Max attributes</div>
        <div class="flex items-center gap-2">
          <input
            type="number"
            min="1"
            aria-label="Max attributes"
            placeholder="∞"
            value={display.maxAttributes ?? ""}
            disabled={attrDisabled}
            oninput={(e) => {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isFinite(n) && n >= 1) patch({ maxAttributes: Math.floor(n) });
            }}
            class="w-16 rounded-md border border-slate-300 px-2 py-1 text-[13px] disabled:opacity-40"
          />
          <button
            type="button"
            aria-label="Unlimited attributes"
            disabled={attrDisabled}
            onclick={() => {
              if (!attrDisabled) patch({ maxAttributes: undefined });
            }}
            class="rounded-md px-2 py-1 text-[12px] font-semibold {display.maxAttributes === undefined
              ? 'bg-white text-[#1e88e5] shadow-sm'
              : 'text-slate-500'}"
          >
            Unlimited
          </button>
        </div>
      </div>
    </div>
  </section>

  <section class="flex flex-col">
    <h3 class="px-2 {labelCls}">Relationships</h3>
    <div>
      {@render toggleRow("Show roles", display.showRoles, () =>
        patch({ showRoles: !display.showRoles }), disabledAll,
      )}
      {@render toggleRow("Show cardinality", display.showCardinality, () =>
        patch({ showCardinality: !display.showCardinality }), disabledAll,
      )}
      {@render toggleRow("Show labels", display.showLabels, () =>
        patch({ showLabels: !display.showLabels }), disabledAll,
      )}
    </div>
  </section>

  <section class="flex flex-col">
    <h3 class="px-2 {labelCls}">Stereotypes</h3>
    <div>
      {@render toggleRow("Show stereotype", display.showStereotype, () =>
        patch({ showStereotype: !display.showStereotype }), disabledAll,
      )}

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
    </div>
  </section>
</div>
```

- [ ] **Step 4: Run the DiagramPropertiesBody suite — all green**

Run: `pnpm --filter @waml/web exec vitest run DiagramPropertiesBody`
Expected: PASS — 19 tests (the original 18 plus the new heading test).

- [ ] **Step 5: Run the full web suite — nothing else regressed**

Run: `pnpm --filter @waml/web test`
Expected: `Test Files 49 passed (49)`, `Tests 280 passed (280)`.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/DiagramPropertiesBody.svelte \
        packages/web/src/components/canvas/DiagramPropertiesBody.test.ts
git commit -m "refactor(web): group diagram properties into titled sections"
```

---

## Verification (after both tasks)

- `pnpm --filter @waml/web test` → 49 files, 280 tests, 0 failures.
- Visual check in the running app: open the central edit panel on a diagram, confirm the four titled sections (Identity / Attributes / Relationships / Stereotypes) render top to bottom, Title/Note inputs show the focus ring on focus, and every toggle / segment / number / color / clear control still drives its field. Toggle "Show attributes" off and confirm the four attribute-dependent controls dim and go inert.
