---
type: uml.Activity
title: Layout Solving
description: An activity that validates layout inputs and produces view geometry or diagnostics.
---

# Layout Solving

## Notes
- [Layout Solving](./../concepts/workflows/layout-solving.md) applies to one [Diagram](./../concepts/model/diagram.md).
- The activity produces [View Geometry](./../concepts/model/view-geometry.md). It makes no change to the model.
- The two ends are different results, not a success and a failure of the product: a conflict is a report to the author.

## Nodes

### initial
- transitions to Selected Diagram

### Selected Diagram
- transitions to Collect Layout Inputs

### Collect Layout Inputs
- do: `read the members, the groups, and the constraints`
- transitions to References and Constraints Valid?

### decision References and Constraints Valid?
- when `valid` transitions to Measure Members
- else transitions to Report Diagnostics

### Measure Members
- do: `measure each member from the text that it shows`
- transitions to Place Members and Groups

### Place Members and Groups
- do: `satisfy the placement and the alignment statements`
- transitions to Constraints Satisfiable?

### decision Constraints Satisfiable?
- when `satisfiable` transitions to Route Connections
- else transitions to Report Diagnostics

### Route Connections
- do: `route each connection between the borders of its two elements`
- transitions to Solved View Geometry

### Solved View Geometry
- transitions to final

### Report Diagnostics
- transitions to final

### final
