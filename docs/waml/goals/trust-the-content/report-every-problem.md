# Report Every Problem

**Goal:** Diagnostics from each layer come to the reader at each surface. The
layers are the shell, the frontmatter, the syntax, the model, and the layout.

**Why:** A problem that the tool finds and then discards is worse than a
problem that the tool does not find. The tool looks correct and is not correct.

**Done when:** A document with a problem at any layer shows that problem in the
editor, in the command-line output, and in the language server. Each report has
a position. No layer discards its diagnostics before the output.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Diagnostics from the shell layer and the frontmatter layer disappear at the
  public boundaries, because the tool does not collect diagnostics across the
  parse layers. This is the highest-priority defect in the product.
- One change corrects all three surfaces. Thus this item has the highest value
  in the tree.
- The quality of the text of a diagnostic is a different subject. It is not
  part of this goal.
