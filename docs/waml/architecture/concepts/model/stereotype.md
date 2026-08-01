---
type: uml.Class
title: Stereotype
description: An open-ended label that adds domain meaning to a model element without adding a new kind of element.
---

# Stereotype

## Relationships
- associates [Model Element](./model-element.md): 0..* stereotypes to 1 element

## Notes
- The set of element kinds is closed. The set of stereotypes is open. A new domain word is a new label, not a new kind.
- An element can have more than one stereotype.
- A stereotype has no effect on resolution and no effect on validation. It is domain vocabulary. A [Profile](./profile.md) can give it a special appearance.
