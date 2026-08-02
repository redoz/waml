# Save and Undo

**Goal:** An author never loses work and can always step back.

**Why:** Losing an author's work once ends the tool's credibility permanently.

**Done when:** Every edit is undoable and redoable at a granularity an author
would expect, a save writes every changed file atomically or writes none, a
failed save says so instead of reporting success, and closing with unsaved
changes asks.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Prepared-and-committed undo, savepoint identity, and savepoint-aware history
  compaction all exist in the session layer.
- The open risk recorded in `issues.md` is that native multi-file persistence
  has weaker guarantees than the command-line path — the multi-file atomicity
  half of "Done when" is the part to verify first.
- Undo granularity for prose editing is inherited from [Edit
  Prose](./edit-prose.md) and is not separately tracked here.
- There are two histories and they are not the same history. Undo and redo step
  through *operations* on the model. Back and forward step through *positions*
  in the bundle. Undo must never navigate as a side effect, and back must never
  change content. An undo whose target is not visible reveals the editor it
  affected before applying — otherwise the author sees nothing happen.
- An inline edit applies to the exact tab that was active when it started, not
  to whichever tab is active when it commits. A preview tab that gets replaced
  mid-edit is the case that breaks this.
