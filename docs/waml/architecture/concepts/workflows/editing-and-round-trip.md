---
type: uml.Class
title: Editing and Round Trip
description: A responsibility that keeps authored documents authoritative while rebuilding derived views after canonical serialization.
---

# Editing and Round Trip

## Relationships
- depends [Authored Document](../model/authored-document.md)
- depends [WAML Model](../model/waml-model.md)
- associates [Canonical Serialization](./canonical-serialization.md): 1 to 1
- associates [Model Projection](./model-projection.md): 1 to 1

## Notes
- Keeps authored documents authoritative and rebuilds derived views after canonical serialization.
