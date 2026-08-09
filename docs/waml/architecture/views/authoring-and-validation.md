---
type: uml.Sequence
title: Authoring and Validation
description: An interaction that evaluates authored content and presents its derived view and diagnostics.
---

# Authoring and Validation

## Notes
- Read this interaction from the top to the bottom.
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) evaluates bundle relationships, not only the open document.
- [UML Analysis](../concepts/implementation/uml-analysis.md) keeps freshness for each island and can retain a stale dependent projection.
- A malformed document can be quarantined. It does not make unrelated projections stale.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) as validation
- [Model Projection](./../concepts/workflows/model-projection.md) as projection
- [Layout Solving](./../concepts/workflows/layout-solving.md) as solver

## Messages
- author calls editor `supply authored content`
- editor calls validation `prepare Markdown and catalog; quarantine shell-failed documents`
- validation returns `accepted catalog, diagnostics, and quarantine state` to editor
- editor calls projection `analyze every claimed UML concept and build projections`
- projection returns `model, views, affected metadata, and per-island freshness` to editor
- alt
  - when `the active projection is current`
    - editor calls solver `solve geometry for the active view`
    - solver returns `view geometry` to editor
    - editor returns `view and diagnostics` to author
  - else
    - editor returns `retained projection, quarantine state, and diagnostics` to author
