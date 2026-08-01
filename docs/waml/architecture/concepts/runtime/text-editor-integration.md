---
type: uml.Class
title: Text Editor Integration
description: An extension that starts the language server for Markdown documents in a general text editor.
---

# Text Editor Integration

## Relationships
- depends [Language Server](./language-server.md)

## Notes
- The extension starts the language server and shows its diagnostics in the text editor.
- The extension becomes active for Markdown documents. The author needs no special file type.
- The extension is a separate product part with its own release. The editor and the command-line tool do not need it.
- A setting gives the location of the program that supplies the server.
