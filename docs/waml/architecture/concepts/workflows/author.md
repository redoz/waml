---
type: uml.Actor
title: Author
description: A person who creates or imports an OKF Bundle and responds to diagnostics.
---

# Author

## Relationships
- associates [OKF Bundle](../model/okf-bundle.md): 1 author to 0..* bundles
- associates [Diagnostic](../model/diagnostic.md): 1 author to 0..* diagnostics

## Notes
- Creates or imports an OKF Bundle and responds to diagnostics associated with its authored content.
