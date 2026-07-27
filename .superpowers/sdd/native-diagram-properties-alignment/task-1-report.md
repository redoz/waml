# Task 1 report — Native Diagram Properties alignment

## Status

DONE

## Commit hashes

- Implementation: `64ff08c`

## Files and behavior changed

- `crates/waml/src/model.rs`, `parse.rs`, and `ops/mod.rs`
  - Restored independent `show_cardinality` / `showCardinality` persistence.
  - Kept `cardinality` as the attribute `CardinalityVisibility` enum.
  - Kept `showAttributeMultiplicity` as an attribute-only compatibility field.
  - Whole-display writes now persist both `cardinality` and `showCardinality`.
- `crates/waml-editor/src/diagram_display.rs`
  - Resolves attribute cardinality to `Explicit` by default.
  - Resolves relationship cardinality independently to `true` by default.
- `crates/waml-editor/src/scene.rs`, `edge_labels.rs`, and `main.rs`
  - Attribute rows continue to use the three-state enum.
  - Connector-end labels now use only the relationship boolean.
  - Only authored connector multiplicities render; implicit `{1}` is never synthesized.
- `crates/waml-editor/src/diagram_properties.rs`
  - Rebuilt the panel as Identity, Attributes, Relationships, Stereotypes.
  - Renamed Description to Note and removed consequence copy/legacy headings.
  - Added On / Explicit / Off attribute segments (`On` maps to `All`).
  - Added an independent Show cardinality relationship toggle.
  - Replaced the 70-pixel dock gutter with the normal 14-pixel inset.
- `crates/waml-editor/src/class_diagram_view.rs`
  - Hides the tool dock wrapper in properties mode, ignores queued dock actions,
    and restores the dock on close.
- `crates/waml-ops-dto/src/lib.rs`
  - Carries `showCardinality` independently across the Rust operation DTO.
- `packages/okf`, `packages/core`, and
  `packages/wasm/src/generated/waml_wasm.d.ts`
  - Updated the shared TypeScript contract, defaults, operation adapter, tests,
    and generated declaration mirror.
- `native-diagram-properties.png`
  - Native PrintWindow capture of the completed panel.

## RED evidence

- `rtk cargo test -p waml-editor diagram_display::tests`
  - Failed with missing `show_cardinality` fields on `DiagramDisplay` and
    `ResolvedDiagramDisplay`.
- `rtk cargo test -p waml diagram_cardinality`
  - Failed with missing `show_cardinality` fields on `DiagramDisplay` and
    `DiagramDisplaySet`.
- `rtk cargo test -p waml-editor relationship_cardinality_is_independent_and_never_synthesized`
  - Failed because attribute `Off` incorrectly hid the authored connector
    multiplicity (`[]` instead of `["{0..*}"]`).
- `rtk cargo test -p waml-editor changing_relationship_cardinality_preserves_the_attribute_mode`
  - Failed on the missing `ShowCardinality` property change, segment provider,
    and dock visibility policy.
- `rtk pnpm --filter @waml/okf test`
  - Failed because `DEFAULT_DISPLAY` lacked `showCardinality: true`.
- Core adapter mutation check:
  `rtk pnpm --filter @waml/core exec vitest run src/state/ops-adapter.test.ts`
  - With the new mapping temporarily removed, the focused test failed because
    the DTO omitted `showCardinality`; restoring the mapping returned it to green.

## Final verification

- `rtk cargo test -p waml-editor`
  - PASS: 697 tests across 5 suites.
- `rtk cargo test -p waml`
  - PASS: 400 tests across 8 suites.
- `rtk cargo test -p waml-ops-dto`
  - PASS: 15 tests across 2 suites.
- `rtk cargo clippy -p waml-editor --all-targets -- -D warnings`
  - PASS: exit 0, 0 clippy errors. Cargo printed its existing two duplicate-package
    selection warnings for the Makepad checkout.
- `rtk pnpm --filter @waml/okf test`
  - PASS: 54 tests.
- `rtk pnpm --filter @waml/core exec vitest run src/state/ops-adapter.test.ts`
  - PASS: 34 tests.
- `rtk pnpm --filter @waml/wasm build`
  - PASS.
- `rtk pnpm --filter @waml/okf build`
  - PASS.
- `rtk cargo fmt --all -- --check`
  - PASS after formatting.
- `rtk git diff --check`
  - PASS.

## Screenshot verification

- Screenshot: `native-diagram-properties.png`
- Capture command:
  `rtk pwsh -File scripts/capture-window.ps1 -Out native-diagram-properties.png -Process waml-editor`
- Verified visually:
  - Identity, Attributes, Relationships, Stereotypes appear in order.
  - Attribute cardinality shows On / Explicit / Off.
  - Relationship cardinality is a separate toggle.
  - The body uses the normal left inset.
  - The native diagram tool dock is absent and occupies no layout space.

## Self-review findings

- No critical or moderate findings remained after the final diff/data-flow audit.
- Attribute and relationship cardinality each have a distinct model field,
  resolver default, operation field, native state change, and renderer branch.
- The legacy attribute gate never populates or controls connector visibility.
- Properties mode has no remaining tool-dock action path.
- The TypeScript mirror is intentionally limited to shared schema/default/DTO
  alignment; no unrelated web panel redesign was included.

## Remaining concerns

- None.
