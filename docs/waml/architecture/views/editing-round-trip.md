---
type: uml.Sequence
title: Editing Round Trip
description: An interaction that serializes a semantic edit and returns its rebuilt derived view.
---

# Editing Round Trip

## Notes
- Read this interaction from the top to the bottom.
- The authored documents stay the source of the rebuilt view. See [Editing and Round Trip](./../concepts/workflows/editing-and-round-trip.md).
- The editor records the reverse edit before it shows the result. Undo then repeats this same interaction with that reverse edit.
- The alternative shows the rejection: if the changed bundle does not analyze, the editor keeps the previous documents.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [OKF Bundle](./../concepts/model/okf-bundle.md) as bundle
- [Canonical Serialization](./../concepts/workflows/canonical-serialization.md) as serialization
- [Model Projection](./../concepts/workflows/model-projection.md) as projection
- [Edit History](./../concepts/workflows/edit-history.md) as history

## Messages
- author calls editor `perform semantic edit`
- editor calls bundle `update the affected authored documents`
- editor calls serialization `canonicalize the changed documents`
- serialization returns `stable supported document form` to editor
- editor calls projection `derive current model and views`
- alt
  - when `bundle analyzes`
    - projection returns `model and views` to editor
    - editor calls history `record the reverse edit`
    - editor returns `updated view` to author
  - else
    - editor calls bundle `restore the previous documents`
    - editor returns `previous view and diagnostics` to author
