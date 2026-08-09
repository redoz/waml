# Architecture overview

Use these guides to move from the product model to its workflows and runtime.

## Understand the model

- [Model concepts](./concepts/model/index.md): The authored bundle and resolved WAML model.
- [Implementation concepts](./concepts/implementation/index.md): The current crates, analysis state, editor runtime, and platform boundaries.
- [WAML Domain Model](./views/domain-model.md): The authored bundle, resolved model, views, and diagnostics.
- [Model Vocabulary](./views/model-vocabulary.md): The element kinds, presentation terms, constraints, and solved geometry.

## Follow a workflow

- [Workflow concepts](./concepts/workflows/index.md): The responsibilities that author, validate, edit, query, and lay out WAML content.
- [Preparation Pipeline](./views/preparation-pipeline.md): The immutable analysis sequence that prepares a candidate snapshot.
- [Incremental Analysis](./views/incremental-analysis.md): The affected-analysis and quarantine flow after exact text changes.
- [Revisioned Edit Transaction](./views/revisioned-edit-transaction.md): The source-edit and semantic-edit revision paths.
- [Authoring and Validation](./views/authoring-and-validation.md): The interaction that evaluates content and reports local failures.
- [Editing Round Trip](./views/editing-round-trip.md): The prepare-then-commit interaction for a semantic edit.
- [Layout Solving](./views/layout-solving.md): The activity that validates layout inputs and produces geometry or diagnostics.

## Run the product

- [System Context](./views/system-context.md): The author, bundle, and primary product surfaces.
- [Runtime concepts](./concepts/runtime/index.md): The editor, command-line tool, language server, and browser delivery.
- [Deployment Surfaces](./views/deployment-surfaces.md): The read, write, and editor-host boundaries of each user surface.
- [Share Round Trip](./views/share-round-trip.md): The link encoding and browser reconstruction path.
- [Web Delivery](./views/web-delivery.md): The publication of the editor as a static WebAssembly artifact.
