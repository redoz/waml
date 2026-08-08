# Class Feature Cut

**Goal:** A class diagram expresses the structural model that an architecture
document needs.

**Done when:** Every MVP language and model row below is `done`, and every
other row is `done` or `horizon`.

**Status:** partial
**MVP:** yes

This document records language and model coverage. [Interact with a Class
Diagram](./interact-with-a-class-diagram.md) owns class-diagram UI behavior.

## Classifiers

| Feature | Status | MVP |
| --- | --- | --- |
| Class | done | yes |
| Interface | done | yes |
| Enum, with literals | done | yes |
| DataType | done | yes |
| Package as a container | done | yes |
| Note, attached to an element | done | yes |
| Association as a first-class element | done | no |
| Instance specification, with slots | partial | no |
| Abstract classifier | planned | no |
| Generic or templated classifier | planned | no |

## Members

| Feature | Status | MVP |
| --- | --- | --- |
| Attributes, with a type | done | yes |
| Attribute multiplicity | done | no |
| Attribute default value | partial | no |
| Operations, with parameters | partial | yes |
| Operation return type | partial | yes |
| Visibility markers | partial | no |
| Static member | planned | no |
| Abstract operation | planned | no |
| Derived member | horizon | no |

## Relationships

| Feature | Status | MVP |
| --- | --- | --- |
| `associates` | done | yes |
| `aggregates` | done | yes |
| `composes` | done | yes |
| `specializes` | done | yes |
| `implements` | done | yes |
| `depends` | done | yes |
| `annotates` | done | yes |
| `includes` | done | no |
| `extends` | done | no |
| `instance of` | done | no |
| `links` | done | no |
| Named ends on the ended kinds | done | yes |
| End multiplicity | done | yes |
| End role name | partial | no |
| Navigability | planned | no |
| Association class | partial | no |
| Qualified association | horizon | no |

## Evidence

- Supported classifier types are covered by
  `crates/waml/tests/uml_attribute_syntax.rs::catalog_claims_each_supported_uml_type_once_and_leaves_generic_types_unclaimed`.
- Classifier sections, values, slots, members, and relationships are covered by
  `crates/waml/tests/uml_classifier_syntax.rs::classifier_sections_are_lossless_and_expose_fixed_typed_slots`.
- Attribute fields and multiplicity are covered by
  `crates/waml/tests/uml_attribute_syntax.rs::attributes_are_lossless_and_expose_declared_partial_fields`.
- Relationship kinds and ended-kind rules are defined by
  `crates/waml/src/model.rs::RelationshipKind`.
- Nested package membership is covered by
  `crates/waml/tests/golden.rs::nested_packages_round_trip_through_reindex`.

## Notes

- The eleven relationship kinds are in the shared model. A `done` row here
  means that the relationship parses and resolves for a class diagram. It does
  not give that relationship a meaning in another diagram kind.
- `includes` and `extends` are also part of the [Use Case feature
  cut](../use-case/feature-cut.md).
- Operations remain `partial` because parameter lists and return types do not
  yet meet the full goal.
