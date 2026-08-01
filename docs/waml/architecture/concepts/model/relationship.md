---
type: uml.Class
title: Relationship
description: A Model Element that expresses a connection between model elements.
---

# Relationship

## Attributes
- verb: RelationshipVerb
- name: Text {0..1}
- ends: RelationshipEnd {0..2}

## Relationships
- specializes [Model Element](./model-element.md)
- associates [Association Class](./association-class.md): 1 relationship to 0..1 detail

## Notes
- A relationship connects two elements. It does not replace them.
- The verb gives the category of the relationship: association, dependency, or generalization. The category gives the meaning of the line.
- One element declares the relationship and points to the other element. For a connection in the two directions, the two elements must each declare their side. The system then makes one connection from the two declarations.
- An association verb has two ends. Each end has a multiplicity and can have a role. The other verbs have no ends.
- A relationship can have a name. The name gives an identity, and a [Note](./note.md) can use that identity as its target.
