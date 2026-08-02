# Select and Inspect

**Goal:** A click on an element in a diagram shows what that element is and
lets the author change it.

**Why:** The diagram is the most direct control of the model. An inspector that
the author can reach from the tree only wastes that control.

**Done when:** The author can select each drawn element: a node, a member, an
edge, an end, a label, and a note. The inspector shows the full property set of
the selected element. The author can change each property there. Each change is
one transaction.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The inspector, the property controls, a selection toolbar, and a context menu
  on a node operate.
- Edges and edge ends are the probable defect. To select a node is easy. To
  select a multiplicity is not easy.
- Hit tests have a known defect in this code. The tool calculates the draw
  rectangle before alignment. The tool receives events after alignment. Thus an
  aligned parent moves the hit rectangle of each child. Apply the difference
  between the two positions.
