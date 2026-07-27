---
type: Diagram
title: System Context
description: Structural view of authors, bundles, native editor, CLI/LSP, and VS Code integration.
profile: uml-domain
---

# System Context

## Members
- [Author](./../concepts/workflows/author.md)
- [Editor](./../concepts/workflows/editor.md)
- [OKF Bundle](./../concepts/model/okf-bundle.md)
- [Local Bundle](./../concepts/runtime/local-bundle.md)
- [Native Editor](./../concepts/runtime/native-editor.md)

## Reading guide

Begin with [Authoring and Validation](./authoring-and-validation.md) for the
bundle and diagnostics boundary. Then read [Editing Round Trip](./editing-round-trip.md)
for native semantic edits. The Rust CLI provides `waml lsp --stdio`; the
independent VS Code extension starts that server through
`vscode-languageclient`.
