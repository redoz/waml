# Arrange a Diagram

**Goal:** An author overrides a solver result and keeps the override after a
reload.

**Why:** A solver cannot select the intended result in all documents.

**Done when:** The author can hold a position, order, or side. The document
stores the constraint as reviewable text, the solver applies it after reload,
and the editor reports an impossible constraint.

**Status:** partial
**MVP:** no

## Planned behavior

Complete author controls for persistent layout constraints have no passing
acceptance scenario in the frozen inventory.

## Notes

- Diagram interaction goals own diagram-specific controls.
- [Keep the Map Stable](../uml/shared/keep-the-map-stable.md) owns the soft
  stability rule. An authored constraint is a hard override.
- [Solve the Layout](../uml/shared/solve-the-layout.md) owns solver results and
  conflict feedback.
