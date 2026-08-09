---
type: uml.Class
title: Native Editor
description: A product responsibility that provides WAML editing in native and web-delivered forms.
---

# Native Editor

## Relationships
- depends [Editor](../workflows/editor.md)
- depends [Local Bundle](./local-bundle.md)
- depends [Native Web Delivery](./native-web-delivery.md)

## Notes
- The native editor is one application. It runs as a desktop application and in a browser. The two forms show the same views.
- The desktop build reads and writes local bundles through its native adapter. The browser build reads a configured boot source and writes through share, export, or API paths.
- The editor draws all views with the graphics processor. It does not use a document object model.
- The window has three areas: a document tree, the view of the active document, and an inspector. The reader can hide the tree and the inspector.
- A narrow window puts the tree and the inspector above the view. A wide window puts them at the side.
- Document entries in the tree use preview tabs. A single click opens or selects the shared preview tab. A double click opens or selects the same tab and makes the tab permanent.
- A double click on an open preview tab makes that tab permanent in its position. The editor does not duplicate a permanent tab and does not make it a preview tab again.
- The editor keeps a navigation history. The reader can go back to the previous position and forward again.
- A start screen lists the recent bundles. The reader can attach a bundle to that list to keep it in the first position.
