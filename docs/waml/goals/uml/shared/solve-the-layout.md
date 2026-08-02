# Solve the Layout

**Goal:** Nodes get positions and sizes a reader would not argue with, without
the author placing anything.

**Why:** Automatic layout is the whole reason diagrams-as-text is bearable. An
author who must position every node is drawing, not writing.

**Done when:** Every diagram in this bundle solves with no overlap, no clipped
content, stable output across runs, and a result an author accepts without
override.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Two substrates exist: a structural solver for class-like diagrams and a flow
  solver shared by Activity and State Machine. Sequence has its own interaction
  substrate.
- Sizing is measured in Rust so the native and web forms agree. A frontend that
  guesses sizes independently will disagree with the solver.
- Spacing floors and group titles are tunable; connected gaps are sized to hold
  their terminal labels.
- Stability matters as much as quality: a solver that reorders nodes between
  runs makes [Format Canonically](../../trust-the-content/format-canonically.md)
  unachievable.
