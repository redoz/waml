# Select and Inspect

**Goal:** Clicking anything in a diagram shows what it is and lets an author
change it.

**Why:** The diagram is the most direct handle on the model. An inspector
reachable only from the tree wastes it.

**Done when:** Every drawn thing — node, member, edge, end, label, note — can
be selected, is shown in the inspector with its full property set, and is
editable there, with the edit landing as one transaction.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The inspector, property controls, a selection toolbar, and a node context
  menu all exist.
- Edges and edge ends are the likely gap: selecting a node is easy, selecting a
  multiplicity is not.
- Hit-testing has a standing trap in this codebase: draw rectangles are
  pre-alignment while events are post-alignment, so an aligned parent offsets
  every child's hit rectangle unless the difference is applied.
