# Round-Trip Losslessly

**Goal:** The bytes that the author wrote and did not change come back without
a change.

**Why:** A tool that reformats a file that it only opened makes each diff
incorrect. Then the tool is not usable in a repository with reviews.

**Done when:** To parse and then write a document in this bundle changes no
byte. An edit changes the region of the edit only. No input format removes
bytes that the author wrote.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The syntax layer keeps all bytes and parses incrementally. It has property
  tests and fuzz tests. It is the strongest part of the code.
- One input format removes bytes that the author wrote. This one defect is the
  reason for the status `partial`.
- Line ends and space characters at the end of a line are bytes that the author
  wrote.
- There is one parse authority. Each derived view uses that one tree with
  revisions. The derived views are the OKF structure, the UML analysis, the
  diagnostics, and the model in the editor. A second parser causes two readings
  of the same bytes that do not agree in all conditions.
- The compiler and Cargo hold that boundary. Reviewers do not hold it. A
  boundary that needs a person to remember it is not a boundary.
