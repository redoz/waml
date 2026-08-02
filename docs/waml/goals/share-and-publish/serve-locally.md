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

- The web form has no way to write to disk today; its save backend is a stub.
  This command is that backend. Until it exists, authoring in a browser is
  reading with extra steps, which makes this a dependency of web-form
  authoring rather than a convenience.
- The served editor and the published editor are the same artifact. A serve
  command that shipped its own build would be a second product to keep
  correct.
- Loopback only. A serve command that binds a public interface is a different
  product with a security model this project has not designed.
