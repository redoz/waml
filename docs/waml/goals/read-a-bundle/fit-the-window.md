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

- Specified in `docs/superpowers/plans/2026-07-26-responsive-viewport-chrome.md`
  as one hysteretic wide/narrow chrome mode.
- The narrow form moves the tree and inspector above the view; the wide form
  puts them at the side.
- Docked collapsible panels, dock splitters, and a two-row caption bar all
  exist and interact with this. Chrome changes here have a history of blanking
  text when a fixed child fills a fixed parent.
- Touch input works on the web form, including canvas pinch, so a narrow
  viewport is a real target rather than a hypothetical one.
