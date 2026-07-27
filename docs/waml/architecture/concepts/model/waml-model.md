---
type: uml.Class
title: WAML Model
description: A resolved semantic model derived from an OKF Bundle.
stereotype: model
---

# WAML Model

## Relationships
- depends [OKF Bundle](./okf-bundle.md)
- composes [Model Element](./model-element.md): 1 model to 0..* elements

## Notes
- Provides resolved meaning for model elements while remaining distinct from the authored bundle it depends on.
