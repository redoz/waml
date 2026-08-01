---
type: uml.Class
title: GitHub Pages
description: A publication responsibility that makes the static web artifact available.
---

# GitHub Pages

## Relationships
- depends [WASM Web Artifact](./wasm-web-artifact.md)

## Notes
- This host makes the static artifact available to the browser. [Native Web Delivery](./native-web-delivery.md) controls the publication.
- The host serves files only. It runs no program of the product.
- The host cannot add response headers. For this reason the editor cannot use shared memory and cannot use threads.
