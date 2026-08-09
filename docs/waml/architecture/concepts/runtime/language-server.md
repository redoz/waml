---
type: uml.Class
title: Language Server
description: A responsibility that gives bundle diagnostics and navigation data to a text editor while the author types.
---

# Language Server

## Relationships
- depends [Validation and Diagnostics](../workflows/validation-and-diagnostics.md)
- depends [Local Bundle](./local-bundle.md)
- associates [Diagnostic](../model/diagnostic.md): 1 server to 0..* diagnostics

## Notes
- The server gives the diagnostics of the bundle to a text editor. The author sees a problem during the work in the document.
- The server reads the full bundle from the directory of the workspace. It does not read only the open documents.
- The server sends the diagnostics for all documents of the bundle. A change in one document can change the diagnostics of a different document.
- The server uses the standard protocol of the language servers. It speaks through the standard input and the standard output.
- The server gives diagnostics, document symbols, document links, definitions, and full semantic tokens.
- The server does not give completion or formatting, and it does not advertise a bundle-write operation.
