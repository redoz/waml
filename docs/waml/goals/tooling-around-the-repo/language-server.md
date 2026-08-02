# Language Server

**Goal:** A text editor gets diagnostics, navigation, and completion for WAML
documents.

**Why:** An author who prefers a different editor must not lose access to WAML.
The language server gives that access.

**Done when:** Diagnostics, movement to a definition in another document, and
completion of element names and relationship names operate against a live
buffer. The server never answers from content on disk when the editor holds
newer content.

**Status:** partial — unverified
**MVP:** no

## Notes

- The server operates and the VS Code extension starts it.
- The server can answer from a file on disk while the editor holds newer text.
  Correct this defect first.
- Completion of relationship kinds and link targets is the function with the
  highest value that does not exist.
