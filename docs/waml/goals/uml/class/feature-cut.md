# Class Feature Cut

**Goal:** A class diagram in WAML expresses everything an architecture document
needs to say about structure.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** yes

Every status in this table is a first-pass guess. `Evidence` reads
`unverified` until an audit replaces it with a `file:line` or a test name.

## Classifiers

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Class | done | yes | unverified |
| Interface | done | yes | unverified |
| Enum, with literals | done | yes | unverified |
| DataType | done | yes | unverified |
| Package as a container | done | yes | unverified |
| Note, attached to an element | done | yes | unverified |
| Association as a first-class element | done | no | unverified |
| Instance specification, with slots | partial | no | unverified |
| Abstract classifier | planned | no | unverified |
| Generic or templated classifier | planned | no | unverified |

## Members

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Attributes, with a type | done | yes | unverified |
| Attribute multiplicity | done | no | unverified |
| Attribute default value | partial | no | unverified |
| Operations, with parameters | partial | yes | unverified |
| Operation return type | partial | yes | unverified |
| Visibility markers | partial | no | unverified |
| Static member | planned | no | unverified |
| Abstract operation | planned | no | unverified |
| Derived member | horizon | no | unverified |

## Relationships

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| `associates` | done | yes | unverified |
| `aggregates` | done | yes | unverified |
| `composes` | done | yes | unverified |
| `specializes` | done | yes | unverified |
| `implements` | done | yes | unverified |
| `depends` | done | yes | unverified |
| `annotates` | done | yes | unverified |
| `includes` | done | no | unverified |
| `extends` | done | no | unverified |
| `instance of` | done | no | unverified |
| `links` | done | no | unverified |
| Named ends on the ended kinds | done | yes | unverified |
| End multiplicity | done | yes | unverified |
| End role name | partial | no | unverified |
| Navigability | planned | no | unverified |
| Association class | partial | no | unverified |
| Qualified association | horizon | no | unverified |

## Presentation

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Stereotype label above a title | done | no | unverified |
| Profile-defined stereotypes | partial | no | unverified |
| Per-kind accent and node styling | done | no | unverified |
| Member compartments collapse | planned | no | unverified |
| Package nesting drawn as containment | partial | yes | unverified |

## Notes

- The eleven relationship kinds are model-level and shared with every other
  kind. A row marked `done` here means it parses, resolves, and draws in a
  class diagram — not that it is meaningful in a sequence diagram.
- `includes` and `extends` are `MVP: no` here because they belong to [Use
  Case](../use-case/feature-cut.md); they appear in this table only because
  they are structural relationships in the same model.
- Operations are marked `partial` rather than `done` deliberately: parameter
  lists and return types are the part most likely to be thin, and this bundle
  will exercise them.
