# Author with the Keyboard

**Goal:** An author makes and changes a full diagram with the keyboard only.

**Why:** A fast author uses the keyboard. To move a hand to the pointer for the
twentieth class is the difference between writing a diagram and drawing one.

**Done when:** Each action on the canvas has a keyboard route. The actions
include selection, movement to an adjacent element, addition of a connected
node, rename, retype, connection, and deletion. The author can find the key
assignments in the product. The author does not memorize them.

**Status:** partial — unverified
**MVP:** no

## Notes

- An overlay that shows the shortcuts operates. Thus the product has a position
  to show new assignments.
- Movement is the design question. The key assignments are not. To move the
  selection along an edge, or to the next element in the same group, is what
  makes keyboard authoring fast. There is no obvious default behavior.
- `MVP: no`. The bar needs an author to write `docs/waml`. The bar does not
  need the author to write it quickly. This goal is a strong second-stage goal.
- [Draw on the Canvas](./draw-on-the-canvas.md) must first set which actions
  exist. Then those actions get key assignments.
