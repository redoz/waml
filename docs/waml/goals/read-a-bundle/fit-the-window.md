# Fit the Window

**Goal:** The editor stays usable from a viewport as narrow as a telephone to a
wide desktop viewport. A change of size does not remove the state of the
reader.

**Why:** A reader opens a share link on the device that the reader has. A
layout for desktop widths only makes the link useless on many devices.

**Done when:** At a width of approximately 390 pixels, the caption controls,
the document, the canvas, and the dock panels stay usable. A change across the
width threshold does not change the open document and does not change the
panels that the reader selected. The threshold has hysteresis, thus a drag near
the threshold does not oscillate.

**Status:** partial — unverified
**MVP:** yes

## Notes

- There is one chrome mode with two states: wide and narrow. There is no set of
  intermediate layouts. The narrow state puts the tree and the inspector above
  the view. The wide state puts them at the side.
- The width that starts the narrow state is less than the width that starts the
  wide state. This difference prevents oscillation.
- A change across the threshold moves chrome only. It must not close a
  document, change the active tab, or change the panels that the reader
  selected.
- Docked collapsible panels, dock splitters, and a caption bar with two rows
  operate together with this goal. A change to chrome can make text disappear
  when one fixed child fills a fixed parent.
- Touch input operates in the web form, and this includes pinch on the canvas.
  Thus a narrow viewport is a real target.
