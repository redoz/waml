---
type: uml.Class
title: Share Link
description: A link whose address fragment contains a full OKF Bundle.
stereotype: document
---

# Share Link

## Relationships
- composes [OKF Bundle](../model/okf-bundle.md): 1 link to 1 bundle
- depends [Native Editor](./native-editor.md)

## Notes
- The link contains the full bundle in the fragment part of its address. The receiver needs no service and no account.
- A browser does not send the fragment to the server. The content stays with the sender and the receiver.
- The content is compressed and encoded. A version mark starts the fragment. A later format can change the content without a risk of a wrong result for an old link.
- The encoding is deterministic. The same bundle always gives the same link.
- The size of a link has a limit. A large bundle can become longer than the address limit of the browser.
