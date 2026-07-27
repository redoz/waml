---
type: uml.Class
title: OKF Bundle
description: A hierarchical collection of Markdown documents exchanged as one bundle.
stereotype: document
---

# OKF Bundle

## Relationships
- composes [Authored Document](./authored-document.md): 1 bundle to 1..* documents

## Notes
- A hierarchical collection of Markdown documents with YAML frontmatter.
- A document's path supplies its identity in the bundle, while frontmatter declares its kind and display metadata.
- Standard Markdown links connect concepts, and optional index documents support navigation.
- Documents outside the diagram grammar remain bundle content and are preserved.
