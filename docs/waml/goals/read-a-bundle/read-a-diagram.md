# Read a Diagram

**Goal:** A reader reads a UML document as a diagram.

**Why:** The diagram is the result that the reader wants. Text that stays text
does not need this tool.

**Done when:** Each diagram in this bundle draws with no overlap, with no
clipped label, and with no crossing that a reader calls an error. This is true
at the default zoom and at each zoom that the reader can select.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Each kind controls its own diagram quality. Refer to the [UML](../uml/) cuts
  for the content of each kind. Refer to [UML shared](../uml/shared/) for the
  layout, the routing, and the labels below them.
- Zoom is slow. The tool makes a raster of the text for each zoom size. Thus
  interactive zoom gives only a few frames each second.
- A diagram switcher and a view bar let the reader move between the views of a
  document.
