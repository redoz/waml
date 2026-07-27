---
type: uml.Activity
title: GitHub Pages Deployment
description: An activity that orders publication of the native editor for browser delivery.
---

# GitHub Pages Deployment

## Notes
- [Native Web Delivery](./../concepts/runtime/native-web-delivery.md)

## Nodes

### initial
- transitions to Main-Branch Push or Manual Dispatch

### Main-Branch Push or Manual Dispatch
- transitions to Build Non-Threaded Native WebAssembly Artifact

### Build Non-Threaded Native WebAssembly Artifact
- transitions to Prune Unused Fonts

### Prune Unused Fonts
- transitions to Brand Artifact

### Brand Artifact
- transitions to Inject Loading and Deployed-Version Runtime Shell

### Inject Loading and Deployed-Version Runtime Shell
- transitions to Upload Static Pages Artifact

### Upload Static Pages Artifact
- transitions to Deploy GitHub Pages

### Deploy GitHub Pages
- transitions to Browser Loads Deployed Native Editor

### Browser Loads Deployed Native Editor
- transitions to final

### final
