# Report Every Problem

**Goal:** Diagnostics from every layer — shell, frontmatter, syntax, model,
layout — reach the reader at every surface.

**Why:** A problem that is detected and then dropped at a layer boundary is
worse than one never detected: the tool looks confident and is wrong.

**Done when:** A document with a problem at any layer shows that problem in the
editor, in the command-line output, and in the language server, each with a
position, and no layer's diagnostics are discarded on the way out.

**Status:** partial — unverified
**MVP:** yes

## Notes

- This is the standing P1 in `issues.md`: shell and frontmatter diagnostics
  disappear at public boundaries because diagnostics are not aggregated across
  parsing layers.
- Aggregation is one change that fixes all three surfaces. It is the highest-
  value single item in this tree.
- Diagnostic *quality* — wording, suggested fix — is a separate concern and not
  part of this goal's bar.
