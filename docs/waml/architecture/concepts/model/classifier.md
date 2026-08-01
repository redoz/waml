---
type: uml.Class
title: Classifier
description: A Model Element that names a type or category in the model.
---

# Classifier

## Attributes
- attributes: Attribute {0..*}
- abstract: Flag

## Relationships
- specializes [Model Element](./model-element.md)
- associates [Note](./note.md): 1 classifier to 0..* notes

## Notes
- A classifier names a type or a category in the model.
- Each attribute of a classifier has a name and a type. If the type is a link to a different document, the reader can go to that document. If the type is a plain word, it is text only.
- These kinds are classifiers: class, interface, enumeration, data type, actor, use case, and association class.
- These kinds are not classifiers: package, note, and view.
- A behavioral document is also a classifier. It names a behavior, and a different element can refer to that behavior.
