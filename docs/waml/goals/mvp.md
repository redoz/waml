# The MVP Definition

This document consolidates the MVP scope, its aggregate status, the ordered
backlog, the explicit deferrals, and the completion conditions. The [Root
Goal](./root-goal.md) states the bar. The goal leaves are the source of truth.
This document derives its status from those leaves.

## Scope statement

**The MVP is:** an author writes and reads `docs/waml` fully in the native
editor, with no text editor, and sends it as a link that a reader opens in a
browser with no installation and no account.

The MVP includes:

- Plain Markdown on disk that is lossless, canonical, and diffable.
- One editor with native and browser delivery forms.
- A tree, preview and permanent tabs, document views, diagram views,
  navigation history, and a responsive window.
- Native authoring for documents, prose, typed models, canvas operations,
  save, and undo.
- Class, sequence, and activity diagrams with their shared layout, routing,
  label, selection, and theme behavior.
- Diagnostics, references, indexes, sharing, and publication.

The MVP does not require:

- A full state-machine or use-case diagram cut.
- Browser authoring, `waml serve`, or a browser save-to-disk path.
- Image export.
- Canvas-authored layout overrides, keyboard-only authoring, more templates,
  or effort budgets.
- The language server, the VS Code extension, or marketplace publication.
- Multi-user editing, comments, search, cross-bundle links, or non-UML typed
  projections.

## Aggregate status

An area is `done` only when all required leaves satisfy their completion
conditions. An area is `partial` when shipped behavior exists and at least one
required leaf is `partial` or `planned`. A verification gap stays separate
from product state and does not change shipped behavior into a discrepancy.

| Area | Status | Evidence-derived reason |
| --- | --- | --- |
| Read a bundle | partial | Seven leaves are `done`; [Read a Diagram](read-a-bundle/read-a-diagram.md) is `planned`. |
| Author in the editor | partial | Prose editing and save/undo are `done`; required document, model, and canvas authoring leaves are `partial`. |
| Trust the content | partial | Round-trip and formatting are `done`; diagnostics, references, and index correction are `partial`. |
| Class diagrams | partial | Interaction is `done`; the [class feature cut](uml/class/feature-cut.md) is `partial`. |
| Sequence diagrams | partial | Interaction is `done`; the [sequence feature cut](uml/sequence/feature-cut.md) is `partial`. |
| Activity diagrams | partial | Interaction is `done`; the [activity feature cut](uml/activity/feature-cut.md) is `partial`. |
| Shared diagram behavior | partial | Layout stability and label placement are `planned`; layout, routing, and selection goals are `partial`. |
| Share and publish | partial | Share is `done`; browser parity and publication are `partial`. |
| Tooling around the repo | partial | The CLI is `done`; the language server and VS Code integration are `partial`. This area is not in the MVP bar. |
| Beyond UML | horizon | [Beyond UML](./beyond-uml.md) is a direction with no delivery condition. |

The root remains `partial`. Required MVP leaves are not complete, and required
shared UML goals still have `planned` work.

## Verification gaps

Shipped scenarios without target-boundary automation remain shipped. Each
owning leaf records the exact native or browser gap under `## Verification
gaps`. These records are test work. They are not product discrepancies.

The highest concentrations are the sequence-language contract, browser and
local-serve workflows, and native diagram presentation. The inventory and the
goal leaves must contain the same target and reason for each gap.

## Gap backlog

The order follows the dependencies in the goal tree.

1. Complete diagnostics, reference resolution, and index correction.
2. Complete document, model, and canvas authoring in the native editor.
3. Complete the class, sequence, and activity feature cuts.
4. Implement shared layout stability and label placement, then complete
   routing and selection behavior.
5. Complete diagram reading for every supported kind.
6. Close browser parity and publication gaps, including visible API boot
   failures and diagonal-content rendering.
7. Add target-boundary tests for the recorded verification gaps without
   changing product state.

## Explicit deferrals

| Deferral | Owner | Reason |
| --- | --- | --- |
| State-machine full cut | [State-machine cut](uml/state-machine/feature-cut.md) | The MVP does not require a complete lifecycle-diagram cut. |
| Specialized use-case view | [Use-case cut](uml/use-case/feature-cut.md) | The semantic model exists; specialized presentation is later work. |
| Canvas-authored layout overrides | [Arrange a Diagram](author-in-the-editor/arrange-a-diagram.md) | This goal is outside the MVP bar. |
| More template tiers | [Start from a Template](author-in-the-editor/start-from-a-template.md) | This goal is outside the MVP bar. |
| Keyboard-only authoring | [Author with the Keyboard](author-in-the-editor/author-with-the-keyboard.md) | This goal is outside the MVP bar. |
| Effort budgets | [Reduce the Effort](author-in-the-editor/reduce-the-effort.md) | This measurement goal is outside the MVP bar. |
| Local serve and browser save-to-disk | [Serve Locally](share-and-publish/serve-locally.md) | The product can serve and write a local directory, but the MVP does not require that workflow. |
| Image export | [Export a Bundle](share-and-publish/export-a-bundle.md) | This output is a post-MVP function. |
| LSP completion and extension publication | [Language Server](tooling-around-the-repo/language-server.md), [Text Editor Integration](tooling-around-the-repo/text-editor-integration.md) | Tooling is outside the MVP bar. |
| Search, collaboration, and non-UML projections | [Beyond UML](./beyond-uml.md) | These functions are horizon work. |

## Definition of done

| Area | Done when |
| --- | --- |
| Language and trust | The parser, formatter, diagnostics, references, and indexes satisfy their linked goal conditions. |
| Native editor | An author creates, reads, edits, saves, and undoes the required `docs/waml` workflows without a text editor. |
| UML | Each required class, sequence, and activity cut is complete, and required shared goals have no `planned` work. |
| Browser | A shared link opens the same required views, and browser failures have visible results. |
| Publication | A push publishes a complete artifact or stops with a reported failure. |
| Documentation | `docs/waml` validates, formats canonically, resolves its links, and keeps its indexes correct. |

## Discrepancies

- BHV-BRW-014 — This MVP scope says that browser authoring and `waml serve`
  are post-MVP. Current source serves the editor at
  `crates/waml-cli/src/serve/mod.rs:30` and sends document writes at
  `crates/waml-editor/src/api_save.rs:43`.
