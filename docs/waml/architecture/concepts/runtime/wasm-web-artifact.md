---
type: uml.Class
title: WASM Web Artifact
description: A static web-delivery artifact for the native editor.
stereotype: document
---

# WASM Web Artifact

## Relationships
- depends [Native Editor](./native-editor.md)

## Notes
- Contains WebAssembly, JavaScript glue, and required resources for browser delivery.
- [Native Web Delivery](./native-web-delivery.md) owns the publication pipeline that produces this artifact.
