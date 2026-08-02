# Edit the Model

**Goal:** An author changes typed structure — elements, members, relationships,
properties — through the inspector and the diagram rather than through text.

**Why:** Typed structure is what distinguishes WAML from a Markdown viewer. An
author who must hand-write frontmatter and relationship lines is using the
parser, not the editor.

**Done when:** Every element kind and relationship kind in the model can be
created, retyped, renamed, connected, and deleted from the inspector or the
diagram, and each such edit serializes back to canonical Markdown in one
transaction.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The edit path is real: typed operations, in-memory transactions, prepared and
  committed undo, and a save path all exist.
- Coverage per element kind is the unknown. The per-kind feature cuts under
  [UML](../uml/) carry the authoritative row-by-row status; this goal is done
  when those rows are.
- Drag-to-place with constraints, a node context menu, and a conflict list
  exist for structural editing on the canvas.
