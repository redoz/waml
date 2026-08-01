---
type: uml.Class
title: Behavioral View
description: A curated view that presents behavior from a WAML Model.
stereotype: view
---

# Behavioral View

## Relationships
- depends [WAML Model](./waml-model.md)
- associates [Classifier](./classifier.md): 0..* views to 0..1 subject

## Notes
- A behavioral view shows behavior from the model.
- A sequence shows an interaction in order. An activity and a state machine show a directed flow.
- A behavioral view arranges itself. The order of the participants or the structure of the transitions gives the drawing. It has no layout constraints.
- An interaction reads in document order, and that order is the order in time. A flow reads along its transitions, and a condition selects each branch.
- A behavioral view can name the element whose behavior it shows. The view is then connected to that element, and the element stays in one place.
