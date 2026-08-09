---
type: Diagram
title: System Context
description: Structural view of the author, the bundle, and the four product surfaces that read or change it.
profile: uml-domain
---

# System Context

## Members

### People
- [Author](./../concepts/workflows/author.md)

### Content
- [OKF Bundle](./../concepts/model/okf-bundle.md)
- [Local Bundle](./../concepts/runtime/local-bundle.md)
- [Share Link](./../concepts/runtime/share-link.md)

### Surfaces
- [Native Editor](./../concepts/runtime/native-editor.md)
- [Command-Line Tool](./../concepts/runtime/command-line-tool.md)
- [Language Server](./../concepts/runtime/language-server.md)
- [Text Editor Integration](./../concepts/runtime/text-editor-integration.md)

### Behavior
- [Editor](./../concepts/workflows/editor.md)
- [Validation and Diagnostics](./../concepts/workflows/validation-and-diagnostics.md)

## Reading guide

The author is at the left. The content is in the middle. The four surfaces are
at the right. Each surface reads or changes the same bundle. No surface owns a
private form of the content.

The native editor and command-line tool share source, analysis, formatting,
query, edit, share, and delivery services. They expose different operations.
The editor owns interactive document and diagram workflows. The tool owns
commands for automation and has no editor user interface. The
[App Shell](../concepts/implementation/app-shell.md) and the other
implementation concepts define these boundaries.

The language server gives diagnostics, symbols, links, definitions, and
semantic tokens to a general text editor. The text editor integration starts
that server for Markdown documents.

Read [Authoring and Validation](./authoring-and-validation.md) for the boundary
between the bundle and the diagnostics. Read
[Editing Round Trip](./editing-round-trip.md) for a semantic edit. Read
[Deployment Surfaces](./deployment-surfaces.md) for read, write, and hosting
boundaries. Read [Web Delivery](./web-delivery.md) for browser publication.
