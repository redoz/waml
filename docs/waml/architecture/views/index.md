# Architecture views

- [waml Core Crate](../concepts/implementation/waml-core-crate.md): One source-backed implementation concept used by these views.
- [WAML Domain Model](./domain-model.md): Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.
- [Model Vocabulary](./model-vocabulary.md): Structural view of the element kinds, their labels, the presentation lens, and the solved geometry.
- [System Context](./system-context.md): Structural view of the author, the bundle, and the four product surfaces that read or change it.
- [Crate Ownership](./crate-ownership.md): The dependency direction and ownership of the six production crates.
- [Editor Ownership](./editor-ownership.md): The ownership boundaries of editor state, documents, views, and platform effects.
- [Preparation Pipeline](./preparation-pipeline.md): The immutable source-to-candidate analysis sequence.
- [Incremental Analysis](./incremental-analysis.md): The affected-analysis and quarantine activity after exact text changes.
- [Revisioned Edit Transaction](./revisioned-edit-transaction.md): The revision, commit, and stale-completion paths for edits.
- [Authoring and Validation](./authoring-and-validation.md): An interaction that evaluates authored content and presents its derived view and diagnostics.
- [Editing Round Trip](./editing-round-trip.md): An interaction that serializes a semantic edit and returns its rebuilt derived view.
- [Layout Solving](./layout-solving.md): An activity that validates layout inputs and produces view geometry or diagnostics.
- [Share Round Trip](./share-round-trip.md): An interaction that packs a bundle into a link and rebuilds that bundle in a browser.
- [Deployment Surfaces](./deployment-surfaces.md): The read, write, and editor-host boundaries of each user surface.
- [Web Delivery](./web-delivery.md): An activity that builds the native editor for a browser and publishes it as a static artifact.
