# Diagram Display Controls Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the diagram-properties `attributeDetail` enum into a real `showType` boolean, relabel the attribute multiplicity toggle to "Show cardinality", and add hairline dividers between the panel's titled sections.

**Architecture:** Three sequential tasks along the data-flow. Task 1 migrates the persisted-format across the Rust crates (model + parse + ops + DTO) with a legacy-key read. Task 2 regenerates the WASM types and updates the okf/core TypeScript. Task 3 updates the web components (canvas render + properties panel UI) and their tests.

**Tech Stack:** Rust (serde, tsify_next, wasm-bindgen), pnpm workspace, TypeScript, Svelte 5 runes, Tailwind, Vitest + `@testing-library/svelte`, `cargo test`.

## Global Constraints

- **`showType` default is `true`** — preserves the old `attributeDetail: "name-type"` default (types shown by default).
- **Legacy read, no silent reset** — `parse.rs` must map a legacy `attributeDetail` key (`"name-type"`→`true`, `"name-only"`→`false`) when `showType` is absent. New key wins when both are present.
- **Part 2 is label-only** — the visible text changes; the field `showAttributeMultiplicity` and the attribute data property `a.multiplicity` are NOT renamed.
- **No accessible-name collision** — the Relationships section already has `aria-label="Show cardinality"`. The new attribute toggle shows "Show cardinality" but carries `aria-label="Attribute cardinality"` so it stays programmatically unique.
- **No new colors** — the divider reuses the existing field-border hex `#d8dee8`.
- **Fresh-worktree build** — before any test can resolve `@waml/okf` / `@waml/wasm`, the workspace packages must be built: `pnpm build:wasm` then `pnpm -r --filter=!@waml/web build`. Task 2's WASM regen is what makes the new `showType` type visible to TypeScript.

---

## File Structure

- **Modify (Rust):**
  - `crates/waml/src/model.rs` — `DiagramDisplay.attribute_detail: Option<String>` → `show_type: Option<bool>`
  - `crates/waml/src/parse.rs` — read `showType`, map legacy `attributeDetail`
  - `crates/waml/src/ops/mod.rs` — `DiagramDisplaySet.attribute_detail` → `show_type`; `DISPLAY_KEYS`; `fm_set` emit
  - `crates/waml/src/serialize.rs` — round-trip fixture uses `showType`
  - `crates/waml-ops-dto/src/lib.rs` — `DisplayDto.attribute_detail` → `show_type`; both conversion fns; tests
- **Regenerate:** `packages/wasm/src/generated/waml_wasm.d.ts` (via `pnpm build:wasm`)
- **Modify (TS):**
  - `packages/okf/src/types.ts` — `DiagramDisplay.showType: boolean`; `DEFAULT_DISPLAY`
  - `packages/core/src/state/ops-adapter.ts` — `toDisplayDto`
  - `packages/web/src/components/canvas/nodes/ClassifierBox.svelte` — `showTypes` derive
  - `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` — toggle, label, dividers, snippet cleanup
- **Modify (tests):**
  - `packages/okf/test/display.test.ts`
  - `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`
  - `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`

---

## Task 1: Rust — `attributeDetail` → `show_type` across crates

Rust field renames are tightly coupled (the crate must compile as a whole), so all of `crates/` lands in one task, verified by `cargo test`.

**Files:**
- Modify: `crates/waml/src/model.rs:751`
- Modify: `crates/waml/src/parse.rs:974`, test at `:1006,:1016`
- Modify: `crates/waml/src/ops/mod.rs:43,648-653,677`, test at `:1118`
- Modify: `crates/waml/src/serialize.rs:155`
- Modify: `crates/waml-ops-dto/src/lib.rs:212,230,246,669,709`

**Interfaces:**
- Produces: wire key `showType: boolean` on the diagram `display` frontmatter block, replacing `attributeDetail: string`. Rust structs expose `show_type: Option<bool>` (`DiagramDisplay`) and `show_type: bool` (`DiagramDisplaySet`, `DisplayDto`).

- [ ] **Step 1: Confirm the Rust baseline is green**

Run: `cargo test -p waml -p waml-ops-dto`
Expected: all pass (baseline before edits).

- [ ] **Step 2: Rename the model field**

In `crates/waml/src/model.rs`, replace line 751:

```rust
    pub attribute_detail: Option<String>, // "name-only" | "name-type"
```

with:

```rust
    pub show_type: Option<bool>,
```

- [ ] **Step 3: Read `showType` + legacy `attributeDetail` in parse**

In `crates/waml/src/parse.rs`, replace line 974:

```rust
            attribute_detail: fm.get_str("attributeDetail").map(String::from),
```

with:

```rust
            // New boolean key; fall back to the legacy `attributeDetail` enum so
            // pre-migration .waml files still resolve ("name-type" ⇒ show types).
            show_type: fm.get_bool("showType").or_else(|| fm.get_str("attributeDetail").map(|s| s == "name-type")),
```

- [ ] **Step 4: Update the parse test to assert the mapped boolean**

In `crates/waml/src/parse.rs`, the `build_diagrams_reads_all_display_keys` fixture (line 1006) keeps `attributeDetail: name-only` on purpose — it now doubles as legacy coverage. Replace the assertion at line 1016:

```rust
        assert_eq!(x.attribute_detail.as_deref(), Some("name-only"));
```

with:

```rust
        assert_eq!(x.show_type, Some(false)); // legacy "name-only" ⇒ false
```

- [ ] **Step 5: Add a focused legacy/precedence test**

In `crates/waml/src/parse.rs`, immediately after the `build_diagrams_distinguishes_absent_vs_empty_stereotype_filter` test (ends ~line 1034), add:

```rust
    #[test]
    fn build_diagrams_reads_show_type_and_legacy_attribute_detail() {
        let n = build_model(&diagram_bundle("showType: false\n"));
        assert_eq!(n.diagrams[0].display.show_type, Some(false));
        let t = build_model(&diagram_bundle("attributeDetail: name-type\n"));
        assert_eq!(t.diagrams[0].display.show_type, Some(true));
        let o = build_model(&diagram_bundle("attributeDetail: name-only\n"));
        assert_eq!(o.diagrams[0].display.show_type, Some(false));
        // Explicit new key wins over the legacy key.
        let both = build_model(&diagram_bundle("showType: false\nattributeDetail: name-type\n"));
        assert_eq!(both.diagrams[0].display.show_type, Some(false));
    }
```

- [ ] **Step 6: Rename the ops `DiagramDisplaySet` field**

In `crates/waml/src/ops/mod.rs`, replace line 43:

```rust
    pub attribute_detail: String,
```

with:

```rust
    pub show_type: bool,
```

- [ ] **Step 7: Add `showType` to `DISPLAY_KEYS`, keep `attributeDetail` for legacy stripping**

In `crates/waml/src/ops/mod.rs`, replace the `DISPLAY_KEYS` array (lines 648-653):

```rust
const DISPLAY_KEYS: &[&str] = &[
    "showAttributes", "attributeDetail", "showAttributeVisibility",
    "showAttributeMultiplicity", "maxAttributes", "showRoles",
    "showCardinality", "showLabels", "showStereotype", "stereotypeFilter",
    "stereotypeColors",
];
```

with:

```rust
const DISPLAY_KEYS: &[&str] = &[
    // `attributeDetail` stays listed so a legacy key is stripped on the next
    // whole-block rewrite, even though we only ever emit `showType` now.
    "showAttributes", "showType", "attributeDetail", "showAttributeVisibility",
    "showAttributeMultiplicity", "maxAttributes", "showRoles",
    "showCardinality", "showLabels", "showStereotype", "stereotypeFilter",
    "stereotypeColors",
];
```

- [ ] **Step 8: Emit `showType` in the diagram.set writer**

In `crates/waml/src/ops/mod.rs`, replace line 677:

```rust
            fm_set(&mut doc.frontmatter, "attributeDetail", FmValue::Str(ds.attribute_detail.clone()));
```

with:

```rust
            fm_set(&mut doc.frontmatter, "showType", FmValue::Bool(ds.show_type));
```

- [ ] **Step 9: Update the ops test builder**

In `crates/waml/src/ops/mod.rs`, in `full_display()` replace line 1118:

```rust
            attribute_detail: "name-only".into(),
```

with:

```rust
            show_type: false,
```

- [ ] **Step 10: Sweep remaining `attributeDetail` references in `crates/`**

Run: `grep -rn "attribute_detail\|attributeDetail" crates/`
Any remaining hit is a test assertion on emitted frontmatter (e.g. `assert!(out.contains("attributeDetail: ..."))`) — update it to the `showType: <bool>` form the writer now emits. There should be no non-test hits left.

- [ ] **Step 11: Update the serialize round-trip fixture**

In `crates/waml/src/serialize.rs`, in the `serialize_is_a_semantic_fixpoint_with_diagram_display_frontmatter` fixture (line 155), change the substring `attributeDetail: name-only` to `showType: false`. (The test asserts fixpoint + a few `contains` on other keys; `attributeDetail` is not asserted, so this keeps the fixture schema-current.)

- [ ] **Step 12: Rename the DTO field**

In `crates/waml-ops-dto/src/lib.rs`, replace line 212:

```rust
    pub attribute_detail: String,
```

with:

```rust
    pub show_type: bool,
```

- [ ] **Step 13: Update both DTO conversion functions**

In `crates/waml-ops-dto/src/lib.rs`, replace line 230:

```rust
        attribute_detail: d.attribute_detail.clone(),
```

with:

```rust
        show_type: d.show_type,
```

and replace line 246:

```rust
        attribute_detail: ds.attribute_detail.clone(),
```

with:

```rust
        show_type: ds.show_type,
```

- [ ] **Step 14: Update the DTO tests**

In `crates/waml-ops-dto/src/lib.rs`, replace line 669 (`attribute_detail: "name-only".into(),`) with `show_type: false,` and line 709 (`attribute_detail: "name-type".into(),`) with `show_type: true,`.

- [ ] **Step 15: Run the Rust tests**

Run: `cargo test -p waml -p waml-ops-dto`
Expected: all pass, including the new `build_diagrams_reads_show_type_and_legacy_attribute_detail`.

- [ ] **Step 16: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/ops/mod.rs \
        crates/waml/src/serialize.rs crates/waml-ops-dto/src/lib.rs
git commit -m "feat(waml): replace attributeDetail enum with showType boolean + legacy read"
```

---

## Task 2: Regenerate WASM types + migrate okf/core TypeScript

**Files:**
- Regenerate: `packages/wasm/src/generated/waml_wasm.d.ts`
- Modify: `packages/okf/src/types.ts:110,134`
- Modify: `packages/core/src/state/ops-adapter.ts:228`
- Test: `packages/okf/test/display.test.ts:12,37-41`

**Interfaces:**
- Consumes: the `showType` wire key from Task 1.
- Produces: `DiagramDisplay.showType: boolean` (default `true`) in `@waml/okf`, consumed by ClassifierBox + DiagramPropertiesBody in Task 3.

- [ ] **Step 1: Regenerate the WASM bindings**

Run: `pnpm build:wasm`
Expected: `packages/wasm/src/generated/waml_wasm.d.ts` now has `showType?: boolean` on `DiagramDisplay` and `showType: boolean` on `DisplayDto` (the `attributeDetail` lines are gone). This is a generated file — do not hand-edit; if it still shows `attributeDetail`, Task 1 is incomplete.

- [ ] **Step 2: Update the okf type + default**

In `packages/okf/src/types.ts`, replace lines 109-110:

```ts
  /** How much of each attribute row shows: just the name, or name + type. */
  attributeDetail: "name-only" | "name-type";
```

with:

```ts
  /** Whether each attribute row shows its type name (true) or name only (false). */
  showType: boolean;
```

and in `DEFAULT_DISPLAY` replace line 134:

```ts
  attributeDetail: "name-type",
```

with:

```ts
  showType: true,
```

- [ ] **Step 3: Update the display test to the new field**

In `packages/okf/test/display.test.ts`, replace line 12 (`attributeDetail: "name-type",`) with `showType: true,`. Then replace the overlay test body (lines 37-41):

```ts
    expect(resolveDisplay({ showAttributes: false, attributeDetail: "name-only" })).toEqual({
      ...DEFAULT_DISPLAY,
      showAttributes: false,
      attributeDetail: "name-only",
    });
```

with:

```ts
    expect(resolveDisplay({ showAttributes: false, showType: false })).toEqual({
      ...DEFAULT_DISPLAY,
      showAttributes: false,
      showType: false,
    });
```

- [ ] **Step 4: Update the ops-adapter mapping**

In `packages/core/src/state/ops-adapter.ts`, replace line 228:

```ts
    attributeDetail: d.attributeDetail,
```

with:

```ts
    showType: d.showType,
```

- [ ] **Step 5: Run the okf tests + typecheck core**

Run: `pnpm --filter @waml/okf test`
Expected: all pass (default + overlay tests now on `showType`).

Run: `pnpm -r --filter=!@waml/web build`
Expected: `@waml/okf` and `@waml/core` build clean (the ops-adapter `DisplayDto` field matches the regenerated type). Web is excluded here — ClassifierBox still references the old field and is fixed in Task 3.

- [ ] **Step 6: Commit**

```bash
git add packages/wasm/src/generated/waml_wasm.d.ts packages/okf/src/types.ts \
        packages/okf/test/display.test.ts packages/core/src/state/ops-adapter.ts
git commit -m "feat(okf): showType boolean on DiagramDisplay; regen wasm bindings"
```

---

## Task 3: Web — canvas render + properties panel UI

**Files:**
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.svelte:18`
- Modify: `packages/web/src/components/canvas/nodes/AttributeRow.svelte:4` (stale comment)
- Modify: `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts:54-63,93-104`
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.svelte` (toggleRow signature, segmented removal, Attributes section, section dividers)
- Modify: `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts` (controls list, showType test, disabled test, non-default test, cardinality label)

**Interfaces:**
- Consumes: `DiagramDisplay.showType` from Task 2.
- Produces: nothing downstream — same `onChange` / `onUpdateDiagram` contract.

- [ ] **Step 1: Update the canvas type derive**

In `packages/web/src/components/canvas/nodes/ClassifierBox.svelte`, replace line 18:

```svelte
  let showTypes = $derived(display.attributeDetail === "name-type");
```

with:

```svelte
  let showTypes = $derived(display.showType);
```

- [ ] **Step 1b: Fix the stale AttributeRow comment**

In `packages/web/src/components/canvas/nodes/AttributeRow.svelte`, replace the comment at line 4:

```svelte
  // `showTypes` (from the diagram's attributeDetail) toggles ONLY type name.
```

with:

```svelte
  // `showTypes` (from the diagram's showType) toggles ONLY type name.
```

- [ ] **Step 2: Update the ClassifierBox type-column test**

In `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`, replace the test at lines 54-63:

```ts
test("attributeDetail name-type shows the type column; name-only hides it", () => {
  const nameType = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, attributeDetail: "name-type" })) } });
  expect(nameType.container.textContent).toContain("STRING");
  expect(nameType.container.textContent).toContain("MONEY");

  const nameOnly = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, attributeDetail: "name-only" })) } });
  expect(nameOnly.container.textContent).toContain("id");
  expect(nameOnly.container.textContent).not.toContain("STRING");
  expect(nameOnly.container.textContent).not.toContain("MONEY");
});
```

with:

```ts
test("showType true shows the type column; false hides it", () => {
  const withType = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, showType: true })) } });
  expect(withType.container.textContent).toContain("STRING");
  expect(withType.container.textContent).toContain("MONEY");

  const noType = render(ClassifierBox, { props: { data: mkData(disp({ showAttributes: true, showType: false })) } });
  expect(noType.container.textContent).toContain("id");
  expect(noType.container.textContent).not.toContain("STRING");
  expect(noType.container.textContent).not.toContain("MONEY");
});
```

- [ ] **Step 3: Update the multiplicity-independence test**

In `packages/web/src/components/canvas/nodes/ClassifierBox.test.ts`, replace the test at lines 93-104:

```ts
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

with:

```ts
test("showAttributeMultiplicity drives the {mult} suffix independent of showType", () => {
  const shown = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showType: false, showAttributeMultiplicity: true })) },
  });
  expect(shown.container.textContent).toContain("{0..*}");

  const hidden = render(ClassifierBox, {
    props: { data: mkAttrData(disp({ showAttributes: true, showType: true, showAttributeMultiplicity: false })) },
  });
  expect(hidden.container.textContent).toContain("STRING");
  expect(hidden.container.textContent).not.toContain("{0..*}");
});
```

- [ ] **Step 4: Give `toggleRow` an optional distinct accessible name**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, replace the snippet signature (line 58):

```svelte
{#snippet toggleRow(label: string, checked: boolean, onToggle: () => void, disabled = false)}
```

with:

```svelte
{#snippet toggleRow(label: string, checked: boolean, onToggle: () => void, disabled = false, ariaLabel = label)}
```

and replace the button's `aria-label` (line 63):

```svelte
    aria-label={label}
```

with:

```svelte
    aria-label={ariaLabel}
```

- [ ] **Step 5: Remove the now-unused `segmented` snippet**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, delete the entire `segmented` snippet block including its preceding comment (lines 85-108, from `<!-- A two-option segmented control ... -->` through the closing `{/snippet}`). It has no remaining caller after Step 6.

- [ ] **Step 6: Replace the segmented control with a "Show type" toggle**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, in the Attributes section, replace the `{@render segmented(...)}` block (lines 162-171):

```svelte
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
```

with:

```svelte
      {@render toggleRow("Show type", display.showType, () =>
        patch({ showType: !display.showType }), attrDisabled,
      )}
```

- [ ] **Step 7: Relabel the multiplicity toggle to "Show cardinality" (distinct aria)**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, replace the multiplicity toggle (lines 175-177):

```svelte
      {@render toggleRow("Show multiplicity", display.showAttributeMultiplicity, () =>
        patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), attrDisabled,
      )}
```

with:

```svelte
      {@render toggleRow("Show cardinality", display.showAttributeMultiplicity, () =>
        patch({ showAttributeMultiplicity: !display.showAttributeMultiplicity }), attrDisabled, "Attribute cardinality",
      )}
```

- [ ] **Step 8: Add hairline dividers between sections**

In `packages/web/src/components/canvas/DiagramPropertiesBody.svelte`, for the three sections after Identity — Attributes (line 156), Relationships (line 212), Stereotypes (line 227) — replace each opening tag:

```svelte
  <section class="flex flex-col">
```

with:

```svelte
  <section class="flex flex-col border-t border-[#d8dee8] pt-4">
```

Leave the Identity section (line 121) as `<section class="flex flex-col">` — no top divider on the first group.

- [ ] **Step 9: Update the properties-panel tests**

In `packages/web/src/components/canvas/DiagramPropertiesBody.test.ts`, make these edits:

(a) In `renders all five display controls`, replace the radiogroup assertion (line 21):

```ts
  expect(screen.getByRole("radiogroup", { name: "Attribute detail" })).toBeTruthy();
```

with:

```ts
  expect(screen.getByRole("switch", { name: "Show type" })).toBeTruthy();
```

(b) Replace the whole `picking an attribute-detail option emits that value` test (lines 37-44):

```ts
test("picking an attribute-detail option emits that value", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("radio", { name: "Name + type" }));
  expect(onChange).toHaveBeenCalledWith({ attributeDetail: "name-type" });
});
```

with:

```ts
test("toggling 'Show type' emits the inverted showType flag", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showType: true }, onChange }),
  });
  await fireEvent.click(screen.getByRole("switch", { name: "Show type" }));
  expect(onChange).toHaveBeenCalledWith({ showType: false });
});
```

(c) In `a non-default display value drives every control's rendered state` (lines 46-76): change the display object's `attributeDetail: "name-only" as const,` (line 49) to `showType: false,`, then replace the two trailing radio assertions (lines 72-75):

```ts
  expect(screen.getByRole("radio", { name: "Name only" }).getAttribute("aria-checked")).toBe("true");
  expect(screen.getByRole("radio", { name: "Name + type" }).getAttribute("aria-checked")).toBe(
    "false",
  );
```

with:

```ts
  expect(screen.getByRole("switch", { name: "Show type" }).getAttribute("aria-checked")).toBe("false");
```

(d) Replace the whole `attribute-detail options are disabled and inert when 'Show attributes' is off` test (lines 89-102):

```ts
test("attribute-detail options are disabled and inert when 'Show attributes' is off", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: false }, onChange }),
  });

  const nameOnly = screen.getByRole("radio", { name: "Name only" }) as HTMLButtonElement;
  const nameType = screen.getByRole("radio", { name: "Name + type" }) as HTMLButtonElement;
  expect(nameOnly.disabled).toBe(true);
  expect(nameType.disabled).toBe(true);

  await fireEvent.click(nameOnly);
  expect(onChange).not.toHaveBeenCalled();
});
```

with:

```ts
test("'Show type' is disabled and inert when 'Show attributes' is off", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, {
    props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: false }, onChange }),
  });

  const showType = screen.getByRole("switch", { name: "Show type" }) as HTMLButtonElement;
  expect(showType.disabled).toBe(true);

  await fireEvent.click(showType);
  expect(onChange).not.toHaveBeenCalled();
});
```

(e) Replace the whole `Show multiplicity toggle emits showAttributeMultiplicity` test (lines 129-134):

```ts
test("Show multiplicity toggle emits showAttributeMultiplicity", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeMultiplicity: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Show multiplicity" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeMultiplicity: false });
});
```

with:

```ts
test("Attribute cardinality toggle emits showAttributeMultiplicity", async () => {
  const onChange = vi.fn();
  render(DiagramPropertiesBody, { props: props({ display: { ...DEFAULT_DISPLAY, showAttributes: true, showAttributeMultiplicity: true }, onChange }) });
  await fireEvent.click(screen.getByRole("switch", { name: "Attribute cardinality" }));
  expect(onChange).toHaveBeenCalledWith({ showAttributeMultiplicity: false });
});
```

- [ ] **Step 10: Run the web suite**

Run: `pnpm --filter @waml/web test`
Expected: all pass (the `Attribute cardinality` and `Show cardinality` switches resolve unambiguously; `renders the four section headings` still green).

- [ ] **Step 11: Commit**

```bash
git add packages/web/src/components/canvas/nodes/ClassifierBox.svelte \
        packages/web/src/components/canvas/nodes/AttributeRow.svelte \
        packages/web/src/components/canvas/nodes/ClassifierBox.test.ts \
        packages/web/src/components/canvas/DiagramPropertiesBody.svelte \
        packages/web/src/components/canvas/DiagramPropertiesBody.test.ts
git commit -m "feat(web): Show type toggle, cardinality label, section dividers"
```

---

## Verification (after all tasks)

- `cargo test -p waml -p waml-ops-dto` → green, incl. the legacy/precedence test.
- `pnpm build:wasm && pnpm -r --filter=!@waml/web build` → clean.
- `pnpm --filter @waml/okf test` and `pnpm --filter @waml/web test` → green.
- Legacy round-trip: a `.waml` diagram with `attributeDetail: name-only` (no `showType`) parses to `show_type: Some(false)`; after any diagram edit it resaves with `showType: false` and no `attributeDetail`.
- In-app: Attributes section shows a "Show type" toggle and a "Show cardinality" toggle; hairlines separate the four sections; toggling "Show type" adds/removes the type column on canvas boxes and toggling "Show cardinality" adds/removes the `{mult}` suffix.
```
