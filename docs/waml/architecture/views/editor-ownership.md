---
type: Diagram
title: Editor Ownership
description: The current ownership boundaries of the editor composition root, state, documents, views, and platform effects.
profile: uml-domain
---

# Editor Ownership

## Notes
- The app shell owns composition and user-action coordination. `EditorSession` owns the installed revisioned snapshot and edit transaction.
- The document host owns open-tab state and the live `DocView` registry.
- Navigation resolves semantic targets before the app shell sends open, activate, promote, close, or history commands to this boundary.
- The document host calls the common `DocView` lifecycle. It does not dispatch on Markdown, class, activity, or sequence document families.
- The Markdown editor owns its document-local session, input, layout, and widget. It does not own `EditorSession`.
- Diagram renderers consume installed UML analysis. They do not commit source or analysis state.
- Platform adapters own native and browser effects. The app shell invokes them at the product boundary.
- A removed tab cannot retain a live view. Reconciliation releases the old view when preview replacement removes its tab.
- [FG-009 — component ports and transactions](../../waml-feature-gaps.md#fg-009-—-component-ports-and-transactions) records why this view uses dependencies and notes instead of typed component ports.

## Members

### Coordination
- [App Shell](../concepts/implementation/app-shell.md)
- [Editor Session](../concepts/implementation/editor-session.md)
- [Document Host](../concepts/implementation/document-host.md)

### Content presentation
- [Markdown Editor](../concepts/implementation/markdown-editor.md)
- [Diagram Renderer](../concepts/implementation/diagram-renderer.md)

### Platform boundary
- [Platform Adapter](../concepts/implementation/platform-adapter.md)
