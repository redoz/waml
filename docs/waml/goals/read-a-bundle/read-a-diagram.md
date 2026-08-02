# Read a Diagram

**Goal:** A reader reads a UML document as a drawn diagram.

**Why:** The diagram is the payoff. Text that only ever stays text needs no
tool.

**Done when:** Every diagram in this bundle draws without overlap, without a
clipped label, and without a crossing that a reader would call a mistake, at
the default zoom and at every zoom the reader can reach.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Diagram quality is owned per kind. See the [UML](../uml/) cuts for what each
  kind must draw, and [UML shared](../uml/shared/) for the layout, routing, and
  label machinery underneath.
- Zoom is slow: text is rasterized per zoom-scaled size, which caps interactive
  zoom at a few frames a second. Not correctness, but a reader notices.
- A diagram switcher and a view bar exist for moving between the views a
  document offers.
