# Round-Trip Losslessly

**Goal:** Bytes the author wrote and did not touch come back unchanged.

**Why:** A tool that reformats a file it merely opened poisons every diff and
makes itself unusable in a reviewed repository.

**Done when:** Parsing and reserializing any document in this bundle is a
byte-identical no-op, an edit changes only the region it touches, and no input
format discards authored bytes.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The syntax layer preserves losslessly and reparses incrementally, with
  property and fuzz coverage. This is the strongest part of the codebase.
- There is exactly one parse authority. Every derived view — OKF structure, UML
  analysis, diagnostics, the editor's model — is computed from that one
  revisioned tree. A second parser is the failure mode this design exists to
  prevent: two readings of the same bytes that disagree only sometimes.
- That boundary is enforced by the compiler and by Cargo, not by convention. A
  boundary that depends on reviewers remembering it is not a boundary.
- `issues.md` records that one input format can discard authored bytes. That
  single hole is what keeps this `partial`.
- Line endings and trailing whitespace count as authored bytes.
