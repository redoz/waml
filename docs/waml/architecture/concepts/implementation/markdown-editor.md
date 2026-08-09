---
type: uml.Class
title: Markdown Editor
description: The WAML-owned Markdown document session, input controller, layout pipeline, and Makepad widget.
stereotype: runtime
sources:
  - { id: markdown-widget, resource: ../../../../../crates/waml-markdown-editor/src/widget.rs, title: crates/waml-markdown-editor/src/widget.rs::MarkdownEditor }
  - { id: markdown-session, resource: ../../../../../crates/waml-markdown-editor/src/session.rs, title: crates/waml-markdown-editor/src/session.rs::MarkdownDocumentSession }
---

# Markdown Editor

## Relationships
- depends [Markdown Syntax](./markdown-syntax.md)

## Notes
- `MarkdownDocumentSession` owns a document snapshot, selections, local revision, edit history, IME state, and scroll state.
- `MarkdownEditor` owns Makepad event handling, layout installation, drawing, focus, pointer input, and the bridge to the input controller.
- The Markdown editor crate does not depend on `EditorSession`. An adapter in `waml-editor` supplies the document session and promotes accepted source proposals.
