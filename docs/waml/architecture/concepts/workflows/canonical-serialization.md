---
type: uml.Class
title: Canonical Serialization
description: A responsibility that produces a stable supported document form from authored content.
---

# Canonical Serialization

## Relationships
- depends [Authored Document](../model/authored-document.md)

## Notes
- This responsibility produces a stable document form. It does not regenerate the documents from the WAML Model.
- The operation is idempotent. A second run on a canonical document makes no change.
- The operation changes only the supported sections. It keeps other text, unknown frontmatter fields, and unrelated documents as the author wrote them.
- The operation can also report the documents that are not in canonical form. In this mode it writes no file.
