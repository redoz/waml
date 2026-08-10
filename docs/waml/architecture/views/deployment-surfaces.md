---
type: uml.ActivityDiagram
title: Deployment Surfaces
description: The read, write, and editor-host boundaries of the desktop, browser, command-line, language-server, and VS Code surfaces.
---

# Deployment Surfaces

## Notes
- [Platform Adapter](../concepts/implementation/platform-adapter.md) owns native and browser external effects.
- [Native Editor](../concepts/runtime/native-editor.md) is one editor product with desktop and WebAssembly builds.
- A browser boot source can be a share fragment, an exported-site bundle, or a local API. These inputs do not create a second editor implementation.
- [Command-Line Tool](../concepts/runtime/command-line-tool.md) and the editor share core services but expose different operations.
- The language server advertises diagnostics, document symbols, document links, definitions, and full semantic tokens. It does not advertise writes.
- The VS Code extension starts the language server for Markdown. VS Code remains the text editor and does not host the WAML editor.

## Nodes

### initial
- transitions to Select User Surface

### decision Select User Surface
- when `desktop application` transitions to Desktop Editor
- when `static WebAssembly application` transitions to Static WebAssembly Editor
- when `share link` transitions to Share Link Boot
- when `exported site` transitions to Exported Site Boot
- when `local serve or API` transitions to Local Serve and API
- when `command line` transitions to Command-Line Tool
- when `language server` transitions to Language Server
- when `VS Code` transitions to VS Code Integration

### Desktop Editor
- do: `host the editor and read or write a local bundle through the native adapter`
- transitions to final

### Static WebAssembly Editor
- do: `host the same editor build and read a bundle from the selected browser boot source`
- transitions to Browser Output

### Share Link Boot
- do: `decode the complete bundle from the URL fragment and host it in the browser editor`
- transitions to Browser Output

### Exported Site Boot
- do: `read the site's bundle file and host it in the browser editor without changing that static file`
- transitions to Browser Output

### Local Serve and API
- do: `host the browser editor and read or write authorized documents with revision checks`
- transitions to Browser Output

### Browser Output
- do: `keep edits in editor state and write through a share fragment, export, or configured API`
- transitions to final

### Command-Line Tool
- do: `read a bundle and run the selected check, format, index, query, mutation, share, export, or serve command`
- transitions to final

### Language Server
- do: `read workspace and open-document state and publish diagnostics, symbols, links, definitions, and semantic tokens`
- transitions to final

### VS Code Integration
- do: `edit Markdown with VS Code and start the WAML language server for analysis features`
- transitions to final

### final
