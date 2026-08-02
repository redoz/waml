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
- The native editor runs as a desktop application and as WebAssembly. The publication makes the WebAssembly form available.
- A change on the main branch or a manual command starts the publication.
- The publication builds the editor without threads, because the static host cannot send the two headers that a browser needs for shared memory.
- The artifact contains the WebAssembly module, the JavaScript that starts it, and the necessary resources.
- The publication removes the fonts that the editor does not use. It then puts the product name and the product icon in the page.
- The publication adds a start screen and a version file. The page compares its own version with the version file and tells the reader when a newer version is available.
- The publication examines the artifact before the upload. If a referenced file is absent, the publication stops. An incomplete artifact does not become available.
- The publication does not assemble the upload by hand. It puts the verified artifact inside the command-line tool and asks the tool to write the site, so the published site and a site a reader writes on their own machine come from one procedure.
- The command-line tool writes a site for any model: the editor and one bundle file of that model. The site needs no program on the host.
- The page carries one placeholder for its start address. The tool replaces the placeholder with the address of the bundle file. A page whose placeholder is absent is refused.
- The bundle file in a site does not change. An edit in the browser moves the whole model into the address of the page, so a reload keeps the edit, and the reader can take the edited model out of the editor as one file.
- Only one publication runs at one time. The most recent change has priority.
