---
type: uml.Sequence
title: Authoring and Validation
description: An interaction that evaluates authored content and presents its derived view and diagnostics.
---

# Authoring and Validation

## Notes
- Read this interaction from the top to the bottom.
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) evaluates the full bundle, not only the document that the author changed.
- The alternative shows the two results. A warning does not prevent the view. An error prevents the new view, and the previous view stays on the screen.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) as validation
- [Model Projection](./../concepts/workflows/model-projection.md) as projection
- [Layout Solving](./../concepts/workflows/layout-solving.md) as solver

## Messages
- author calls editor: `supply authored content`
- editor calls validation: `evaluate bundle`
- validation replies editor: `diagnostics`
- alt
  - when `no error`
    - editor calls projection: `derive current model and views`
    - projection replies editor: `model and views`
    - editor calls solver: `solve geometry for the active view`
    - solver replies editor: `view geometry`
    - editor replies author: `view and diagnostics`
  - else
    - editor replies author: `previous view and diagnostics`
