---
type: uml.Class
title: View Geometry
description: The solved positions, group shapes, and connection routes a view is drawn from.
stereotype: derived
---

# View Geometry

## Attributes
- placements: Rect {0..*}
- groups: GroupShape {0..*}
- routes: Route {0..*}

## Relationships
- depends [Diagram](./diagram.md)
- depends [Layout Constraint](./layout-constraint.md)

## Notes
- The geometry is a result, not an input. The solver calculates it again for each drawing. The bundle does not contain it.
- The solver measures each member from the text that the member shows. A longer title changes the arrangement. The text does not go outside the element.
- A group reserves space also when the view does not draw the group. Two adjacent groups can then stay near each other without an overlap.
- A connection is a straight or an orthogonal path. Each end point of the path stays on the border of its element.
