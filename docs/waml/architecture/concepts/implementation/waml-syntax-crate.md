---
type: uml.Class
title: waml Syntax Crate
description: The crate that owns immutable Markdown green and red syntax and incremental reparse.
stereotype: crate
sources:
  - id: manifest
    resource: ../../../../../crates/waml-syntax/Cargo.toml
    title: crates/waml-syntax/Cargo.toml
---

# waml Syntax Crate

## Notes
- `waml-syntax` owns immutable Markdown green and red syntax trees, text revisions, syntax queries, and incremental reparse.
- It is the lowest WAML-owned crate in this six-crate dependency view.
