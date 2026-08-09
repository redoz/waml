---
type: uml.Class
title: Editor
description: A responsibility that presents derived views and applies semantic edits to authored documents.
---

# Editor

## Relationships
- depends [Authored Document](../model/authored-document.md)
- depends [OKF Bundle](../model/okf-bundle.md)
- associates [Edit History](./edit-history.md): 1 editor to 1 history

## Notes
- The editor shows the derived views. It applies semantic edits to the authored documents. The derived model does not become authoritative.
- The editor shows one bundle at one time. The reader can open more than one document of that bundle.
- The editor shows three kinds of document: a diagram, a behavioral view, and a plain document. A plain document shows its text.
- The editor keeps the changed documents in memory. The author must save these documents to make the change permanent.
- The editor reports the diagnostics of the full bundle, also for the documents that are not open.
- Semantic operations prepare a complete candidate and install one immutable snapshot. A failed preparation leaves the live snapshot unchanged.
- Exact Markdown edits install source revisions before a separate semantic-completion phase. Stale completion guards do not replace newer state.
