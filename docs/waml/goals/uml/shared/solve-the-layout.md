# Solve the Layout

**Goal:** The solver gives each node a position and a size that a reader
accepts. The author places no node.

**Why:** Automatic layout is the reason that diagrams as text are usable. An
author who must place each node draws a picture and does not write a document.

**Done when:** Each diagram in this bundle solves with no overlap and no
clipped content. The solver gives the same output for the same input. The
author accepts the result and makes no override.

**Status:** partial — unverified
**MVP:** yes

## Notes

- There are two substrates. A structural solver solves class diagrams. A flow
  solver solves activity diagrams and state machine diagrams. Sequence diagrams
  use a third substrate for interactions.
- The tool measures sizes in Rust. Thus the native form and the web form agree.
  A frontend that calculates its own sizes does not agree with the solver.
- The floors for spacing and the titles of groups are adjustable. The solver
  makes each connected gap sufficiently large for the labels at its ends.
- Stability is as important as quality. If the solver changes the order of the
  nodes between two runs, [Format
  Canonically](../../trust-the-content/format-canonically.md) becomes
  impossible.
