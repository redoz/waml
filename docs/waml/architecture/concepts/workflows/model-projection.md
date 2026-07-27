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

## Notes
- Derives the current model and view representation from an OKF Bundle.
