---
type: uml.Class
title: Diagnostic
description: A record that identifies a problem in authored content.
---

# Diagnostic

## Attributes
- severity: Severity
- message: Text
- location: DocumentSpan

## Relationships
- associates [Authored Document](./authored-document.md): 0..* diagnostics to 1 document

## Notes
- A diagnostic identifies a problem in the authored content. It does not become part of the model.
- A diagnostic gives the document and the position of the problem. Each problem points to text that a person wrote.
- The severity separates the two conditions. An error prevents the meaning. A warning does not prevent the model and does not prevent the drawing.
- A diagnostic can point to a document that is not the document in work, because a reference resolves across the full bundle.
