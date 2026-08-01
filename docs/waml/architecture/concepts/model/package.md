---
type: uml.Class
title: Package
description: A Model Element that groups other elements for containment and disclosure.
---

# Package

## Relationships
- specializes [Model Element](./model-element.md)
- aggregates [Model Element](./model-element.md): 1 package to 0..* members

## Notes
- A package groups elements. A reader can then read a large bundle one level at a time.
- The group states where an element belongs. It is not a connection between the grouped elements. It gives no dependency between them.
- A package is not a classifier. It names no type, it has no features, and an attribute cannot have a package as its type.
