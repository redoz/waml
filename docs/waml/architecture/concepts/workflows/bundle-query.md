---
type: uml.Class
title: Bundle Query
description: A responsibility that answers questions about the resolved model without a change to the bundle.
---

# Bundle Query

## Relationships
- depends [WAML Model](../model/waml-model.md)
- depends [Model Projection](./model-projection.md)

## Notes
- This responsibility answers three questions: show one element, list the elements of one kind, and list the elements that refer to one element.
- A query reads the resolved model. It does not read one document alone.
- A query makes no change to the bundle.
- The answer is available as text for a person and as data for a program.
