# Serve Locally

**Goal:** A single command serves the editor over loopback against a local
directory, so a reader can open a bundle in a browser without a build step and
without a hosted site.

**Why:** It closes the gap between the desktop form and the published form: the
same web artifact, the same views, but reading and writing the author's own
files. It is also the honest backend for saving from the web form, which is a
stub today.

**Done when:** `waml serve` serves the embedded web editor over loopback and
exposes an operations API over a chosen directory, edits made in the browser
land on disk through that API, and the command refuses to bind anywhere but
loopback.

**Status:** planned — unverified
**MVP:** no

## Notes

- Specified in full: `docs/superpowers/plans/2026-07-25-waml-serve.md` and its
  design. Nothing here is an open question, only unbuilt.
- The plan names the native `save_backend` stub as the hole this fills, which
  makes this goal a dependency of web-form authoring rather than a convenience.
- Loopback only. A serve command that binds a public interface is a different
  product with a security model this project has not designed.
