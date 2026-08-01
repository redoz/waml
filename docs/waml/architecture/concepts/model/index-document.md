---
type: uml.Class
title: Index Document
description: A navigation document that lists the direct contents of one bundle directory.
stereotype: document
---

# Index Document

## Attributes
- directory: DirectoryAddress
- members: DocumentPath {0..*}

## Relationships
- associates [OKF Bundle](./okf-bundle.md): 0..* indexes to 1 bundle

## Notes
- An index lists the direct contents of one directory. A reader can go down one level at each step.
- An index gives navigation only. It gives no model element. It is not a diagram member, not a lifeline, and not a relationship target.
- A person can write an index. The system can also derive an index from the contents of the directory. A bundle stays navigable before a person writes each index.
