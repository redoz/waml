---
type: uml.Class
title: Layout Solving
description: A responsibility that turns diagram inputs and declarative constraints into view geometry without changing domain semantics.
---

# Layout Solving

## Relationships
- depends [Diagram](../model/diagram.md)
- depends [WAML Model](../model/waml-model.md)
- associates [Layout Constraint](../model/layout-constraint.md): 1 solver to 0..* constraints
- associates [View Geometry](../model/view-geometry.md): 1 solver to 0..1 geometry

## Notes
- The solver reads the members, the sizes, the connections, and the constraints of one diagram. It produces the view geometry.
- The solver does not change the domain meaning of the model.
- The solver measures each member from the text that the member shows. The author does not give a size.
- The solver keeps each group compact. A group reserves space even when the view does not draw the group.
- The solver routes each connection as a straight or an orthogonal path. Each end point stays on the border of the element.
- The solver is deterministic. The same diagram and the same constraints give the same geometry.
- If two constraints are in conflict, the solver reports a diagnostic for the conflict. It does not select one of the two constraints.
