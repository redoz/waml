---
type: uml.Class
title: Command-Line Tool
description: A product responsibility that validates, formats, queries, and changes a bundle without a user interface.
---

# Command-Line Tool

## Relationships
- depends [Local Bundle](./local-bundle.md)
- depends [Validation and Diagnostics](../workflows/validation-and-diagnostics.md)
- depends [Canonical Serialization](../workflows/canonical-serialization.md)
- depends [Bundle Query](../workflows/bundle-query.md)
- depends [Share Link](./share-link.md)

## Notes
- The tool and editor share core source, analysis, formatting, query, edit, share, and delivery services. They expose different operations.
- The tool has commands for automation and no editor user interface. A person or a program can start it.
- It validates a bundle and reports the diagnostics. It can read the bundle from files or from the standard input.
- It writes the documents in canonical form. It can also report the documents that are not in canonical form and change no file.
- It can rebuild deterministic directory indexes or check them without a write.
- It applies semantic edits. Examples are: add an element, rename an element, change a feature, add a connection.
- It can apply a sequence of edits from one file. This makes an automated change repeatable.
- It answers the queries of the bundle and gives the answer as text or as data.
- It creates a share link for a bundle.
- It can export a static site or start the local editor and operations API.
- It gives the diagnostics and the answers as text for a person or as data for a program.
