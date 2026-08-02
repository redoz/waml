# Fit the Window

**Goal:** The editor stays usable from a phone-width viewport to a wide desktop
one, without the reader losing state when the window changes size.

**Why:** A share link is opened on whatever the reader has. A layout that only
works at desktop width makes the link useless on half the devices that receive
it.

**Done when:** At roughly 390 pixels wide the caption controls, the document,
the canvas, and the dock panels are all usable; crossing the width threshold
does not change which document is open or which panels the reader had chosen;
and the threshold has hysteresis so a drag near it does not flicker.

**Status:** partial — unverified
**MVP:** yes

## Notes

- There is one chrome mode with two states, wide and narrow, not a spectrum of
  layouts. The narrow form moves the tree and inspector above the view; the
  wide form puts them at the side.
- The transition is hysteretic: the width that switches to narrow is smaller
  than the width that switches back to wide. Without that gap, a drag that
  hovers near the threshold thrashes.
- Crossing the threshold changes only where chrome is drawn. It must not close
  a document, change the active tab, or reset which panels the reader opened.
- Docked collapsible panels, dock splitters, and a two-row caption bar all
  exist and interact with this. Chrome changes here have a history of blanking
  text when a fixed child fills a fixed parent.
- Touch input works on the web form, including canvas pinch, so a narrow
  viewport is a real target rather than a hypothetical one.
