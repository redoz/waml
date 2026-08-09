---
type: uml.Class
title: Validation and Diagnostics
description: A responsibility that evaluates an OKF Bundle, reports errors and warnings, and retains unknown content.
---

# Validation and Diagnostics

## Relationships
- depends [OKF Bundle](../model/okf-bundle.md)
- associates [Diagnostic](../model/diagnostic.md): 1 validation to 0..* diagnostics

## Notes
- This responsibility evaluates the full bundle. It does not evaluate only the open document.
- A document that fails shell or size validation can enter quarantine. Other documents continue through analysis.
- A failed UML island can retain its previous dependent projection. Unrelated projections stay current.
- It reports incorrect syntax in a supported section.
- It reports a reference that does not resolve.
- It reports a connection in a form that its category does not permit.
- It reports a member statement or an arrangement statement that names an element outside the view.
- It reports a behavioral step that names an unknown participant or an unknown target.
- It does not report unknown content. It keeps an unknown document kind, an unknown frontmatter field, and text outside the supported sections. A bundle from a different tool stays usable.
- An edit in one document can add or remove a diagnostic in a different document, because references resolve across the bundle.
- A warning does not prevent the model or the view. The system reports the problem and continues.
