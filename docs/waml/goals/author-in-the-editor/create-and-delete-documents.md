# Create and Delete Documents

**Goal:** An author adds, renames, moves, and removes documents and packages
inside the editor.

**Why:** The dogfood bar fails at the first new file if this needs a file
manager.

**Done when:** All four operations work from the tree, every index that names
the affected document is regenerated, every link that pointed at it is either
updated or reported as broken, and the operation is undoable.

**Status:** partial — unverified
**MVP:** yes

## Notes

- A new-package flow exists. Whether rename and move exist at all is the
  unverified part.
- Index regeneration is the sharp edge: `reindex_source` exists in the library
  but has no product caller, so the editor is not currently rebuilding indexes
  after a structural change. See [Keep Indexes
  Correct](../trust-the-content/keep-indexes-correct.md).
- Link rewriting on rename is a real design question, not a detail: silently
  rewriting an author's links and leaving them broken are both defensible, and
  the tree must pick one.
