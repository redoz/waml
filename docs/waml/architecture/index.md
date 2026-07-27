# WAML architecture

This is the current product architecture. It is not a source-code map.

## Understand the model

- [Model concepts](./concepts/model/index.md): The authored bundle and resolved WAML model.
- [WAML Domain Model](./views/domain-model.md): Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.

## Follow a workflow

- [Authoring and Validation](./views/authoring-and-validation.md): An interaction that evaluates authored content and presents its derived view and diagnostics.
- [Editing Round Trip](./views/editing-round-trip.md): An interaction that serializes a semantic edit and returns its rebuilt derived view.
- [Layout Solving](./views/layout-solving.md): An activity that validates layout inputs and produces view geometry or diagnostics.

## Run the product

- [System Context](./views/system-context.md): Structural view of authors, bundles, native editor, CLI/LSP, and VS Code integration.
- [Runtime concepts](./concepts/runtime/index.md): Native editing and local bundle responsibilities.
