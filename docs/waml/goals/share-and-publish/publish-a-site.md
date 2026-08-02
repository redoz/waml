# Publish a Site

**Goal:** Every push to main republishes the web artifact without a human step.

**Why:** A publish that needs remembering does not happen.

**Done when:** A push to main produces a working published artifact or fails
the workflow loudly, and no defect that the local gate would catch reaches the
published build.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The Pages workflow builds the artifact and post-processes it: font pruning,
  branding, and a runtime shell injection.
- `issues.md` records that delivery automation does not stop defects reaching
  the web build. That is what keeps this `partial`.
- `cargo-makepad` must be installed at the same makepad revision the editor
  pins. Pin the revision, never a branch tip.
