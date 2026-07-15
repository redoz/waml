# Diagram Properties Panel v2

**Status:** Proposed (not yet planned)
**Date:** 2026-07-15
**Product:** WAML Model Canvas (`packages/web`, Svelte 5 + SvelteFlow)
**Depends on:** `2026-07-15-diagram-field-persistence-design.md` (**hard
prerequisite** — this spec assumes the persistence path, the `DiagramDisplay`
field set, `Diagram.description`, and the real `store.updateDiagram` it ships).
No session-store fallback exists after that spec; every setting here reads
`activeDiagram.display` / `activeDiagram.description` and writes through
`store.updateDiagram`.

## Context

The diagram properties panel today (`DiagramPropertiesBody.svelte`, rendered by
`CentralEditPanelHost.svelte` in the `{ kind: "diagram" }` branch, opened by the
Dock sliders button) is **display toggles only**. It exposes five controls:
Show attributes, Attribute detail, Associations, Emphasize multiplicity, Show
stereotype. It has **no** title or note editing (`CentralEditPanelHost` only
wires `display` / `onDisplayChange` for the diagram branch, L16-24, L47-50).

The prerequisite spec adds five new `DiagramDisplay` fields
(`showAttributeVisibility`, `showAttributeMultiplicity`, `maxAttributes`,
`stereotypeFilter`, `stereotypeColors`) plus `Diagram.description`, and makes all
of them persist. This spec builds the **UI to author them** and the **canvas
rendering** that honours them.

Canvas render sites this spec touches (verified current state):

- `nodes/AttributeRow.svelte` — `showVisibility?` gates the `+/-/#/~` marker;
  `showTypes` (default `true`) currently bundles **both** the type name **and**
  the `{mult}` suffix into one conditional (L12-16).
- `nodes/ClassifierBox.svelte` — derives `display = resolveDisplay(data._display)`
  (L16), `st = stereotypeStyle(profile, data.stereotypes)` (L14),
  `showVisibility = !profile.hide.includes("visibility")` (L20), and `boxStyle`
  applying `st.header` as a 4px `border-top-color` (L23-31). Renders
  `<StereotypeRow stereotypes={data.stereotypes} …>` (L43) and the attribute
  `<RowsCompartment>` (L57-63).
- `nodes/StereotypeRow.svelte` — renders `keyword` then every `stereotypes[]` tag
  (L5-10).
- `nodes/RowsCompartment.svelte` — folds rows past `ERD_COLLAPSED_ROWS` behind an
  **interactive** "+N more" expand toggle (L12-32).
- `profiles/index.ts` — `stereotypeStyle(profile, stereotypes)` reduces every
  named stereotype's style, **later stereotype wins per property** (L23-25). The
  color override mirrors this precedence.

## Goal

Extend the diagram properties panel into a full authoring surface for the
prerequisite spec's field set, and make the canvas render every new field:

1. A **title + note** section at the top of the panel (the panel gains its first
   text-editing controls).
2. New display controls: attribute **visibility** toggle, attribute
   **multiplicity** toggle, a **max attributes** cap, a **stereotype allowlist**
   picker, and **per-stereotype colors**.
3. Canvas rendering for all of the above, including a light background **color
   wash** keyed on stereotype color.

## Panel UI (`DiagramPropertiesBody.svelte` v2)

The component's props grow from `{ display, onChange }` to:

```ts
let { display, diagram, candidateStereotypes, editable, onChange, onUpdateDiagram }: {
  display: DiagramDisplay;                 // resolved (full)
  diagram: Diagram;                        // the active diagram (title/description/key)
  candidateStereotypes: string[];          // live scan; may be empty
  editable: boolean;                       // false for the implicit "All" view
  onChange: (patch: Partial<DiagramDisplay>) => void;      // display fields
  onUpdateDiagram: (patch: Partial<Diagram>) => void;      // title / description
} = $props();
```

Layout, top to bottom:

### 1. Title + Note (new section)

- **Title** — a single-line text input bound to `diagram.title`. Commits on blur
  (and Enter) via `onUpdateDiagram({ title })`, matching the node inspector's
  title-field cadence (one op per commit, not per keystroke).
- **Note** — a multi-line `<textarea>` bound to `diagram.description ?? ""`.
  Commits on blur via `onUpdateDiagram({ description })`. Placeholder: "Notes about
  this diagram (not shown on the canvas)." Explicitly **panel-only metadata** — the
  note is never rendered on the canvas; it exists so a reader can document intent
  where the properties panel is open. An empty textarea commits `description: ""`
  (which the prerequisite spec writes as an empty scalar; acceptable — no
  round-trip surprise since an authored-then-cleared note simply becomes `""`).

### 2. Attributes

- **Show attributes** — toggle (`showAttributes`). Existing.
- **Attribute detail** — segmented *Name only | Name + type* (`attributeDetail`;
  disabled when `showAttributes` off). Existing. Now governs **only** the type
  name, not multiplicity (see rendering).
- **Show visibility** — toggle (`showAttributeVisibility`; disabled when
  `showAttributes` off). New.
- **Show multiplicity** — toggle (`showAttributeMultiplicity`; disabled when
  `showAttributes` off). New. Independent of Attribute detail.
- **Max attributes** — a small number control (stepper or numeric input) for
  `maxAttributes`, plus an "Unlimited" affordance. "Unlimited" emits
  `onChange({ maxAttributes: undefined })`; a number `n >= 1` emits
  `onChange({ maxAttributes: n })`. Disabled when `showAttributes` off. New.

### 3. Associations (unchanged)

- **Associations** — *Show labels | Hide labels* (`associationLabels`).
- **Emphasize multiplicity** — toggle (`emphasizeMultiplicity`).

### 4. Stereotypes

- **Show stereotype** — toggle (`showStereotype`). Existing — master on/off for
  the whole stereotype/keyword row.
- **Stereotype filter** — an allowlist picker over `candidateStereotypes`
  (checkbox list or multi-select chips). Semantics map exactly to the prerequisite
  field `stereotypeFilter`:
  - A dedicated **"Show all"** state (default) — no allowlist. Selecting it emits
    `onChange({ stereotypeFilter: undefined })`.
  - Toggling individual names builds an explicit allowlist:
    `onChange({ stereotypeFilter: [selected…] })`. Deselecting every name yields
    `stereotypeFilter: []` ⇒ **show none** — which is distinct from "Show all". The
    UI must present "Show all" as a separate control (e.g. a leading "All"
    radio/checkbox) so the user can return to it; otherwise there is no path from
    an empty allowlist back to unfiltered.
  - Disabled when `showStereotype` off.
  - **Known UX property (not a bug):** `candidateStereotypes` is scanned **live**
    from stereotypes actually present on the active diagram's member nodes, not
    from a fixed profile palette. The list is empty on a diagram whose members
    carry no stereotypes yet, and it grows/shrinks as the model changes. Names in
    a saved `stereotypeFilter` that are no longer present on any member simply do
    not appear as candidates but remain stored (they re-appear if the stereotype
    returns). The panel shows a muted "No stereotypes on this diagram's members
    yet" hint when the candidate list is empty.
- **Stereotype colors** — for each name in `candidateStereotypes`, a swatch +
  color picker bound to `display.stereotypeColors[name]`. Picking a color emits
  `onChange({ stereotypeColors: { ...display.stereotypeColors, [name]: hex } })`;
  a "clear" affordance removes the key
  (`onChange({ stereotypeColors: <copy without name> })`). Disabled when
  `showStereotype` off.

### 5. Implicit "All" view

When `editable` is `false` (the active diagram is the implicit "All" view, which
has no backing document and therefore cannot persist — see the prerequisite
spec's "Known limitation"), the panel renders a persistent muted banner at the
top — "Display and note settings save to a diagram. The **All** view can't store
them — create a diagram to customize." — and renders every control **disabled**.
This is deliberately honest: enabled controls that silently no-op would mislead.
Wiring `store.addDiagram` to materialize a real diagram (removing this
limitation) is out of scope for both specs.

## Panel host + Canvas wiring

### `CentralEditPanelHost.svelte`

The diagram branch (L47-50) grows to pass the new props through. Host props gain
`diagram`, `candidateStereotypes`, `editable`, `onUpdateDiagram`:

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

### `CanvasInner.svelte`

- `candidateStereotypes` — a new `$derived`: unique stereotype names across the
  active diagram's member nodes,
  `[...new Set($model.nodes.filter(n => memberSet.has(n.key)).flatMap(n => n.stereotypes))].sort()`.
- `editable` — `activeDiagram.key !== ALL_DIAGRAM_KEY && diagrams.some(d => d.key === activeDiagram.key && d !== implicit)`; in practice
  `activeDiagram.key !== ALL_DIAGRAM_KEY` (the implicit view is the only
  non-persistable case; `effectiveDiagrams` gives it that key).
- Pass `diagram={activeDiagram}`, `candidateStereotypes`, `editable`, and
  `onUpdateDiagram={(patch) => store.updateDiagram(activeDiagram.key, patch)}` into
  `<CentralEditPanelHost>` (the existing `onDisplayChange={handleDisplayChange}`
  stays — `handleDisplayChange` already persists via the prerequisite spec).

## Canvas rendering changes

All new fields arrive at the box via `data._display`
(`resolveDisplay(data._display)` in `ClassifierBox.svelte`), which the
prerequisite spec already routes end to end (`toRFNode` → `_display`). No new
plumbing into the node data is needed beyond the fields themselves.

### Attribute visibility gate (`ClassifierBox` + `AttributeRow`)

`ClassifierBox.svelte` computes the effective visibility flag as the **AND** of
the profile floor and the diagram flag:

```ts
let showVisibility = $derived(!profile.hide.includes("visibility") && display.showAttributeVisibility);
```

**Interaction (explicit, no ambiguity):** `profile.hide.includes("visibility")`
is a hard floor — when the profile hides visibility, the marker never renders,
regardless of the diagram flag. When the profile allows it, `showAttributeVisibility`
decides. Effective = `profileAllows AND diagramWants`. `AttributeRow`'s
`showVisibility` prop is unchanged in meaning; only its source expression changes.

### Attribute multiplicity split (`AttributeRow`)

`AttributeRow` gains an independent `showMultiplicity` prop; `showTypes` reverts
to meaning **only** the type name:

```svelte
let { a, showVisibility, showTypes = true, showMultiplicity = true }:
  { a: Attribute; showVisibility?: boolean; showTypes?: boolean; showMultiplicity?: boolean } = $props();
```

The trailing column renders when `showTypes || showMultiplicity`, emitting the
type name when `showTypes` and the `{mult}` suffix (only when `a.multiplicity !==
"1"`) when `showMultiplicity`:

```svelte
{#if showTypes || showMultiplicity}
  <span class="text-slate-400 font-mono text-[10.5px] truncate">
    {#if showTypes}{a.type.name}{/if}{#if showMultiplicity && a.multiplicity !== "1"} {`{${a.multiplicity}}`}{/if}
  </span>
{/if}
```

`ClassifierBox` passes `showTypes={display.attributeDetail === "name-type"}` (as
today) and `showMultiplicity={display.showAttributeMultiplicity}` into
`<AttributeRow>`.

### Max attributes cap (`RowsCompartment`)

`RowsCompartment` gains an optional `max?: number` prop. When `max` is set, it
**overrides** the interactive `ERD_COLLAPSED_ROWS` expand/collapse: it shows at
most `max` rows and a **static, non-interactive** "+K more" footer for the
remainder (`K = rows - max`), because the cap is a diagram authoring choice, not
a per-box UI toggle. When `max` is `undefined`, today's interactive expand/collapse
is unchanged.

`ClassifierBox` passes `max={display.maxAttributes}` **only** to the attribute
`<RowsCompartment>` (L57-63), never to the enum-values compartment (L48-55) —
`maxAttributes` caps attribute rows only. When `showAttributes` is off the box
already shows a collapsed count, so the cap has no effect there.

### Stereotype filter (`ClassifierBox` + `StereotypeRow`)

`ClassifierBox` computes the filtered tag list and passes it to `StereotypeRow`:

```ts
let stereotypeTags = $derived(
  display.stereotypeFilter === undefined
    ? data.stereotypes
    : data.stereotypes.filter((s) => display.stereotypeFilter!.includes(s)),
);
```

`<StereotypeRow stereotypes={stereotypeTags} {keyword} />`. `StereotypeRow` is
unchanged internally. **Interaction (explicit):**

- `showStereotype: false` — the whole `{#if showStereotype}` wrapper in
  `ClassifierBox` (L42-44) is skipped: no keyword, no tags.
- `showStereotype: true` + `stereotypeFilter: undefined` — every stereotype shows
  (today's behavior).
- `showStereotype: true` + `stereotypeFilter: []` — **no tags**, but the
  `keyword` still renders (StereotypeRow shows the row when `keyword ||
  stereotypes.length > 0`). This is the documented distinction from
  `showStereotype: false`.
- `showStereotype: true` + `stereotypeFilter: ["entity", …]` — only listed tags.

### Stereotype colors (`ClassifierBox`)

A per-box override color is chosen by mirroring `stereotypeStyle`'s
**later-stereotype-wins** precedence: reduce over `data.stereotypes`, keeping the
color of the **last** stereotype that has an entry in `display.stereotypeColors`.

```ts
let overrideHeader = $derived(
  data.stereotypes.reduce<string | undefined>(
    (acc, s) => display.stereotypeColors[s] ?? acc, undefined),
);
let headerColor = $derived(overrideHeader ?? st.header);
```

`headerColor` (not `st.header`) drives both accents:

1. **4px top accent** — `boxStyle` uses `headerColor` where it used `st.header`
   (L24-25, and the `st.border === "thick"` fallback L26).
2. **Background wash (new)** — when `overrideHeader` is set, the box background
   becomes a light tint of that color instead of plain white:
   `background-color: color-mix(in srgb, {overrideHeader} 12%, white)`. The **12%**
   mix on white is a fixed, deliberately low tint so **text contrast is preserved
   for any chosen color** — the text stays `text-slate-900`, and a 12%-toward-any-hue
   wash keeps a near-white background well within contrast tolerance. `color-mix`
   is broadly supported in the app's target browsers; no per-color luminance math
   is needed precisely because the tint is capped so low. When no override color
   applies, the background stays the existing `bg-white` (the wash is override-only,
   not applied to profile-fixed `st.header`, to avoid changing today's look for
   un-colored stereotypes).

The wash is applied via `boxStyle` (a `background-color` declaration) so it
composes with the existing inline style string; the base `bg-white` utility class
stays as the fallback for the no-override case.

## Edge cases

- **Node with no stereotypes** — `stereotypeTags` empty, `overrideHeader`
  undefined; box renders exactly as today.
- **A colored stereotype filtered out** — colors and the filter are independent:
  a stereotype can be hidden from the tag row by `stereotypeFilter` yet still be
  the box's `overrideHeader` source (it is present on the node). This is
  intentional — color reflects classification, the filter only declutters the tag
  text. Documented so it is not read as a bug.
- **`maxAttributes` larger than the attribute count** — no fold, all rows show.
- **`maxAttributes: 0`** — not reachable from the UI (the control floors at 1;
  "unlimited" is `undefined`), and the prerequisite reader maps `< 1` to unlimited
  anyway.
- **Implicit "All" view** — panel disabled per the section above; rendering still
  honours the resolved default display for that view (nothing to persist, but the
  canvas draws with `DEFAULT_DISPLAY`).

## Out of scope

- Diagram create / delete / membership (still stubbed).
- Removing the implicit-"All" persistence limitation.
- Any change to node-level (per-classifier) properties — this panel is diagram-scoped.

## Testing

**Panel (`DiagramPropertiesBody.svelte`)**
- Renders title/note bound to `diagram`; blur on the note calls
  `onUpdateDiagram({ description })`; blur on title calls `onUpdateDiagram({ title })`.
- New toggles call `onChange` with the right single field
  (`showAttributeVisibility`, `showAttributeMultiplicity`).
- Max attributes: "Unlimited" emits `{ maxAttributes: undefined }`; a number emits
  `{ maxAttributes: n }`.
- Stereotype filter: "Show all" emits `{ stereotypeFilter: undefined }`;
  selecting names emits an allowlist; clearing all names emits `[]`; empty
  `candidateStereotypes` shows the hint.
- Stereotype colors: picking a color merges into the record; clearing removes the
  key; controls disabled when `showStereotype` off.
- `editable: false` renders the banner and disables every control.

**Rendering**
- `AttributeRow`: type name gated by `showTypes`, `{mult}` gated independently by
  `showMultiplicity`; both off ⇒ no trailing column; visibility marker gated by
  `showVisibility`.
- `ClassifierBox`: `showVisibility` is `profileAllows && showAttributeVisibility`
  (test both profile-hides and diagram-hides paths).
- `RowsCompartment` with `max`: caps rows, shows a static "+K more", no expand
  button; without `max`, the interactive expand still works.
- `StereotypeRow` via `ClassifierBox`: `undefined` filter shows all; `[]` shows
  keyword only; a name list shows only those tags; `showStereotype: false` shows
  nothing.
- Stereotype colors: `overrideHeader` follows later-wins precedence across
  multiple stereotypes; `headerColor` drives the 4px accent and a
  `color-mix(... 12%, white)` background wash; no override ⇒ `bg-white` + profile
  `st.header`.

**Canvas wiring (`CanvasInner`)**
- `candidateStereotypes` reflects only active-diagram members' stereotypes and
  updates as membership/stereotypes change.
- `editable` is false for `ALL_DIAGRAM_KEY`, true for a real diagram.
- `onUpdateDiagram` routes to `store.updateDiagram(activeDiagram.key, patch)`.
