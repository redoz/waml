---
type: uml.Class
title: Local Bundle
description: A locally stored form of an OKF Bundle.
stereotype: document
---

# Local Bundle

## Relationships
- composes [OKF Bundle](../model/okf-bundle.md): 1 local to 1 bundle

## Notes
- The local bundle makes a bundle available to the native editor.
- On the desktop the local bundle is a directory of files. The editor writes each changed document to its file.
- In the browser there is no directory. The address of the page holds the full bundle. A [Share Link](./share-link.md) is the transport for that content.
- The editor keeps the changed documents apart from the saved documents. It can tell the author which documents are not yet saved.
