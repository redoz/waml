# Publish a Site

**Goal:** Each push to the main branch publishes the web artifact again. A
person does no step.

**Why:** A publication step that a person must remember does not occur.

**Done when:** A push to the main branch makes an artifact that operates, or
the workflow fails and reports the failure. No defect that the local gate finds
comes to the published build.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The workflow builds the artifact and then processes it. The steps remove
  unused fonts, add the brand, and add the runtime shell.
- The delivery automation does not stop defects before the web build. This is
  the reason for the status `partial`.
- The build tool must have the same revision of the graphics library as the
  editor. Use an exact revision. Do not use the most recent commit of a branch.
