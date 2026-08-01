---
type: uml.Class
title: Model Projection
description: A responsibility that derives a model and view representation from the current OKF Bundle.
---

# Model Projection

## Relationships
- depends [OKF Bundle](../model/okf-bundle.md)
- associates [WAML Model](../model/waml-model.md): 1 projection to 1 model
- associates [Diagram](../model/diagram.md): 1 projection to 0..* diagrams
- associates [Behavioral View](../model/behavioral-view.md): 1 projection to 0..* views

## Notes
- This responsibility derives the model and the views from the current bundle.
- The projection has two stages. The first stage reads each document as a plain document. The second stage gives the UML meaning to those documents.
- The first stage keeps the full text and the unknown fields of each document. A document with an unknown kind stays available to the reader.
- The projection is complete. It builds all views of the bundle, not only the view that the author looks at.
- The projection does not change the bundle.
