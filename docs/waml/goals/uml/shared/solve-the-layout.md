# Solve the Layout

**Goal:** The solver gives each node a position and size that a reader accepts.

**Why:** Automatic layout lets an author write a diagram as text without
placing each node.

**Done when:** Each supported diagram solves without overlap or clipped
content, gives the same result for the same input, and reports each conflict in
readable form.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### NATIVE-036 — solver conflicts identify their participants

**Applies to:** native

**Given** class-diagram constraints cannot all be satisfied
**When** the editor solves the layout
**Then** each visible conflict is a readable statement that identifies its participants

**Evidence:** `crates/waml-editor/src/scene.rs::conflict_statement` `crates/waml-editor/src/scene.rs::conflict_participants`

## Verification gaps

- NATIVE-036 — target: native; Solver golden tests do not assert native conflict presentation.

## Notes

- Structural, flow, and interaction diagrams use different solver substrates.
- The frozen inventory has no shipped shared scenario for overlap, clipping,
  or deterministic layout. These missing contracts keep this goal `partial`.
- [Keep the Map Stable](./keep-the-map-stable.md) owns movement between
  successive solves.
