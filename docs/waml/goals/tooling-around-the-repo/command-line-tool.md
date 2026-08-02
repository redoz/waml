# Command-Line Tool

**Goal:** A person validates, formats, and queries a bundle from a shell or
from a build step.

**Why:** A continuous integration job cannot open a window. A repository that
keeps documentation as source needs a check that operates without a window.

**Done when:** The tool validates, formats, and queries a bundle from the
command line. A validation failure gives a non-zero exit code and a message
with a position. To format content that is already canonical changes no byte.

**Status:** done — unverified
**MVP:** no

## Notes

- The command-line path is the strongest persistence path in the code. It is
  stronger than the multi-file save in the native editor.
- The bundle query is an established behavior.
- [Report Every Problem](../trust-the-content/report-every-problem.md) also
  controls the diagnostics here. This surface has the same defect.
- `MVP: no`. The bar is about the editor. This tool exists and gives value.
