# Serve Locally

**Goal:** One command serves the editor on the loopback interface against a
local directory. A reader opens a bundle in a browser with no build step and
with no published site.

**Why:** This command removes the difference between the desktop form and the
published form. It uses the same web artifact and the same views, but it reads
and writes the files of the author.

**Done when:** The command serves the embedded web editor on the loopback
interface. The command gives an operations interface for a selected directory.
An edit in the browser writes to disk through that interface. The command binds
to the loopback interface only.

**Status:** planned — unverified
**MVP:** no

## Notes

- The web form cannot write to disk at this time. Its save function is
  incomplete. This command is that function. Until this command exists,
  authoring in a browser is not possible. Thus web authoring needs this goal.
- The served editor and the published editor are the same artifact. A serve
  command with its own build is a second product to keep correct.
- The command uses the loopback interface only. A serve command on a public
  interface is a different product and needs a security design that this
  project does not have.
