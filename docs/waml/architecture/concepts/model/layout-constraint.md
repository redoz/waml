---
type: uml.Class
title: Layout Constraint
description: An authored statement of how a Diagram's members sit relative to one another.
---

# Layout Constraint

## Attributes
- operands: LayoutOperand {1..*}

## Relationships
- associates [Diagram](./diagram.md): 0..* constraints to 1 diagram

## Notes
- A constraint puts one member at a side of another member.
- A constraint can also put one edge in line with an edge of a different member.
- A constraint can give a direction to a group, and a space or an emphasis to one member.
- An author writes no coordinate, and the bundle holds no coordinate. An absolute position exists only as [View Geometry](./view-geometry.md). The solver calculates it again for each drawing.
- A group and an element are operands of the same kind. Each statement about a member is also possible for a full group.
- A constraint is a view concern. It arranges the members of a diagram. It does not change the meaning of the model.
- If the author moves an element in the editor, the editor writes a constraint in this same language. The change stays visible in the document.
