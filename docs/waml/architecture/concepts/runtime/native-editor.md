---
type: uml.Class
title: Native Editor
description: A product responsibility that provides WAML editing in native and web-delivered forms.
---

# Native Editor

## Relationships
- depends [Editor](../workflows/editor.md)
- depends [Local Bundle](./local-bundle.md)

## Notes
- Provides the Editor responsibility as a desktop application and as WebAssembly.
- [Native Web Delivery](./native-web-delivery.md) owns the browser-publication pipeline.
