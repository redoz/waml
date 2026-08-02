# Draw on the Canvas

**Goal:** An author builds and changes a diagram by direct manipulation, not by
round-tripping through the inspector.

**Why:** The canvas is where the author is looking. Every edit that forces them
to look somewhere else costs attention, and the cost compounds across the
hundreds of small edits a real diagram takes.

**Done when:** Adding a node, connecting two nodes, re-targeting an edge
endpoint, moving, multi-selecting, and deleting all work on the canvas with the
pointer alone; each shows what it will do before the button is released; and
each lands as one undoable transaction.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Drag-to-place with constraints, a node context menu, a selection toolbar, and
  a hover-preview radial dial all exist. Edge manipulation is the gap: dragging
  an endpoint onto a different target is the single most-used gesture in every
  diagram tool and is not known to work here.
- Rubber-band selection and multi-select editing are unbuilt.
- Copy and paste of a subgraph is unbuilt and deserves its own leaf once the
  single-element gestures are solid.
- Six deferred threads for the drag-to-place work are parked in
  `docs/superpowers/backlog/drag-place-viz-threads.md`: hover-trace, conflict
  ring attribution, override amber against conflict red, group-scoped drag,
  real glyph art, and a visualization toggle. They are the known next moves
  here, already thought through.
- Preview-before-commit matters more than it sounds: a drag whose result is
  only visible after release forces the author to undo to explore.
- Related: [Select and Inspect](../uml/shared/select-and-inspect.md) owns what
  happens *after* a selection; this goal owns making the selection and the
  change.
