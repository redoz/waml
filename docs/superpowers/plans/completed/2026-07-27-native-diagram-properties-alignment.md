# Native Diagram Properties Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the native Diagram Properties grouping, separate attribute and connector cardinality, and hide the tool dock while the panel is open.

**Architecture:** Keep `CardinalityVisibility` as the attribute policy and add a separate resolved/persisted connector-cardinality boolean. Rewire the native property view into four semantic sections, then make the class-diagram shell suppress the dock while properties mode is active.

**Tech Stack:** Rust, Makepad live design, WAML model/ops/parser, Cargo tests.

## Global Constraints

- Sections and order are exactly Identity, Attributes, Relationships, Stereotypes.
- Attribute cardinality is On / Explicit / Off; user-facing On maps to internal `CardinalityVisibility::All`.
- Connector cardinality is a plain on/off `Show cardinality` setting and never synthesizes implicit `{1}`.
- Existing serialized `cardinality` remains the attribute policy; `showCardinality` is the connector boolean.
- Legacy `showAttributeMultiplicity` falls back to attribute cardinality only.
- The tool dock is hidden and non-interactive while Diagram Properties is open, then restored on close.
- Preserve unrelated user changes and avoid unrelated refactors.

---

### Task 1: Align the native diagram-properties feature end to end

**Files:**
- Modify: `crates/waml/src/model.rs`
- Modify: parser/render/ops files that own `DiagramDisplay` and `DiagramDisplaySet`
- Modify: `crates/waml-editor/src/diagram_display.rs`
- Modify: `crates/waml-editor/src/diagram_properties.rs`
- Modify: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml-editor/src/edge_labels.rs`
- Modify: the class-diagram shell/layout file that owns the tool dock and properties mode
- Test: colocated Rust test modules for every changed behavior
- Update only if required by the shared schema: `packages/okf` and native/web mirrors

**Interfaces:**
- Produces: resolved attribute `cardinality: CardinalityVisibility`.
- Produces: resolved connector `show_cardinality: bool`.
- Persists: `cardinality: off|explicit|all` for attributes and `showCardinality: bool` for connectors.
- UI emits independent attribute-cardinality and connector-cardinality changes.

- [ ] **Step 1: Write failing model/resolver tests**

Add tests demonstrating that `cardinality: off` can coexist with
`showCardinality: true`, that the legacy attribute boolean affects only the
attribute enum, and that defaults are `Explicit` for attributes and `true`
for connectors.

- [ ] **Step 2: Run the focused tests and verify RED**

Run the smallest relevant `cargo test -p waml <test-filter>` and
`cargo test -p waml-editor <test-filter>` commands. Confirm failures are due
to the missing independent connector field.

- [ ] **Step 3: Split the model, resolver, operations, and persistence**

Add/restore `show_cardinality` independently of the attribute enum across the
Rust model, diagram display resolution, `DiagramDisplaySet`, parsing, and
serialization. Preserve the compatibility rules in Global Constraints.

- [ ] **Step 4: Write failing renderer tests**

Cover attributes in On/Explicit/Off modes and connector ends with the boolean
on/off. Include a connector with no authored multiplicity and assert enabling
connector cardinality does not produce an implicit `{1}`.

- [ ] **Step 5: Run renderer tests and verify RED**

Run focused scene and edge-label tests; confirm the shared-field behavior is
what makes them fail.

- [ ] **Step 6: Rewire scene and edge rendering**

Use the attribute enum only in classifier attribute projection. Use the
connector boolean only in relationship-end label projection, displaying
authored multiplicity when enabled.

- [ ] **Step 7: Write failing native property-state and layout tests**

Test independent emitted changes, the four required section labels/order, and
tool-dock visibility behavior where the existing test harness permits.

- [ ] **Step 8: Run native UI tests and verify RED**

Run focused `waml-editor` tests and confirm failures represent the old
headings, shared cardinality control, and visible dock.

- [ ] **Step 9: Rebuild the native panel**

Implement the four sections and exact control ownership from Global
Constraints. Remove consequence copy and the standalone cardinality section.
Use the visible segment labels On / Explicit / Off while mapping On to `All`.
Rename Description to Note. Use normal panel padding after removing the dock
gutter.

- [ ] **Step 10: Hide and restore the tool dock**

Make properties mode suppress drawing, layout space, and hit handling for the
tool dock. Restore it when properties mode closes.

- [ ] **Step 11: Verify GREEN**

Run `cargo test -p waml-editor`, the relevant `waml` tests, and
`cargo clippy -p waml-editor --all-targets -- -D warnings`. Resolve all
failures and warnings.

- [ ] **Step 12: Visual verification**

Launch the native editor, open Diagram Properties, capture it with
`pwsh -File scripts/capture-window.ps1 -Out native-diagram-properties.png -Process waml-editor`,
and verify the four sections, independent controls, normal left inset, and
hidden toolbar.

- [ ] **Step 13: Commit**

Commit all scoped changes and tests with a concise conventional commit.
