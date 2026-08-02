# Theme the Diagram

**Goal:** Diagrams are legible in light and dark, at every zoom, in both the
native and the web form.

**Why:** A reader opens a share link in whatever theme their system is in. A
diagram that is only correct in one of them is broken half the time.

**Done when:** Every diagram in this bundle is legible in both themes, per-kind
accents stay distinguishable, and switching theme redraws without a stale
frame.

**Status:** partial — unverified
**MVP:** no

## Notes

- Light and dark only — no third theme. Light is the reference.
- Per-kind accents and node styling exist; a theme atlas backs them.
- Zoom is the weak axis, not colour: text is rasterized per zoom-scaled size,
  which makes zooming cost hundreds of milliseconds per step.
- `MVP: no` because the bar asks for readable, not beautiful. Promote if a
  diagram turns out to be unreadable in dark.
