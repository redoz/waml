# Arrange a Diagram

**Goal:** An author overrides the solver where the solver is wrong, and the
override survives.

**Why:** No solver is right every time. Without an override an author's only
recourse is to accept a bad drawing.

**Done when:** An author can pin a position, an order, or a side; the
constraint is written into the document as text a reviewer can diff; the solver
honours it on reload; and a constraint that has become unsatisfiable is
reported rather than silently dropped.

**Status:** partial — unverified
**MVP:** no

## Notes

- Layout constraints exist as a model concept and the solver consumes them.
  Authoring them from the canvas is the part to audit.
- `MVP: no` is a judgment call: the dogfood bar asks that `docs/waml` be
  readable, and the solver's default output may well be good enough for this
  bundle. Promote to `yes` the first time a diagram here cannot be made
  acceptable without an override.
- Constraint conflicts have a badge and a grouped conflict list already.
