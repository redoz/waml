---
type: uml.Class
title: Model Element
description: An abstract resolved item within a WAML Model.
abstract: true
---

# Model Element

## Attributes
- key: ElementKey
- title: Text
- stereotypes: [Stereotype](./stereotype.md) {0..*}

## Notes
- This is the abstract item of the model. Each more specific kind of element is one of these items.
- The key of an element comes from the document that declares the element. A change of the document path is therefore a change in the full model.
- An element with an unknown kind still resolves. The system shows it as a plain element with a label. It does not remove it from the model.
