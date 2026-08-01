# WAML architecture

This is the current product architecture. It is not a source-code map.

## Understand the model

- [Model concepts](./concepts/model/index.md): The authored bundle and resolved WAML model.
- [WAML Domain Model](./views/domain-model.md): Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.
- [Model Vocabulary](./views/model-vocabulary.md): Structural view of the element kinds, their labels, the presentation lens, and the solved geometry.

## Follow a workflow

- [Workflow concepts](./concepts/workflows/index.md): The responsibilities that author, validate, edit, query, and lay out WAML content.
- [Authoring and Validation](./views/authoring-and-validation.md): An interaction that evaluates authored content and presents its derived view and diagnostics.
- [Editing Round Trip](./views/editing-round-trip.md): An interaction that serializes a semantic edit and returns its rebuilt derived view.
- [Layout Solving](./views/layout-solving.md): An activity that validates layout inputs and produces view geometry or diagnostics.

## Run the product

- [System Context](./views/system-context.md): Structural view of the author, the bundle, and the four product surfaces that read or change it.
- [Runtime concepts](./concepts/runtime/index.md): The editor, the command-line tool, the language server, and the delivery to a browser.
- [Share Round Trip](./views/share-round-trip.md): An interaction that packs a bundle into a link and rebuilds that bundle in a browser.
- [Web Delivery](./views/web-delivery.md): An activity that builds the native editor for a browser and publishes it as a static artifact.
