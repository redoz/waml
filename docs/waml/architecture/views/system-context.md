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

## Layout
- Content as column with frame
- Surfaces as column with frame
- Behavior as column with frame
- People left of Content
- Content left of Surfaces
- Surfaces left of Behavior
- center of People aligned with center of Content

## Reading guide

The author is at the left. The content is in the middle. The four surfaces are
at the right. Each surface reads or changes the same bundle. No surface owns a
private form of the content.

The native editor and the command-line tool do the same operations. The editor
adds a user interface. The tool has no user interface and a program can start
it.

The language server gives diagnostics to a general text editor. The text editor
integration starts that server. Both report the diagnostics of the full bundle.

Read [Authoring and Validation](./authoring-and-validation.md) for the boundary
between the bundle and the diagnostics. Read
[Editing Round Trip](./editing-round-trip.md) for a semantic edit. Read
[Web Delivery](./web-delivery.md) for the publication of the editor to a
browser.
