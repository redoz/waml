# Create and Delete Documents

**Goal:** An author adds, renames, moves, and removes documents and packages in
the editor.

**Why:** The MVP bar fails at the first new file if the author must use a file
manager.

**Done when:** The four operations start from the tree. Each index that names
the document becomes correct again. Each link to the document is corrected or
is reported as broken. The author can undo the operation.

**Status:** partial — unverified
**MVP:** yes

## Notes

- A flow to make a new package operates. Verify whether rename and move
  operate.
- Index correction is the weak point. The library has a function that makes
  each index again from the model, but no product code calls it. Thus the
  editor does not correct an index after a change to structure. Refer to [Keep
  Indexes Correct](../trust-the-content/keep-indexes-correct.md).
- The behavior of a link after a rename is a decision, not a detail. To correct
  the link automatically is acceptable. To report the link as broken is also
  acceptable. This tree must select one behavior.
