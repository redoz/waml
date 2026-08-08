# Read a Diagram

**Goal:** A reader reads each supported UML document as a diagram.

**Why:** A diagram gives the visual structure that a reader expects from a UML
document.

**Done when:** Each supported diagram kind has a shipped reading contract for
its visible scene, selection, camera, and empty or diagnostic state.

**Status:** planned
**MVP:** yes

## Notes

- Each diagram-kind goal owns its rendering and interaction behavior. Refer to
  the [UML goals](../uml/).
- Shared layout, routing, labels, selection, and theme behavior is owned by the
  [shared UML goals](../uml/shared/).
- The current prose workaround for semantic view anchors and post-draw results
  is recorded in [FG-003](../../waml-feature-gaps.md#fg-003--view-anchors-and-eventual-draw-results).
