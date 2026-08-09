---
type: uml.Class
title: Source Bundle
description: An immutable candidate set of source documents and bundle-relative identities.
stereotype: runtime
sources:
  - { id: source-bundle, resource: ../../../../../crates/waml/src/source.rs, title: crates/waml/src/source.rs::SourceBundle }
---

# Source Bundle

## Notes
- `SourceBundle` owns source documents and their bundle-relative identities.
- Preparation receives an owned candidate bundle. It does not mutate the live editor snapshot.
