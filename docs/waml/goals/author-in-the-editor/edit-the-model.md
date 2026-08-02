# Edit the Model

**Goal:** An author changes typed structure with the inspector and the diagram.
The typed structure includes elements, members, relationships, and properties.
The author does not edit text to make these changes.

**Why:** Typed structure is the difference between WAML and a Markdown viewer.
An author who must write frontmatter and relationship lines by hand uses the
parser, not the editor.

**Done when:** The author can make, retype, rename, connect, and delete each
element kind and each relationship kind from the inspector or from the diagram.
Each such edit writes canonical Markdown in one transaction.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The edit path operates. It has typed operations, transactions in memory, undo
  with prepare and commit steps, and a save path.
- The coverage for each element kind is not known. The feature cuts in
  [UML](../uml/) have the status of each row. This goal is complete when those
  rows are complete.
- Drag to place with constraints, a context menu on a node, and a list of
  conflicts operate for changes to structure on the canvas.
