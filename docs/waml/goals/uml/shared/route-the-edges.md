# Route the Edges

**Goal:** Each edge connects its endpoints without an avoidable obstruction.

**Why:** Clear edge routes let a reader follow relationships without tracing
them through unrelated content.

**Done when:** Each edge stops on its target border, avoids unrelated nodes,
uses no unnecessary step, and keeps enough space from parallel edges and
labels.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### NATIVE-051 — class relations keep their group constraints

**Applies to:** native

**Given** a class diagram has relations between grouped classifiers
**When** the editor routes those relations
**Then** each relation route retains the group constraints of its endpoints

**Evidence:** `crates/waml-editor/src/scene.rs::route_with_groups`

## Verification gaps

- NATIVE-051 — target: native; No native class-diagram test checks relation routing at the target boundary.

## Notes

- Routes are orthogonal polylines. The renderer draws the solved path and does
  not create a separate route.
- The frozen inventory has no shipped shared scenario for crossings, parallel
  edge distance, or unnecessary steps. These missing contracts keep this goal
  `partial`.
- Route quality and [label placement](./place-the-labels.md) share clearance
  constraints.
