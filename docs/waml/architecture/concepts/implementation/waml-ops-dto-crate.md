---
type: uml.Class
title: waml Operations DTO Crate
description: The crate that owns the serde wire contract for command-line semantic operations.
stereotype: crate
sources:
  - id: manifest
    resource: ../../../../../crates/waml-ops-dto/Cargo.toml
    title: crates/waml-ops-dto/Cargo.toml
---

# waml Operations DTO Crate

## Relationships
- depends [waml Core Crate](./waml-core-crate.md)

## Notes
- `waml-ops-dto` owns the serialized request and response contract for command-line semantic operations.
- It depends on `waml` for shared semantic types and serde support.
