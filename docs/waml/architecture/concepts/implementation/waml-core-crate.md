---
type: uml.Class
title: waml Core Crate
description: The crate that owns source bundles, analysis, semantic edits, projection, layout, and index generation.
stereotype: crate
sources:
  - { id: manifest, resource: ../../../../../crates/waml/Cargo.toml, title: crates/waml/Cargo.toml }
---

# waml Core Crate

## Relationships
- depends [waml Syntax Crate](./waml-syntax-crate.md)

## Notes
- `waml` owns `SourceBundle`, Markdown catalog analysis, OKF and UML analysis, exact and semantic edits, projections, layout, and index generation.
- Its only local production-crate dependency in this six-crate view is `waml-syntax`.
