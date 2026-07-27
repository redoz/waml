---
type: uml.Sequence
title: Authoring and Validation
description: An interaction that evaluates authored content and presents its derived view and diagnostics.
---

# Authoring and Validation

## Notes
- Read this interaction from top to bottom as the [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) workflow returns evaluation results before the Editor presents the derived result.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Editor](./../concepts/workflows/editor.md) as editor
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md) as validation
- [Model Projection](./../concepts/workflows/model-projection.md) as projection

## Messages
- author calls editor: `supply authored content`
- editor calls validation: `evaluate bundle`
- validation replies editor: `diagnostics`
- editor calls projection: `derive current model and view`
- projection replies editor: `model and view`
- editor replies author: `view and diagnostics`
