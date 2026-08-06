---
type: Diagram
title: Model Vocabulary
description: Structural view of the element kinds, their labels, the presentation lens, and the solved geometry.
profile: uml-domain
---

# Model Vocabulary

## Members

### Kinds
- [Model Element](./../concepts/model/model-element.md)
- [Classifier](./../concepts/model/classifier.md)
- [Relationship](./../concepts/model/relationship.md)
- [Association Class](./../concepts/model/association-class.md)
- [Package](./../concepts/model/package.md)
- [Note](./../concepts/model/note.md)

### Presentation
- [Stereotype](./../concepts/model/stereotype.md)
- [Profile](./../concepts/model/profile.md)
- [Diagram](./../concepts/model/diagram.md)

### Arrangement
- [Layout Constraint](./../concepts/model/layout-constraint.md)
- [View Geometry](./../concepts/model/view-geometry.md)

## Reading guide

This view separates three questions that are easy to mix.

The left group answers "what kinds of element exist". The set of kinds is
closed. An author does not add a kind.

The middle group answers "how does a view present them".
[Stereotype](../concepts/model/stereotype.md) is the open set: a new domain term
is a new label. A [Profile](../concepts/model/profile.md) gives these labels an
appearance for one [Diagram](../concepts/model/diagram.md).

The right group answers "where are they drawn". An author writes
[Layout Constraint](../concepts/model/layout-constraint.md) statements. The
solver produces [View Geometry](../concepts/model/view-geometry.md). Read
[Layout Solving](./layout-solving.md) for that activity.
