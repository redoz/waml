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
- `docs/superpowers/plans/2026-07-28-undo-redo-view-history.md` specifies
  operation-based global undo and redo as one thing and Back/Forward logical
  view history as a separate one, including preview-tab replacement, manual tab
  switching, and undo revealing the editor it affected. The two-histories
  distinction is the important part: undo must not navigate and Back must not
  edit.
- `2026-07-31-easy-correctness-fixes.md` covers portable repeated configuration
  writes and pinning inline edits to the exact active preview tab.
