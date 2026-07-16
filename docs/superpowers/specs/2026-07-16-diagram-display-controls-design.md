# Diagram Display Controls Refresh — Design

**Date:** 2026-07-16
**Status:** Approved, pending plan

## Goal

Three refinements to the diagram-properties panel (`DiagramPropertiesBody.svelte`), shaped
while looking at the just-shipped titled-sections layout:

1. Replace the "Attribute detail" two-option segmented control with a single **"Show type"**
   toggle.
2. Rename the attribute-row **"Show multiplicity"** control's visible label to **"Show
   cardinality"** — matching the vocabulary already used, and the wording the author uses.
3. Add a **visual divider** between the four titled sections, which currently flow together.

No change to what any control *does* — only how it is modeled (1), labeled (2), and
separated (3).

## Part 1 — `attributeDetail` enum → `showType: boolean`

### Why

`attributeDetail: "name-only" | "name-type"` is a boolean cosplaying as a string enum. Every
consumer already collapses it: `ClassifierBox.svelte` derives
`showTypes = display.attributeDetail === "name-type"`. A real boolean is the honest model and
lets the UI be a plain toggle instead of a two-button segment.

### Blast radius

`attributeDetail` is a **persisted OKF frontmatter key** that crosses TypeScript and Rust.
This is a wire-format change, not a TS-local rename. Every site that names the old key moves
to `showType`:

**Rust (`crates/`):**
- `waml/src/model.rs` — `attribute_detail: Option<String>` → `show_type: Option<bool>`
- `waml/src/parse.rs` — read new `showType` bool; **also** read legacy `attributeDetail` and,
  when `showType` is absent, map `"name-type"` → `true`, `"name-only"` → `false`. Legacy
  `.waml` files keep working, no silent reset.
- `waml/src/serialize.rs` — round-trip test frontmatter uses `showType`
- `waml/src/ops/mod.rs` — `DiagramDisplaySet.attribute_detail` field, the `DISPLAY_KEYS` list,
  the `fm_set` emit, and tests
- `waml-ops-dto/src/lib.rs` — `DisplayDto.attribute_detail` field, both conversion fns, tests

**Generated:** regenerate `packages/wasm/src/generated/waml_wasm.d.ts` via `pnpm build:wasm`
(`DiagramDisplay.attributeDetail?` and `DisplayDto.attributeDetail` become `showType`).

**TypeScript:**
- `packages/okf/src/types.ts` — `DiagramDisplay.showType: boolean`; `DEFAULT_DISPLAY.showType =
  true` (preserves the old `"name-type"` default = types shown)
- `packages/core/src/state/ops-adapter.ts` — `toDisplayDto` maps `showType`
- `packages/web/src/components/canvas/nodes/ClassifierBox.svelte` — `showTypes =
  display.showType`

**Tests to update** (reference the old key/values):
- `packages/okf/test/display.test.ts`
- `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`

### UI

In `DiagramPropertiesBody.svelte`, the Attributes section drops the `segmented("Attribute
detail", …)` render and gains a `toggleRow("Show type", display.showType, () => patch({ showType:
!display.showType }), attrDisabled)`, placed where the segment was (after "Show attributes").
The `segmented` snippet may become unused — remove it if no other caller remains.

### Migration semantics

- New saves: emit `showType: true|false`, never `attributeDetail`.
- Old files (`attributeDetail: name-only|name-type`, no `showType`): `parse.rs` maps them on
  read. Once resaved, they carry `showType` and drop `attributeDetail`.
- Old file that somehow has *both*: `showType` wins (explicit new key beats legacy).

## Part 2 — "Show multiplicity" label → "Show cardinality"

**Label text only.** In `DiagramPropertiesBody.svelte`, the attribute-row toggle label changes
from `"Show multiplicity"` to `"Show cardinality"`.

**Not renamed:** the field `showAttributeMultiplicity` and the attribute data property
`a.multiplicity`. Renaming either is a *second* cross-language persisted-key migration for a
cosmetic word, and `multiplicity` is the correct UML term baked into the grammar/parser and the
`{mult}` render suffix. The visible label is decoupled from the field name — the field stays
`showAttributeMultiplicity`, only the button text reads "Show cardinality".

The Relationships section's "Show cardinality" already matches; this makes the two sections use
one word for the concept.

### Test impact

`DiagramPropertiesBody.test.ts` queries the toggle by its accessible name. Any test asserting
`aria-label="Show multiplicity"` / `getByLabelText("Show multiplicity")` for the *attribute*
control updates to `"Show cardinality"`. The relationships-section cardinality test is
untouched.

## Part 3 — Section dividers

Each `<section>` after the first gets a top hairline: `border-t border-[#d8dee8] pt-4`
(reusing the existing field-border hex — no new colors). The outer container keeps its
`flex flex-col gap-4`; the border + top padding turn the flat flow into visually separated
groups. A `renders the four section headings` test already guards the headings; add nothing
new here unless a divider needs its own assertion (it does not — it is presentational).

## Out of scope

- Renaming `showAttributeMultiplicity` / `a.multiplicity` (Part 2 is label-only).
- Any change to relationship cardinality wording (already "cardinality").
- Design-token layer / palette rework (deliberately deferred, per the prior cleanup).

## Verification

- Rust: `cargo test` green in `crates/` (parse, serialize round-trip, ops).
- Full workspace build: `pnpm build:wasm` then `pnpm -r --filter=!@waml/web build`.
- Web: `pnpm --filter @waml/web test` green (updated label + showType tests).
- Legacy round-trip: a `.waml` with `attributeDetail: name-only` parses to `showType: false`,
  resaves as `showType: false` with no `attributeDetail`.
- In-app: Attributes section shows a "Show type" toggle and a "Show cardinality" toggle; the
  four sections are separated by hairlines; toggling drives the canvas as before.
