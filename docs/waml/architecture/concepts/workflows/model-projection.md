---
type: uml.Class
title: Model Projection
description: A responsibility that derives a model and view representation from the current OKF Bundle.
---

# Model Projection

## Relationships
- depends [OKF Bundle](../model/okf-bundle.md)
- associates [WAML Model](../model/waml-model.md): 1 projection to 1 model
- associates [Diagram](../model/diagram.md): 1 projection to 0..* diagrams
- associates [Behavioral View](../model/behavioral-view.md): 1 projection to 0..* views

## Notes
- This responsibility derives Markdown syntax and a document catalog from the current source bundle.
- It lowers the catalog and Markdown structure into OKF analysis.
- It analyzes UML syntax islands and semantics, then builds domain, diagram, behavioral, diagnostic, and freshness projections.
- It keeps the full text and unknown fields of accepted documents. A document with an unknown kind stays available to the reader.
- The projection is complete. It builds all views of the bundle, not only the view that the author looks at.
- A malformed document can be quarantined while unrelated projections remain current.
- The projection does not change the bundle.
