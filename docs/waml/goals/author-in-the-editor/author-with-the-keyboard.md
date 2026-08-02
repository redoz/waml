# Author with the Keyboard

**Goal:** A whole diagram can be built and changed without touching the mouse.

**Why:** A fast author is a keyboard author. Reaching for the pointer to add
the twentieth class is the difference between writing a diagram and drawing
one.

**Done when:** Every action available on the canvas has a keyboard route —
select, traverse to a neighbour, add a connected node, rename, retype,
connect, delete — and the bindings are discoverable rather than memorised.

**Status:** partial — unverified
**MVP:** no

## Notes

- A shortcuts overlay exists, so discovery has a home already.
- Traversal is the design question, not the bindings: moving the selection
  "along an edge" or "to the next sibling" is what makes keyboard authoring
  fast, and it has no obvious default.
- `MVP: no` — the dogfood bar asks that `docs/waml` be authorable, not that it
  be authorable quickly. This is a strong second-wave goal.
- Depends on [Draw on the Canvas](./draw-on-the-canvas.md) settling what the
  actions *are* before they get bindings.
