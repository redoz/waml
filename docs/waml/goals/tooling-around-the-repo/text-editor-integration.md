# Text Editor Integration

**Goal:** VS Code connects to the language server. The author sets no
configuration.

**Why:** An integration that needs instructions is an integration that few
persons install.

**Done when:** After the installation of the extension, a WAML document shows
diagnostics with no configuration. A missing server program or a server program
with an incorrect version causes a clear message.

**Status:** partial — unverified
**MVP:** no

## Notes

- The extension is a separate Node project. It starts the WAML language server
  through the standard language client.
- The extension is not in a marketplace. Thus to install it is to build it. The
  publication of the extension is the difference between `partial` and `done`.
- The test, lint, and build steps of the extension are part of the repository
  gate with the Rust tests.
