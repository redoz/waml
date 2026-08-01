---
type: uml.Sequence
title: Share Round Trip
description: An interaction that packs a bundle into a link and rebuilds that bundle in a browser.
---

# Share Round Trip

## Notes
- Read this interaction from the top to the bottom.
- [Share Link](./../concepts/runtime/share-link.md) states the format and the limits of the link.
- The browser does not send the fragment of the address to the host. The host receives a request for the page only.
- The receiver gets a full bundle, not a picture. The receiver can read it, change it, and share it again.

## Lifelines
- [Author](./../concepts/workflows/author.md) as author
- [Command-Line Tool](./../concepts/runtime/command-line-tool.md) as tool
- [Share Link](./../concepts/runtime/share-link.md) as link
- [Browser](./../concepts/runtime/browser.md) as browser
- [Native Editor](./../concepts/runtime/native-editor.md) as editor

## Messages
- author calls tool: `create a link for this bundle`
- tool calls link: `compress and encode the documents`
- link replies tool: `address with the bundle in its fragment`
- tool replies author: `share link`
- author sends browser: `open the share link`
- browser calls editor: `start with the fragment of the address`
- editor calls link: `decode the fragment`
- link replies editor: `the same documents`
- editor replies author: `the bundle in the editor`
