---
type: uml.Class
title: Diagram
description: A curated structural view of a WAML Model.
stereotype: view
---

# Diagram

## Attributes
- members: [Model Element](./model-element.md) {0..*}
- groups: MemberGroup {0..*}

## Relationships
- depends [WAML Model](./waml-model.md)
- associates [Profile](./profile.md): 0..* diagrams to 1 profile
- aggregates [Layout Constraint](./layout-constraint.md): 1 diagram to 0..* constraints

## Notes
- A diagram is a view of a selection of the model.
- A diagram keeps three concerns apart: which elements are members, which [Profile](./profile.md) shows them, and how the solver arranges them.
- The members can be in groups, and a group can contain a group. A group states membership only. The appearance and the direction of a group are arrangement concerns.
- A diagram is a view. It gives no element to the model. It is not a member of a different view, and it is not a relationship target.
- If you add an element to a diagram, the element does not change. If you remove an element from a diagram, the model keeps its connections.
