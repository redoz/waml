---
type: uml.Activity
title: Layout Solving
description: An activity that validates layout inputs and produces view geometry or diagnostics.
---

# Layout Solving

## Notes
- [Layout Solving](./../concepts/workflows/layout-solving.md) applies to a [Diagram](./../concepts/model/diagram.md).

## Nodes

### initial
- transitions to Selected Diagram

### Selected Diagram
- transitions to Collect Layout Inputs

### Collect Layout Inputs
- transitions to References and Constraints Valid?

### decision References and Constraints Valid?
- when `valid` transitions to Solve View Geometry
- else transitions to Report Diagnostics

### Solve View Geometry
- transitions to final

### Report Diagnostics
- transitions to final

### final
