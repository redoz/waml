---
type: uml.Class
title: waml CLI Crate
description: The crate that owns check, format, index, query, mutation, delivery, API, and language-server hosts.
stereotype: crate
sources:
  - { id: manifest, resource: ../../../../../crates/waml-cli/Cargo.toml, title: crates/waml-cli/Cargo.toml }
---

# waml CLI Crate

## Relationships
- depends [waml Core Crate](./waml-core-crate.md)
- depends [waml Operations DTO Crate](./waml-ops-dto-crate.md)

## Notes
- `waml-cli` owns check, format, index, query, mutation, share, site, export, serve, API, and language-server hosts.
- Its local production dependencies point to `waml` and `waml-ops-dto`.
- The command-line tool and the editor share core services. They expose different operations.
