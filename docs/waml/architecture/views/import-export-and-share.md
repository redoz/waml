---
type: uml.Activity
title: Import, Export, and Share
description: An activity that routes a requested exchange action to its supported outcome.
---

# Import, Export, and Share

## Notes
- [Exchange and Sharing](./../concepts/workflows/exchange-and-sharing.md)

## Nodes

### initial
- transitions to Requested Action?

### decision Requested Action?
- when `import` transitions to Preview Supported Content
- when `export` transitions to Produce SVG
- when `image sharing` transitions to Produce PNG
- else transitions to Encode Complete Bundle in URL Fragment

### Preview Supported Content
- transitions to Replace or Merge?

### decision Replace or Merge?
- when `replace` transitions to Replace Bundle
- else transitions to Merge Bundle

### Replace Bundle
- transitions to final

### Merge Bundle
- transitions to final

### Produce SVG
- transitions to final

### Produce PNG
- transitions to final

### Encode Complete Bundle in URL Fragment
- transitions to Recipient Opens Shared Bundle

### Recipient Opens Shared Bundle
- transitions to final

### final
