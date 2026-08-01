---
type: uml.Class
title: Edit History
description: A responsibility that makes each applied edit reversible and replayable in order.
---

# Edit History

## Relationships
- depends [Authored Document](../model/authored-document.md)
- associates [Editing and Round Trip](./editing-and-round-trip.md): 1 history to 1 roundtrip

## Notes
- Each applied edit records the reverse edit. Undo applies that reverse edit through the usual edit path.
- The system does not keep a copy of the bundle for each step.
- Undo produces the reverse of the reverse edit. Redo applies it. Undo and redo stay symmetrical.
- Each step has a label. The label states the author intent, not the text change.
- A new edit removes all steps that undo made available for redo. The history stays a single line of steps.
- The history can group related consecutive edits into one step. Undo of a rename is one step, not one step for each character.
- The history is session data. The bundle does not contain it. A new session starts with an empty history.
