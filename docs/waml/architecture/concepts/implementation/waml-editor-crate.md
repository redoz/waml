---
type: uml.Class
title: waml Editor Crate
description: The crate that owns the app shell, editor session, document host, navigation, renderers, and platform adapters.
stereotype: crate
sources:
  - id: manifest
    resource: ../../../../../crates/waml-editor/Cargo.toml
    title: crates/waml-editor/Cargo.toml
---

# waml Editor Crate

## Relationships
- depends [waml Core Crate](./waml-core-crate.md)
- depends [waml Markdown Editor Crate](./waml-markdown-editor-crate.md)

## Notes
- `waml-editor` owns the app shell, `EditorSession`, the document host, navigation, tabs, diagram renderers, and native and browser adapters.
- Its local production dependencies point to `waml` and `waml-markdown-editor`.
