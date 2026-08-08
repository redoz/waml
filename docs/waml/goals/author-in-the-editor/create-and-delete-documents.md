# Create and Delete Documents

**Goal:** An author adds, renames, moves, and removes documents and packages in
the editor.

**Why:** The author must not need a separate file manager for bundle structure.

**Done when:** Each operation starts from the project tree. Each affected index
becomes current, and each affected link is corrected or reported. The author
can undo the operation.

**Status:** partial
**MVP:** yes

## Planned behavior

Rename, move, delete, link repair, and transactional index correction have no
passing acceptance scenario in the frozen inventory.

## Notes

- A new-package path exists, but the complete four-operation workflow does not
  have target-boundary evidence.
- [Keep Indexes Correct](../trust-the-content/keep-indexes-correct.md) owns
  generated index content.
- [Resolve References](../trust-the-content/resolve-references.md) owns broken
  reference detection.
