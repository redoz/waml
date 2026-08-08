# Draw on the Canvas

**Goal:** An author changes a diagram with direct manipulation.

**Why:** Direct manipulation keeps the author's attention on the diagram.

**Done when:** The author can add, connect, retarget, move, select, and delete
diagram elements with the pointer. The editor previews each change, and the
author can undo it as one transaction.

**Status:** partial
**MVP:** yes

## Planned behavior

Pointer-only edge retargeting, marquee selection, and multi-element editing
have no passing acceptance scenario in the frozen inventory.

## Notes

- Class-diagram manipulation and tool selection are owned by
  [Interact with a Class Diagram](../uml/class/interact-with-a-class-diagram.md).
  This document does not copy those scenarios.
- [Select and Inspect](../uml/shared/select-and-inspect.md) owns shared
  selection results.
- Semantic canvas targets and drag paths use prose.
  [FG-007](../../waml-feature-gaps.md#fg-007--semantic-canvas-targets-and-drag-paths)
  records this WAML expression gap.
- Typed gestures and consumed input use prose.
  [FG-002](../../waml-feature-gaps.md#fg-002--typed-gestures-and-consumed-input)
  records this WAML expression gap.
