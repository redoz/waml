---
type: uml.Class
title: Browser
description: The execution environment for the web-delivered native editor.
---

# Browser

## Relationships
- depends [WASM Web Artifact](./wasm-web-artifact.md)

## Notes
- Executes the deployed native editor from its static web artifact.
- [Native Web Delivery](./native-web-delivery.md) owns how that artifact is published.
