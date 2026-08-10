---
type: uml.ActivityDiagram
title: Web Delivery
description: An activity that builds the native editor for a browser and publishes it as a static artifact.
---

# Web Delivery

## Notes
- [Native Web Delivery](./../concepts/runtime/native-web-delivery.md) owns the conditions of this activity.
- [waml Editor Crate](../concepts/implementation/waml-editor-crate.md) is the shared desktop and WebAssembly editor implementation.
- [waml CLI Crate](../concepts/implementation/waml-cli-crate.md) assembles exported and published sites.
- The published program is the native editor. It is not a separate web application.
- The check before the upload is the reason for the two ends: an incomplete artifact stops the activity and does not become available.

## Nodes

### initial
- transitions to Publication Requested

### Publication Requested
- do: `accept a change on the main branch or a manual command`
- transitions to Build Without Threads

### Build Without Threads
- do: `build the editor as WebAssembly without threads`
- transitions to Remove Unused Fonts

### Remove Unused Fonts
- transitions to Add Product Identity

### Add Product Identity
- do: `add the product name and the product icon to the page`
- transitions to Add Start Screen and Version

### Add Start Screen and Version
- do: `add the start screen and the version file`
- transitions to Artifact Complete?

### decision Artifact Complete?
- when `every referenced file is present` transitions to Publish Artifact
- else transitions to Stop Publication

### Publish Artifact
- transitions to Browser Runs the Editor

### Browser Runs the Editor
- do: `start the module and compare the deployed version`
- transitions to final

### Stop Publication
- transitions to final

### final
