# Place the Labels

**Goal:** Each diagram label is legible, has one clear owner, and is not
clipped.

**Why:** A reader cannot use a diagram when its labels overlap or disappear.

**Done when:** Node titles, members, edge names, multiplicities, roles,
stereotypes, and guards remain legible at every supported zoom.

**Status:** planned
**MVP:** yes

## Notes

- The frozen inventory has no shipped label-placement scenario for this goal.
- The solver, not the renderer, must own world-space label placement.
- [Solve the Layout](./solve-the-layout.md) and this goal must use the same text
  measurement.
- [Route the Edges](./route-the-edges.md) owns edge paths. Both goals use the
  same clearance constraints.
