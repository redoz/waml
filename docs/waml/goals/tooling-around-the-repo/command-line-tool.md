# Command-Line Tool

**Goal:** A bundle is validated, formatted, and queried from a shell or a build
step.

**Why:** Continuous integration cannot open a window. A repository that keeps
documentation as source needs a check that runs without one.

**Done when:** A bundle can be validated, formatted, and queried from the
command line; a validation failure exits non-zero with a positioned message;
and formatting is a no-op on already-canonical input.

**Status:** done — unverified
**MVP:** no

## Notes

- The command-line surface is the strongest persistence path in the codebase —
  stronger than the native editor's multi-file save.
- Bundle query is an established workflow concept.
- Diagnostics coverage here inherits the aggregation hole from [Report Every
  Problem](../trust-the-content/report-every-problem.md).
- `MVP: no`: the dogfood bar is about the editor. This exists and is useful
  anyway.
