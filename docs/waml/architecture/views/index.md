# Architecture views

Implementation ownership is defined by the [waml Core Crate](../concepts/implementation/waml-core-crate.md).

* [WAML Domain Model](./domain-model.md) - Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.
* [Model Vocabulary](./model-vocabulary.md) - Structural view of the element kinds, their labels, the presentation lens, and the solved geometry.
* [System Context](./system-context.md) - Structural view of the author, the bundle, and the four product surfaces that read or change it.
* [Crate Ownership](./crate-ownership.md) - The current dependency direction and ownership of the six production WAML crates.
* [Editor Ownership](./editor-ownership.md) - The current ownership boundaries of the editor composition root, state, documents, views, and platform effects.
* [Preparation Pipeline](./preparation-pipeline.md) - The immutable pipeline that prepares source, Markdown, OKF, UML, affected, and revision state for installation.
* [Incremental Analysis](./incremental-analysis.md) - The document-local edit, source promotion, and semantic analysis flow for an incremental Markdown change.
* [Revisioned Edit Transaction](./revisioned-edit-transaction.md) - The document-revision, session-revision, preparation, commit, and stale-completion paths for edits.
* [Authoring and Validation](./authoring-and-validation.md) - An interaction that evaluates authored content and presents its derived view and diagnostics.
* [Editing Round Trip](./editing-round-trip.md) - An interaction that serializes a semantic edit and returns its rebuilt derived view.
* [Layout Solving](./layout-solving.md) - An activity that validates layout inputs and produces view geometry or diagnostics.
* [Share Round Trip](./share-round-trip.md) - An interaction that packs a bundle into a link and rebuilds that bundle in a browser.
* [Deployment Surfaces](./deployment-surfaces.md) - The read, write, and editor-host boundaries of the desktop, browser, command-line, language-server, and VS Code surfaces.
* [Web Delivery](./web-delivery.md) - An activity that builds the native editor for a browser and publishes it as a static artifact.
