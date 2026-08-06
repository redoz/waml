---
type: Diagram
title: WAML Domain Model
description: Structural view of WAML's authored bundle, resolved model, model elements, views, and diagnostics.
profile: uml-domain
---

# WAML Domain Model

## Members

### Authored
- [OKF Bundle](./../concepts/model/okf-bundle.md)
- [Authored Document](./../concepts/model/authored-document.md)
- [Index Document](./../concepts/model/index-document.md)

### Resolved
- [WAML Model](./../concepts/model/waml-model.md)
- [Model Element](./../concepts/model/model-element.md)
- [Classifier](./../concepts/model/classifier.md)
- [Relationship](./../concepts/model/relationship.md)
- [Package](./../concepts/model/package.md)
- [Note](./../concepts/model/note.md)

### Views
- [Diagram](./../concepts/model/diagram.md)
- [Behavioral View](./../concepts/model/behavioral-view.md)
- [Profile](./../concepts/model/profile.md)

### Reported
- [Diagnostic](./../concepts/model/diagnostic.md)



## Reading guide

The three framed groups are the three tiers of the product. The left group is
what a person writes. The middle group is what the system resolves from it. The
right group is what the system draws. Read the groups from left to right.

Start with [OKF Bundle](../concepts/model/okf-bundle.md) and
[Authored Document](../concepts/model/authored-document.md). Continue with
[WAML Model](../concepts/model/waml-model.md) and
[Model Element](../concepts/model/model-element.md). Read
[Classifier](../concepts/model/classifier.md),
[Relationship](../concepts/model/relationship.md),
[Package](../concepts/model/package.md), and
[Note](../concepts/model/note.md) for the kinds of element.

This view does not show [Stereotype](../concepts/model/stereotype.md),
[Association Class](../concepts/model/association-class.md),
[Layout Constraint](../concepts/model/layout-constraint.md), or
[View Geometry](../concepts/model/view-geometry.md). The
[Model Vocabulary](./model-vocabulary.md) view shows these elements. A view that
does not show an element does not remove the element from the model.
