# Keep the Map Stable

**Goal:** An edit moves the elements near the edit. The other elements keep
their positions.

**Why:** This goal decides whether an author can edit a solved diagram. An
author makes a mental map of the positions. A solver that moves the full
diagram after one new node removes that map at each edit. Good pointer targets
give no help against this problem.

**Done when:** To add, remove, rename, or reconnect one element keeps each
other node within a small distance of its previous position. The change is
visible as a movement and not as a new diagram.

**Status:** planned — unverified
**MVP:** yes

## Notes

- The solvers calculate a layout from the start each time. No solver uses the
  previous solution. Thus an edit can give a fully different arrangement.
- The usual method is to start the solve from the previous positions and to
  give a penalty to movement. This method does not need a different algorithm.
- [Arrange a Diagram](../../author-in-the-editor/arrange-a-diagram.md) is
  different. A constraint is a hard override from the author. Stability is a
  soft rule for the solver. The product needs both. One does not replace the
  other.
- `MVP: yes`. The bar needs an author to write `docs/waml` in the editor. The
  diagrams here are sufficiently large that a full movement at each edit
  prevents that work.
