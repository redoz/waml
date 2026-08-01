---
type: uml.Class
title: Browser
description: The execution environment for the web-delivered native editor.
---

# Browser

## Relationships
- depends [WASM Web Artifact](./wasm-web-artifact.md)
- depends [Share Link](./share-link.md)

## Notes
- The browser runs the deployed editor from the static artifact. [Native Web Delivery](./native-web-delivery.md) publishes that artifact.
- The editor runs in one thread, because the static host cannot send the headers for shared memory.
- The browser gives the editor its content through the address of the page. There is no file system.
- The editor draws in one canvas element. It accepts a mouse, a keyboard, and touch input.
