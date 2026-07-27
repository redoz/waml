---
type: uml.Class
title: Validation and Diagnostics
description: A responsibility that evaluates an OKF Bundle, reports errors and warnings, and retains unknown content.
---

# Validation and Diagnostics

## Relationships
- depends [OKF Bundle](../model/okf-bundle.md)
- associates [Diagnostic](../model/diagnostic.md): 1 to 0..*

## Notes
- Evaluates the bundle as a whole, reports errors and warnings, and retains unknown content.
