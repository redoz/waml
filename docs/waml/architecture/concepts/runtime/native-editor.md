---
type: uml.Class
title: Native Editor
description: A product responsibility that provides WAML editing on the desktop.
---

# Native Editor

## Relationships
- depends [Editor](../workflows/editor.md)
- depends [Local Bundle](./local-bundle.md)

## Notes
- Project-tree document entries use preview tabs: a single click opens or
  focuses the shared preview, while a double click opens or focuses the same
  tab and makes it persistent. Double-clicking an already-open preview promotes
  it in place; persistent tabs are not duplicated or demoted. Folder expansion
  is unchanged.
