# Place the Labels

**Goal:** Each label is legible, is in a position, and is not clipped. The
labels include node titles, members, edge names, multiplicities, roles,
stereotypes, and guards.

**Why:** A label that a reader cannot read makes the diagram incorrect. A
failure in the layout becomes visible first at the labels.

**Done when:** No label in this bundle is clipped, is below another element, or
is between two edges in a position that gives no owner. This is true at the
default zoom and at each zoom that the reader can select.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The solver places labels in world space. The renderer does not place them. A
  renderer does not know the other content of the canvas, thus labels overlap
  each other and go below node cards.
- Placement does not fail silently. If the solver cannot place a label
  correctly, the label gets a leader line to its owner. To draw a label above
  another element is not an acceptable result for any input.
- [Solve the Layout](./solve-the-layout.md) and this goal use the same text
  measurement. If the solver and the renderer measure a label differently, the
  renderer clips the label.
- The font size in a text style is in points. The canvas applied the conversion
  from points to pixels two times in the past. Examine that conversion first
  when a measurement is incorrect.
- A lifeline head shows the authored title, and the tool measures that title.
  The model uses the resolved reference key to correlate messages. The reader
  does not see that key.
