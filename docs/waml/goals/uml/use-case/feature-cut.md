# Use Case Feature Cut

**Goal:** WAML expresses the actors, use cases, and semantic relationships in a
use-case model.

**Done when:** Every language and model row below is `done` or `horizon`.

**Status:** partial
**MVP:** no

This document records use-case language and model coverage. [Interact with a
Use Case Diagram](./interact-with-a-use-case-diagram.md) owns editor and
renderer behavior.

## Elements

| Feature | Status | MVP |
| --- | --- | --- |
| Actor | done | no |
| Use case | done | no |
| Note anchored to an element | partial | no |

## Relationships

| Feature | Status | MVP |
| --- | --- | --- |
| Actor associates use case | done | no |
| `includes` between use cases | done | no |
| `extends` between use cases | done | no |
| Extension point on an extend | horizon | no |
| Actor generalization via `specializes` | done | no |
| Use case generalization via `specializes` | done | no |

## Evidence

- Actor and use-case metaclasses are covered by
  `crates/waml/src/model.rs::actor_and_usecase_metaclasses_parse_and_round_trip`.
- UML catalog claims for actors and use cases are covered by
  `crates/waml/tests/uml_attribute_syntax.rs::catalog_claims_each_supported_uml_type_once_and_leaves_generic_types_unclaimed`.
- The `associates`, `includes`, `extends`, and `specializes` relationship kinds
  are defined by `crates/waml/src/model.rs::RelationshipKind`.

## Notes

- This feature cut owns only the use-case language and model.
- It does not specify a specialized actor shape, use-case shape, system
  boundary, layout, position, size, or route.
- [Interact with a Use Case
  Diagram](./interact-with-a-use-case-diagram.md) owns the editor and renderer
  product feature.
