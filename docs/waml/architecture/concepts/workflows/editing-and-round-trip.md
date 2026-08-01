---
type: uml.Class
title: Editing and Round Trip
description: A responsibility that keeps authored documents authoritative while rebuilding derived views after canonical serialization.
---

# Editing and Round Trip

## Relationships
- depends [Authored Document](../model/authored-document.md)
- depends [WAML Model](../model/waml-model.md)
- associates [Canonical Serialization](./canonical-serialization.md): 1 roundtrip to 1 serialization
- associates [Model Projection](./model-projection.md): 1 roundtrip to 1 projection
- associates [Edit History](./edit-history.md): 1 roundtrip to 1 history

## Notes
- The authored documents stay authoritative. The system rebuilds the derived views after canonical serialization.
- An author states an edit against the model. Examples are: rename an element, add a feature, connect two elements, place one element.
- The system applies the edit to the documents that declare these elements.
- One edit can change more than one document. A rename also changes each reference to the renamed element.
- The system accepts the result only if it can analyze the changed bundle. If it cannot, the previous state stays in effect.
- The system does not write the model back to the documents. An edit that a document cannot express is not possible.
