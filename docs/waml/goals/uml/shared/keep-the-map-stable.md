# Keep the Map Stable

**Goal:** An edit moves nearby elements while other elements keep their
positions.

**Why:** A reader keeps a mental map of a diagram and must not rebuild that map
after each edit.

**Done when:** Adding, removing, renaming, or reconnecting one element keeps
each unrelated node near its prior position.

**Status:** planned
**MVP:** yes

## Notes

- The frozen inventory has no shipped map-stability scenario for this goal.
- Stability is a soft solver rule. [Arrange a
  Diagram](../../author-in-the-editor/arrange-a-diagram.md) owns hard placement
  constraints from an author.
