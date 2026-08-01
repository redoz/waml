---
type: uml.Class
title: WASM Web Artifact
description: A static web-delivery artifact for the native editor.
stereotype: document
---

# WASM Web Artifact

## Relationships
- depends [Native Editor](./native-editor.md)
- depends [Native Web Delivery](./native-web-delivery.md)

## Notes
- The artifact is a set of static files. It contains no server part.
- The artifact contains the page, the WebAssembly module, the JavaScript that starts the module, the fonts, and the other resources.
- The artifact contains a version file. The page uses this file to find a newer version.
- The build removes the symbol names from the module and makes the module smaller. A browser then needs less time to start the editor.
