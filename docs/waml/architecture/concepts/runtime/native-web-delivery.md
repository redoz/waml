---
type: uml.Class
title: Native Web Delivery
description: A responsibility that publishes the native editor as a static browser-delivered artifact.
---

# Native Web Delivery

## Relationships
- depends [Native Editor](./native-editor.md)
- depends [WASM Web Artifact](./wasm-web-artifact.md)
- depends [GitHub Pages](./github-pages.md)

## Notes
- The Native Editor runs as a desktop application and as WebAssembly.
- A push to `main` or manual dispatch starts Pages publication.
- Publication builds the Native Editor as non-threaded WebAssembly because Pages cannot provide the shared-memory browser headers.
- The static artifact contains WebAssembly, JavaScript glue, and required resources.
- Publication prunes unused fonts, brands the artifact, injects the loading and deployed-version runtime shell, uploads the artifact, and deploys it to GitHub Pages.
