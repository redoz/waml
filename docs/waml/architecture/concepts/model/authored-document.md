---
type: uml.Class
title: Authored Document
description: A human-maintained document that contributes authored WAML content.
stereotype: document
---

# Authored Document

## Attributes
- path: DocumentPath
- type: TypeKey
- title: Text {0..1}
- description: Text {0..1}
- tags: Text {0..*}
- body: Markdown

## Relationships
- associates [OKF Bundle](./okf-bundle.md): 0..* documents to 1 bundle

## Notes
- A person writes and maintains this document. It stays different from the model that the system derives from it.
- The type field has the form `family.Metaclass`. It selects how the system reads and draws the document.
- The system keeps each frontmatter field that it does not use. Data from a different tool stays in the document.
- The system keeps the full Markdown body. An unknown document and an incomplete document stay readable.
