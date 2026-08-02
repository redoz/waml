# Text Editor Integration

**Goal:** VS Code speaks to the language server without the author configuring
anything.

**Why:** An integration that needs setup instructions is an integration nobody
installs.

**Done when:** Installing the extension gives diagnostics on a WAML document
with no configuration, and a missing or mismatched server binary is reported
clearly.

**Status:** partial — unverified
**MVP:** no

## Notes

- The extension is a standalone Node project under `editors/vscode` and
  launches `waml lsp --stdio` through `vscode-languageclient`.
- It is not published to a marketplace, so "installing" today means building
  it. Publishing is the gap between `partial` and `done`.
- Its test, lint, and build steps are part of the repository gate alongside the
  Rust workspace tests.
