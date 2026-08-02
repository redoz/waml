# Draw on the Canvas

**Goal:** An author makes and changes a diagram with direct manipulation. The
author does not go to the inspector and back for each change.

**Why:** The author looks at the canvas. Each edit that moves the attention of
the author to another position has a cost. A diagram needs many small edits,
thus the total cost is large.

**Done when:** The author can add a node, connect two nodes, move an edge
endpoint to a different target, move a node, select more than one node, and
delete, with the pointer only. Each operation shows its result before the
author releases the button. Each operation is one transaction that the author
can undo.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Drag to place with constraints, a context menu on a node, a selection
  toolbar, and a radial dial with a preview operate. Edge manipulation is the
  defect. To drag an endpoint onto a different target is the most frequent
  operation in a diagram tool. It is not known to operate here.
- Selection with a rubber band and edits to more than one element do not exist.
- Copy and paste of a subgraph does not exist. Give it a separate goal after
  the operations for one element operate correctly.
- The feedback for placement has incomplete parts. Each part tells the author
  why the tool refuses a drop: which relations hold the node, the full cycle of
  a contradiction and not one edge of it, the difference between an override
  that the author can make and a conflict that the author cannot make, and a
  limit of drop targets to the group of the node.
- A preview before the commit is necessary. If the author sees the result after
  the release only, the author must use undo to examine each option.
- [Select and Inspect](../uml/shared/select-and-inspect.md) controls what
  occurs after a selection. This goal controls how the author makes the
  selection and the change.
