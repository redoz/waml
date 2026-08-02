# Language Server

**Goal:** A text editor gets diagnostics, navigation, and completion for WAML
documents.

**Why:** Authors who prefer their own editor should not be locked out, and the
language server is how WAML reaches them.

**Done when:** Diagnostics, go-to-definition across documents, and completion
of element and relationship names work over a live buffer, and the server never
answers from stale disk content.

**Status:** partial — unverified
**MVP:** no

## Notes

- `waml lsp --stdio` exists and is what the VS Code extension launches.
- `issues.md` records that the language server's disk authority becomes stale —
  it can answer from a file on disk while the editor holds newer text. That is
  the defect to fix first here.
- Completion of relationship kinds and link targets is the highest-value
  feature not yet built.
