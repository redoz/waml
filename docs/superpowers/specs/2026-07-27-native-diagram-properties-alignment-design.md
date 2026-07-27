# Native Diagram Properties Alignment

**Date:** 2026-07-27
**Status:** Approved in conversation

## Goal

Bring the native Rust/Makepad Diagram Properties view back into the semantic
grouping of the earlier Vite design while preserving the newer three-state
attribute-cardinality behavior.

## Information architecture

The panel has four sections, in this order:

1. **Identity** — Title, Note.
2. **Attributes** — Show attributes, Show type, Show visibility, attribute
   cardinality, Max attributes.
3. **Relationships** — Show roles, Show cardinality, Show labels.
4. **Stereotypes** — Show stereotype and any existing stereotype-specific
   controls.

Remove the native-only `DIAGRAM`, `CLASSIFIERS`, and standalone `CARDINALITY`
headings and their explanatory “Controls …” lines. Separate sections with the
same light rules and spacing used by the reference design.

`Description (one line)` becomes `Note`. It remains single-line unless
Makepad's current text-editing constraints can support the reference
multiline behavior without destabilizing input.

## Cardinality semantics

Attribute and relationship cardinality are independent:

- Attribute cardinality is a three-state value presented as **On / Explicit /
  Off**. Internally, `On` may continue to map to
  `CardinalityVisibility::All`; `Explicit` renders only authored attribute
  multiplicities, and `Off` renders none.
- Relationship/connector cardinality is a simple **Show cardinality** boolean.
  When enabled it renders authored relationship-end multiplicities. It does
  not synthesize implicit `{1}` labels. When disabled it renders none.

The shared display model, resolver, persistence operations, native property
state, scene projection, and edge-label renderer must keep these values
separate. The existing serialized `cardinality` enum remains the attribute
policy for compatibility; the connector boolean uses/restores
`showCardinality`.

Legacy `showAttributeMultiplicity` remains a fallback for the attribute enum
only. It must never control connector labels.

## Native shell behavior

While Diagram Properties is open, the diagram tool dock is hidden and does
not receive pointer input. Removing it also removes the dock-compensation
gutter: the properties body uses the normal panel inset rather than the
current 70-pixel left inset.

Closing Diagram Properties restores the dock and its normal interaction.

## Verification

- Unit tests prove the two cardinality settings resolve, persist, and render
  independently.
- Native property-state tests prove each control emits only its own setting.
- UI structure tests or script-gate assertions cover the four headings,
  control order, and toolbar visibility where practical.
- `cargo test -p waml-editor` and relevant `waml` crate tests pass.
- A native screenshot confirms the final grouping and hidden toolbar.
