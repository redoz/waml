---
type: uml.Class
title: Note
description: A Model Element that annotates other elements without changing what they mean.
---

# Note

## Relationships
- specializes [Model Element](./model-element.md)
- associates [Model Element](./model-element.md): 1 note to 1..* targets

## Notes
- A note holds text and points to one or more elements. It has no effect on the meaning of the model.
- The target of a note can be an element or a connection with a name. One note can have more than one target.
- A remark on an element is the same thing in a shorter form. The system reads it as a note with that element as its target. It writes the remark again in the short form.
