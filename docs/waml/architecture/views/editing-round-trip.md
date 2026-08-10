---
type: uml.SequenceDiagram
title: Editing Round Trip
description: An interaction that serializes a semantic edit and returns its rebuilt derived view.
---

# Editing Round Trip

## Notes
- Read this interaction from the top to the bottom.
- The authored documents stay the source of the rebuilt view. See [Editing and Round Trip](./../concepts/workflows/editing-and-round-trip.md).
- [Editor Session](../concepts/implementation/editor-session.md) prepares a complete candidate before it changes the live snapshot.
- The editor records the reverse edit only after the candidate installs. Undo repeats this interaction with that reverse edit.
- If preparation fails, the immutable live snapshot and edit history stay unchanged.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [OKF Bundle](./../concepts/model/okf-bundle.md) as bundle
- [Canonical Serialization](./../concepts/workflows/canonical-serialization.md) as serialization
- [Model Projection](./../concepts/workflows/model-projection.md) as projection
- [Edit History](./../concepts/workflows/edit-history.md) as history

## Messages
- author calls editor `perform semantic edit`
- editor calls serialization `lower and canonicalize the changed documents in candidate source`
- serialization returns `stable supported document form` to editor
- editor calls projection `prepare Markdown, OKF, UML, affected, and revision state`
- alt
  - when `candidate preparation succeeds`
    - projection returns `prepared candidate` to editor
    - editor calls bundle `install the prepared snapshot atomically`
    - bundle returns `new immutable live snapshot` to editor
    - editor calls history `record the reverse edit`
    - editor returns `updated view` to author
  - else
    - projection returns `analysis error` to editor
    - editor returns `rejected edit; old snapshot intact` to author
