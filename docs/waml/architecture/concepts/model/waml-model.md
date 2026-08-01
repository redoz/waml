---
type: uml.Class
title: WAML Model
description: A resolved semantic model derived from an OKF Bundle.
stereotype: model
---

# WAML Model

## Attributes
- elements: [Model Element](./model-element.md) {0..*}
- views: [Diagram](./diagram.md) {0..*}
- revision: Revision

## Relationships
- depends [OKF Bundle](./okf-bundle.md)
- composes [Model Element](./model-element.md): 1 model to 0..* elements

## Notes
- The model gives the resolved meaning of the elements. It stays different from the documents that it comes from.
- A view shows a selection of the model. If a view does not show an element, the model keeps that element and its connections.
- The system builds the model again from the bundle. It does not change the model in place. The bundle and the model cannot become different.
- Resolution uses the full bundle. A link is a reference to an element in a different document. The meaning of an element can depend on a document that the author did not open.
