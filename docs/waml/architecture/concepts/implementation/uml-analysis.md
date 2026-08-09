---
type: uml.Class
title: UML Analysis
description: UML syntax, semantic analysis, projection, diagnostics, freshness, and affected closure for one bundle revision.
stereotype: runtime
sources:
  - { id: uml-analysis, resource: ../../../../../crates/waml/src/uml/analysis.rs, title: crates/waml/src/uml/analysis.rs::Analysis }
---

# UML Analysis

## Relationships
- depends [OKF Analysis](./okf-analysis.md)
- depends [Affected Analysis](./affected-analysis.md)

## Notes
- `waml::uml::Analysis` owns analyzed UML islands, the UML projection, diagnostics, per-island freshness, affected analysis, and its session revision.
- A malformed document can be quarantined while unrelated projections remain current.
