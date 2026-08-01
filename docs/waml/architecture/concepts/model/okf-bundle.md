---
type: uml.Class
title: OKF Bundle
description: A hierarchical collection of Markdown documents exchanged as one bundle.
stereotype: document
---

# OKF Bundle

## Attributes
- documents: [Authored Document](./authored-document.md) {1..*}
- indexes: [Index Document](./index-document.md) {0..*}

## Relationships
- composes [Authored Document](./authored-document.md): 1 bundle to 1..* documents
- composes [Index Document](./index-document.md): 1 bundle to 0..* indexes

## Notes
- A bundle is a set of Markdown documents in a directory tree. Each document has a YAML frontmatter block.
- The path of a document gives the identity of the document in the bundle. The frontmatter gives the kind and the display data.
- A standard Markdown link connects two documents. An index document gives navigation.
- A document that the diagram grammar does not describe stays in the bundle. The system keeps it.
- The bundle is the unit of validation, of exchange, and of the derived model. The open document is not that unit.
- The structural role of a document comes from its shape, not from a declaration. An index gives navigation, a document with a member list is a view, and each other document gives one model element.
