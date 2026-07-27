---
type: uml.Sequence
title: Editing Round Trip
description: An interaction that serializes a semantic edit and returns its rebuilt derived view.
---

# Editing Round Trip

## Notes
- Read this interaction from top to bottom as the [Editing and Round Trip](./../concepts/workflows/editing-and-round-trip.md) workflow preserves authored documents as the source of the rebuilt view.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [OKF Bundle](./../concepts/model/okf-bundle.md) as bundle
- [Canonical Serialization](./../concepts/workflows/canonical-serialization.md) as serialization
- [Model Projection](./../concepts/workflows/model-projection.md) as projection

## Messages
- author calls editor: `perform semantic edit`
- editor calls bundle: `update authored documents`
- editor calls serialization: `canonicalize authored documents`
- serialization replies editor: `stable supported document form`
- editor calls projection: `derive current model and view`
- projection replies editor: `model and view`
- editor replies author: `updated view`
