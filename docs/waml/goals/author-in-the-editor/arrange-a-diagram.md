# Arrange a Diagram

**Goal:** An author overrides the solver where the solver gives an incorrect
result. The override stays after a reload.

**Why:** No solver is correct in all conditions. Without an override, an author
must accept a bad diagram.

**Done when:** The author can hold a position, an order, or a side. The tool
writes the constraint into the document as text that a reviewer can diff. The
solver obeys the constraint after a reload. If a constraint becomes impossible,
the tool reports it. The tool does not remove it silently.

**Status:** partial — unverified
**MVP:** no

## Notes

- Layout constraints exist in the model and the solver reads them. Verify
  whether the author can make them from the canvas.
- `MVP: no` is a judgement. The bar needs `docs/waml` to be readable. The
  default output of the solver can be sufficient for this bundle. Change the
  flag to `yes` when a diagram here is not acceptable without an override.
- A badge and a list of conflicts show constraint conflicts.
- [Keep the Map Stable](../uml/shared/keep-the-map-stable.md) is different from
  this goal. A constraint is a hard override from the author. Stability is a
  soft rule for the solver. The product needs both.
