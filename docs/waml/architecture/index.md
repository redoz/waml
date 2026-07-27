# WAML architecture

This is the current product architecture. It is not a source-code map.

## Understand the model

- [Model concepts](./concepts/model/index.md): The authored bundle and resolved WAML model.
- [WAML Domain Model](./views/domain-model.md): Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.

## Follow a workflow

- [Authoring and Validation](./views/authoring-and-validation.md): An interaction that evaluates authored content and presents its derived view and diagnostics.
- [Editing Round Trip](./views/editing-round-trip.md): An interaction that serializes a semantic edit and returns its rebuilt derived view.
- [Import, Export, and Share](./views/import-export-and-share.md): An activity that routes a requested exchange action to its supported outcome.
- [Layout Solving](./views/layout-solving.md): An activity that validates layout inputs and produces view geometry or diagnostics.

## Run the product

- [System Context](./views/system-context.md): Structural view of the people, bundles, editor, browser environment, and web delivery artifact in the current product.
- [Runtime and delivery concepts](./concepts/runtime/index.md): The product context and browser-delivery responsibilities.
- [GitHub Pages Deployment](./views/github-pages-deployment.md): An activity that orders publication of the native editor for browser delivery.
