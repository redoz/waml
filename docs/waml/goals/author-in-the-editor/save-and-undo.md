# Save and Undo

**Goal:** An author does not lose work. An author can always go back one step.

**Why:** If the tool loses the work of an author one time, the author does not
trust the tool again.

**Done when:** The author can undo and redo each edit, at a size of step that
the author expects. A save writes each changed file, or writes no file. A save
that fails reports the failure. A save that fails does not report success. If
the author closes the editor with unsaved changes, the editor asks first.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Undo with prepare and commit steps, savepoint identity, and history
  compaction that knows the savepoints operate in the session layer.
- Multi-file persistence in the native editor has weaker guarantees than the
  command-line path. Verify the atomic behavior of a multi-file save first.
- There are two histories and they are different. Undo and redo move through
  operations on the model. Back and forward move through positions in the
  bundle. Undo must not navigate. Back must not change content.
- If the target of an undo is not visible, the editor shows that editor first
  and then applies the undo. Without this step, the author sees no result.
- An inline edit applies to the tab that was active when the edit started. It
  does not apply to the tab that is active when the edit completes. A preview
  tab that another document replaces during the edit is the condition that
  causes the defect.
- [Edit Prose](./edit-prose.md) controls the size of an undo step for text.
  This goal does not.
