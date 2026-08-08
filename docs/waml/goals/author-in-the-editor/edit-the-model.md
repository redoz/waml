# Edit the Model

**Goal:** An author changes typed structure with the inspector or the diagram.

**Why:** Typed structure is a main product function. The author must not edit
frontmatter and relationship lines for each model change.

**Done when:** The author can add, retype, rename, connect, and delete each
supported element and relationship kind. Each edit writes canonical Markdown
as one transaction.

**Status:** partial
**MVP:** yes

## Notes

- The UML feature-cut documents own language and model coverage.
- Diagram-specific selection, tools, and direct manipulation are owned by the
  interaction goal for that diagram kind.
- [Save and Undo](./save-and-undo.md) owns transaction history and persistence.
