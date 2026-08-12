---
type: uml.Class
title: waml Markdown Editor Crate
description: The crate that owns WAML Markdown reading and editing sessions, input, layout, and the Makepad widget.
stereotype: crate
sources:
  - id: manifest
    resource: ../../../../../crates/waml-markdown-editor/Cargo.toml
    title: crates/waml-markdown-editor/Cargo.toml
---

# waml Markdown Editor Crate

## Relationships
- depends [waml Syntax Crate](./waml-syntax-crate.md)

## Notes
- `waml-markdown-editor` owns Markdown reading and editing sessions, input processing, text layout, selection state, and the Makepad widget.
- It depends on `waml-syntax`. It does not depend on `waml` or `waml-editor`.
